//! Java discovery, headless process runner, and in-app JRE download
//! (Windows zip, Linux/macOS tar.gz).

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use crate::platform::{hidden_command, java_in};

#[derive(Clone, Serialize)]
pub struct JavaInfo {
    pub path: String,
    pub version: String,
    pub major: u32,
    pub bits: u32,
}

/// Output of a hidden process run with its full console text.
pub struct RunOutput {
    pub status: ExitStatus,
    pub log: String,
}

/// Runs a program with no console window and captures both streams.
pub fn run_logged(exe: &str, args: &[String]) -> Result<RunOutput> {
    let out = hidden_command(exe)
        .args(args)
        .output()
        .with_context(|| format!("Не запустить процесс: {exe}"))?;
    let mut log = String::new();
    log.push_str(&String::from_utf8_lossy(&out.stdout));
    log.push_str(&String::from_utf8_lossy(&out.stderr));
    Ok(RunOutput { status: out.status, log })
}

/// Probing spawns `java -version` per candidate; the UI asks often, so the
/// list is reused for a minute and dropped after a runtime download.
static PROBE_CACHE: std::sync::Mutex<Option<(String, std::time::Instant, Vec<JavaInfo>)>> =
    std::sync::Mutex::new(None);

/// `java` executables found in the directories of a PATH value.
fn java_on_path(path: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    path.map(|p| std::env::split_paths(p).map(|dir| dir.join(exe)).filter(|f| f.is_file()).collect())
        .unwrap_or_default()
}

/// Every java.exe the launcher can see, sorted best-first.
pub fn probe_all(custom: &str) -> Vec<JavaInfo> {
    if let Ok(guard) = PROBE_CACHE.lock() {
        if let Some((key, at, list)) = guard.as_ref() {
            if key == custom && at.elapsed() < std::time::Duration::from_secs(60) {
                return list.clone();
            }
        }
    }
    let list = probe_all_fresh(custom);
    if let Ok(mut guard) = PROBE_CACHE.lock() {
        *guard = Some((custom.to_string(), std::time::Instant::now(), list.clone()));
    }
    list
}

/// The last probe result without probing again — for the UI thread, which
/// must never spawn `java -version` processes while drawing.
pub fn cached_all() -> Vec<JavaInfo> {
    PROBE_CACHE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|(_, _, list)| list.clone()))
        .unwrap_or_default()
}

/// The Java that would run a build (same rule as `pick_fitting`), taken
/// from the cached probe.
pub fn fitting_cached(major: u32, loader: crate::loader::Loader) -> Option<JavaInfo> {
    let all = cached_all();
    if let Some(exact) = all.iter().find(|j| j.major == major) {
        return Some(exact.clone());
    }
    all.into_iter()
        .filter(|j| crate::loader::java_fits(j.major, major, loader))
        .min_by_key(|j| j.major)
}

pub fn invalidate_probe() {
    if let Ok(mut guard) = PROBE_CACHE.lock() {
        *guard = None;
    }
}

fn probe_all_fresh(custom: &str) -> Vec<JavaInfo> {
    let mut found: Vec<JavaInfo> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in candidates(custom) {
        let key = path.display().to_string().to_lowercase();
        if !seen.insert(key) {
            continue;
        }
        if let Some(info) = read_version(&path) {
            found.push(info);
        }
    }
    found.sort_by(|a, b| b.major.cmp(&a.major).then(b.bits.cmp(&a.bits)));
    found
}

/// Looks for a usable java.exe. A configured path wins when it exists.
pub fn probe(custom: &str) -> Option<JavaInfo> {
    probe_all(custom).into_iter().next()
}

/// Java that can actually run a build: the exact major first, otherwise the
/// smallest newer one the loader tolerates (Forge wants the exact major).
pub fn pick_fitting(custom: &str, major: u32, loader: crate::loader::Loader) -> Option<JavaInfo> {
    let all = probe_all(custom);
    if let Some(exact) = all.iter().find(|j| j.major == major) {
        return Some(exact.clone());
    }
    all.into_iter()
        .filter(|j| crate::loader::java_fits(j.major, major, loader))
        .min_by_key(|j| j.major)
}

