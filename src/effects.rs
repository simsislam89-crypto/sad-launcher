//! Эффекты — настройки освещения, видимости в жидкостях и очистки экрана.
//! Применяются к активной сборке в чистом Rust: патчит `options.txt` и
//! генерирует ресурспак `resourcepacks/SadEffects.zip` с нужными текстурами.

use anyhow::{Context, Result};
use image::{ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    let options_updated = updated != existing || !options_path.exists();
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
}
