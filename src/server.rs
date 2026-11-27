//! The launcher hosts the Madrid Minecraft server itself: the official
//! Forge installer runs headless once, then `java` boots the server with no
//! console window — logs stream into the launcher UI instead.

use crate::config::{self, Config};
use crate::minecraft;
use crate::shared::{LogKind, Shared};
use anyhow::{anyhow, bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct ServerMarker {
    pub mc: String,
    pub build: String,
    pub loader: String,
}

fn marker_path(server: &Path) -> PathBuf {
    server.join("madrid-server.json")
}

fn read_marker(server: &Path) -> Option<ServerMarker> {
    std::fs::read_to_string(marker_path(server))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

fn write_marker(server: &Path, mc: &str, build: &str, loader: &str) {
    let marker = ServerMarker {
        mc: mc.to_string(),
        build: build.to_string(),
        loader: loader.to_string(),
    };
    if let Ok(text) = serde_json::to_string_pretty(&marker) {
        let _ = std::fs::write(marker_path(server), text);
    }
}

/// Called once when the launcher window opens: respects the setting,
/// then drives install + start in the background.
pub fn bootstrap(shared: Arc<Shared>) {
    if !shared.cfg().auto_server {
        shared.set_mc_stage("отключён в настройках");
        shared.log(LogKind::Info, "Хостинг сервера выключен в настройках");
        return;
    }
    start_async(shared);
}

/// Installs (when needed) and starts the server in a background thread.
pub fn start_async(shared: Arc<Shared>) {
    if shared.is_mc_busy() {
        return;
    }
    if shared.is_mc_running() {
        return;
    }
    shared.mc_busy.store(true, Ordering::SeqCst);
    std::thread::Builder::new()
        .name("mc-host".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ensure_and_start(&shared)
            }));
            if let Ok(Err(e)) = result {
                shared.set_mc_stage("ошибка");
                shared.set_mc_error(Some(format!("{e:#}")));
                shared.log(LogKind::Error, format!("Сервер: {e:#}"));
            }
            shared.mc_busy.store(false, Ordering::SeqCst);
        })
        .ok();
}

fn target_pack(shared: &Shared, cfg: &Config) -> (String, String) {
    let installed = shared.installed();
    if let Some(v) = installed.iter().find(|v| v.id == cfg.selected_version) {
        let loader = if v.loader == "vanilla" {
            "vanilla"
        } else {
            "forge"
        };
        return (v.mc.clone(), loader.into());
    }
    (config::MADRID_MC.to_string(), "forge".into())
}

fn required_java_major(shared: &Shared, cfg: &Config, mc: &str) -> u32 {
    let installed = shared.installed();
    if let Some(v) = installed.iter().find(|v| v.id == cfg.selected_version) {
        return minecraft::required_java(cfg, &v.id);
    }
    crate::java::required_for(mc)
}

fn ensure_and_start(shared: &Arc<Shared>) -> Result<()> {
    let cfg = shared.cfg();
    let server = cfg.server_dir();
    let (mc, loader) = target_pack(shared, &cfg);
    let major = required_java_major(shared, &cfg, &mc);

    shared.set_mc_error(None);
    shared.set_mc_stage(format!("проверка Java {major}…"));

    let client = minecraft::http_client()?;

    let java = resolve_java(shared, &client, &cfg.java_path, major, &loader)?;

    let build = ensure_server_installed(&client, shared, &cfg, &server, &mc, &loader)?;

    minecraft::patch_server_props(&server, cfg.server_port);
    let mods = minecraft::sync_server_mods(&server, &cfg.madrid_mods_dir());
    if mods > 0 {
        shared.log(
            LogKind::Info,
            format!("Сервер: синхронизировано модов Madrid — {mods}"),
        );
    }

    shared.set_mc_stage("запуск…");
    spawn_server(shared, &java, &server, &mc, &build, &loader, cfg.server_port)
}

