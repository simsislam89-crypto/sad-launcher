//! Builds (profiles). Every build is its own game folder — mods, configs,
//! saves and options never mix. The default build is the main game folder;
//! the others live in meta/profiles/{id}. Switching copies nothing.

use crate::config::Config;
use crate::loader::{Loader, Spec};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub const DEFAULT_ID: &str = "default";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileKind {
    Default,
    Custom,
    Modpack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub kind: ProfileKind,
    pub mc_version: String,
    pub loader: String,
    /// Exact loader build the pack asked for; empty = recommended.
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: String,
    /// Catalog project a pack came from (`modrinth:abc`, `curseforge:123`):
    /// reinstalling the same pack refreshes its build instead of cloning it.
    #[serde(default)]
    pub source: String,
    /// Installed pack version: file/version id at its source (compared
    /// with the newest one to offer «Обновить сборку»), its name and date.
    #[serde(default)]
    pub pack_file: String,
    #[serde(default)]
    pub pack_version: String,
    #[serde(default)]
    pub pack_date: String,
}

/// Which version of a pack a build holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackStamp {
    pub file: String,
    pub version: String,
    pub date: String,
}

impl Profile {
    pub fn spec(&self) -> Spec {
        Spec::new(&self.mc_version, Loader::parse(&self.loader), &self.loader_version)
    }

