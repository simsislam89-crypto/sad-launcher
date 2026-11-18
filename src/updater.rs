//! Launcher self-update via GitHub Releases or `site/version.json`.
//! Downloads the official installer binary and passes `--update <current_exe>`
//! so only the launcher executable is replaced — `%APPDATA%\Sad Launcher`
//! (nickname, worlds, builds, settings) is never touched.

use crate::config::GITHUB_REPO;
use anyhow::{anyhow, bail, Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub size_bytes: u64,
    pub url: String,
}

#[derive(Clone, Debug)]
pub enum UpdateEvent {
    Stage(String),
    Progress { done: u64, total: u64 },
    Ready(PathBuf),
    Error(String),
}

#[derive(Deserialize)]
struct GhRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    browser_download_url: String,
}

#[derive(Deserialize)]
struct VersionJson {
    #[serde(default)]
    version: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    url: String,
}

/// The release asset this build updates itself from (names match
/// `.github/workflows/release.yml`).
pub const ASSET_NAME: &str = if cfg!(windows) {
    "SadLauncherSetup.exe"
} else if cfg!(target_os = "macos") {
    "SadLauncher-macos-universal.tar.gz"
} else if cfg!(target_arch = "aarch64") {
    "sad-launcher-linux-aarch64.tar.gz"
} else {
    "sad-launcher-linux-x86_64.tar.gz"
};

pub fn parse_semver(raw: &str) -> Option<(u32, u32, u32)> {
    let cleaned = raw.trim().trim_start_matches(['v', 'V']);
    let core = cleaned.split(['-', '+']).next().unwrap_or(cleaned);
    let mut parts = core.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_semver(remote), parse_semver(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

pub fn is_allowed_url(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url.trim()) else {
        return false;
    };
    if parsed.scheme() != "https" {
        return false;
    }
    matches!(
        parsed.host_str(),
        Some(
            "github.com"
                | "objects.githubusercontent.com"
                | "release-assets.githubusercontent.com"
                | "raw.githubusercontent.com"
                | "rkt5jy.bot-keep.xyz"
        )
    )
}

pub fn parse_release_json(json: &str) -> Option<UpdateInfo> {
    let rel: GhRelease = serde_json::from_str(json).ok()?;
    if rel.draft || rel.prerelease {
        return None;
    }
    let version = rel.tag_name.trim().trim_start_matches(['v', 'V']).to_string();
    let asset = rel.assets.iter().find(|a| a.name.eq_ignore_ascii_case(ASSET_NAME)).or_else(|| {
        if cfg!(windows) {
            rel.assets.iter().find(|a| a.name.to_lowercase().ends_with(".exe"))
        } else {
            None
        }
    })?;
    if !is_allowed_url(&asset.browser_download_url) {
        return None;
    }
    Some(UpdateInfo {
        version,
        size_bytes: asset.size,
        url: asset.browser_download_url.clone(),
    })
}

pub fn parse_version_json(json: &str) -> Option<UpdateInfo> {
    let vj: VersionJson = serde_json::from_str(json).ok()?;
    let version = vj.version.trim().trim_start_matches(['v', 'V']).to_string();
    if version.is_empty() || !is_allowed_url(&vj.url) {
        return None;
    }
    Some(UpdateInfo {
        version,
        size_bytes: vj.size,
        url: vj.url,
    })
}

/// [`probe`] collapsed to «newer version or nothing» (live end-to-end test).
#[cfg(test)]
pub async fn check_against(client: &reqwest::Client, current: &str) -> Option<UpdateInfo> {
    probe(client, current).await.ok().flatten()
}

/// `Ok(Some)` — newer version, `Ok(None)` — this one is the latest,
/// `Err` — no update source answered (offline, GitHub down). The manual
/// «Проверить обновления» button tells these apart.
pub async fn probe(client: &reqwest::Client, current: &str) -> Result<Option<UpdateInfo>, ()> {
    let mut reached = false;
    let release_url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    if let Ok(resp) = client
        .get(&release_url)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(6))
        .send()
        .await
    {
        // 404 = no release published yet: GitHub did answer.
        reached |= resp.status().is_success() || resp.status() == reqwest::StatusCode::NOT_FOUND;
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if let Some(info) = parse_release_json(&text) {
                    if is_newer(&info.version, current) {
                        return Ok(Some(info));
                    }
                }
            }
        }
    }

    // site/version.json describes the Windows installer only.
    if cfg!(windows) {
        let raw_url = format!("https://raw.githubusercontent.com/{GITHUB_REPO}/main/site/version.json");
        let site = crate::config::SITE_URL.trim().trim_end_matches('/');
        let mut urls = vec![raw_url];
        if !site.is_empty() {
            urls.push(format!("{site}/version.json"));
        }
        for url in urls {
            match fetch_version_json(client, &url, current).await {
                Ok(Some(info)) => return Ok(Some(info)),
                Ok(None) => reached = true,
                Err(()) => {}
            }
        }
    }
    if reached {
        Ok(None)
    } else {
        Err(())
    }
}