/// Finds the best Java that fits `major` and `loader`; downloads a Temurin JRE when none fits.
fn resolve_java(
    shared: &Arc<Shared>,
    client: &reqwest::Client,
    custom: &str,
    major: u32,
    loader: &str,
) -> Result<crate::java::JavaInfo> {
    let parsed_loader = crate::loader::Loader::parse(loader);
    if let Some(j) = crate::java::pick_fitting(custom, major, parsed_loader) {
        return Ok(j);
    }
    shared.set_mc_stage(format!("скачиваю Java {major}…"));
    shared.log(
        LogKind::Info,
        format!(
            "Java {major} не найдена — качаю Temurin JRE, это один раз",
        ),
    );
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let path = tokio_block(async {
        let mut noop = |_e: crate::java::JavaEvent| {};
        crate::java::download_runtime(client, major, &cancel, &mut noop).await
    })?;
    shared.log(LogKind::Info, format!("Java {major} установлена: {path}"));
    crate::java::pick_fitting(custom, major, parsed_loader)
        .ok_or_else(|| anyhow!("После установки Java {major} она не нашлась"))
}

fn tokio_block<T, F: std::future::Future<Output = Result<T>>>(fut: F) -> Result<T> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Не удалось создать runtime")?;
    rt.block_on(fut)
}

/// Downloads and installs the server (Forge `--installServer`, or the vanilla
/// server jar), reusing a previous install when the marker matches.
fn ensure_server_installed(
    client: &reqwest::Client,
    shared: &Arc<Shared>,
    cfg: &Config,
    server: &Path,
    mc: &str,
    loader: &str,
) -> Result<String> {
    std::fs::create_dir_all(server).context("Не создать папку сервера")?;
    minecraft::prepare_server_dir(server, cfg.server_port);

    let java = resolve_java(shared, client, &cfg.java_path, required_java_major(shared, cfg, mc), loader)?;

    if loader == "vanilla" {
        let marker = read_marker(server);
        let jar = server.join("minecraft_server.jar");
        if jar.exists() {
            if let Some(m) = marker {
                if m.mc == mc && m.loader == "vanilla" {
                    return Ok(m.build);
                }
            }
        }
        shared.set_mc_stage(format!("Скачивание сервера {mc}…"));
        shared.log(LogKind::Info, format!("Сервер {mc}: скачивание vanilla"));
        let build = tokio_block(download_vanilla_server(client, mc, server))?;
        write_marker(server, mc, "vanilla", "vanilla");
        let _ = java;
        return Ok(build);
    }

    let build = tokio_block(minecraft::forge_build(client, mc))?;
    let marker = read_marker(server);
    if let Some(m) = &marker {
        if m.mc == mc && m.build == build && server_ready(server, mc, &build, "forge") {
            shared.log(
                LogKind::Info,
                format!("Сервер Forge {mc}-{build} уже установлен"),
            );
            return Ok(build);
        }
    }

    shared.set_mc_stage(format!("Установщик Forge {mc}-{build}…"));
    shared.log(
        LogKind::Info,
        format!("Сервер Forge {mc}-{build}: установка"),
    );
    let installer = tokio_block(minecraft::forge_installer(client, mc, &build))?;

    let out = crate::java::run_logged(&java.path, &[
        "-jar".to_string(),
        installer.display().to_string(),
        "--installServer".to_string(),
        server.display().to_string(),
    ])
    .context("Установщик Forge не запустился")?;

    if !out.status.success() {
        let tail: Vec<&str> = out
            .log
            .lines()
            .filter(|l| !l.trim().is_empty())
            .collect::<Vec<_>>();
        let tail = tail.iter().rev().take(6).rev().cloned().collect::<Vec<_>>();
        bail!("Установка сервера Forge не удалась:\n{}", tail.join("\n"));
    }
    if !server_ready(server, mc, &build, "forge") {
        bail!("Установщик завершился, но файлы сервера не появились");
    }

    // Installer jar is re-downloadable — keep the disk lean.
    let _ = std::fs::remove_file(&installer);
    write_marker(server, mc, &build, "forge");
    shared.log(
        LogKind::Info,
        format!("Сервер Forge {mc}-{build} установлен"),
    );
    Ok(build)
}

fn server_ready(server: &Path, mc: &str, build: &str, loader: &str) -> bool {
    if loader == "vanilla" {
        return server.join("minecraft_server.jar").exists();
    }
    if win_args(server, mc, build).is_some() {
        return true;
    }
    legacy_server_jar(server).is_some()
}

/// Forge's JVM argfile for this OS (`win_args.txt` / `unix_args.txt`).
fn win_args(server: &Path, mc: &str, build: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) { "win_args.txt" } else { "unix_args.txt" };
    let rel = format!("libraries/net/minecraftforge/forge/{mc}-{build}/{file}");
    let path = server.join(rel);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn legacy_server_jar(server: &Path) -> Option<PathBuf> {
    std::fs::read_dir(server)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.starts_with("forge-") && s.ends_with("universal.jar"))
                .unwrap_or(false)
        })
}

