//! Persisted settings in %APPDATA%\Sad Launcher\config.json

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_DIR: &str = "Sad Launcher";
pub const APP_TITLE: &str = "Sad Launcher";
/// Name used before v0.7 — its data folder, shortcut and autostart value
/// are moved over once (`migrate_legacy`).
pub const LEGACY_NAME: &str = "LLM Launcher Madrid";
/// One source of truth: Cargo.toml. (Was a second hand-written copy — a bump
/// in one place only made every user see «update available» forever.)
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "simsislam89-crypto/sad-launcher";
/// The launcher's website hosted via Botkeep / custom domain.
pub const SITE_URL: &str = "https://rkt5jy.bot-keep.xyz";
pub const DEFAULT_MEMORY_MB: u32 = 4096;
pub const DEFAULT_SERVER_MEMORY_MB: u32 = 2048;
/// The Madrid pack target — the launcher installs Forge for this release.
pub const MADRID_MC: &str = "1.20.1";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub nickname: String,
    /// Empty means auto-detect.
    pub java_path: String,
    pub memory_mb: u32,
    /// Empty means %APPDATA%\Sad Launcher\game
    pub game_dir: String,
    pub selected_version: String,
    /// "forge" is the Madrid loader; "vanilla" remains as a fallback.
    pub loader: String,
    pub cf_api_key: String,
    /// 0.20 .. 0.99 — opacity of the window slab
    pub transparency: f32,
    /// Minecraft server port, not a web port.
    pub server_port: u16,
    /// Host the Minecraft server as soon as the launcher opens.
    /// Off by default: the server starts only by its button.
    pub auto_server: bool,
    pub server_memory_mb: u32,
    pub autostart: bool,
    /// Path to a user image drawn behind the window slab.
    pub wallpaper: String,
    /// Chosen skin png; its face renders as the avatar.
    pub skin_file: String,
    /// Desktop shortcut was created already.
    pub shortcut_done: bool,
    /// The first-run nickname screen was accepted.
    pub first_run_done: bool,
    /// Active profile / custom build / modpack id (defaults to "default")
    pub active_profile: String,
    /// Builds live in their own folders (v0.5). False on configs written
    /// by older versions — triggers the one-time move of mods.
    /// Field-level default: a missing key reads as false, not as the
    /// struct default (true), so old configs do migrate.
    #[serde(default)]
    pub instances_v2: bool,
    /// Random id of this install in rooms (stable across restarts).
    pub device_id: String,
    /// Last room joined — offered for a quick return.
    pub last_room: String,
    /// Port announced to the room when hosting.
    pub host_port: u16,
    /// Version the user asked never to be reminded about again.
    pub skip_update_version: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            nickname: String::new(),
            java_path: String::new(),
            memory_mb: DEFAULT_MEMORY_MB,
            game_dir: String::new(),
            selected_version: String::new(),
            loader: "forge".into(),
            cf_api_key: String::new(),
            transparency: 0.86,
            server_port: 25565,
            auto_server: false,
            server_memory_mb: DEFAULT_SERVER_MEMORY_MB,
            autostart: false,
            wallpaper: String::new(),
            skin_file: String::new(),
            shortcut_done: false,
            first_run_done: false,
            active_profile: "default".into(),
            instances_v2: true,
            device_id: String::new(),
            last_room: String::new(),
            host_port: 25565,
            skip_update_version: String::new(),
        }
    }
}

impl Config {
    pub fn game_dir(&self) -> PathBuf {
        let custom = self.game_dir.trim();
        if custom.is_empty() {
            let mut p = data_dir();
            p.push("game");
            p
        } else {
            PathBuf::from(custom)
        }
    }

    /// Game folder of a build: the default build is the main game folder,
    /// every other build lives in meta/profiles/{id}. Versions, libraries
    /// and assets always stay shared in `game_dir()`.
    pub fn instance_dir_for(&self, profile_id: &str) -> PathBuf {
        let id = profile_id.trim();
        if id.is_empty() || id == "default" {
            self.game_dir()
        } else {
            data_dir().join("meta").join("profiles").join(id)
        }
    }

    /// Game folder of the active build (mods, configs, saves, options).
    pub fn instance_dir(&self) -> PathBuf {
        self.instance_dir_for(&self.active_profile)
    }

    /// Mods of the active build.
    pub fn mods_dir(&self) -> PathBuf {
        self.instance_dir().join("mods")
    }

    /// Mods of the main Madrid build — what the hosted server runs.
    pub fn madrid_mods_dir(&self) -> PathBuf {
        self.game_dir().join("mods")
    }

    /// The hosted Minecraft server lives outside the client dir.
    pub fn server_dir(&self) -> PathBuf {
        data_dir().join("server")
    }

    pub fn nickname_or_default(&self) -> String {
        let n = self.nickname.trim();
        if n.is_empty() {
            "Player".into()
        } else {
            n.chars().filter(|c| c.is_alphanumeric() || *c == '_').take(16).collect()
        }
    }
}

fn roaming() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// %APPDATA%\Sad Launcher. While the old «LLM Launcher Madrid» folder could
/// not be renamed yet (an old copy still holds a file), the old folder keeps
/// being used — nothing is lost and nothing is split in two.
pub fn data_dir() -> PathBuf {
    let new = roaming().join(APP_DIR);
    if !new.exists() {
        let old = roaming().join(LEGACY_NAME);
        if old.join("config.json").exists() {
            return old;
        }
    }
    new
}

/// One-time move of the pre-0.7 data folder to the new name. Absolute
/// paths stored inside json files (skin, wallpaper, custom folders) are
/// rewritten to the new location.
pub fn migrate_legacy() {
    let new = roaming().join(APP_DIR);
    let old = roaming().join(LEGACY_NAME);
    if new.exists() || !old.exists() {
        return;
    }
    if std::fs::rename(&old, &new).is_err() {
        return;
    }
    let (from, to) = (old.display().to_string(), new.display().to_string());
    // Paths sit in json with escaped backslashes.
    let esc = |s: &str| s.replace('\\', "\\\\");
    let mut files = vec![new.join("config.json")];
    for dir in [new.join("meta"), new.join("meta").join("packs")] {
        if let Ok(rd) = std::fs::read_dir(dir) {
            files.extend(
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == "json")),
            );
        }
    }
    for f in files {
        if let Ok(text) = std::fs::read_to_string(&f) {
            let fixed = text.replace(&esc(&from), &esc(&to));
            if fixed != text {
                let _ = std::fs::write(&f, fixed);
            }
        }
    }
}

pub fn ensure_dirs() {
    let root = data_dir();
    for sub in ["", "meta", "logs", "game", "game/mods", "server"] {
        let mut p = root.clone();
        if !sub.is_empty() {
            p.push(sub);
        }
        let _ = std::fs::create_dir_all(p);
    }
}

fn config_path() -> PathBuf {
    let mut p = data_dir();
    p.push("config.json");
    p
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        // Some editors write a BOM; serde rejects it — strip before parsing.
        Ok(text) => {
            let text = text.trim_start_matches('\u{feff}');
            serde_json::from_str::<Config>(text).unwrap_or_default()
        }
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) {
    ensure_dirs();
    if let Ok(text) = serde_json::to_string_pretty(cfg) {
        // Temp file + rename: a power cut never leaves a half-written config.
        let path = config_path();
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}
