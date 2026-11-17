//! Resolving version inheritance, building the java command line, and launching.

use crate::config::{self, Config};
use crate::minecraft::{
    self, java_major_for, maven_path, rules_allow, ArgItem, Arguments, VersionJson,
};
use crate::shared::Shared;
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

pub struct Resolved {
    pub id: String,
    pub json: VersionJson,
    pub client_jar: PathBuf,
    pub natives_dir: PathBuf,
    pub java_major: u32,
}

pub fn resolve(game: &Path, id: &str) -> Result<Resolved> {
    let (json, jar_id) = load_chain(game, id, 0)?;
    let client_jar = game
        .join("versions")
        .join(&jar_id)
        .join(format!("{jar_id}.jar"));
    let natives_dir = game.join("versions").join(id).join("natives");
    let java_major = java_major_for(&json);
    Ok(Resolved {
        id: id.to_string(),
        json,
        client_jar,
        natives_dir,
        java_major,
    })
}

fn load_chain(game: &Path, id: &str, depth: usize) -> Result<(VersionJson, String)> {
    if depth > 6 {
        bail!("Слишком глубокая цепочка версий");
    }
    let path = game
        .join("versions")
        .join(id)
        .join(format!("{id}.json"));
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Нет файла версии: {}", path.display()))?;
    let mut version: VersionJson =
        serde_json::from_str(&text).context("Файл версии не читается")?;
    if version.id.is_empty() {
        version.id = id.to_string();
    }
    let Some(parent_id) = version.inherits_from.clone() else {
        let jar_id = version.jar.clone().unwrap_or_else(|| version.id.clone());
        return Ok((version, jar_id));
    };

    let (parent, parent_jar) = load_chain(game, &parent_id, depth + 1)?;
    let child_id = version.id.clone();
    let child_has_client = version
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .is_some();
    let child_jar = version.jar.clone();

    let mut merged = parent.clone();
    merged.id = child_id.clone();
    if !version.kind.is_empty() {
        merged.kind = version.kind.clone();
    }
    if !version.main_class.is_empty() {
        merged.main_class = version.main_class.clone();
    }
    if version.minecraft_arguments.is_some() {
        merged.minecraft_arguments = version.minecraft_arguments.clone();
    }
    if let Some(args) = &version.arguments {
        let base = merged.arguments.get_or_insert_with(Arguments::default);
        base.game.extend(args.game.iter().cloned());
        base.jvm.extend(args.jvm.iter().cloned());
    }
    merged.libraries.extend(version.libraries.iter().cloned());
    if version.asset_index.is_some() {
        merged.asset_index = version.asset_index.clone();
    }
    if version.assets.is_some() {
        merged.assets = version.assets.clone();
    }
    if child_has_client {
        merged.downloads = version.downloads.clone();
    }
    if version.java_version.is_some() {
        merged.java_version = version.java_version.clone();
    }
    merged.jar = child_jar.clone();

    let jar_id = if let Some(jar) = child_jar {
        jar
    } else if child_has_client {
        child_id
    } else {
        parent_jar
    };
    Ok((merged, jar_id))
}

