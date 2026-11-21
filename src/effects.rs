//! Эффекты — настройки освещения, видимости в жидкостях и очистки экрана.
//! Применяются к активной сборке в чистом Rust: патчит `options.txt` и
//! генерирует ресурспак `resourcepacks/SadEffects.zip` с нужными текстурами.

use crate::config::Config;
use crate::loader::{Loader, Spec};
use crate::mods::{self, InstalledMod, ModEvent, ModHit};
use anyhow::{bail, Context, Result};
use image::{ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectsConfig {
    // Освещение и видимость в жидкостях
    pub fullbright: bool,
    pub fullbright_gamma: f32,
    pub no_fluid: bool,
    pub no_fluid_water: bool,
    pub no_fluid_lava: bool,
    pub custom_fog: bool,
    pub fog_disable: bool,
    pub fog_density: f32,
    pub fog_tint: String,

    // Очистка экрана от помех (No Render)
    pub no_fire: bool,
    pub no_fire_hide: bool,
    pub no_bad_effects: bool,
    pub anti_blindness: bool,
    pub anti_nausea: bool,
    pub anti_darkness: bool,
    pub no_hurt_cam: bool,
    pub no_weather: bool,
    pub no_particles: bool,
    pub part_explosions: bool,
    pub part_smoke: bool,
    pub part_potions: bool,
    pub part_blocks: bool,
    pub no_overlay: bool,
    pub ov_pumpkin: bool,
    pub ov_snow: bool,
    pub ov_totem: bool,

    // Освещение мира и окружения
    pub world_time: bool,
    pub world_time_ticks: u32,
    pub ambience: bool,
    pub ambience_soft_shadows: bool,
    pub ambience_tone: String,

    // Автоматизация (работает только в запущенном Minecraft)
    pub anti_afk: bool,
    pub anti_afk_secs: u32,
    pub anti_afk_bind: String,
    pub autoclicker: bool,
    pub autoclicker_cps: f32,
    pub autoclicker_ms: u32,
    pub autoclicker_rmb: bool,
    pub autoclicker_bind: String,
}

impl Default for EffectsConfig {
    fn default() -> Self {
        Self {
            fullbright: false,
            fullbright_gamma: 15.0,
            no_fluid: false,
            no_fluid_water: true,
            no_fluid_lava: true,
            custom_fog: false,
            fog_disable: true,
            fog_density: 0.25,
            fog_tint: "Нейтральный".into(),

            no_fire: false,
            no_fire_hide: false,
            no_bad_effects: false,
            anti_blindness: true,
            anti_nausea: true,
            anti_darkness: true,
            no_hurt_cam: false,
            no_weather: false,
            no_particles: false,
            part_explosions: true,
            part_smoke: true,
            part_potions: true,
            part_blocks: false,
            no_overlay: false,
            ov_pumpkin: true,
            ov_snow: true,
            ov_totem: true,

            world_time: false,
            world_time_ticks: 6000,
            ambience: false,
            ambience_soft_shadows: true,
            ambience_tone: "Контрастный".into(),

            anti_afk: false,
            anti_afk_secs: 60,
            anti_afk_bind: "F7".into(),
            autoclicker: false,
            autoclicker_cps: 100.0,
            autoclicker_ms: 10,
            autoclicker_rmb: false,
            autoclicker_bind: "F6".into(),
        }
    }
}

impl EffectsConfig {
    pub fn needs_resourcepack(&self) -> bool {
        self.no_fire
            || self.no_weather
            || (self.no_fluid && self.no_fluid_water)
            || (self.no_overlay && (self.ov_pumpkin || self.ov_snow || self.ov_totem))
            || (self.no_particles
                && (self.part_explosions || self.part_smoke || self.part_potions || self.part_blocks))
    }

    pub fn active_count(&self) -> usize {
        [
            self.fullbright,
            self.no_fluid,
            self.custom_fog,
            self.no_fire,
            self.no_bad_effects,
            self.no_hurt_cam,
            self.no_weather,
            self.no_particles,
            self.no_overlay,
            self.world_time,
            self.ambience,
        ]
        .into_iter()
        .filter(|x| *x)
        .count()
    }
}

pub struct ApplyReport {
    pub pack_written: bool,
    pub options_updated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppliedState {
    pub options_ok: bool,
    pub pack_ok: bool,
}

impl AppliedState {
    pub fn ready(self) -> bool {
        self.options_ok && self.pack_ok
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EffectModSpec {
    pub id: &'static str,
    pub title: &'static str,
    pub slugs: &'static [&'static str],
    pub search_query: &'static str,
    pub patterns: &'static [&'static str],
}

pub const EFFECT_MODS: &[EffectModSpec] = &[
    EffectModSpec {
        id: "fullbright",
        title: "Gamma Utils (Fullbright)",
        slugs: &["gamma-utils", "fullbright", "fullbright-forge", "neofullbright"],
        search_query: "fullbright",
        patterns: &["gamma", "fullbright"],
    },
    EffectModSpec {
        id: "no_fluid",
        title: "Clear Water / Lava",
        slugs: &["clear-waterlavapowdersnow", "clear-water", "clear-lava"],
        search_query: "clear water",
        patterns: &["clear-water", "clearwater", "clear_water", "clear-lava", "clearlava"],
    },
    EffectModSpec {
        id: "custom_fog",
        title: "No Fog / Custom Fog",
        slugs: &["fog-looks-modern-now", "fog-looks-good-now", "clear-fog", "nofog", "no_fog"],
        search_query: "no-fog",
        patterns: &["nofog", "no-fog", "no_fog", "clear-fog", "fog-looks", "foglooks"],
    },
    EffectModSpec {
        id: "no_bad_effects",
        title: "Анти-слепота (CleanView / NoBlindness)",
        slugs: &["noblindnesseffect", "fogbegone", "cleanview", "cleanview-particles"],
        search_query: "no blindness",
        patterns: &["blindness", "cleanview", "fogbegone"],
    },
    EffectModSpec {
        id: "world_time",
        title: "Time Changer",
        slugs: &["time-changer", "polytime", "time-weather-changer"],
        search_query: "time changer",
        patterns: &["time-changer", "timechanger", "polytime", "time-weather"],
    },
    EffectModSpec {
        id: "ambience",
        title: "Sky Aesthetics / Ambience",
        slugs: &["sky-aesthetics", "ambient-environment"],
        search_query: "sky-aesthetics",
        patterns: &["sky-aesthetic", "skyaesthetic", "ambient"],
    },
];

pub fn effect_mod_spec(id: &str) -> Option<EffectModSpec> {
    EFFECT_MODS.iter().copied().find(|s| s.id == id)
}

pub fn mc_minor_version(mc: &str) -> u32 {
    let base = mc.split('-').next().unwrap_or(mc);
    let mut parts = base.split('.');
    let _major = parts.next();
    parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(20)
}

pub fn is_effect_mod_installed(effect_id: &str, mods_cache: &[InstalledMod]) -> bool {
    let Some(spec) = effect_mod_spec(effect_id) else {
        return false;
    };
    // Runs for every effect row on every frame: no lowercase copies.
    mods_cache
        .iter()
        .any(|m| m.enabled && spec.patterns.iter().any(|p| contains_ignore_ascii_case(&m.name, p)))
}

/// `hay.to_lowercase().contains(needle)` for an ASCII-lowercase needle,
/// without allocating.
fn contains_ignore_ascii_case(hay: &str, needle: &str) -> bool {
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    n.is_empty() || h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
}

pub fn missing_mod_targets(
    cfg: &EffectsConfig,
    spec: &Spec,
    mods_cache: &[InstalledMod],
) -> Vec<EffectModSpec> {
    let mut need: Vec<&'static str> = Vec::new();
    if cfg.fullbright && mc_minor_version(&spec.mc) >= 19 {
        need.push("fullbright");
    }
    if cfg.no_fluid && cfg.no_fluid_lava {
        need.push("no_fluid");
    }
    if cfg.custom_fog {
        need.push("custom_fog");
    }
    if cfg.no_bad_effects && (cfg.anti_blindness || cfg.anti_nausea) {
        need.push("no_bad_effects");
    }
    if cfg.world_time {
        need.push("world_time");
    }
    if cfg.ambience {
        need.push("ambience");
    }
    need.into_iter()
        .filter(|id| !is_effect_mod_installed(id, mods_cache))
        .filter_map(effect_mod_spec)
        .collect()
}

pub fn inspect_instance_applied(instance: &Path, cfg: &EffectsConfig) -> AppliedState {
    let pack_path = instance.join("resourcepacks").join("SadEffects.zip");
    let pack_ok = if cfg.needs_resourcepack() {
        pack_path.is_file()
    } else {
        !pack_path.exists()
    };
    let options_path = instance.join("options.txt");
    let existing = std::fs::read_to_string(&options_path).unwrap_or_default();
    let patched = patch_options_text(&existing, cfg);
    let options_ok = patched == existing;
    AppliedState { options_ok, pack_ok }
}

fn make_modrinth_hit(slug: &str, title: &str) -> ModHit {
    ModHit {
        source: "modrinth".into(),
        id: slug.into(),
        slug: slug.into(),
        title: title.into(),
        summary: String::new(),
        downloads: 0,
        author: String::new(),
        kind: mods::KIND_MOD.into(),
        icon_url: None,
        file_id: None,
        file_url: None,
        file_name: None,
        tags: String::new(),
        categories: Vec::new(),
        updated: String::new(),
    }
}

/// Downloads compatible Modrinth mods for the given effect targets into active build's `mods/`.
pub async fn install_effect_mods<F: FnMut(String) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    spec: &Spec,
    targets: Vec<EffectModSpec>,
    cancel: &Arc<AtomicBool>,
    mut on_stage: F,
) -> Result<(Vec<String>, Vec<String>)> {
    if spec.loader == Loader::Vanilla {
        bail!("Моды не ставятся на Vanilla — выбери Fabric, Forge или NeoForge во вкладке «Сборка»");
    }
    let mut installed = Vec::new();
    let mut missing = Vec::new();
    let already = mods::installed(cfg);

    for target in targets {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        if is_effect_mod_installed(target.id, &already) {
            installed.push(target.title.to_string());
            continue;
        }
        on_stage(format!("Подбираю мод: {}…", target.title));
        let mut ok_for_target = false;

        for slug in target.slugs {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let hit = make_modrinth_hit(slug, target.title);
            let mut nop = |_ev: ModEvent| {};
            if let Ok(path) = mods::install(client, cfg, &hit, spec, mods::KIND_MOD, cancel, &mut nop).await {
                let fname = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(target.title)
                    .to_string();
                installed.push(fname);
                ok_for_target = true;
                break;
            }
        }

        if !ok_for_target && !cancel.load(std::sync::atomic::Ordering::Relaxed) {
            if let Ok(hits) = mods::search(
                client,
                cfg,
                target.search_query,
                "modrinth",
                spec,
                mods::KIND_MOD,
                "downloads",
                "",
                0,
            )
            .await
            {
                for hit in hits.into_iter().take(3) {
                    if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    let mut nop = |_ev: ModEvent| {};
                    if let Ok(path) = mods::install(client, cfg, &hit, spec, mods::KIND_MOD, cancel, &mut nop).await {
                        let fname = path
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&hit.title)
                            .to_string();
                        installed.push(fname);
                        ok_for_target = true;
                        break;
                    }
                }
            }
        }

        if !ok_for_target {
            missing.push(format!("{} (нет версии для {})", target.title, spec.title()));
        }
    }

    Ok((installed, missing))
}