/// `Err` — the file could not be fetched or read.
async fn fetch_version_json(client: &reqwest::Client, url: &str, current: &str) -> Result<Option<UpdateInfo>, ()> {
    let resp = client.get(url).timeout(Duration::from_secs(6)).send().await.map_err(|_| ())?;
    if !resp.status().is_success() {
        return Err(());
    }
    let text = resp.text().await.map_err(|_| ())?;
    let mut info = parse_version_json(&text).ok_or(())?;
    if !is_newer(&info.version, current) {
        return Ok(None);
    }
    if info.size_bytes == 0 {
        if let Ok(head) = client.head(&info.url).timeout(Duration::from_secs(5)).send().await {
            if let Some(len) = head.content_length() {
                info.size_bytes = len;
            }
        }
    }
    Ok(Some(info))
}

fn temp_installer_path() -> PathBuf {
    std::env::temp_dir().join(if cfg!(windows) { "sad-launcher-update.exe" } else { "sad-launcher-update.tar.gz" })
}

/// Removes any leftover installer binary from a previous self-update.
pub fn cleanup_temp_installer() {
    let p = temp_installer_path();
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(p.with_extension("exe.part"));
    let _ = std::fs::remove_file(p.with_extension("part"));
}

pub async fn download_installer<F: FnMut(UpdateEvent) + Send>(
    client: &reqwest::Client,
    info: &UpdateInfo,
    cancel: &AtomicBool,
    send: &mut F,
) -> Result<PathBuf> {
    if !is_allowed_url(&info.url) {
        bail!("Недопустимый источник обновления");
    }
    send(UpdateEvent::Stage("Скачиваю установщик…".into()));

    // No total timeout: 18 MB at 100 KB/s takes 3 minutes and used to be
    // cut off at 180 s. A stalled read is still cut by the client.
    let resp = client
        .get(&info.url)
        .send()
        .await
        .context("Не удалось скачать обновление с GitHub")?;
    if !resp.status().is_success() {
        bail!("GitHub ответил ошибкой {}", resp.status().as_u16());
    }

    let total = resp.content_length().unwrap_or(info.size_bytes);
    if total > 200 * 1024 * 1024 {
        bail!("Файл обновления подозрительно большой");
    }

    let dest = temp_installer_path();
    let part = dest.with_extension("part");
    let _ = std::fs::remove_file(&part);
    let mut file = std::fs::File::create(&part).context("Не создать временный файл обновления")?;

    let mut done: u64 = 0;
    let mut stream = resp.bytes_stream();
    let mut magic = [0u8; 2];
    let mut magic_len = 0usize;

    while let Some(chunk) = stream.next().await {
        if cancel.load(Ordering::Relaxed) {
            drop(file);
            let _ = std::fs::remove_file(&part);
            bail!("Отменено");
        }
        let bytes = chunk.context("Обрыв связи при загрузке обновления")?;
        if magic_len < 2 {
            for &b in bytes.iter().take(2 - magic_len) {
                magic[magic_len] = b;
                magic_len += 1;
            }
        }
        done += bytes.len() as u64;
        if done > 200 * 1024 * 1024 {
            drop(file);
            let _ = std::fs::remove_file(&part);
            bail!("Файл обновления превысил лимит размера");
        }
        file.write_all(&bytes).context("Ошибка записи файла обновления")?;
        crate::net::count(bytes.len());
        send(UpdateEvent::Progress { done, total });
    }
    file.flush()?;
    drop(file);

    // Windows: an exe (MZ). macOS/Linux: a gzip archive.
    let expected: [u8; 2] = if cfg!(windows) { *b"MZ" } else { [0x1f, 0x8b] };
    if done < 1_000_000 || magic != expected {
        let _ = std::fs::remove_file(&part);
        bail!("Скачанный файл не похож на сборку Sad Launcher");
    }

    let _ = std::fs::remove_file(&dest);
    std::fs::rename(&part, &dest).context("Не подготовить установщик к запуску")?;
    send(UpdateEvent::Ready(dest.clone()));
    Ok(dest)
}

