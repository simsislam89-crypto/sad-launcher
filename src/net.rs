//! One async runtime and one HTTP client for the whole launcher.
//! Connections and TLS sessions are pooled instead of re-handshaking on
//! every search, icon or file — the catalog feels instant because of it.
//! Every request made with `client()` must run on `rt()`.
//!
//! Downloads go through [`fetch_to`]: big files are split into parallel
//! byte ranges (like a download manager), every received byte is counted
//! in one global counter, and the UI turns that counter into «МБ/с».

use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
static BULK: OnceLock<reqwest::Client> = OnceLock::new();
/// Every byte received by downloads since start — the speed meter source.
static BYTES: AtomicU64 = AtomicU64::new(0);

pub fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        // Network work is IO-bound: 2–4 workers saturate any connection,
        // and each extra thread is memory the launcher keeps for nothing.
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .clamp(2, 4);
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(workers)
            .max_blocking_threads(8)
            .thread_stack_size(1024 * 1024)
            .thread_name("madrid-net")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

/// Shared client: compressed responses (gzip/br/zstd), keep-alive pool.
pub fn client() -> reqwest::Client {
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(format!(
                    "MinecraftLauncherMadrid/{} (Windows)",
                    crate::config::VERSION
                ))
                .connect_timeout(Duration::from_secs(12))
                .read_timeout(Duration::from_secs(40))
                .pool_max_idle_per_host(48)
                .pool_idle_timeout(Duration::from_secs(60))
                .tcp_nodelay(true)
                .tcp_keepalive(Duration::from_secs(30))
                // Bigger HTTP/2 flow-control windows on fast links: a single
                // stream is not capped at ~64 KB in flight.
                .http2_adaptive_window(true)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

/// HTTP/1.1-only client for the extra byte ranges of a big file.
///
/// Over HTTP/2 every request to a host is multiplexed onto ONE connection,
/// so eight «parallel» ranges shared one TCP window and downloaded no faster
/// than a single stream. Here each range gets its own connection — what a
/// download manager does. Must be used on [`rt()`] (its pool lives there).
pub fn bulk_client() -> reqwest::Client {
    BULK.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!(
                "MinecraftLauncherMadrid/{} (Windows)",
                crate::config::VERSION
            ))
            .http1_only()
            .connect_timeout(Duration::from_secs(12))
            .read_timeout(Duration::from_secs(40))
            .pool_max_idle_per_host(16)
            .pool_idle_timeout(Duration::from_secs(30))
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
    .clone()
}

/// True when running on the shared runtime (where [`bulk_client`] may be
/// used); isolated runtimes (server bootstrap) keep their own client.
fn on_shared_rt() -> bool {
    match (tokio::runtime::Handle::try_current(), RT.get()) {
        (Ok(cur), Some(rt)) => cur.id() == rt.handle().id(),
        _ => false,
    }
}

/// Runs a background job on the shared runtime.
pub fn spawn<F>(fut: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    rt().spawn(fut);
}

/// Adds received bytes to the global meter.
pub fn count(bytes: usize) {
    BYTES.fetch_add(bytes as u64, Ordering::Relaxed);
}

/// Total bytes received so far (monotonic).
pub fn bytes_total() -> u64 {
    BYTES.load(Ordering::Relaxed)
}

/// Turns the global byte counter into a smoothed speed.
pub struct SpeedMeter {
    last_bytes: u64,
    last_at: std::time::Instant,
    /// Bytes per second, exponentially smoothed.
    pub bps: f64,
}

impl Default for SpeedMeter {
    fn default() -> Self {
        Self {
            last_bytes: bytes_total(),
            last_at: std::time::Instant::now(),
            bps: 0.0,
        }
    }
}

impl SpeedMeter {
    /// Samples at most every 400 ms; returns the smoothed speed.
    pub fn tick(&mut self) -> f64 {
        let elapsed = self.last_at.elapsed().as_secs_f64();
        if elapsed >= 0.4 {
            let now = bytes_total();
            let inst = now.saturating_sub(self.last_bytes) as f64 / elapsed;
            // Fast rise, slower fall: the number reads calm but honest.
            let k = if inst > self.bps { 0.6 } else { 0.3 };
            self.bps += (inst - self.bps) * k;
            if self.bps < 1024.0 && inst == 0.0 {
                self.bps = 0.0;
            }
            self.last_bytes = now;
            self.last_at = std::time::Instant::now();
        }
        self.bps
    }

    /// «12,4 МБ/с» / «640 КБ/с»; empty when idle.
    pub fn label(&self) -> String {
        speed_label(self.bps)
    }
}