async fn download_vanilla_server(
    client: &reqwest::Client,
    mc: &str,
    server: &Path,
) -> Result<String> {
    let manifest = minecraft::fetch_manifest(client).await?;
    let url = manifest
        .versions
        .iter()
        .find(|v| v.id == mc)
        .map(|v| v.url.clone())
        .ok_or_else(|| anyhow!("Версия {mc} не найдена в манифесте"))?;
    let text = client
        .get(&url)
        .send()
        .await
        .context("Файл версии недоступен")?
        .error_for_status()?
        .text()
        .await?;
    let version: minecraft::VersionJson =
        serde_json::from_str(&text).context("Файл версии не читается")?;
    let artifact = version
        .downloads
        .and_then(|d| d.server)
        .ok_or_else(|| anyhow!("Mojang не даёт серверный файл для {mc}"))?;
    let dest = server.join("minecraft_server.jar");
    minecraft::download_file(client, &artifact.url, &dest, None)
        .await
        .context("Серверный jar не скачался")?;
    Ok(mc.to_string())
}

/// Boots `java` with no console and pipes its output into the launcher.
fn spawn_server(
    shared: &Arc<Shared>,
    java: &crate::java::JavaInfo,
    server: &Path,
    mc: &str,
    build: &str,
    loader: &str,
    port: u16,
) -> Result<()> {
    let mem = cfg_mem(shared).clamp(1024, 16384);
    let mut args: Vec<String> = vec![
        format!("-Xmx{mem}M"),
        format!("-Xms{}M", (mem / 2).max(512)),
    ];
    args.extend(crate::launch::perf_gc_flags(mem as u64));

    if loader == "forge" {
        if let Some(file) = win_args(server, mc, build) {
            let text = std::fs::read_to_string(&file)
                .with_context(|| format!("Не читать {}", file.display()))?;
            // Java argfiles are whitespace tokens; each line may hold several.
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                args.extend(tokenize(line));
            }
        } else if let Some(jar) = legacy_server_jar(server) {
            args.push("-jar".into());
            args.push(jar.display().to_string());
        } else {
            bail!("Файлы сервера Forge не найдены — переустановите сервер");
        }
    } else {
        args.push("-jar".into());
        args.push(server.join("minecraft_server.jar").display().to_string());
    }
    args.push("nogui".into());

    let mut cmd = crate::platform::hidden_command(&java.path);
    cmd.args(&args);
    cmd.current_dir(server);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("Java не запустилась: {}", java.path))?;

    let log_path = {
        let mut p = config::data_dir();
        p.push("logs");
        let _ = std::fs::create_dir_all(&p);
        p.push(format!("server-{}.log", crate::util::file_stamp()));
        p
    };
    shared.start_mc_log(log_path.clone());
    shared.push_mc(format!(
        "[Madrid] сервер {mc} · {} · {} МБ · порт {port}",
        if loader == "forge" {
            format!("Forge {build}")
        } else {
            "vanilla".to_string()
        },
        mem
    ));
    shared.log(
        LogKind::Info,
        format!("Сервер запущен на порту {port} ({mc})"),
    );

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        spawn_reader(stdout, tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(stderr, tx);
    }
    shared.set_mc_child(Some(child));
    shared.mc_running.store(true, Ordering::SeqCst);
    shared.set_mc_port(port);
    shared.set_mc_stage("работает");
    shared.set_mc_error(None);
    if let Ok(mut t) = shared.mc_started.lock() {
        *t = Some(std::time::Instant::now());
    }

    {
        let shared = shared.clone();
        std::thread::spawn(move || {
            while let Ok(line) = rx.recv() {
                shared.push_mc(line);
            }
        });
    }
    {
        let shared = shared.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(500));
            let mut guard = match shared.mc_child.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            let Some(child) = guard.as_mut() else {
                break;
            };
            match child.try_wait() {
                Ok(Some(status)) => {
                    drop(guard);
                    shared.mc_running.store(false, Ordering::SeqCst);
                    shared.set_mc_port(0);
                    shared.set_mc_stage("остановлен");
                    shared.push_mc(format!("[Madrid] сервер завершился · {status}"));
                    shared.log(LogKind::Warn, "Сервер остановился");
                    if let Ok(mut g) = shared.mc_child.lock() {
                        *g = None;
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    drop(guard);
                    shared.mc_running.store(false, Ordering::SeqCst);
                    shared.set_mc_port(0);
                    shared.set_mc_stage("остановлен");
                    shared.push_mc(format!("[Madrid] ошибка процесса: {e}"));
                    if let Ok(mut g) = shared.mc_child.lock() {
                        *g = None;
                    }
                    break;
                }
            }
        });
    }

    Ok(())
}