fn config_path() -> PathBuf {
    crate::config::data_dir().join("effects.json")
}

pub fn load() -> EffectsConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or_default(),
        Err(_) => EffectsConfig::default(),
    }
}

pub fn save(cfg: &EffectsConfig) {
    crate::config::ensure_dirs();
    if let Ok(text) = serde_json::to_string_pretty(cfg) {
        let path = config_path();
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn pack_format_for_mc(mc: &str) -> u32 {
    let base = mc.split('-').next().unwrap_or(mc);
    let mut parts = base.split('.');
    let major = parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(1);
    let minor = parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(20);
    let patch = parts.next().and_then(|p| p.parse::<u32>().ok()).unwrap_or(0);
    match (major, minor, patch) {
        (1, 21, p) if p >= 4 => 46,
        (1, 21, _) => 34,
        (1, 20, p) if p >= 5 => 32,
        (1, 20, p) if p >= 2 => 18,
        (1, 20, _) => 15,
        (1, 19, 4..) => 13,
        (1, 19, _) => 12,
        (1, 18, _) => 8,
        (1, 17, _) => 7,
        (1, 16, _) => 6,
        (1, 15, _) => 5,
        (1, 13..=14, _) => 4,
        _ => 3,
    }
}

fn png_bytes(img: &RgbaImage) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .context("Не удалось кодировать PNG ресурспака")?;
    Ok(out)
}

fn transparent_png(size: u32) -> Result<Vec<u8>> {
    let img = RgbaImage::from_pixel(size, size, Rgba([0, 0, 0, 0]));
    png_bytes(&img)
}

fn low_fire_png(soul: bool) -> Result<Vec<u8>> {
    let mut img = RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]));
    let col = if soul {
        Rgba([95, 205, 240, 120])
    } else {
        Rgba([255, 145, 45, 120])
    };
    for y in 13..16 {
        for x in 0..16 {
            if (x + y) % 2 == 0 || y == 15 {
                img.put_pixel(x, y, col);
            }
        }
    }
    png_bytes(&img)
}