fn candidates(custom: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let custom = custom.trim();
    if !custom.is_empty() {
        let p = PathBuf::from(custom);
        if p.is_file() {
            out.push(p);
        } else if p.is_dir() {
            out.push(java_in(&p));
        }
    }

    if let Ok(home) = std::env::var("JAVA_HOME") {
        let home = PathBuf::from(home);
        out.push(java_in(&home));
    }

    // Every java on PATH, in PATH order (what `where java` / `which -a`
    // printed, without starting a process).
    out.extend(java_on_path(std::env::var_os("PATH").as_deref()));

    #[cfg(windows)]
    {
        let roots = [
            r"C:\Program Files\Java",
            r"C:\Program Files\Eclipse Adoptium",
            r"C:\Program Files\Microsoft",
            r"C:\Program Files\BellSoft",
            r"C:\Program Files\Zulu",
            r"C:\Program Files\Amazon Corretto",
            r"C:\Program Files\Semeru",
            r"C:\Program Files (x86)\Java",
        ];
        for root in roots {
            scan_vendor(Path::new(root), &mut out);
        }
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            scan_vendor(
                &PathBuf::from(local).join("Programs").join("Eclipse Adoptium"),
                &mut out,
            );
        }
    }
    #[cfg(unix)]
    {
        // Distro packages, macOS JDK bundles (`Contents/Home`), SDKMAN.
        for root in ["/usr/lib/jvm", "/usr/lib64/jvm", "/opt/java", "/Library/Java/JavaVirtualMachines"] {
            scan_vendor(Path::new(root), &mut out);
        }
        if let Some(home) = dirs::home_dir() {
            scan_vendor(&home.join(".sdkman").join("candidates").join("java"), &mut out);
            scan_vendor(&home.join("Library").join("Java").join("JavaVirtualMachines"), &mut out);
        }
    }

    // Runtimes downloaded by the launcher itself.
    let runtimes = crate::config::data_dir().join("runtime");
    if let Ok(entries) = std::fs::read_dir(&runtimes) {
        for entry in entries.flatten() {
            let dir = entry.path();
            // Only finished runtimes (`runtime/17`), never `17.unpacking`.
            let finished = entry
                .file_name()
                .to_str()
                .map(|n| n.parse::<u32>().is_ok())
                .unwrap_or(false);
            if dir.is_dir() && finished {
                if let Some(exe) = runtime_java(&dir) {
                    out.push(exe);
                }
            }
        }
    }
    out
}

fn scan_vendor(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        for home in [dir.clone(), dir.join("Contents").join("Home")] {
            let exe = java_in(&home);
            if exe.is_file() {
                out.push(exe);
            }
        }
    }
}

fn read_version(exe: &Path) -> Option<JavaInfo> {
    let out = hidden_command(exe)
        .arg("-version")
        .output()
        .ok()?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let first = text.lines().next().unwrap_or("");
    let raw = quoted(first).or_else(|| {
        first
            .split_whitespace()
            .find(|w| w.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false))
            .map(|w| w.trim_matches('"').to_string())
    })?;
    let major = if raw.starts_with("1.") {
        raw.split('.').nth(1)?.parse().ok()?
    } else {
        raw.split(|c: char| c == '.' || c == '_' || c == '-')
            .next()?
            .parse()
            .ok()?
    };
    let bits = if text.contains("64-Bit") { 64 } else { 32 };
    Some(JavaInfo {
        path: exe.display().to_string(),
        version: raw,
        major,
        bits,
    })
}