pub fn speed_label(bps: f64) -> String {
    if bps < 1.0 {
        return String::new();
    }
    let kb = bps / 1024.0;
    if kb < 1000.0 {
        format!("{} КБ/с", kb.round() as u64)
    } else {
        format!("{:.1} МБ/с", kb / 1024.0).replace('.', ",")
    }
}

/// `name.jar` → `name.jar.part`: unique per target, never shared by two files.
pub fn part_path(dest: &Path) -> PathBuf {
    let mut os = dest.as_os_str().to_owned();
    os.push(".part");
    PathBuf::from(os)
}

/// First range asked for: small files arrive whole in this one request.
const HEAD_CHUNK: u64 = 2 * 1024 * 1024;
/// Smallest extra segment worth its own connection.
const MIN_SEGMENT: u64 = 2 * 1024 * 1024;
/// Parallel connections per big file.
const MAX_SEGMENTS: u64 = 8;

/// Downloads `url` into `dest` as fast as the link allows.
///
/// One request asks for the first 2 MB as a range. If the server answers
/// with a range and the file is bigger, the rest is split into up to eight
/// parallel ranges written straight to their offsets. Servers without range
/// support simply stream the whole body. Interrupted segments resume where
/// they stopped. `progress(done, total)` is called a few times a second.
pub async fn fetch_to(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: Option<&AtomicBool>,
    progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = part_path(dest);
    let result = fetch_inner(client, url, &tmp, cancel, progress).await;
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if dest.exists() {
        let _ = std::fs::remove_file(dest);
    }
    std::fs::rename(&tmp, dest)?;
    Ok(())
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false)
}

/// CurseForge's CDN (forgecdn) answers every Range request with 404 —
/// files there are fetched in one stream.
pub fn ranges_supported(url: &str) -> bool {
    let host = url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or("");
    !(host.ends_with("forgecdn.net") || host.ends_with("curseforge.com"))
}

/// Total size from `Content-Range: bytes 0-2097151/73400320`.
fn range_total(resp: &reqwest::Response) -> Option<u64> {
    let value = resp.headers().get(reqwest::header::CONTENT_RANGE)?.to_str().ok()?;
    value.rsplit('/').next()?.trim().parse().ok()
}