fn build_pack_zip(mc_version: &str, cfg: &EffectsConfig) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();

        let mcmeta = format!(
            "{{\"pack\":{{\"pack_format\":{},\"description\":\"Sad Launcher: Эффекты\"}}}}",
            pack_format_for_mc(mc_version)
        );
        zip.start_file("pack.mcmeta", opts)?;
        zip.write_all(mcmeta.as_bytes())?;

        let blank = transparent_png(16)?;

        if cfg.no_fire {
            let normal = if cfg.no_fire_hide { blank.clone() } else { low_fire_png(false)? };
            let soul = if cfg.no_fire_hide { blank.clone() } else { low_fire_png(true)? };
            for (name, bytes) in [
                ("assets/minecraft/textures/block/fire_0.png", &normal),
                ("assets/minecraft/textures/block/fire_1.png", &normal),
                ("assets/minecraft/textures/block/soul_fire_0.png", &soul),
                ("assets/minecraft/textures/block/soul_fire_1.png", &soul),
            ] {
                zip.start_file(name, opts)?;
                zip.write_all(bytes)?;
                zip.start_file(format!("{name}.mcmeta"), opts)?;
                zip.write_all(b"{\"animation\":{\"frametime\":2}}")?;
            }
        }

        if cfg.no_fluid && cfg.no_fluid_water {
            zip.start_file("assets/minecraft/textures/misc/underwater.png", opts)?;
            zip.write_all(&blank)?;
        }

        if cfg.no_overlay {
            if cfg.ov_pumpkin {
                zip.start_file("assets/minecraft/textures/misc/pumpkinblur.png", opts)?;
                zip.write_all(&blank)?;
            }
            if cfg.ov_snow {
                zip.start_file("assets/minecraft/textures/misc/powder_snow_outline.png", opts)?;
                zip.write_all(&blank)?;
            }
            if cfg.ov_totem {
                zip.start_file("assets/minecraft/textures/item/totem_of_undying.png", opts)?;
                zip.write_all(&blank)?;
            }
        }

        if cfg.no_weather {
            for p in [
                "assets/minecraft/textures/environment/rain.png",
                "assets/minecraft/textures/environment/snow.png",
            ] {
                zip.start_file(p, opts)?;
                zip.write_all(&blank)?;
            }
        }

        if cfg.no_particles {
            if cfg.part_explosions {
                for i in 0..16 {
                    zip.start_file(format!("assets/minecraft/textures/particle/explosion_{i}.png"), opts)?;
                    zip.write_all(&blank)?;
                }
                zip.start_file("assets/minecraft/textures/particle/flash.png", opts)?;
                zip.write_all(&blank)?;
            }
            if cfg.part_smoke {
                for i in 0..8 {
                    zip.start_file(format!("assets/minecraft/textures/particle/generic_{i}.png"), opts)?;
                    zip.write_all(&blank)?;
                }
                for i in 0..12 {
                    zip.start_file(format!("assets/minecraft/textures/particle/big_smoke_{i}.png"), opts)?;
                    zip.write_all(&blank)?;
                }
            }
            if cfg.part_potions {
                for i in 0..8 {
                    zip.start_file(format!("assets/minecraft/textures/particle/effect_{i}.png"), opts)?;
                    zip.write_all(&blank)?;
                    zip.start_file(format!("assets/minecraft/textures/particle/spell_{i}.png"), opts)?;
                    zip.write_all(&blank)?;
                }
                zip.start_file("assets/minecraft/textures/particle/glint.png", opts)?;
                zip.write_all(&blank)?;
            }
            if cfg.part_blocks {
                for name in ["crit.png", "enchanted_hit.png"] {
                    zip.start_file(format!("assets/minecraft/textures/particle/{name}"), opts)?;
                    zip.write_all(&blank)?;
                }
            }
        }

        zip.finish()?;
    }
    Ok(buf.into_inner())
}

