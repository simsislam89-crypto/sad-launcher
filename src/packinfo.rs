//! «212 модов · 1,4 ГБ» for a modpack without downloading it.
//!
//! A zip keeps its table of contents at the very end. Two or three small
//! HTTP range requests read that table, then only the pack index
//! (`modrinth.index.json` / `manifest.json`) is fetched and inflated.
//! Modrinth indexes carry every file size; for CurseForge the sizes come
//! from one batch request. Results live in memory and in one small JSON.

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackInfo {
    /// Files that land in `mods/`.
    pub mods: u32,
    /// Everything the pack downloads, bytes (≈ for CurseForge overrides).
    pub bytes: u64,
}

impl PackInfo {
    /// «212 модов · 1,4 ГБ»; «архив 49 МБ» when only the size is known.
    pub fn label(&self) -> String {
        if self.mods == 0 {
            return format!("архив {}", crate::util::human_bytes(self.bytes));
        }
        format!("{} · {}", mods_word(self.mods), crate::util::human_bytes(self.bytes))
    }
}

/// «1 мод», «3 мода», «212 модов».
pub fn mods_word(n: u32) -> String {
    let word = match (n % 10, n % 100) {
        (1, r) if r != 11 => "мод",
        (2..=4, r) if !(12..=14).contains(&r) => "мода",
        _ => "модов",
    };
    format!("{n} {word}")
}

fn cache_path() -> std::path::PathBuf {
    crate::config::data_dir().join("meta").join("packinfo.json")
}

/// Disk cache: `source:id` → info. Small, capped at 400 packs.
pub fn load_cache() -> HashMap<String, PackInfo> {
    std::fs::read_to_string(cache_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_cache(map: &HashMap<String, PackInfo>) {
    if map.len() > 400 {
        return;
    }
    if let Ok(text) = serde_json::to_string(map) {
        let _ = std::fs::write(cache_path(), text);
    }
}

/// Reads count + size of the newest version of a pack. `deep` allows
/// reading a range-less archive (CurseForge) of up to 100 MB whole — used
/// for the one pack whose card is open, never for the grid.
pub async fn fetch(client: &reqwest::Client, cfg: &crate::config::Config, hit: &crate::mods::ModHit, deep: bool) -> Result<PackInfo> {
    let (url, _name) = crate::mods::pack_archive(client, cfg, hit).await?;
    const NAMES: [&str; 2] = ["modrinth.index.json", "manifest.json"];
    let remote = if crate::net::ranges_supported(&url) {
        read_remote_entry(client, &url, &NAMES).await
    } else {
        Err(anyhow!("без диапазонов"))
    };
    let (name, bytes, archive_len) = match remote {
        Ok(found) => found,
        // CurseForge's CDN has no ranges: small archives are read whole,
        // big ones report just their size.
        Err(_) => match read_whole_entry(client, &url, &NAMES, if deep { 100 } else { 24 } * 1024 * 1024).await? {
            Whole::Found(found) => found,
            Whole::TooBig(len) => return Ok(PackInfo { mods: 0, bytes: len }),
        },
    };
    let text = String::from_utf8_lossy(&bytes);
    if name == "modrinth.index.json" {
        modrinth_info(&text, archive_len)
    } else {
        curse_info(client, cfg, &text, archive_len).await
    }
}

fn modrinth_info(text: &str, archive_len: u64) -> Result<PackInfo> {
    #[derive(Deserialize)]
    struct Index {
        #[serde(default)]
        files: Vec<F>,
    }
    #[derive(Deserialize)]
    struct F {
        path: String,
        #[serde(default, rename = "fileSize")]
        file_size: u64,
        #[serde(default)]
        env: Option<Env>,
    }
    #[derive(Deserialize)]
    struct Env {
        #[serde(default)]
        client: String,
    }
    let index: Index = serde_json::from_str(text).context("Индекс пака не читается")?;
    let mut info = PackInfo { mods: 0, bytes: archive_len };
    for f in index.files {
        if f.env.as_ref().map(|e| e.client == "unsupported").unwrap_or(false) {
            continue;
        }
        info.bytes += f.file_size;
        if f.path.replace('\\', "/").starts_with("mods/") {
            info.mods += 1;
        }
    }
    Ok(info)
}

async fn curse_info(client: &reqwest::Client, cfg: &crate::config::Config, text: &str, archive_len: u64) -> Result<PackInfo> {
    #[derive(Deserialize)]
    struct Manifest {
        #[serde(default)]
        files: Vec<F>,
    }
    #[derive(Deserialize)]
    struct F {
        #[serde(rename = "fileID")]
        file_id: u64,
        #[serde(default = "yes")]
        required: bool,
    }
    fn yes() -> bool {
        true
    }
    let m: Manifest = serde_json::from_str(text).context("Манифест пака не читается")?;
    let ids: Vec<u64> = m.files.iter().filter(|f| f.required).map(|f| f.file_id).collect();
    let mut info = PackInfo {
        mods: ids.len() as u32,
        bytes: archive_len,
    };
    info.bytes += crate::mods::curse_file_sizes(client, cfg, &ids).await.unwrap_or(0);
    Ok(info)
}

// ---------------------------------------------------------------- remote zip

/// Fetches `[start, end]` (inclusive); `None` start = suffix of `end` bytes.
/// Returns (bytes, total file length when the server told it).
async fn range(client: &reqwest::Client, url: &str, start: Option<u64>, end: u64) -> Result<(Vec<u8>, Option<u64>, bool)> {
    let header = match start {
        Some(s) => format!("bytes={s}-{end}"),
        None => format!("bytes=-{end}"),
    };
    let resp = client
        .get(url)
        .header(reqwest::header::RANGE, header)
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await?
        .error_for_status()?;
    let partial = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let total = resp
        .headers()
        .get(reqwest::header::CONTENT_RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next())
        .and_then(|v| v.trim().parse().ok());
    if !partial {
        // No ranges: refuse to pull a huge archive just for a number.
        if resp.content_length().unwrap_or(u64::MAX) > 48 * 1024 * 1024 {
            bail!("Сервер не отдаёт части файла");
        }
    }
    let bytes = resp.bytes().await?.to_vec();
    crate::net::count(bytes.len());
    Ok((bytes, total, partial))
}

fn u16_at(b: &[u8], i: usize) -> Option<u64> {
    b.get(i..i + 2).map(|s| u16::from_le_bytes([s[0], s[1]]) as u64)
}

fn u32_at(b: &[u8], i: usize) -> Option<u64> {
    b.get(i..i + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]) as u64)
}