#[cfg(windows)]
pub fn launch_installer_update(installer_exe: &Path, current_exe: &Path) -> Result<()> {
    let mut cmd = std::process::Command::new(installer_exe);
    cmd.arg("--update").arg(current_exe);
    cmd.spawn()
        .map_err(|e| anyhow!("Не удалось запустить установщик обновления: {e}"))?;
    Ok(())
}

/// macOS / Linux: unpack the release archive, swap the running binary for
/// the new one (a rename — the old inode lives on until this process exits)
/// and start it. Data in the user's data folder is untouched.
#[cfg(not(windows))]
pub fn launch_installer_update(archive: &Path, current_exe: &Path) -> Result<()> {
    let stage = std::env::temp_dir().join("sad-launcher-update");
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage)?;
    let out = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(&stage)
        .output()
        .context("Не запустить tar")?;
    if !out.status.success() {
        bail!("Архив обновления не распаковался");
    }
    let fresh = find_binary(&stage).context("В архиве нет sad-launcher")?;
    let next = current_exe.with_extension("new");
    std::fs::copy(&fresh, &next).context("Нет прав на папку лаунчера")?;
    crate::platform::make_executable(&next);
    std::fs::rename(&next, current_exe).context("Не заменить файл лаунчера")?;
    let _ = std::fs::remove_dir_all(&stage);
    let _ = std::fs::remove_file(archive);
    std::process::Command::new(current_exe)
        .env("MADRID_RELAUNCH_WAIT", "1")
        .spawn()
        .map_err(|e| anyhow!("Не удалось запустить новую версию: {e}"))?;
    Ok(())
}