pub fn launch(shared: &Arc<Shared>, cfg: &Config, id: &str, connect: Option<&str>) -> Result<PathBuf> {
    if id.trim().is_empty() {
        bail!("Версия не выбрана");
    }
    if shared.is_game_running() {
        bail!("Игра уже запущена");
    }
    let game = cfg.game_dir();
    minecraft::ensure_layout(&game);
    let instance = cfg.instance_dir();
    let _ = std::fs::create_dir_all(instance.join("mods"));

    // The hosted server's mods are the source of truth for the Madrid build:
    // anything its client is missing gets copied over — no «missing mods».
    // Other builds (modpacks, own builds) are never touched.
    let server_mods = crate::config::data_dir().join("server").join("mods");
    let madrid = cfg.active_profile.trim().is_empty() || cfg.active_profile == "default";
    if madrid && server_mods.is_dir() {
        let client_mods = cfg.mods_dir();
        let _ = std::fs::create_dir_all(&client_mods);
        let mut filled = 0u32;
        if let Ok(entries) = std::fs::read_dir(&server_mods) {
            for entry in entries.flatten() {
                let name = match entry.file_name().into_string() {
                    Ok(n) => n,
                    Err(_) => continue,
                };
                if !name.to_lowercase().ends_with(".jar") {
                    continue;
                }
                let from = server_mods.join(&name);
                let to = client_mods.join(&name);
                // A mod the player (or self-healing) switched off stays off.
                let disabled = client_mods.join(format!("{name}.disabled"));
                if !to.exists() && !disabled.exists() && from.is_file() {
                    if std::fs::copy(&from, &to).is_ok() {
                        filled += 1;
                    }
                }
            }
        }
        if filled > 0 {
            shared.push_game(format!(
                "[Madrid] клиенту добавлено модов с сервера: {filled}"
            ));
        }
    }

    // Jars that would kill ModLauncher before the first frame (a service
    // provider listed but missing — usually a build for another loader)
    // are switched off up front instead of crashing the game.
    let (loader, _, _) = minecraft::describe_version(&game, id);
    let off = preflight_mods(&cfg.mods_dir(), loader);
    if !off.is_empty() {
        let shown: Vec<&str> = off.iter().take(5).map(|s| s.as_str()).collect();
        shared.push_game(format!(
            "[Madrid] Отключено модов, несовместимых с {}: {} ({}{}) — они не дали бы игре запуститься",
            loader.label(),
            off.len(),
            shown.join(", "),
            if off.len() > 5 { ", …" } else { "" }
        ));
        shared.log(
            crate::shared::LogKind::Warn,
            format!("Перед запуском отключено несовместимых модов: {}", off.len()),
        );
    }

    let resolved = resolve(&game, id)?;
    if resolved.json.main_class.is_empty() {
        bail!("В файле версии нет mainClass");
    }
    let java = java_for(cfg, id, resolved.java_major).ok_or_else(|| {
        anyhow!(
            "Нужна Java {} — нажми «Играть», лаунчер скачает её сам.",
            resolved.java_major
        )
    })?;

    std::fs::create_dir_all(&resolved.natives_dir)?;
    for lib in &resolved.json.libraries {
        if !rules_allow(&lib.rules) {
            continue;
        }
        if let Some(natives) = &lib.natives {
            if let Some(template) = natives.get(crate::platform::MOJANG_OS) {
                let key = template.replace("${arch}", "64");
                let artifact = lib
                    .downloads
                    .as_ref()
                    .and_then(|d| d.classifiers.as_ref())
                    .and_then(|c| c.get(&key));
                if let Some(art) = artifact {
                    let rel = art
                        .path
                        .clone()
                        .or_else(|| maven_path(&format!("{}:{key}", lib.name)))
                        .unwrap_or_default();
                    let jar = game.join("libraries").join(rel);
                    if jar.exists() {
                        let _ = minecraft::extract_natives(
                            &jar,
                            &resolved.natives_dir,
                            lib.extract.as_ref(),
                        );
                    }
                }
            }
        }
    }

    let args = build_args(shared, cfg, &resolved, connect.map(|s| s.to_string()))?;
    fps_defaults(&instance);
    let effects_cfg = crate::effects::load();
    if let Err(e) = crate::effects::apply_to_instance(&instance, &resolved.id, &effects_cfg) {
        shared.log(crate::shared::LogKind::Warn, format!("Эффекты не применились: {e:#}"));
    }
    crate::util::prefer_dedicated_gpu(&java.path);
    // No console + ABOVE_NORMAL priority on Windows: the game wins CPU time
    // over background apps (browser tabs, updaters), never over the system.
    let mut cmd = crate::platform::game_command(&java.path);
    cmd.args(&args);
    cmd.current_dir(&instance);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .with_context(|| format!("Java не запустилась: {}", java.path))?;

    let log_path = {
        let mut p = config::data_dir();
        p.push("logs");
        let _ = std::fs::create_dir_all(&p);
        p.push(format!("game-{}.log", crate::util::file_stamp()));
        p
    };
    shared.start_game_log(log_path.clone());
    shared.push_game(format!(
        "[Madrid] {} · Java {} · {} МБ",
        resolved.id, java.version, cfg.memory_mb
    ));
    shared.log(
        crate::shared::LogKind::Info,
        format!("Запуск {} на Java {}", resolved.id, java.version),
    );

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(stdout) = child.stdout.take() {
        spawn_reader(stdout, tx.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_reader(stderr, tx);
    }
    shared.set_game(Some(child));
    shared.game_running.store(true, Ordering::SeqCst);

    if let Ok(mut m) = shared.missing_mods.lock() {
        *m = None;
    }
    {
        let shared = shared.clone();
        std::thread::spawn(move || {
            // Watches for «missing dependencies» so the launcher can fetch
            // them and restart, instead of leaving an error screen.
            let mut parser = crate::deps::MissingParser::default();
            while let Ok(line) = rx.recv() {
                let before = parser.pairs.len();
                let new_id = parser.feed(&line);
                if new_id || parser.pairs.len() != before {
                    if let Ok(mut m) = shared.missing_mods.lock() {
                        *m = Some((parser.found.clone(), parser.pairs.clone(), std::time::Instant::now()));
                    }
                }
                shared.push_game(line);
            }
        });
    }
    {
        let shared = shared.clone();
        let log_path_watch = log_path.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(400));
            let mut guard = match shared.game.lock() {
                Ok(g) => g,
                Err(_) => break,
            };
            let Some(child) = guard.as_mut() else {
                break;
            };
            match child.try_wait() {
                Ok(Some(status)) => {
                    drop(guard);
                    shared.game_running.store(false, Ordering::SeqCst);
                    shared.push_game(format!("[Madrid] Игра завершилась · {status}"));
                    if status.code() == Some(1) {
                        heal_broken_mod(shared.clone(), &log_path_watch);
                    }
                    if let Ok(mut g) = shared.game.lock() {
                        *g = None;
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    drop(guard);
                    shared.game_running.store(false, Ordering::SeqCst);
                    shared.push_game(format!("[Madrid] Ошибка процесса: {e}"));
                    if let Ok(mut g) = shared.game.lock() {
                        *g = None;
                    }
                    break;
                }
            }
        });
    }

    Ok(log_path)
}