/// One entry of the central directory.
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    name: String,
    method: u64,
    comp: u64,
    offset: u64,
}

/// End-of-central-directory → (cd_offset, cd_size).
fn find_eocd(tail: &[u8]) -> Option<(u64, u64)> {
    if tail.len() < 22 {
        return None;
    }
    let mut i = tail.len() - 22;
    loop {
        if tail[i..i + 4] == [0x50, 0x4b, 0x05, 0x06] {
            let size = u32_at(tail, i + 12)?;
            let offset = u32_at(tail, i + 16)?;
            return Some((offset, size));
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

fn parse_central(cd: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 46 <= cd.len() && cd[i..i + 4] == [0x50, 0x4b, 0x01, 0x02] {
        let (Some(method), Some(comp), Some(n), Some(e), Some(c), Some(offset)) = (
            u16_at(cd, i + 10),
            u32_at(cd, i + 20),
            u16_at(cd, i + 28),
            u16_at(cd, i + 30),
            u16_at(cd, i + 32),
            u32_at(cd, i + 42),
        ) else {
            break;
        };
        let name_end = i + 46 + n as usize;
        let Some(name) = cd.get(i + 46..name_end) else {
            break;
        };
        out.push(Entry {
            name: String::from_utf8_lossy(name).replace('\\', "/"),
            method,
            comp,
            offset,
        });
        i = name_end + e as usize + c as usize;
    }
    out
}

/// Data of a local entry given the bytes starting at its local header.
fn inflate_local(local: &[u8], entry: &Entry) -> Result<Vec<u8>> {
    if local.get(0..4) != Some(&[0x50, 0x4b, 0x03, 0x04]) {
        bail!("Повреждённая запись архива");
    }
    let n = u16_at(local, 26).ok_or_else(|| anyhow!("короткая запись"))? as usize;
    let e = u16_at(local, 28).ok_or_else(|| anyhow!("короткая запись"))? as usize;
    let start = 30 + n + e;
    let data = local
        .get(start..start + entry.comp as usize)
        .ok_or_else(|| anyhow!("Запись архива пришла не целиком"))?;
    match entry.method {
        0 => Ok(data.to_vec()),
        8 => {
            let mut out = Vec::new();
            flate2::read::DeflateDecoder::new(data)
                .take(64 * 1024 * 1024)
                .read_to_end(&mut out)?;
            Ok(out)
        }
        m => bail!("Неизвестное сжатие {m}"),
    }
}

enum Whole {
    Found((String, Vec<u8>, u64)),
    TooBig(u64),
}

/// Downloads a small archive into memory (≤ 24 MB) and reads one entry.
async fn read_whole_entry(client: &reqwest::Client, url: &str, names: &[&str], cap: u64) -> Result<Whole> {
    use futures_util::StreamExt;
    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(180))
        .send()
        .await?
        .error_for_status()?;
    if let Some(len) = resp.content_length() {
        if len > cap {
            return Ok(Whole::TooBig(len));
        }
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        crate::net::count(chunk.len());
        buf.extend_from_slice(&chunk);
        if buf.len() as u64 > cap {
            return Ok(Whole::TooBig(buf.len() as u64));
        }
    }
    let len = buf.len() as u64;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(buf)).context("Это не zip-архив")?;
    for name in names {
        if let Ok(mut entry) = zip.by_name(name) {
            let mut out = Vec::new();
            entry.read_to_end(&mut out)?;
            return Ok(Whole::Found((name.to_string(), out, len)));
        }
    }
    bail!("В архиве нет индекса пака")
}

/// Reads the first of `names` found in the remote zip.
/// Returns (entry name, contents, archive length).
async fn read_remote_entry(client: &reqwest::Client, url: &str, names: &[&str]) -> Result<(String, Vec<u8>, u64)> {
    // Some CDNs (Modrinth's) refuse suffix ranges: learn the size with a
    // one-byte range first, then ask for the tail explicitly.
    let (probe, size, probe_partial) = range(client, url, Some(0), 0).await?;
    let (tail, total, partial) = if !probe_partial {
        let len = probe.len() as u64;
        (probe, Some(len), false)
    } else {
        let size = size.ok_or_else(|| anyhow!("Размер архива неизвестен"))?;
        let start = size.saturating_sub(65_557);
        range(client, url, Some(start), size - 1).await?
    };
    let total = if partial { total.unwrap_or(0) } else { tail.len() as u64 };
    if total == 0 {
        bail!("Размер архива неизвестен");
    }
    let tail_start = total.saturating_sub(tail.len() as u64);
    let (cd_offset, cd_size) = find_eocd(&tail).ok_or_else(|| anyhow!("Это не zip-архив"))?;
    if cd_offset == 0xFFFF_FFFF {
        bail!("Слишком большой архив");
    }
    let cd: Vec<u8> = if cd_offset >= tail_start {
        let from = (cd_offset - tail_start) as usize;
        tail.get(from..from + cd_size as usize).map(<[u8]>::to_vec).unwrap_or_default()
    } else {
        range(client, url, Some(cd_offset), cd_offset + cd_size - 1).await?.0
    };
    let entries = parse_central(&cd);
    let entry = names
        .iter()
        .find_map(|want| entries.iter().find(|e| e.name == *want))
        .ok_or_else(|| anyhow!("В архиве нет индекса пака"))?;
    // Local headers can carry a different extra field: ask for some slack.
    let end = entry.offset + 30 + entry.name.len() as u64 + 1024 + entry.comp;
    let local = if entry.offset >= tail_start && end <= total {
        tail[(entry.offset - tail_start) as usize..].to_vec()
    } else {
        range(client, url, Some(entry.offset), end.min(total) - 1).await?.0
    };
    let data = inflate_local(&local, entry)?;
    Ok((entry.name.clone(), data, total))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn mods_word_forms() {
        assert_eq!(mods_word(1), "1 мод");
        assert_eq!(mods_word(3), "3 мода");
        assert_eq!(mods_word(11), "11 модов");
        assert_eq!(mods_word(212), "212 модов");
        assert_eq!(mods_word(22), "22 мода");
    }

    #[test]
    fn reads_entry_from_zip_bytes() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("overrides/a.txt", opts).unwrap();
            zip.write_all(&[7u8; 5000]).unwrap();
            zip.start_file("modrinth.index.json", opts).unwrap();
            zip.write_all(br#"{"files":[{"path":"mods/a.jar","fileSize":100},{"path":"mods/b.jar","fileSize":50,"env":{"client":"unsupported"}},{"path":"shaderpacks/s.zip","fileSize":10}]}"#).unwrap();
            zip.finish().unwrap();
        }
        let bytes = buf.into_inner();
        let (off, size) = find_eocd(&bytes).unwrap();
        let entries = parse_central(&bytes[off as usize..(off + size) as usize]);
        let e = entries.iter().find(|e| e.name == "modrinth.index.json").unwrap();
        let data = inflate_local(&bytes[e.offset as usize..], e).unwrap();
        let info = modrinth_info(&String::from_utf8(data).unwrap(), 1000).unwrap();
        assert_eq!(info, PackInfo { mods: 1, bytes: 1110 });
    }
}