    fn madrid() -> Profile {
        Profile {
            id: DEFAULT_ID.into(),
            name: "Основная сборка".into(),
            kind: ProfileKind::Default,
            mc_version: crate::config::MADRID_MC.into(),
            loader: "forge".into(),
            loader_version: String::new(),
            description: "Стандартный профиль Мадрид".into(),
            created_at: String::new(),
            source: String::new(),
            pack_file: String::new(),
            pack_version: String::new(),
            pack_date: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProfileMod {
    pub name: String,
    pub file: String,
    pub size: u64,
    pub enabled: bool,
}

pub fn profiles_dir() -> PathBuf {
    crate::config::data_dir().join("meta").join("profiles")
}

pub fn profiles_file() -> PathBuf {
    profiles_dir().join("profiles.json")
}

pub fn profile_mods_dir(cfg: &Config, profile_id: &str) -> PathBuf {
    cfg.instance_dir_for(profile_id).join("mods")
}

fn sanitize_id(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let mut out = String::with_capacity(clean.len());
    for c in clean.chars() {
        if !(c == '-' && out.ends_with('-')) {
            out.push(c);
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "build".to_string()
    } else {
        trimmed.chars().take(40).collect()
    }
}

fn unique_id(profiles: &[Profile], base: &str) -> String {
    let mut id = base.to_string();
    let mut n = 1;
    while profiles.iter().any(|p| p.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

// ---------------------------------------------------------------- storage

/// Parsed profiles.json keyed by its mtime: the UI asks every frame, the
/// disk is read only when the file actually changed.
static CACHE: Mutex<Option<(Option<SystemTime>, Vec<Profile>)>> = Mutex::new(None);

fn stamp() -> Option<SystemTime> {
    std::fs::metadata(profiles_file()).and_then(|m| m.modified()).ok()
}

pub fn load_profiles() -> Vec<Profile> {
    let now = stamp();
    if let Ok(guard) = CACHE.lock() {
        if let Some((at, list)) = guard.as_ref() {
            if now.is_some() && *at == now {
                return list.clone();
            }
        }
    }
    let mut profiles: Vec<Profile> = std::fs::read_to_string(profiles_file())
        .ok()
        .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default();
    if !profiles.iter().any(|p| p.id == DEFAULT_ID) {
        profiles.insert(0, Profile::madrid());
    }
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((now, profiles.clone()));
    }
    profiles
}

pub fn save_profiles(profiles: &[Profile]) -> Result<()> {
    let dir = profiles_dir();
    std::fs::create_dir_all(&dir)?;
    let json = serde_json::to_string_pretty(profiles)?;
    let tmp = dir.join("profiles.json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, profiles_file())?;
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some((stamp(), profiles.to_vec()));
    }
    Ok(())
}

pub fn get_active_profile(cfg: &Config) -> Profile {
    let active = cfg.active_profile.trim();
    let active = if active.is_empty() { DEFAULT_ID } else { active };
    load_profiles()
        .into_iter()
        .find(|p| p.id == active)
        .unwrap_or_else(Profile::madrid)
}

// ---------------------------------------------------------------- actions

/// Makes a build active. Nothing is copied: each build has its own folder.
pub fn switch_profile(cfg: &mut Config, target_id: &str) -> Result<()> {
    let target = load_profiles()
        .into_iter()
        .find(|p| p.id == target_id)
        .with_context(|| format!("Сборка не найдена: {target_id}"))?;
    let _ = std::fs::create_dir_all(profile_mods_dir(cfg, &target.id));
    cfg.active_profile = target.id.clone();
    let installed = crate::minecraft::as_installed(&crate::minecraft::installed_versions(cfg));
    cfg.selected_version = crate::loader::pick_installed(&installed, &target.spec()).unwrap_or_default();
    if !target.loader.is_empty() {
        cfg.loader = target.loader.clone();
    }
    crate::config::save(cfg);
    Ok(())
}

/// Creates a new personal custom build with an empty mods folder.
pub fn create_custom_profile(
    cfg: &Config,
    name: &str,
    mc_version: &str,
    loader: &str,
    description: &str,
) -> Result<Profile> {
    let name_clean = name.trim();
    if name_clean.is_empty() {
        bail!("Название сборки не может быть пустым");
    }
    let mut profiles = load_profiles();
    let id = unique_id(&profiles, &format!("custom-{}", sanitize_id(name_clean)));
    let profile = Profile {
        id: id.clone(),
        name: name_clean.to_string(),
        kind: ProfileKind::Custom,
        mc_version: if mc_version.trim().is_empty() {
            crate::config::MADRID_MC.to_string()
        } else {
            mc_version.trim().to_string()
        },
        loader: if loader.trim().is_empty() {
            "forge".to_string()
        } else {
            loader.trim().to_string()
        },
        loader_version: String::new(),
        description: description.trim().to_string(),
        created_at: crate::util::file_stamp(),
        source: String::new(),
        pack_file: String::new(),
        pack_version: String::new(),
        pack_date: String::new(),
    };
    std::fs::create_dir_all(profile_mods_dir(cfg, &id))?;
    profiles.push(profile.clone());
    save_profiles(&profiles)?;
    Ok(profile)
}

/// The build of a modpack. The same pack installed again reuses its build:
/// mods and configs are replaced, worlds, screenshots and options stay.
pub fn upsert_modpack_profile(
    cfg: &Config,
    title: &str,
    source: &str,
    spec: &Spec,
    stamp: &PackStamp,
) -> Result<Profile> {
    let mut profiles = load_profiles();
    let existing = profiles.iter().position(|p| {
        p.kind == ProfileKind::Modpack && ((!source.is_empty() && p.source == source) || p.name == title)
    });
    let description = format!("Модпак · {}", spec.title());
    let profile = match existing {
        Some(i) => {
            let p = &mut profiles[i];
            p.mc_version = spec.mc.clone();
            p.loader = spec.loader.id().to_string();
            p.loader_version = spec.version.clone();
            p.description = description;
            p.source = source.to_string();
            p.pack_file = stamp.file.clone();
            p.pack_version = stamp.version.clone();
            p.pack_date = stamp.date.clone();
            let p = p.clone();
            clear_for_reinstall(&cfg.instance_dir_for(&p.id));
            p
        }
        None => {
            let id = unique_id(&profiles, &format!("pack-{}", sanitize_id(title)));
            let p = Profile {
                id,
                name: title.trim().to_string(),
                kind: ProfileKind::Modpack,
                mc_version: spec.mc.clone(),
                loader: spec.loader.id().to_string(),
                loader_version: spec.version.clone(),
                description,
                created_at: crate::util::file_stamp(),
                source: source.to_string(),
                pack_file: stamp.file.clone(),
                pack_version: stamp.version.clone(),
                pack_date: stamp.date.clone(),
            };
            profiles.push(p.clone());
            p
        }
    };
    save_profiles(&profiles)?;
    std::fs::create_dir_all(profile_mods_dir(cfg, &profile.id))?;
    Ok(profile)
}

/// Wipes a pack build for a clean reinstall, keeping the player's own data.
fn clear_for_reinstall(dir: &Path) {
    const KEEP: [&str; 5] = ["saves", "screenshots", "options.txt", "servers.dat", "logs"];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if KEEP.contains(&name.as_str()) {
            continue;
        }
        crate::remove::queue("Старые файлы пака", &entry.path());
    }
}

/// Deletes a build and its folder. The active one falls back to default.
pub fn delete_profile(cfg: &mut Config, profile_id: &str) -> Result<()> {
    if profile_id == DEFAULT_ID {
        bail!("Нельзя удалить основную сборку");
    }
    if cfg.active_profile == profile_id {
        switch_profile(cfg, DEFAULT_ID)?;
    }
    let mut profiles = load_profiles();
    let name = profiles
        .iter()
        .find(|p| p.id == profile_id)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    profiles.retain(|p| p.id != profile_id);
    save_profiles(&profiles)?;
    let dir = cfg.instance_dir_for(profile_id);
    if dir.starts_with(profiles_dir()) && dir.exists() {
        // Renamed away instantly, files go in the background.
        crate::remove::queue(format!("Сборка «{name}»"), &dir);
    }
    Ok(())
}

/// What «Удалить все сборки» removed.
pub struct Sweep {
    pub builds: u32,
    pub versions: u32,
}

/// Removes every custom build and modpack with their folders, then the
/// loader versions only they used. The main Madrid build stays.
pub fn delete_all_builds(cfg: &mut Config) -> Result<Sweep> {
    let profiles = load_profiles();
    if cfg.active_profile != DEFAULT_ID {
        switch_profile(cfg, DEFAULT_ID)?;
    }
    let mut sweep = Sweep { builds: 0, versions: 0 };
    // Every folder is renamed into the trash at once (instant); the
    // background remover deletes the files and reports freed space.
    for p in profiles.iter().filter(|p| p.id != DEFAULT_ID) {
        let dir = cfg.instance_dir_for(&p.id);
        if dir.starts_with(profiles_dir()) && dir.exists() {
            crate::remove::queue(format!("Сборка «{}»", p.name), &dir);
        }
        sweep.builds += 1;
    }
    let kept: Vec<Profile> = profiles.into_iter().filter(|p| p.id == DEFAULT_ID).collect();
    save_profiles(&kept)?;

    // Orphan folders (crashed installs, old layouts) go too.
    if let Ok(entries) = std::fs::read_dir(profiles_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                crate::remove::queue("Брошенная папка сборки", &path);
            }
        }
    }

    // Loader versions no remaining build runs are dead weight.
    let game = cfg.game_dir();
    let installed = crate::minecraft::as_installed(&crate::minecraft::installed_versions(cfg));
    let keep: Vec<String> = kept
        .iter()
        .filter_map(|p| crate::loader::pick_installed(&installed, &p.spec()))
        .collect();
    for v in installed.iter().filter(|v| v.loader != Loader::Vanilla) {
        if keep.contains(&v.id) {
            continue;
        }
        let dir = game.join("versions").join(&v.id);
        if crate::remove::queue(format!("Версия {}", v.id), &dir) {
            sweep.versions += 1;
        }
    }
    let packs = crate::config::data_dir().join("meta").join("packs");
    crate::remove::queue("Архивы паков", &packs);
    Ok(sweep)
}

// ---------------------------------------------------------------- mods of a build

/// Mod listings keyed by folder mtime: adding, removing or renaming a file
/// bumps it, so the cache is exact while the UI redraws freely.
/// Shared (`Arc`), not cloned: the Play/Build/Server tabs ask every frame,
/// often only for `.len()` of a few hundred jars.
type ModsCache = HashMap<PathBuf, (SystemTime, std::sync::Arc<Vec<ProfileMod>>)>;
static MODS_CACHE: Mutex<Option<ModsCache>> = Mutex::new(None);

pub fn list_profile_mods(cfg: &Config, profile_id: &str) -> std::sync::Arc<Vec<ProfileMod>> {
    let dir = profile_mods_dir(cfg, profile_id);
    let Some(mtime) = std::fs::metadata(&dir).and_then(|m| m.modified()).ok() else {
        return Default::default();
    };
    if let Ok(guard) = MODS_CACHE.lock() {
        if let Some((at, list)) = guard.as_ref().and_then(|m| m.get(&dir)) {
            if *at == mtime {
                return list.clone();
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            let enabled = file.ends_with(".jar");
            if !enabled && !file.ends_with(".jar.disabled") {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            out.push(ProfileMod {
                name: file.trim_end_matches(".disabled").trim_end_matches(".jar").to_string(),
                file,
                size: meta.len(),
                enabled,
            });
        }
    }
    out.sort_by_cached_key(|m| m.name.to_lowercase());
    let out = std::sync::Arc::new(out);
    if let Ok(mut guard) = MODS_CACHE.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(dir, (mtime, out.clone()));
    }
    out
}

/// Enables/disables a mod file in a build.
pub fn toggle_profile_mod(cfg: &Config, profile_id: &str, file_name: &str, enabled: bool) -> Result<()> {
    let dir = profile_mods_dir(cfg, profile_id);
    let current = dir.join(file_name);
    let base = file_name.trim_end_matches(".disabled").trim_end_matches(".jar");
    let target = if enabled {
        dir.join(format!("{base}.jar"))
    } else {
        dir.join(format!("{base}.jar.disabled"))
    };
    if current.is_file() && current != target {
        std::fs::rename(current, target)?;
    }
    Ok(())
}

pub fn delete_profile_mod(cfg: &Config, profile_id: &str, file_name: &str) -> Result<()> {
    let file = profile_mods_dir(cfg, profile_id).join(file_name);
    if file.is_file() {
        std::fs::remove_file(file)?;
    }
    Ok(())
}

pub fn add_jar_to_profile(cfg: &Config, profile_id: &str, file_name: &str, data: &[u8]) -> Result<()> {
    let dir = profile_mods_dir(cfg, profile_id);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(file_name), data)?;
    Ok(())
}

// ---------------------------------------------------------------- migration

/// One-time move from the old layout (one live game/mods swapped by
/// copying) to folder-per-build. Runs before the window opens.
pub fn migrate_to_instances(cfg: &mut Config) {
    if cfg.instances_v2 {
        return;
    }
    let root = profiles_dir();
    let live = cfg.madrid_mods_dir();
    let active = cfg.active_profile.trim().to_string();
    if !active.is_empty() && active != DEFAULT_ID {
        // game/mods holds the active build's mods: give them to its folder,
        // then bring the stored Madrid mods back to game/mods.
        let own = root.join(&active).join("mods");
        move_files(&live, &own);
        move_files(&root.join(DEFAULT_ID).join("mods"), &live);
    }
    // The old copy of the default build is stale either way.
    let _ = std::fs::remove_dir_all(root.join(DEFAULT_ID));

    // Phantom builds made from the old pack registry own no files.
    let mut profiles = load_profiles();
    profiles.retain(|p| {
        if p.id == DEFAULT_ID || p.kind != ProfileKind::Modpack {
            return true;
        }
        let mods = root.join(&p.id).join("mods");
        let has_files = std::fs::read_dir(&mods)
            .map(|mut e| e.next().is_some())
            .unwrap_or(false);
        if !has_files {
            let _ = std::fs::remove_dir_all(root.join(&p.id));
        }
        has_files
    });
    let _ = save_profiles(&profiles);
    if !profiles.iter().any(|p| p.id == active) {
        cfg.active_profile = DEFAULT_ID.into();
    }

    let data = crate::config::data_dir();
    let _ = std::fs::remove_file(data.join("meta").join("packs").join("installed.json"));
    // Leftovers of the removed stevenarella edition (sources + 1.18 server).
    let _ = std::fs::remove_dir_all(data.join("rustmc").join("repo"));
    let _ = std::fs::remove_dir_all(data.join("rustmc").join("server"));

    cfg.instances_v2 = true;
    crate::config::save(cfg);
}

/// Moves every file of `from` into `to` (rename; copy+delete across drives).
fn move_files(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    let _ = std::fs::create_dir_all(to);
    for entry in entries.flatten() {
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let dst = to.join(entry.file_name());
        if dst.exists() {
            let _ = std::fs::remove_file(&dst);
        }
        if std::fs::rename(&src, &dst).is_err() && std::fs::copy(&src, &dst).is_ok() {
            let _ = std::fs::remove_file(&src);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_short_and_clean() {
        assert_eq!(sanitize_id("Better MC [FORGE] - BMC4"), "better-mc-forge-bmc4");
        assert_eq!(sanitize_id("Мадрид"), "build");
        let list = vec![Profile::madrid()];
        assert_eq!(unique_id(&list, "default"), "default-1");
        assert_eq!(unique_id(&list, "pack-x"), "pack-x");
    }

    #[test]
    fn instance_dirs_per_build() {
        let cfg = Config::default();
        assert_eq!(cfg.instance_dir_for("default"), cfg.game_dir());
        assert_eq!(cfg.instance_dir_for(""), cfg.game_dir());
        assert_eq!(cfg.instance_dir_for("pack-x"), profiles_dir().join("pack-x"));
    }
}