/// Java that runs version `id`: exact major first, loader rules after.
pub fn java_for(cfg: &Config, id: &str, major: u32) -> Option<crate::java::JavaInfo> {
    let (loader, _, _) = minecraft::describe_version(&cfg.game_dir(), id);
    crate::java::pick_fitting(&cfg.java_path, major, loader)
}

/// Whether version `id` can start right now without downloading anything.
pub fn ready_to_launch(cfg: &Config, id: &str) -> bool {
    match resolve(&cfg.game_dir(), id) {
        Ok(r) => r.client_jar.exists() && java_for(cfg, id, r.java_major).is_some(),
        Err(_) => false,
    }
}

pub fn stop(shared: &Arc<Shared>) -> bool {
    if let Ok(mut guard) = shared.game.lock() {
        if let Some(child) = guard.as_mut() {
            let _ = child.kill();
            shared.game_running.store(false, Ordering::SeqCst);
            shared.push_game("[Madrid] Остановлено пользователем");
            return true;
        }
    }
    false
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

fn build_args(_shared: &Arc<Shared>, cfg: &Config, resolved: &Resolved, connect: Option<String>) -> Result<Vec<String>> {
    let game = cfg.game_dir();
    let instance = cfg.instance_dir();
    let assets = game.join("assets");
    let libraries = game.join("libraries");
    let nickname = cfg.nickname_or_default();
    let uuid = minecraft::offline_uuid(&nickname);

    let asset_index = resolved
        .json
        .asset_index
        .as_ref()
        .map(|a| a.id.clone())
        .or_else(|| resolved.json.assets.clone())
        .unwrap_or_else(|| "legacy".into());

    let mut map: HashMap<&str, String> = HashMap::new();
    map.insert("auth_player_name", nickname);
    map.insert("version_name", resolved.id.clone());
    map.insert("game_directory", instance.display().to_string());
    map.insert("assets_root", assets.display().to_string());
    map.insert("assets_index_name", asset_index);
    map.insert("auth_uuid", uuid.replace('-', ""));
    map.insert("auth_access_token", "0".into());
    map.insert("clientid", String::new());
    map.insert("auth_xuid", String::new());
    map.insert("user_type", "legacy".into());
    map.insert("version_type", {
        if resolved.json.kind.is_empty() {
            "release".into()
        } else {
            resolved.json.kind.clone()
        }
    });
    map.insert("user_properties", "{}".into());
    map.insert("natives_directory", resolved.natives_dir.display().to_string());
    map.insert("launcher_name", "SadLauncher".into());
    map.insert("launcher_version", config::VERSION.into());
    map.insert("classpath_separator", crate::platform::CLASSPATH_SEP.into());
    map.insert("library_directory", libraries.display().to_string());
    map.insert("classpath", classpath(resolved, &game)?);

    let mem = cfg.memory_mb.clamp(512, 32768);
    let mut out: Vec<String> = vec![
        format!("-Xmx{mem}M"),
        format!("-Xms{}M", (mem / 2).max(512)),
        "-Dfile.encoding=UTF-8".into(),
        "-Dminecraft.launcher.brand=SadLauncher".into(),
        format!("-Dminecraft.launcher.version={}", config::VERSION),
    ];
    out.extend(client_flags(mem as u64));
    out.push("-XX:+HeapDumpOnOutOfMemoryError".into());
    out.push(format!(
        "-XX:ErrorFile={}",
        config::data_dir().join("logs").join("jvm-crash-%p.log").display()
    ));

    let jvm_args = match &resolved.json.arguments {
        Some(args) if !args.jvm.is_empty() => collect_args(&args.jvm),
        _ => vec![
            "-Djava.library.path=${natives_directory}".into(),
            "-cp".into(),
            "${classpath}".into(),
        ],
    };
    let jar_name = resolved
        .client_jar
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    for arg in jvm_args {
        out.push(fix_ignore_list(subst(&arg, &map), &jar_name));
    }

    out.push(resolved.json.main_class.clone());

    let game_args = match &resolved.json.arguments {
        Some(args) if !args.game.is_empty() => collect_args(&args.game),
        _ => resolved
            .json
            .minecraft_arguments
            .clone()
            .unwrap_or_default()
            .split_whitespace()
            .map(String::from)
            .collect(),
    };
    for arg in game_args {
        out.push(subst(&arg, &map));
    }

    // The game joins a server only when asked explicitly — the «На сервер»
    // button, a friend code, never silently.
    if let Some(addr) = connect {
        let trimmed = addr.trim().to_string();
        if !trimmed.is_empty() {
            if mc_release(&resolved.id) >= (1, 20) {
                out.push("--quickPlayMultiplayer".into());
                out.push(trimmed);
            } else {
                let (host, port) = split_host_port(&trimmed);
                out.push("--server".into());
                out.push(host.to_string());
                out.push("--port".into());
                out.push(port.to_string());
            }
        }
    }
    Ok(out)
}

fn split_host_port(addr: &str) -> (&str, &str) {
    match addr.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && p.parse::<u16>().is_ok() => (h.trim_matches(['[', ']']), p),
        _ => (addr, "25565"),
    }
}