fn quoted(line: &str) -> Option<String> {
    let start = line.find('"')?;
    let rest = &line[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub fn required_for(mc: &str) -> u32 {
    let mut parts = mc.split('.');
    let a = parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(1);
    let b = parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
    let c = parts
        .next()
        .and_then(|p| p.split('-').next())
        .and_then(|p| p.parse::<u32>().ok())
        .unwrap_or(0);
    if a > 1 {
        return 25;
    }
    match (b, c) {
        (21.., _) => 21,
        (20, n) if n >= 5 => 21,
        (18..=20, _) => 17,
        (17, _) => 16,
        _ => 8,
    }
}


// ---------------------------------------------------------------- runtime download

/// Where the launcher installs its own JRE for a given major version.
pub fn runtime_dir(major: u32) -> PathBuf {
    crate::config::data_dir().join("runtime").join(major.to_string())
}

/// `bin/java` of an unpacked runtime: directly inside, one folder down
/// (`jdk-17.0.9+9-jre/bin`), or in a macOS bundle (`…/Contents/Home/bin`).
pub fn runtime_java(runtime: &Path) -> Option<PathBuf> {
    let direct = java_in(runtime);
    if direct.is_file() {
        return Some(direct);
    }
    std::fs::read_dir(runtime)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .flat_map(|p| [java_in(&p), java_in(&p.join("Contents").join("Home"))])
        .find(|exe| exe.is_file())
}

#[derive(Clone)]
pub enum JavaEvent {
    Progress { done: u64, total: u64 },
    Done(String),
    Error(String),
}

/// Marker written after a runtime was unpacked and answered `-version`.
const RUNTIME_OK: &str = ".madrid-ok";

/// One download per major at a time: the settings button and an install
/// asking for the same Java never write the same archive twice.
fn runtime_lock(major: u32) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    static LOCKS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<u32, std::sync::Arc<tokio::sync::Mutex<()>>>>,
    > = std::sync::OnceLock::new();
    let map = LOCKS.get_or_init(Default::default);
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    guard.entry(major).or_default().clone()
}

/// A runtime the launcher unpacked earlier, if it is whole and starts.
/// Half-unpacked folders (closed window, full disk) are removed.
fn healthy_runtime(dir: &Path) -> Option<PathBuf> {
    let exe = runtime_java(dir)?;
    if dir.join(RUNTIME_OK).is_file() {
        return Some(exe);
    }
    if read_version(&exe).is_some() {
        let _ = std::fs::write(dir.join(RUNTIME_OK), b"ok");
        return Some(exe);
    }
    let _ = std::fs::remove_dir_all(dir);
    None
}

/// Download links for this OS/CPU, best first: Temurin JRE, Temurin JDK
/// (some majors ship no JRE), Microsoft OpenJDK (LTS only) as a mirror.
/// Windows gets zip archives, Linux/macOS tar.gz.
fn runtime_urls(major: u32) -> Vec<String> {
    use crate::platform::{ADOPTIUM_ARCH as arch, ADOPTIUM_OS as os, JAVA_ARCHIVE_EXT as ext};
    let mut urls = vec![
        format!("https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jre/hotspot/normal/eclipse?project=jdk"),
        format!("https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse?project=jdk"),
    ];
    if matches!(major, 11 | 17 | 21 | 25) {
        // Microsoft names macOS «macos», not Adoptium's «mac».
        let ms_os = if os == "mac" { "macos" } else { os };
        urls.push(format!("https://aka.ms/download-jdk/microsoft-jdk-{major}-{ms_os}-{arch}.{ext}"));
    }
    // Apple Silicon: Temurin ships no arm64 Java 8 for macOS; the x64 build
    // runs under Rosetta (and old LWJGL 2 natives are x64-only anyway).
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        urls.push(format!("https://api.adoptium.net/v3/binary/latest/{major}/ga/mac/x64/jre/hotspot/normal/eclipse?project=jdk"));
        urls.push(format!("https://api.adoptium.net/v3/binary/latest/{major}/ga/mac/x64/jdk/hotspot/normal/eclipse?project=jdk"));
    }
    urls
}

/// The downloaded file really is a Java archive (a redirect to an HTML
/// error page is not): zip on Windows, gzip elsewhere.
fn is_java_archive(path: &Path) -> bool {
    if cfg!(windows) {
        std::fs::File::open(path)
            .ok()
            .and_then(|f| zip::ZipArchive::new(f).ok())
            .is_some()
    } else {
        use std::io::Read;
        let mut magic = [0u8; 2];
        std::fs::File::open(path)
            .and_then(|mut f| f.read_exact(&mut magic))
            .map(|_| magic == [0x1f, 0x8b])
            .unwrap_or(false)
    }
}