const PACK_ENTRY: &str = "file/SadEffects.zip";

fn set_option_kv(lines: &mut Vec<String>, key: &str, val: String) {
    let prefix = format!("{key}:");
    if let Some(line) = lines.iter_mut().find(|l| l.starts_with(&prefix)) {
        *line = format!("{key}:{val}");
    } else {
        lines.push(format!("{key}:{val}"));
    }
}

/// Patches `options.txt` content in place without losing unrelated user settings.
pub fn patch_options_text(existing: &str, cfg: &EffectsConfig) -> String {
    let mut lines: Vec<String> = existing.lines().map(String::from).collect();

    if cfg.fullbright {
        set_option_kv(&mut lines, "gamma", format!("{:.1}", cfg.fullbright_gamma.clamp(1.0, 20.0)));
    } else {
        // Only reset gamma back to 1.0 if it was previously boosted above normal (>1.5).
        if let Some(line) = lines.iter_mut().find(|l| l.starts_with("gamma:")) {
            if let Ok(g) = line.trim_start_matches("gamma:").trim().parse::<f32>() {
                if g > 1.5 {
                    *line = "gamma:1.0".to_string();
                }
            }
        }
    }

    if cfg.no_hurt_cam {
        set_option_kv(&mut lines, "damageTiltStrength", "0.0".into());
    } else if lines.iter().any(|l| l == "damageTiltStrength:0.0") {
        set_option_kv(&mut lines, "damageTiltStrength", "1.0".into());
    }

    if cfg.no_bad_effects && cfg.anti_darkness {
        set_option_kv(&mut lines, "darknessEffectScale", "0.0".into());
    } else if lines.iter().any(|l| l == "darknessEffectScale:0.0") {
        set_option_kv(&mut lines, "darknessEffectScale", "1.0".into());
    }

    if cfg.no_weather {
        set_option_kv(&mut lines, "hideLightningFlashes", "true".into());
    } else if lines.iter().any(|l| l == "hideLightningFlashes:true") {
        set_option_kv(&mut lines, "hideLightningFlashes", "false".into());
    }

    if cfg.no_particles {
        set_option_kv(&mut lines, "particles", "2".into());
    }

    let want_pack = cfg.needs_resourcepack();
    let mut found_packs = false;
    for line in &mut lines {
        if let Some(raw) = line.strip_prefix("resourcePacks:") {
            found_packs = true;
            let mut list: Vec<String> = serde_json::from_str(raw.trim()).unwrap_or_else(|_| vec!["vanilla".into()]);
            if !list.iter().any(|x| x == "vanilla") {
                list.insert(0, "vanilla".into());
            }
            list.retain(|x| x != PACK_ENTRY);
            if want_pack {
                list.push(PACK_ENTRY.into());
            }
            *line = format!("resourcePacks:{}", serde_json::to_string(&list).unwrap_or_else(|_| "[\"vanilla\"]".into()));
            break;
        }
    }
    if !found_packs && want_pack {
        lines.push(format!("resourcePacks:[\"vanilla\",\"{PACK_ENTRY}\"]"));
    }

    let mut out = lines.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Applies `cfg` to `instance`: generates/removes `resourcepacks/SadEffects.zip`
/// and updates `options.txt`.
pub fn apply_to_instance(instance: &Path, mc_version: &str, cfg: &EffectsConfig) -> Result<ApplyReport> {
    std::fs::create_dir_all(instance).context("Не создать папку сборки")?;
    let rp_dir = instance.join("resourcepacks");
    let pack_path = rp_dir.join("SadEffects.zip");

    let pack_written = if cfg.needs_resourcepack() {
        std::fs::create_dir_all(&rp_dir)?;
        let bytes = build_pack_zip(mc_version, cfg)?;
        std::fs::write(&pack_path, bytes).context("Не записать ресурспак SadEffects.zip")?;
        true
    } else {
        if pack_path.exists() {
            let _ = std::fs::remove_file(&pack_path);
        }
        false
    };

    let options_path = instance.join("options.txt");
    let existing = std::fs::read_to_string(&options_path).unwrap_or_default();
    let updated = patch_options_text(&existing, cfg);
    // Nothing to change → nothing written (an empty options.txt used to be
    // created in every build without one).
    let options_updated = updated != existing;
    if options_updated {
        std::fs::write(&options_path, updated).context("Не обновить options.txt")?;
    }

    Ok(ApplyReport {
        pack_written,
        options_updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_format_matches_mc_releases() {
        assert_eq!(pack_format_for_mc("1.21.4"), 46);
        assert_eq!(pack_format_for_mc("1.21.1"), 34);
        assert_eq!(pack_format_for_mc("1.20.1-forge-47.4.0"), 15);
        assert_eq!(pack_format_for_mc("1.19.2"), 12);
        assert_eq!(pack_format_for_mc("1.16.5"), 6);
        assert_eq!(pack_format_for_mc("1.12.2"), 3);
    }

    #[test]
    fn patch_options_preserves_existing_keys_and_toggles_pack() {
        let orig = "fov:90.0\ngamma:1.0\nresourcePacks:[\"vanilla\"]\n";
        let mut cfg = EffectsConfig::default();
        cfg.fullbright = true;
        cfg.no_hurt_cam = true;
        cfg.no_fire = true;
        let patched = patch_options_text(orig, &cfg);
        assert!(patched.contains("fov:90.0"));
        assert!(patched.contains("gamma:15.0"));
        assert!(patched.contains("damageTiltStrength:0.0"));
        assert!(patched.contains("file/SadEffects.zip"));

        cfg.fullbright = false;
        cfg.no_hurt_cam = false;
        cfg.no_fire = false;
        let restored = patch_options_text(&patched, &cfg);
        assert!(restored.contains("gamma:1.0"));
        assert!(restored.contains("damageTiltStrength:1.0"));
        assert!(!restored.contains("file/SadEffects.zip"));
    }

    #[test]
    fn builds_valid_sad_effects_zip_with_requested_textures() {
        let mut cfg = EffectsConfig::default();
        cfg.no_fire = true;
        cfg.no_overlay = true;
        cfg.no_weather = true;
        cfg.no_particles = true;
        let bytes = build_pack_zip("1.20.1", &cfg).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("pack.mcmeta").is_ok());
        assert!(zip.by_name("assets/minecraft/textures/block/fire_0.png").is_ok());
        assert!(zip.by_name("assets/minecraft/textures/misc/pumpkinblur.png").is_ok());
        assert!(zip.by_name("assets/minecraft/textures/environment/rain.png").is_ok());
        assert!(zip.by_name("assets/minecraft/textures/particle/explosion_0.png").is_ok());
    }

    #[test]
    fn detects_effect_mods_and_mc_minor_version() {
        assert_eq!(mc_minor_version("1.20.1-forge-47.4.0"), 20);
        assert_eq!(mc_minor_version("1.18.2"), 18);
        assert_eq!(mc_minor_version("1.21.4"), 21);

        let fake_mods = vec![
            InstalledMod {
                name: "gamma-utils-1.7.16".into(),
                file: "gamma-utils-1.7.16.jar".into(),
                size: 123,
                enabled: true,
            },
            InstalledMod {
                name: "nofog-1.3.2".into(),
                file: "nofog-1.3.2.jar.disabled".into(),
                size: 456,
                enabled: false,
            },
        ];
        assert!(is_effect_mod_installed("fullbright", &fake_mods));
        assert!(!is_effect_mod_installed("custom_fog", &fake_mods));

        let loaded: EffectsConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(loaded.anti_afk_secs, 60);
        assert_eq!(loaded.autoclicker_bind, "F6");
        assert_eq!(loaded.anti_afk_bind, "F7");
    }
}