/// Forge/NeoForge (1.17+) put `${version_name}.jar` into `-DignoreList` so the
/// vanilla client jar stays off the module layer. The official launcher copies
/// that jar under the loader's id; we keep it shared as `1.20.1.jar`, so its
/// real file name must be in the list — otherwise it loads as module
/// `_1._20._1` next to the patched `minecraft` and the game dies with
/// «Modules minecraft and _1._20._1 export package …».
fn fix_ignore_list(arg: String, jar_name: &str) -> String {
    let Some(list) = arg.strip_prefix("-DignoreList=") else {
        return arg;
    };
    if jar_name.is_empty() || list.split(',').any(|item| item == jar_name) {
        return arg;
    }
    format!("{arg},{jar_name}")
}

/// (major, minor) release of a version id like `1.20.1-forge-47.3.0`.
fn mc_release(id: &str) -> (u32, u32) {
    let base = id.split('-').next().unwrap_or(id);
    let mut parts = base.split('.');
    let a = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let b = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (a, b)
}

/// A new build starts without the FPS caps vanilla ships with: vsync off and
/// «Макс. FPS» unlimited (260). Only when options.txt does not exist yet —
/// whatever the player set in the game is never touched.
fn fps_defaults(instance: &Path) {
    let options = instance.join("options.txt");
    if options.exists() {
        return;
    }
    let _ = std::fs::create_dir_all(instance);
    let _ = std::fs::write(options, "enableVsync:false\nmaxFps:260\n");
}