/// Java argfile tokenizer: whitespace-separated tokens, double quotes grouped.
fn tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                started = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(cur);
    }
    out
}

fn cfg_mem(shared: &Arc<Shared>) -> u32 {
    shared.cfg().server_memory_mb.max(1024)
}

fn spawn_reader<R: std::io::Read + Send + 'static>(
    mut reader: R,
    tx: std::sync::mpsc::Sender<String>,
) {
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut buf = std::io::BufReader::new(&mut reader);
        let mut line = String::new();
        loop {
            line.clear();
            match buf.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let text = line.trim_end_matches(['\r', '\n']).to_string();
                    if !text.is_empty() && tx.send(text).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

/// Graceful stop via the console stdin, then force-kill as a fallback.
pub fn stop(shared: &Arc<Shared>) -> bool {
    if !shared.is_mc_running() {
        return false;
    }
    shared.set_mc_stage("останавливается…");
    let mut child_pid = None;
    let wrote = shared.mc_child.lock().ok().and_then(|mut g| {
        g.as_mut().map(|c| {
            child_pid = Some(c.id());
            if let Some(stdin) = c.stdin.as_mut() {
                let _ = stdin.write_all(b"stop\n");
                let _ = stdin.flush();
                true
            } else {
                false
            }
        })
    });
    if wrote != Some(true) {
        return false;
    }
    let shared = shared.clone();
    std::thread::Builder::new()
        .name("mc-stop".into())
        .spawn(move || {
            for _ in 0..40 {
                std::thread::sleep(Duration::from_millis(500));
                if !shared.is_mc_running() {
                    return;
                }
            }
            if shared.is_mc_running() {
                if let Ok(mut guard) = shared.mc_child.lock() {
                    if let Some(c) = guard.as_mut() {
                        if Some(c.id()) == child_pid {
                            let _ = c.kill();
                        }
                    }
                }
            }
        })
        .ok();
    true
}

/// Stop, wait for the exit, then boot again — for settings like RAM/port.
pub fn restart(shared: &Arc<Shared>) {
    if !shared.is_mc_running() {
        start_async(shared.clone());
        return;
    }
    shared.set_mc_stage("перезапуск…");
    let shared = shared.clone();
    std::thread::Builder::new()
        .name("mc-restart".into())
        .spawn(move || {
            if let Ok(mut guard) = shared.mc_child.lock() {
                if let Some(c) = guard.as_mut() {
                    if let Some(stdin) = c.stdin.as_mut() {
                        let _ = stdin.write_all(b"stop\n");
                        let _ = stdin.flush();
                    }
                }
            }
            for _ in 0..30 {
                std::thread::sleep(Duration::from_millis(500));
                if !shared.is_mc_running() {
                    break;
                }
            }
            if shared.is_mc_running() {
                shutdown(&shared);
            }
            std::thread::sleep(Duration::from_millis(300));
            start_async(shared);
        })
        .ok();
}

/// Hard stop used when the launcher window closes.
pub fn shutdown(shared: &Arc<Shared>) {
    if let Ok(mut guard) = shared.mc_child.lock() {
        if let Some(c) = guard.as_mut() {
            if let Some(stdin) = c.stdin.as_mut() {
                let _ = stdin.write_all(b"stop\n");
                let _ = stdin.flush();
            }
            let _ = c.kill();
        }
        *guard = None;
    }
    shared.mc_running.store(false, Ordering::SeqCst);
    shared.set_mc_port(0);
}

/// Best-effort local LAN address for the "invite friends" hint.
/// This PC in the home network; a VPN adapter never wins here.
pub fn local_ip() -> Option<String> {
    crate::upnp::home_ip().or_else(crate::upnp::default_route_ip)
}