/// Downloads and unpacks a Java runtime into the launcher's runtime dir.
/// Unpacking goes into a temp folder that is renamed only when complete,
/// so a broken half-runtime can never be picked up later.
pub async fn download_runtime<F: FnMut(JavaEvent) + Send>(
    client: &reqwest::Client,
    major: u32,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<String> {
    let lock = runtime_lock(major);
    let _guard = lock.lock().await;
    let dest_dir = runtime_dir(major);
    if let Some(existing) = healthy_runtime(&dest_dir) {
        invalidate_probe();
        on(JavaEvent::Done(existing.display().to_string()));
        return Ok(existing.display().to_string());
    }
    on(JavaEvent::Progress { done: 0, total: 0 });
    let root = dest_dir.parent().map(Path::to_path_buf).unwrap_or_else(|| dest_dir.clone());
    std::fs::create_dir_all(&root)?;
    let zip_path = root.join(format!("{major}.download.{}", crate::platform::JAVA_ARCHIVE_EXT));
    let stage = root.join(format!("{major}.unpacking"));

    let mut last_err = anyhow::anyhow!("Нет ссылки на Java {major}");
    let mut fetched = false;
    for url in runtime_urls(major) {
        let _ = std::fs::remove_file(&zip_path);
        let mut report = |done: u64, total: u64| on(JavaEvent::Progress { done, total });
        match crate::minecraft::download_file_progress(client, &url, &zip_path, Some(cancel), &mut report).await {
            Ok(()) => {
                // A redirect to an HTML error page is not an archive.
                if is_java_archive(&zip_path) {
                    fetched = true;
                    break;
                }
                last_err = anyhow::anyhow!("Сервер Java прислал не архив");
            }
            Err(e) => {
                if e.to_string().contains("Отменено") {
                    let _ = std::fs::remove_file(&zip_path);
                    return Err(e);
                }
                last_err = e;
            }
        }
    }
    if !fetched {
        let _ = std::fs::remove_file(&zip_path);
        return Err(last_err.context(format!("Java {major} не скачалась — проверь интернет")));
    }

    let unpack = {
        let zip_path = zip_path.clone();
        let stage = stage.clone();
        tokio::task::spawn_blocking(move || unpack_zip(&zip_path, &stage)).await
    };
    let _ = std::fs::remove_file(&zip_path);
    match unpack {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            let _ = std::fs::remove_dir_all(&stage);
            return Err(e.context("Архив Java не распаковался"));
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&stage);
            anyhow::bail!("Распаковка Java прервалась: {e}");
        }
    }
    let exe = runtime_java(&stage)
        .ok_or_else(|| anyhow::anyhow!("В архиве Java нет bin/{}", crate::platform::JAVA_BIN))?;
    if read_version(&exe).is_none() {
        let _ = std::fs::remove_dir_all(&stage);
        anyhow::bail!("Скачанная Java {major} не запускается — антивирус мог заблокировать файл");
    }
    let _ = std::fs::write(stage.join(RUNTIME_OK), b"ok");
    let _ = std::fs::remove_dir_all(&dest_dir);
    std::fs::rename(&stage, &dest_dir).context("Не удалось переместить Java в папку лаунчера")?;
    let exe = runtime_java(&dest_dir)
        .ok_or_else(|| anyhow::anyhow!("После распаковки {} не нашёлся", crate::platform::JAVA_BIN))?;
    invalidate_probe();
    on(JavaEvent::Done(exe.display().to_string()));
    Ok(exe.display().to_string())
}

#[cfg(windows)]
fn unpack_zip(zip_path: &Path, dest: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(dest);
    // Parallel writer: a JRE is ~500 files, Defender scans each one.
    crate::unpack::unzip(zip_path, dest).context("Архив JRE не читается")?;
    Ok(())
}

/// Linux/macOS JREs come as tar.gz; the system `tar` keeps the executable
/// bits and symlinks (`bin/java`, `lib/*.dylib`) that a JRE needs.
#[cfg(unix)]
fn unpack_zip(archive: &Path, dest: &Path) -> Result<()> {
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest)?;
    let out = hidden_command("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .output()
        .context("Не запустить tar")?;
    if !out.status.success() {
        anyhow::bail!("Архив JRE не читается: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::required_for;

    #[test]
    fn java_major_per_release() {
        assert_eq!(required_for("1.12.2"), 8);
        assert_eq!(required_for("1.16.5"), 8);
        assert_eq!(required_for("1.17.1"), 16);
        assert_eq!(required_for("1.20.1"), 17);
        assert_eq!(required_for("1.20.4"), 17);
        assert_eq!(required_for("1.20.6"), 21);
        assert_eq!(required_for("1.21"), 21);
        assert_eq!(required_for("1.21.4"), 21);
    }
}