/// JVM flags for the game client: G1 tuned for short pauses (no frame
/// spikes), a big code cache for modded JIT, no perf-data file writes.
/// Unlike the server, no AlwaysPreTouch: it delays the first frame by
/// seconds on big heaps for no FPS gain.
pub fn client_flags(mem_mb: u64) -> Vec<String> {
    let mut flags: Vec<String> = perf_gc_flags(mem_mb)
        .into_iter()
        .filter(|f| f != "-XX:+AlwaysPreTouch")
        .collect();
    flags.extend([
        "-XX:+UseStringDeduplication".to_string(),
        "-XX:ReservedCodeCacheSize=400M".to_string(),
        "-XX:-UsePerfData".to_string(),
        "-XX:+PerfDisableSharedMem".to_string(),
    ]);
    flags
}

/// Aikar-style G1 tuning scaled to the machine: short pauses, all cores
/// on parallel phases — the max-FPS set for the client and the server.
pub fn perf_gc_flags(mem_mb: u64) -> Vec<String> {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(2);
    let parallel = cores.min(12);
    let conc = (cores / 4).clamp(1, 6);
    // Bigger heap — bigger regions: fewer GC cycles on large allocations.
    let region = if mem_mb >= 8192 { "32M" } else { "16M" };
    vec![
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:+UseG1GC".into(),
        "-XX:MaxGCPauseMillis=40".into(),
        "-XX:G1NewSizePercent=30".into(),
        "-XX:G1MaxNewSizePercent=40".into(),
        format!("-XX:G1HeapRegionSize={region}"),
        "-XX:G1ReservePercent=15".into(),
        "-XX:+ParallelRefProcEnabled".into(),
        "-XX:+DisableExplicitGC".into(),
        format!("-XX:ParallelGCThreads={parallel}"),
        format!("-XX:ConcGCThreads={conc}"),
        "-XX:+AlwaysPreTouch".into(),
    ]
}

