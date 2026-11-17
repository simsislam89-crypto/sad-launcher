//! Launcher self-update via GitHub Releases or `site/version.json`.
//! Downloads the official installer binary and passes `--update <current_exe>`
//! so only the launcher executable is replaced — `%APPDATA%\Sad Launcher`
//! (nickname, worlds, builds, settings) is never touched.

use crate::config::{GITHUB_REPO, VERSION};
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
    let asset = rel
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case("SadLauncherSetup.exe"))
        .or_else(|| rel.assets.iter().find(|a| a.name.to_lowercase().ends_with(".exe")))?;
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

/// Checks GitHub for a newer release or `site/version.json`.
/// Silent on offline / 404 / private repo.
pub async fn check(client: &reqwest::Client) -> Option<UpdateInfo> {
    let release_url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    if let Ok(resp) = client
        .get(&release_url)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(6))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if let Some(info) = parse_release_json(&text) {
                    if is_newer(&info.version, VERSION) {
                        return Some(info);
                    }
                }
            }
        }
    }

    let raw_url = format!("https://raw.githubusercontent.com/{GITHUB_REPO}/main/site/version.json");
    if let Some(info) = fetch_version_json(client, &raw_url).await {
        return Some(info);
    }

    let site = crate::config::SITE_URL.trim().trim_end_matches('/');
    if !site.is_empty() {
        let site_url = format!("{site}/version.json");
        if let Some(info) = fetch_version_json(client, &site_url).await {
            return Some(info);
        }
    }
    None
}

async fn fetch_version_json(client: &reqwest::Client, url: &str) -> Option<UpdateInfo> {
    let resp = client.get(url).timeout(Duration::from_secs(6)).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    let mut info = parse_version_json(&text)?;
    if !is_newer(&info.version, VERSION) {
        return None;
    }
    if info.size_bytes == 0 {
        if let Ok(head) = client.head(&info.url).timeout(Duration::from_secs(5)).send().await {
            if let Some(len) = head.content_length() {
                info.size_bytes = len;
            }
        }
    }
    Some(info)
}

fn temp_installer_path() -> PathBuf {
    std::env::temp_dir().join("sad-launcher-update.exe")
}

/// Removes any leftover installer binary from a previous self-update.
pub fn cleanup_temp_installer() {
    let p = temp_installer_path();
    let _ = std::fs::remove_file(&p);
    let _ = std::fs::remove_file(p.with_extension("exe.part"));
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

    let resp = client
        .get(&info.url)
        .timeout(Duration::from_secs(180))
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
    let part = dest.with_extension("exe.part");
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
        send(UpdateEvent::Progress { done, total });
    }
    file.flush()?;
    drop(file);

    if done < 1_000_000 || magic != *b"MZ" {
        let _ = std::fs::remove_file(&part);
        bail!("Скачанный файл не является установщиком Windows");
    }

    let _ = std::fs::remove_file(&dest);
    std::fs::rename(&part, &dest).context("Не подготовить установщик к запуску")?;
    send(UpdateEvent::Ready(dest.clone()));
    Ok(dest)
}

pub fn launch_installer_update(installer_exe: &Path, current_exe: &Path) -> Result<()> {
    let mut cmd = std::process::Command::new(installer_exe);
    cmd.arg("--update").arg(current_exe);
    cmd.spawn()
        .map_err(|e| anyhow!("Не удалось запустить установщик обновления: {e}"))?;
    Ok(())
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

    #[test]
    fn parses_release_and_version_json() {
        let rel = r#"{
            "tag_name": "v0.7.2",
            "draft": false,
            "prerelease": false,
            "assets": [{
                "name": "SadLauncherSetup.exe",
                "size": 18055680,
                "browser_download_url": "https://github.com/simsislam89-crypto/sad-launcher/releases/download/v0.7.2/SadLauncherSetup.exe"
            }]
        }"#;
        let info = parse_release_json(rel).unwrap();
        assert_eq!(info.version, "0.7.2");
        assert_eq!(info.size_bytes, 18055680);

        let vj = r#"{"version":"0.7.1","size":18000000,"url":"https://github.com/simsislam89-crypto/sad-launcher/raw/main/site/download/SadLauncherSetup.exe"}"#;
        let info2 = parse_version_json(vj).unwrap();
        assert_eq!(info2.version, "0.7.1");
        assert_eq!(info2.size_bytes, 18000000);
    }
}