async fn fetch_inner(
    client: &reqwest::Client,
    url: &str,
    tmp: &Path,
    cancel: Option<&AtomicBool>,
    progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<()> {
    // Extra ranges each on their own connection (see `bulk_client`).
    let seg_client = if on_shared_rt() { bulk_client() } else { client.clone() };
    fetch_inner_with(client, &seg_client, url, tmp, cancel, progress).await
}

async fn fetch_inner_with(
    client: &reqwest::Client,
    seg_client: &reqwest::Client,
    url: &str,
    tmp: &Path,
    cancel: Option<&AtomicBool>,
    progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<()> {
    let plain = || async {
        client
            .get(url)
            .send()
            .await
            .with_context(|| format!("Запрос не прошёл: {url}"))
    };
    let resp = if ranges_supported(url) {
        let ranged = client
            .get(url)
            .header(reqwest::header::RANGE, format!("bytes=0-{}", HEAD_CHUNK - 1))
            // Ranges address raw bytes; a compressed body would break offsets.
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .with_context(|| format!("Запрос не прошёл: {url}"))?;
        if ranged.status().is_success() {
            ranged
        } else {
            // Empty file, or a CDN that answers ranges with 404/416/501:
            // the plain request decides whether the file really exists.
            plain().await?
        }
    } else {
        plain().await?
    };
    let resp = resp.error_for_status()?;
    let partial = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let total = if partial {
        range_total(&resp).unwrap_or(0)
    } else {
        resp.content_length().unwrap_or(0)
    };

    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(tmp)?;
    if partial && total > HEAD_CHUNK {
        file.set_len(total)?;
    }
    let done = AtomicU64::new(0);
    let mut head_written = 0u64;

    // Body of the first response: the head range, or the whole file.
    let head = write_stream(resp, &file, 0, None, &done, cancel, &mut head_written);

    if !(partial && total > HEAD_CHUNK) {
        let mut ticker = tokio::time::interval(Duration::from_millis(250));
        tokio::pin!(head);
        loop {
            tokio::select! {
                res = &mut head => { res?; break; }
                _ = ticker.tick() => progress(done.load(Ordering::Relaxed), total),
            }
        }
        progress(done.load(Ordering::Relaxed), total);
        if total > 0 && done.load(Ordering::Relaxed) < total {
            bail!("Файл пришёл не целиком: {url}");
        }
        return Ok(());
    }

    // Split the rest into parallel ranges.
    let client = seg_client;
    let rest = total - HEAD_CHUNK;
    let parts = (rest / MIN_SEGMENT).clamp(1, MAX_SEGMENTS - 1);
    let size = rest.div_ceil(parts);
    let mut tasks = futures_util::stream::FuturesUnordered::new();
    tasks.push(futures_util::future::Either::Left(head));
    let mut start = HEAD_CHUNK;
    while start < total {
        let end = (start + size).min(total) - 1;
        tasks.push(futures_util::future::Either::Right(fetch_segment(
            client, url, &file, start, end, &done, cancel,
        )));
        start = end + 1;
    }
    let mut ticker = tokio::time::interval(Duration::from_millis(250));
    loop {
        tokio::select! {
            next = tasks.next() => match next {
                Some(res) => { res?; }
                None => break,
            },
            _ = ticker.tick() => progress(done.load(Ordering::Relaxed), total),
        }
    }
    progress(done.load(Ordering::Relaxed), total);
    if done.load(Ordering::Relaxed) < total {
        bail!("Файл пришёл не целиком: {url}");
    }
    Ok(())
}

/// One byte range `[start, end]`, resumed after a dropped link.
async fn fetch_segment(
    client: &reqwest::Client,
    url: &str,
    file: &std::fs::File,
    start: u64,
    end: u64,
    done: &AtomicU64,
    cancel: Option<&AtomicBool>,
) -> Result<u64> {
    let want = end - start + 1;
    let mut got = 0u64;
    let mut attempt = 0u32;
    while got < want {
        attempt += 1;
        let from = start + got;
        let mut written = 0u64;
        let res = async {
            let resp = client
                .get(url)
                .header(reqwest::header::RANGE, format!("bytes={from}-{end}"))
                .header(reqwest::header::ACCEPT_ENCODING, "identity")
                .send()
                .await?
                .error_for_status()?;
            if resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                bail!("Сервер перестал отдавать части файла");
            }
            write_stream(resp, file, from, Some(end - from + 1), done, cancel, &mut written).await
        }
        .await;
        // Whatever arrived before a drop stays on disk; resume after it.
        got += written;
        if let Err(e) = res {
            if cancelled(cancel) || attempt >= 5 {
                return Err(e);
            }
            tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
        } else if written == 0 {
            bail!("Часть файла пустая: {url}");
        }
    }
    Ok(got)
}

/// Streams a body into `file` at `offset`. Returns bytes written.
async fn write_stream(
    resp: reqwest::Response,
    file: &std::fs::File,
    offset: u64,
    limit: Option<u64>,
    done: &AtomicU64,
    cancel: Option<&AtomicBool>,
    written: &mut u64,
) -> Result<u64> {
    let mut stream = resp.bytes_stream();
    let mut pos = offset;
    let mut buf: Vec<u8> = Vec::with_capacity(256 * 1024);
    let mut buf_at = offset;
    let max = limit.map(|l| offset + l);
    while let Some(chunk) = stream.next().await {
        let mut chunk = chunk?;
        if let Some(max) = max {
            let room = max.saturating_sub(pos) as usize;
            if chunk.len() > room {
                chunk.truncate(room);
            }
        }
        count(chunk.len());
        done.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        pos += chunk.len() as u64;
        buf.extend_from_slice(&chunk);
        if buf.len() >= 256 * 1024 {
            write_at(file, &buf, buf_at)?;
            *written += buf.len() as u64;
            buf_at = pos;
            buf.clear();
        }
        if cancelled(cancel) {
            bail!("Отменено");
        }
        if max.map(|m| pos >= m).unwrap_or(false) {
            break;
        }
    }
    if !buf.is_empty() {
        write_at(file, &buf, buf_at)?;
        *written += buf.len() as u64;
    }
    Ok(pos - offset)
}

/// Positional write shared by parallel range downloads: no seek, so several
/// threads can fill one file. (`seek_write` / `write_at` per OS.)
fn write_at(file: &std::fs::File, mut data: &[u8], mut offset: u64) -> Result<()> {
    #[cfg(windows)]
    use std::os::windows::fs::FileExt;
    #[cfg(unix)]
    use std::os::unix::fs::FileExt;
    while !data.is_empty() {
        #[cfg(windows)]
        let n = file.seek_write(data, offset)?;
        #[cfg(unix)]
        let n = file.write_at(data, offset)?;
        if n == 0 {
            bail!("Диск не принимает запись");
        }
        data = &data[n..];
        offset += n as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_label_units() {
        assert_eq!(speed_label(0.0), "");
        assert_eq!(speed_label(512.0 * 1024.0), "512 КБ/с");
        assert_eq!(speed_label(12.4 * 1024.0 * 1024.0), "12,4 МБ/с");
    }

    #[test]
    fn curseforge_cdn_is_never_ranged() {
        assert!(!ranges_supported("https://edge.forgecdn.net/files/1/2/a.jar"));
        assert!(!ranges_supported("https://www.curseforge.com/api/v1/mods/1/files/2/download"));
        assert!(ranges_supported("https://cdn.modrinth.com/data/x/a.jar"));
        assert!(ranges_supported("https://piston-data.mojang.com/v1/objects/x/client.jar"));
    }

    /// `cargo test --release live_small_files_speed -- --ignored --nocapture`:
    /// 600 Minecraft asset objects (the «thousands of tiny files» phase)
    /// through one shared HTTP/2 connection vs. many HTTP/1.1 connections.
    #[test]
    #[ignore]
    fn live_small_files_speed() {
        use std::sync::Arc;
        rt().block_on(async {
            let c = client();
            let manifest: serde_json::Value = c
                .get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")
                .send().await.unwrap().json().await.unwrap();
            let v = manifest["versions"].as_array().unwrap().iter()
                .find(|v| v["id"] == "1.20.1").unwrap()["url"].as_str().unwrap().to_string();
            let vj: serde_json::Value = c.get(v).send().await.unwrap().json().await.unwrap();
            let idx: serde_json::Value = c.get(vj["assetIndex"]["url"].as_str().unwrap())
                .send().await.unwrap().json().await.unwrap();
            let hashes: Vec<String> = idx["objects"].as_object().unwrap().values()
                .filter_map(|o| o["hash"].as_str().map(String::from)).collect();
            for (round, chunk) in hashes.chunks(600).take(4).enumerate() {
                let h1 = round % 2 == 1;
                let name = if h1 { "h1-pool" } else { "h2-shared" };
                let dir = std::env::temp_dir().join(format!("sad-small-{round}"));
                let _ = std::fs::remove_dir_all(&dir);
                let cl = if h1 { bulk_client() } else { client() };
                let sem = Arc::new(tokio::sync::Semaphore::new(64));
                let before = bytes_total();
                let t = std::time::Instant::now();
                let mut set = tokio::task::JoinSet::new();
                for h in chunk.iter().cloned() {
                    let cl = cl.clone();
                    let dest = dir.join(&h[..2]).join(&h);
                    let permit = sem.clone().acquire_owned().await.unwrap();
                    set.spawn(async move {
                        let _p = permit;
                        let url = format!("https://resources.download.minecraft.net/{}/{}", &h[..2], h);
                        let _ = fetch_to(&cl, &url, &dest, None, &mut |_, _| {}).await;
                    });
                }
                while set.join_next().await.is_some() {}
                let mb = (bytes_total() - before) as f64 / 1048576.0;
                let secs = t.elapsed().as_secs_f64();
                println!("{name:10} round {round}: 600 files, {mb:.1} MB in {secs:.2}s = {:.1} MB/s, {:.0} files/s", mb / secs, 600.0 / secs);
                let _ = std::fs::remove_dir_all(&dir);
            }
        });
    }

    /// `cargo test live_range_speed -- --ignored --nocapture`: one big file
    /// through the HTTP/2 client vs. per-range HTTP/1.1 connections.
    #[test]
    #[ignore]
    fn live_range_speed() {
        let url = std::env::var("MADRID_SPEED_URL").unwrap_or_else(|_| {
            // Minecraft 1.20.1 client.jar, ~23 MB.
            "https://piston-data.mojang.com/v1/objects/0c3ec587af28e5a785c0b4a7b8a30f9a8f78f838/client.jar".into()
        });
        let dir = std::env::temp_dir().join("sad-range-speed");
        let _ = std::fs::create_dir_all(&dir);
        for round in 0..2 {
            for (name, h1) in [("h2-shared", false), ("h1-ranges", true)] {
                let dest = dir.join(format!("{name}-{round}.bin"));
                let _ = std::fs::remove_file(&dest);
                let url = url.clone();
                let started = std::time::Instant::now();
                let res = rt().block_on(async move {
                    let main = client();
                    if h1 {
                        fetch_to(&main, &url, &dest, None, &mut |_, _| {}).await.map(|_| dest)
                    } else {
                        // Old behaviour: segments on the shared HTTP/2 client.
                        fetch_inner_with(&main, &main, &url, &part_path(&dest), None, &mut |_, _| {})
                            .await
                            .map(|_| part_path(&dest))
                    }
                });
                let path = res.expect("download");
                let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let secs = started.elapsed().as_secs_f64();
                println!("{name:10} round {round}: {:.1} MB in {secs:.2}s = {:.1} MB/s", len as f64 / 1048576.0, len as f64 / 1048576.0 / secs);
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    #[test]
    fn part_path_appends() {
        assert_eq!(part_path(Path::new("a/b.jar")), PathBuf::from("a/b.jar.part"));
    }
}