fn collect_args(items: &[ArgItem]) -> Vec<String> {
    let mut out = Vec::new();
    for item in items {
        match item {
            ArgItem::Plain(s) => out.push(s.clone()),
            ArgItem::Conditional { rules, value } => {
                if !rules_allow(rules) {
                    continue;
                }
                match value {
                    serde_json::Value::String(s) => out.push(s.clone()),
                    serde_json::Value::Array(list) => {
                        for v in list {
                            if let Some(s) = v.as_str() {
                                out.push(s.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

fn classpath(resolved: &Resolved, game: &Path) -> Result<String> {
    let mut entries: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for lib in &resolved.json.libraries {
        if !rules_allow(&lib.rules) {
            continue;
        }
        let rel = lib
            .downloads
            .as_ref()
            .and_then(|d| d.artifact.as_ref())
            .and_then(|a| a.path.clone())
            .or_else(|| maven_path(&lib.name));
        let Some(rel) = rel else {
            continue;
        };
        let path = game.join("libraries").join(rel);
        if path.exists() {
            let text = path.display().to_string();
            if seen.insert(text.clone()) {
                entries.push(text);
            }
        }
    }
    if !resolved.client_jar.exists() {
        bail!(
            "Клиент версии не найден: {}",
            resolved.client_jar.display()
        );
    }
    let jar = resolved.client_jar.display().to_string();
    if seen.insert(jar.clone()) {
        entries.push(jar);
    }
    Ok(entries.join(crate::platform::CLASSPATH_SEP))
}

fn subst(text: &str, map: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            let key = &after[..end];
            out.push_str(map.get(key).map(|s| s.as_str()).unwrap_or(""));
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[start..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

const SERVICE_FILES: [&str; 2] = [
    "META-INF/services/cpw.mods.modlauncher.api.ITransformationService",
    "META-INF/services/net.minecraftforge.forgespi.locating.IModLocator",
];

/// Service providers a jar declares for ModLauncher whose classes are not
/// inside the jar. Such a jar stops the game with «Provider … not found».
pub fn missing_providers(jar: &Path) -> Vec<String> {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(jar) else { return Vec::new() };
    let Ok(mut zip) = zip::ZipArchive::new(std::io::BufReader::new(file)) else { return Vec::new() };
    let mut providers: Vec<String> = Vec::new();
    for service in SERVICE_FILES {
        if let Ok(mut entry) = zip.by_name(service) {
            let mut text = String::new();
            if entry.read_to_string(&mut text).is_ok() {
                providers.extend(
                    text.lines()
                        .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
                        .filter(|l| !l.is_empty()),
                );
            }
        }
    }
    providers
        .into_iter()
        .filter(|p| zip.by_name(&format!("{}.class", p.replace('.', "/"))).is_err())
        .collect()
}

/// Whether a jar cannot load on `loader` at all: a Fabric/Quilt-only mod
/// in a Forge/NeoForge build (Forge stops on it), a broken archive, or a
/// service provider that is declared but missing.
pub fn incompatible(jar: &Path, loader: crate::loader::Loader) -> bool {
    use crate::loader::Loader;
    let Ok(file) = std::fs::File::open(jar) else { return false };
    let Ok(mut zip) = zip::ZipArchive::new(std::io::BufReader::new(file)) else {
        return true; // not a zip: every loader dies on it
    };
    let has = |zip: &mut zip::ZipArchive<_>, n: &str| zip.by_name(n).is_ok();
    let forge_toml = has(&mut zip, "META-INF/mods.toml");
    let neo_toml = has(&mut zip, "META-INF/neoforge.mods.toml");
    let fabric = has(&mut zip, "fabric.mod.json") || has(&mut zip, "quilt.mod.json");
    drop(zip);
    let name = jar.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
    let wrong_loader = match loader {
        Loader::Forge => {
            (fabric && !forge_toml)
                // Multi-loader jars named «…-fabric…» carry a toml Forge
                // rejects as «corrupt or misconfigured».
                || (fabric && name.contains("fabric") && !name.contains("forge"))
        }
        Loader::NeoForge => fabric && !forge_toml && !neo_toml,
        _ => false,
    };
    wrong_loader || (matches!(loader, Loader::Forge | Loader::NeoForge) && !missing_providers(jar).is_empty())
}

/// Disables (renames to `.disabled`) jars that would stop the game.
/// Returns their file names.
pub fn preflight_mods(mods_dir: &Path, loader: crate::loader::Loader) -> Vec<String> {
    let mut off = Vec::new();
    let Ok(entries) = std::fs::read_dir(mods_dir) else { return off };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()).map(str::to_string) else { continue };
        if !name.to_lowercase().ends_with(".jar") || !path.is_file() {
            continue;
        }
        if incompatible(&path, loader) && std::fs::rename(&path, mods_dir.join(format!("{name}.disabled"))).is_ok() {
            off.push(name);
        }
    }
    off
}

/// When ModLauncher dies with `Provider ... not found`, the jar that carries
/// that provider is broken or from another loader — disable it automatically
/// so the next Играть has a chance to boot.
fn heal_broken_mod(shared: Arc<Shared>, log_path: &Path) {
    use std::io::BufRead;
    let Ok(file) = std::fs::File::open(log_path) else {
        return;
    };
    let mut provider = String::new();
    // map_while stops at the first read error instead of spinning on it.
    for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
        if let Some(idx) = line.find("Provider ") {
            let rest = &line[idx + 9..];
            if let Some(end) = rest.find(" not found") {
                provider = rest[..end].to_string();
                break;
            }
        }
    }
    if provider.is_empty() {
        // «Mod candidate X.jar contains a corrupt or misconfigured toml»
        if let Ok(text) = std::fs::read_to_string(log_path) {
            if let Some(idx) = text.find("Mod candidate ") {
                let rest = &text[idx + 14..];
                if let Some(end) = rest.find(" contains a corrupt") {
                    let jar = rest[..end].trim().to_string();
                    let mods_dir = crate::config::load().mods_dir();
                    if mods_dir.join(&jar).is_file()
                        && std::fs::rename(mods_dir.join(&jar), mods_dir.join(format!("{jar}.disabled"))).is_ok()
                    {
                        shared.push_game(format!("[Madrid] Мод {jar} повреждён или не для этого загрузчика — он отключён. Нажми «Играть» снова."));
                    }
                }
            }
        }
        return;
    }
    let needle = provider.replace('.', "/");
    let mods_dir = crate::config::load().mods_dir();
    let mut culprit = String::new();
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !path.extension().map(|e| e == "jar").unwrap_or(false) {
                continue;
            }
            if !missing_providers(&path).is_empty() {
                culprit = path.file_name().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                break;
            }
            if let Ok(f) = std::fs::File::open(&path) {
                if let Ok(mut zip) = zip::ZipArchive::new(f) {
                    for i in 0..zip.len() {
                        if let Ok(name) = zip.by_index(i).map(|e| e.name().to_string()) {
                            if name.starts_with(&needle) {
                                culprit = path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or_default()
                                    .to_string();
                                break;
                            }
                        }
                    }
                }
            }
            if !culprit.is_empty() {
                break;
            }
        }
    }
    if culprit.is_empty() {
        return;
    }
    let from = mods_dir.join(&culprit);
    let to = mods_dir.join(format!("{culprit}.disabled"));
    if std::fs::rename(&from, &to).is_ok() {
        shared.push_game(format!(
            "[Madrid] Сборка не запускалась из-за битого мода {culprit} — он отключён. Нажми «Играть» снова."
        ));
        shared.log(
            crate::shared::LogKind::Warn,
            format!("Самолечение: отключён битый мод {culprit}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_list_gets_the_shared_vanilla_jar() {
        let arg = "-DignoreList=bootstraplauncher,securejarhandler,forge-,1.20.1-forge-47.4.20.jar".to_string();
        assert_eq!(
            fix_ignore_list(arg, "1.20.1.jar"),
            "-DignoreList=bootstraplauncher,securejarhandler,forge-,1.20.1-forge-47.4.20.jar,1.20.1.jar"
        );
        let same = "-DignoreList=a,1.21.1.jar".to_string();
        assert_eq!(fix_ignore_list(same.clone(), "1.21.1.jar"), same);
        assert_eq!(fix_ignore_list("-Xmx4G".into(), "1.20.1.jar"), "-Xmx4G");
    }

    #[test]
    fn legacy_connect_splits_host_and_port() {
        assert_eq!(split_host_port("192.168.1.5:25565"), ("192.168.1.5", "25565"));
        assert_eq!(split_host_port("play.example.com"), ("play.example.com", "25565"));
        assert_eq!(split_host_port("[::1]:25570"), ("::1", "25570"));
    }

    fn make_fabric(path: &Path) {
        use std::io::Write;
        let f = std::fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(f);
        zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"{}").unwrap();
        zip.finish().unwrap();
    }

    #[test]
    fn jar_with_missing_provider_is_caught() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("madrid-preflight-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let make = |name: &str, with_class: bool| {
            let f = std::fs::File::create(dir.join(name)).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file(SERVICE_FILES[0], opts).unwrap();
            zip.write_all(b"toni.missingmodschecker.NeoBootstrap
").unwrap();
            if with_class {
                zip.start_file("toni/missingmodschecker/NeoBootstrap.class", opts).unwrap();
                zip.write_all(b"x").unwrap();
            }
            zip.finish().unwrap();
        };
        make("broken.jar", false);
        make("fine.jar", true);
        make_fabric(&dir.join("sodium-fabric.jar"));
        make_fabric(&dir.join("lib.jar"));
        let mut off = preflight_mods(&dir, crate::loader::Loader::Forge);
        off.sort();
        assert_eq!(off, vec!["broken.jar".to_string(), "lib.jar".to_string(), "sodium-fabric.jar".to_string()]);
        // A Fabric build keeps Fabric mods.
        std::fs::rename(dir.join("lib.jar.disabled"), dir.join("lib.jar")).unwrap();
        assert!(preflight_mods(&dir, crate::loader::Loader::Fabric).is_empty());
        assert!(dir.join("broken.jar.disabled").exists());
        assert!(dir.join("fine.jar").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn client_flags_skip_pretouch() {
        let f = client_flags(8192);
        assert!(!f.iter().any(|x| x == "-XX:+AlwaysPreTouch"));
        assert!(f.iter().any(|x| x == "-XX:+UseG1GC"));
    }

    /// Starts the installed Madrid Forge build and waits until Forge has
    /// built its module layers (where «Modules minecraft and _1._20._1
    /// export package …» used to kill it), then stops the game.
    /// `cargo test live_forge -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_forge_boots_past_module_layer() {
        let mut cfg = crate::config::load();
        cfg.active_profile = "default".into();
        let shared = crate::app::new_shared(cfg.clone());
        let id = "1.20.1-forge-47.4.28";
        launch(&shared, &cfg, id, None).expect("launch");
        let t = std::time::Instant::now();
        let mut verdict = None;
        while t.elapsed() < Duration::from_secs(240) {
            std::thread::sleep(Duration::from_millis(500));
            let tail = shared.game_log_tail(400).join("
");
            if tail.contains("ResolutionException") || tail.contains("export package") {
                verdict = Some(false);
                break;
            }
            let full = std::env::var("MADRID_FULL_BOOT").is_ok();
            if tail.contains("Launching target") && !full {
                verdict = Some(true);
                break;
            }
            if full && (tail.contains("Sound engine started") || tail.contains("OpenAL initialized")) {
                verdict = Some(true);
                break;
            }
            if full && (tail.contains("Crash report saved") || tail.contains("---- Minecraft Crash Report")) {
                verdict = Some(false);
                break;
            }
            if !shared.is_game_running() {
                break;
            }
        }
        let tail = shared.game_log_tail(30).join("
");
        stop(&shared);
        println!("verdict {verdict:?} after {:?}
{tail}", t.elapsed());
        assert_eq!(verdict, Some(true));
    }

    /// What «Играть» does end to end: start, catch missing dependencies,
    /// fetch them, start again — until the game reaches its main menu.
    /// `cargo test live_e2e -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_e2e_madrid_reaches_menu() {
        let mut cfg = crate::config::load();
        cfg.active_profile = "default".into();
        let shared = crate::app::new_shared(cfg.clone());
        let id = "1.20.1-forge-47.4.28";
        let spec = crate::loader::Spec::new("1.20.1", crate::loader::Loader::Forge, "");
        for attempt in 1..=4 {
            launch(&shared, &cfg, id, None).expect("launch");
            let t = std::time::Instant::now();
            let mut outcome = "timeout";
            while t.elapsed() < Duration::from_secs(240) {
                std::thread::sleep(Duration::from_millis(500));
                let missing = shared.missing_mods.lock().unwrap().clone();
                if let Some((ids, pairs, at)) = missing {
                    if at.elapsed() > Duration::from_millis(1500) {
                        stop(&shared);
                        std::thread::sleep(Duration::from_secs(2));
                        println!("attempt {attempt}: missing {ids:?}");
                        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let res = crate::net::rt().block_on(crate::deps::fetch_missing(
                            &crate::net::client(), &cfg, &ids, &pairs, &spec, &cfg.mods_dir(), &cancel, &mut |_| {},
                        ));
                        println!("  fetched {res:?}");
                        outcome = "missing";
                        break;
                    }
                }
                let tail = shared.game_log_tail(300).join("
");
                if tail.contains("Sound engine started") {
                    outcome = "menu";
                    break;
                }
                if !shared.is_game_running() {
                    outcome = "exited";
                    break;
                }
            }
            println!("attempt {attempt}: {outcome} after {:?}", t.elapsed());
            if outcome == "menu" {
                stop(&shared);
                return;
            }
            if outcome != "missing" {
                stop(&shared);
                std::thread::sleep(Duration::from_secs(2));
                let tail = shared.game_log_tail(40).join("
");
                println!("{tail}");
                if outcome == "exited" {
                    // The launcher's self-healing runs on exit code 1.
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                panic!("game did not reach the menu");
            }
        }
        panic!("no menu after 4 attempts");
    }
}