#[cfg(test)]
mod live {
    use crate::loader::{Loader, Spec};

    fn run<F: std::future::Future>(f: F) -> F::Output {
        crate::net::rt().block_on(f)
    }

    /// `cargo test live_ -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_smart_search_and_pack_info() {
        let cfg = crate::config::Config::default();
        let spec = Spec::new("1.20.1", Loader::Forge, "");
        let client = crate::net::client();
        for q in ["проминсе", "promince", "зкщьштутсу"] {
            let hits = run(crate::mods::search(&client, &cfg, q, "modrinth", &spec, crate::mods::KIND_MODPACK, "downloads", "", 0)).unwrap();
            println!("{q} -> {:?}", hits.iter().take(3).map(|h| &h.title).collect::<Vec<_>>());
            assert!(hits[0].title.to_lowercase().contains("prominence"), "{q}");
        }
        let rpg = run(crate::mods::search(&client, &cfg, "", "curseforge", &spec, crate::mods::KIND_MODPACK, "downloads", "4475", 0)).unwrap();
        println!("CF RPG: {:?}", rpg.iter().take(3).map(|h| (&h.title, &h.categories)).collect::<Vec<_>>());
        assert!(!rpg.is_empty());
        let hit = run(crate::mods::search(&client, &cfg, "prominence", "modrinth", &spec, crate::mods::KIND_MODPACK, "downloads", "", 0)).unwrap().remove(0);
        let t = std::time::Instant::now();
        let info = run(super::fetch(&client, &cfg, &hit, false)).unwrap();
        println!("{} -> {} in {:?} cats {:?}", hit.title, info.label(), t.elapsed(), hit.categories);
        assert!(info.mods > 50);
        let cf = rpg[0].clone();
        let info = run(super::fetch(&client, &cfg, &cf, false)).unwrap();
        println!("{} -> {}", cf.title, info.label());
        assert!(info.bytes > 0);
        // A small CurseForge pack is read whole: real mod count + sizes.
        let small = run(crate::mods::search(&client, &cfg, "FTB Skies", "curseforge", &spec, crate::mods::KIND_MODPACK, "downloads", "", 0)).unwrap();
        let info = run(super::fetch(&client, &cfg, &small[0], true)).unwrap();
        println!("{} -> {}", small[0].title, info.label());
    }

    #[test]
    #[ignore]
    fn live_parallel_download() {
        let client = crate::net::client();
        let dir = std::env::temp_dir().join("madrid-live-dl");
        let _ = std::fs::create_dir_all(&dir);
        let dest = dir.join("client.jar");
        let _ = std::fs::remove_file(&dest);
        // Minecraft 1.20.1 client jar (~23 MB) from Mojang's CDN.
        let manifest: serde_json::Value = run(async {
            client.get("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json").send().await.unwrap().json().await.unwrap()
        });
        let vurl = manifest["versions"].as_array().unwrap().iter().find(|v| v["id"] == "1.20.1").unwrap()["url"].as_str().unwrap().to_string();
        let version: serde_json::Value = run(async { client.get(&vurl).send().await.unwrap().json().await.unwrap() });
        let url = version["downloads"]["client"]["url"].as_str().unwrap().to_string();
        let size = version["downloads"]["client"]["size"].as_u64().unwrap();
        let t = std::time::Instant::now();
        run(crate::net::fetch_to(&client, &url, &dest, None, &mut |_, _| {})).unwrap();
        let secs = t.elapsed().as_secs_f64();
        let len = std::fs::metadata(&dest).unwrap().len();
        assert_eq!(len, size);
        println!("{} bytes in {:.2}s = {}", len, secs, crate::net::speed_label(len as f64 / secs));
        assert!(zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).is_ok(), "jar must be a valid zip");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
