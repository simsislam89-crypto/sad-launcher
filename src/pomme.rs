//! Pomme — a native Rust + Vulkan Minecraft client with a built-in
//! singleplayer server. One portable exe: no Java, no local server.
//! Official Pomme builds ask for a Microsoft sign-in once and keep it in
//! the Windows credential store; the launcher never touches that.

use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use crate::shared::Shared;

pub const LATEST: &str = "https://api.github.com/repos/PommeMC/Client/releases/latest";
#[cfg(windows)]
const ASSET: &str = "pomme-client-windows-x64.zip";

/// Release asset for this OS: the exact Windows name, elsewhere any zip
/// whose name mentions the OS (and the CPU, when several match).
fn asset_matches(name: &str) -> bool {
    #[cfg(windows)]
    {
        name == ASSET
    }
    #[cfg(not(windows))]
    {
        let n = name.to_ascii_lowercase();
        let os_ok = if cfg!(target_os = "macos") {
            n.contains("macos") || n.contains("darwin") || n.contains("-mac")
        } else {
            n.contains("linux")
        };
        let arch_ok = if cfg!(target_arch = "aarch64") {
            n.contains("arm64") || n.contains("aarch64") || n.contains("universal")
        } else {
            !(n.contains("arm64") || n.contains("aarch64"))
        };
        os_ok && arch_ok && n.ends_with(".zip")
    }
}

/// Kept at the historical path so existing installs keep working.
fn dir() -> PathBuf {
    crate::config::data_dir().join("rustmc").join("pomme")
}

pub fn exe() -> PathBuf {
    dir().join(if cfg!(windows) { "pomme-client.exe" } else { "pomme-client" })
}

pub fn game_dir() -> PathBuf {
    dir().join("game")
}

pub fn installed() -> bool {
    exe().is_file()
}

#[derive(Clone)]
pub enum PommeEvent {
    Stage(String),
    Progress { done: u64, total: u64 },
    Done,
    Error(String),
}

/// Downloads the latest portable Pomme build and unpacks the exe.
pub async fn install<F: FnMut(PommeEvent) + Send>(client: &reqwest::Client, on: &mut F) -> Result<()> {
    use futures_util::StreamExt;
    use std::io::Write;
    #[derive(serde::Deserialize)]
    struct Release {
        assets: Vec<Asset>,
    }
    #[derive(serde::Deserialize)]
    struct Asset {
        name: String,
        browser_download_url: String,
    }
    std::fs::create_dir_all(dir())?;
    on(PommeEvent::Stage("Ищу свежий релиз Pomme…".into()));
    let rel: Release = client
        .get(LATEST)
        .send()
        .await
        .context("GitHub не отвечает")?
        .error_for_status()?
        .json()
        .await
        .context("Список релизов не читается")?;
    let asset = rel
        .assets
        .into_iter()
        .find(|a| asset_matches(&a.name))
        .ok_or_else(|| anyhow::anyhow!("В релизе нет сборки для {}", crate::platform::OS_LABEL))?;
    on(PommeEvent::Stage("Скачиваю Pomme…".into()));
    let resp = client
        .get(asset.browser_download_url)
        .send()
        .await
        .context("Pomme не скачался")?
        .error_for_status()?;
    let total = resp.content_length().unwrap_or(0);
    let zip_path = dir().join("pomme.zip.part");
    {
        let mut file = std::io::BufWriter::new(std::fs::File::create(&zip_path)?);
        let mut stream = resp.bytes_stream();
        let mut done = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            done += chunk.len() as u64;
            file.write_all(&chunk)?;
            on(PommeEvent::Progress { done, total });
        }
        file.flush()?;
    }
    on(PommeEvent::Stage("Распаковываю…".into()));
    let unpack = tokio::task::spawn_blocking({
        let zip_path = zip_path.clone();
        move || unzip(&zip_path, &dir())
    })
    .await
    .context("Распаковка прервалась")?;
    let _ = std::fs::remove_file(&zip_path);
    unpack?;
    if !installed() {
        bail!("После распаковки {} не нашёлся", exe().display());
    }
    crate::platform::make_executable(&exe());
    on(PommeEvent::Done);
    Ok(())
}

/// The most recently played world folder in Pomme's saves, if any.
pub fn latest_world() -> Option<String> {
    let saves = game_dir().join("saves");
    std::fs::read_dir(saves)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let level = e.path().join("level.dat");
            let at = std::fs::metadata(&level).and_then(|m| m.modified()).ok()?;
            Some((at, e.file_name().to_string_lossy().to_string()))
        })
        .max_by_key(|(at, _)| *at)
        .map(|(_, name)| name)
}

/// Starts Pomme straight into the last world when there is one.
/// Returns the world name it opened.
pub fn launch(shared: &Arc<Shared>, nickname: &str) -> Result<Option<String>> {
    let exe = exe();
    if !exe.is_file() {
        bail!("Pomme не установлен");
    }
    let uuid = crate::minecraft::offline_uuid(nickname);
    let world = latest_world();
    let mut cmd = crate::platform::hidden_command(&exe);
    cmd.args([
        "--username",
        nickname,
        "--uuid",
        &uuid,
        "--game-dir",
        &game_dir().display().to_string(),
        "--width",
        "1280",
        "--height",
        "720",
    ]);
    if let Some(w) = &world {
        cmd.args(["--quick-access-singleplayer", w]);
    }
    cmd.current_dir(dir());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());
    cmd.spawn().context("Pomme не запустился")?;
    shared.log(
        crate::shared::LogKind::Info,
        match &world {
            Some(w) => format!("Pomme запущен · мир «{w}»"),
            None => "Pomme запущен".to_string(),
        },
    );
    Ok(world)
}

pub fn remove() -> Result<()> {
    if dir().exists() {
        std::fs::remove_dir_all(dir())?;
    }
    Ok(())
}

fn unzip(zip_path: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    use std::io::Read;
    let file = std::fs::File::open(zip_path)?;
    let mut zip = zip::ZipArchive::new(file).context("Архив Pomme битый")?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        std::fs::write(&out, buf)?;
    }
    Ok(())
}