/// `sad-launcher` anywhere inside the unpacked archive (the Linux tarball
/// has it at the top, the macOS one in `Sad Launcher.app/Contents/MacOS`).
#[cfg(not(windows))]
fn find_binary(dir: &Path) -> Option<PathBuf> {
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_binary(&path) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some("sad-launcher") {
            return Some(path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_compares_cleanly() {
        assert!(is_newer("0.7.1", "0.7.0"));
        assert!(is_newer("v0.8.0", "0.7.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.7.0", "0.7.0"));
        assert!(!is_newer("0.6.9", "0.7.0"));
        assert!(!is_newer("bad", "0.7.0"));
    }

    #[test]
    fn allowed_urls_only_https_github() {
        assert!(is_allowed_url(
            "https://github.com/simsislam89-crypto/sad-launcher/raw/main/site/download/SadLauncherSetup.exe"
        ));
        assert!(is_allowed_url(
            "https://objects.githubusercontent.com/github-production-release-asset/123"
        ));
        assert!(is_allowed_url(
            "https://rkt5jy.bot-keep.xyz/download/SadLauncherSetup.exe"
        ));
        assert!(!is_allowed_url("http://github.com/a/b.exe"));
        assert!(!is_allowed_url("https://evil.example.com/setup.exe"));
    }

    /// `cargo test --release live_update_e2e -- --ignored --nocapture`:
    /// pretends to be 0.0.1, finds the published update, downloads the real
    /// installer and lets it `--update` a dummy exe in %TEMP%; the relaunched
    /// launcher takes a screenshot and closes itself (MADRID_DEV_SHOT).
    #[test]
    #[ignore]
    fn live_update_e2e() {
        let dir = std::env::temp_dir().join("sad-update-e2e");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("Sad Launcher.exe");
        std::fs::write(&target, b"old launcher").unwrap();
        let shot = dir.join("relaunched.png");
        let rt = crate::net::rt();
        let client = crate::net::client();
        let info = rt.block_on(check_against(&client, "0.0.1")).expect("update found");
        println!("found {} ({} bytes) at {}", info.version, info.size_bytes, info.url);
        let cancel = AtomicBool::new(false);
        let installer = rt
            .block_on(download_installer(&client, &info, &cancel, &mut |_| {}))
            .expect("installer downloaded");
        println!("installer {} bytes", std::fs::metadata(&installer).unwrap().len());
        let status = std::process::Command::new(&installer)
            .arg("--update")
            .arg(&target)
            .env("MADRID_SECOND_INSTANCE", "1")
            .env("MADRID_DEV_TAB", "play")
            .env("MADRID_DEV_SHOT", &shot)
            .env("MADRID_DEV_SHOT_DELAY", "4")
            .status()
            .expect("installer ran");
        assert!(status.success());
        let new = std::fs::read(&target).unwrap();
        assert!(new.len() > 1_000_000 && &new[..2] == b"MZ", "exe replaced");
        println!("replaced: {} bytes", new.len());
        for _ in 0..60 {
            if shot.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        assert!(shot.exists(), "relaunched launcher drew a frame");
        println!("relaunched OK, screenshot {}", shot.display());
    }

    #[test]
    fn parses_release_and_version_json() {
        // A release with every platform's asset: each build picks its own.
        let asset = |name: &str, size: u64| {
            format!(
                r#"{{"name":"{name}","size":{size},"browser_download_url":"https://github.com/simsislam89-crypto/sad-launcher/releases/download/v0.7.2/{name}"}}"#
            )
        };
        let rel = format!(
            r#"{{"tag_name":"v0.7.2","draft":false,"prerelease":false,"assets":[{},{},{},{}]}}"#,
            asset("SadLauncherSetup.exe", 18055680),
            asset("SadLauncher-macos-universal.tar.gz", 21000000),
            asset("sad-launcher-linux-x86_64.tar.gz", 15000000),
            asset("sad-launcher-linux-aarch64.tar.gz", 14000000),
        );
        let info = parse_release_json(&rel).unwrap();
        assert_eq!(info.version, "0.7.2");
        assert!(info.url.ends_with(ASSET_NAME), "{} picks {}", ASSET_NAME, info.url);
        // A Windows-only release offers nothing to macOS/Linux.
        let win_only = format!(
            r#"{{"tag_name":"v0.7.2","draft":false,"prerelease":false,"assets":[{}]}}"#,
            asset("SadLauncherSetup.exe", 18055680)
        );
        assert_eq!(parse_release_json(&win_only).is_some(), cfg!(windows));

        let vj = r#"{"version":"0.7.1","size":18000000,"url":"https://github.com/simsislam89-crypto/sad-launcher/raw/main/site/download/SadLauncherSetup.exe"}"#;
        let info2 = parse_version_json(vj).unwrap();
        assert_eq!(info2.version, "0.7.1");
        assert_eq!(info2.size_bytes, 18000000);
    }
}
