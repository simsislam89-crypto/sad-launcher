//! Эффекты — настройки освещения, видимости в жидкостях и очистки экрана.
//!
//! Всё, что можно, меняется ПРЯМО В ИГРЕ: эффекты живут в ресурспаке-папке
//! `resourcepacks/SadEffects/`, который всегда включён в сборке. Лаунчер
//! переписывает его файлы и шлёт окну игры F3+T — Minecraft перечитывает
//! ресурсы за секунду, без перезапуска. Яркость и туман — через core-шейдеры
//! (`shaders/include/light.glsl`, `fog.glsl`), взятые из клиента этой же
//! версии и пропатченные; набор FPS-ресурспаков с Modrinth вливается в ту же
//! папку. `options.txt` (гамма, тряска камеры, частицы) — после перезапуска.

use crate::config::Config;
use crate::loader::{Loader, Spec};
use crate::mods::{self, InstalledMod, ModEvent, ModHit};
use anyhow::{bail, Context, Result};
use image::{ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
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

    /// Ресурспаки набора «FPS и чистый экран» (slug Modrinth), влитые в
    /// SadEffects — включаются и выключаются прямо в игре.
    pub rp_set: Vec<String>,
}

/// Один ресурспак набора.
pub struct RpPick {
    pub slug: &'static str,
    pub title: &'static str,
    pub desc: &'static str,
}

/// Набор «FPS и чистый экран»: самые скачиваемые паки Modrinth, которым не
/// нужны моды, — меньше огня и щита на экране, прозрачное стекло, лёгкие
/// и понятные текстуры.
pub const RP_SET: [RpPick; 6] = [
    RpPick { slug: "low-fire-pack", title: "Низкий огонь", desc: "Огонь на экране в 2 раза ниже — видно, куда бежать (8 млн скачиваний)" },
    RpPick { slug: "low-shield-pack", title: "Низкий щит", desc: "Щит не закрывает пол-экрана (5,9 млн)" },
    RpPick { slug: "clear-glass", title: "Чистое стекло", desc: "Стекло без рамок и разводов (414 тыс.)" },
    RpPick { slug: "fast-better-grass", title: "Быстрая трава", desc: "Трава по бокам блоков без потерь FPS (14 млн)" },
    RpPick { slug: "even-better-enchants", title: "Понятные зачарования", desc: "Цвет книги показывает зачарование (5,4 млн)" },
    RpPick { slug: "better-lanterns", title: "Аккуратные фонари", desc: "Фонари и цепи без лишних пикселей (4 млн)" },
];

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
            rp_set: Vec::new(),
        }
    }
}

impl EffectsConfig {
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
    let pack_ok = instance.join("resourcepacks").join(PACK_DIR).join("pack.mcmeta").is_file();
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
        // 26.1+ (calendar versions): the exact number comes from the client
        // jar; without it any format of the new scheme fits (see pack_mcmeta).
        (m, _, _) if m >= 26 => NEW_PACK_FORMATS,
        (1, 21, p) if p >= 11 => 75,
        (1, 21, 9..=10) => 69,
        (1, 21, 7..=8) => 64,
        (1, 21, 6) => 63,
        (1, 21, 5) => 55,
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

/// From this resource format on (1.21.9) a pack declares `min_format` /
/// `max_format`; an old-style `pack_format` makes the game drop the pack
/// from options.txt as «no longer compatible».
const NEW_PACK_FORMATS: u32 = 65;

/// The resource pack format of the client itself (`version.json` in the
/// jar): `"pack_version": 8`, `{"resource": 15}` or `{"resource_major": 97}`.
pub fn client_pack_format(jar: &Path) -> Option<u32> {
    let text = jar_text(jar, "version.json")?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let pv = v.get("pack_version")?;
    let n = pv
        .as_u64()
        .or_else(|| pv.get("resource_major").and_then(|x| x.as_u64()))
        .or_else(|| pv.get("resource").and_then(|x| x.as_u64()))?;
    u32::try_from(n).ok().filter(|n| *n > 0)
}

/// `pack.mcmeta` the game with resource format `format` accepts. New
/// scheme: exactly this format (any minor) — or, when the exact number is
/// unknown (`format == NEW_PACK_FORMATS`), every format of the new scheme.
pub fn pack_mcmeta(format: u32) -> String {
    const DESC: &str = "Sad Launcher: эффекты (меняются из лаунчера прямо в игре)";
    if format > NEW_PACK_FORMATS {
        format!("{{\"pack\":{{\"min_format\":{format},\"max_format\":{format},\"description\":\"{DESC}\"}}}}")
    } else if format == NEW_PACK_FORMATS {
        format!("{{\"pack\":{{\"min_format\":{format},\"max_format\":9999,\"description\":\"{DESC}\"}}}}")
    } else {
        format!("{{\"pack\":{{\"pack_format\":{format},\"supported_formats\":[1,{}],\"description\":\"{DESC}\"}}}}", NEW_PACK_FORMATS - 1)
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

/// Files of the SadEffects pack: (path inside the pack, bytes).
type PackFiles = Vec<(String, Vec<u8>)>;

/// `vec4 name(type a, type b) {` → (start of `vec4`, parameter names).
fn find_function(src: &str, ret: &str, name: &str) -> Option<(usize, Vec<String>)> {
    let sig = format!("{ret} {name}(");
    let at = src.find(&sig)?;
    let open = at + sig.len();
    let close = open + src[open..].find(')')?;
    let names = src[open..close]
        .split(',')
        .filter_map(|p| p.split_whitespace().last().map(|n| n.trim().to_string()))
        .filter(|n| !n.is_empty())
        .collect();
    Some((at, names))
}

/// Renames `ret name(` to `ret sad_orig_name(` and puts `wrapper` (which
/// calls the original under the new name) RIGHT AFTER the original's body:
/// GLSL needs a function declared before use, and in 26.x the same file
/// calls it further down (`total_fog_value` → `linear_fog_value`); it also
/// stays inside the `#ifndef` include guard. None when not found.
fn wrap_function(src: &str, ret: &str, name: &str, wrapper: impl Fn(&[String], &str) -> String) -> Option<String> {
    let (at, params) = find_function(src, ret, name)?;
    let orig = format!("sad_orig_{name}");
    let sig_end = at + src[at..].find('(')?;
    let close = sig_end + src[sig_end..].find(')')?;
    let params_text = &src[sig_end + 1..close];
    let body_open = close + src[close..].find('{')?;
    let mut depth = 0usize;
    let mut body_end = None;
    for (i, ch) in src[body_open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = Some(body_open + i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let body_end = body_end?;
    let mut out = String::with_capacity(src.len() + 400);
    out.push_str(&src[..at]);
    out.push_str(&format!("{ret} {orig}"));
    out.push_str(&src[at + ret.len() + 1 + name.len()..body_end]);
    out.push_str(&format!("\n\n// Sad Launcher\n{ret} {name}({params_text}) {{\n    {}\n}}", wrapper(&params, &orig)));
    out.push_str(&src[body_end..]);
    Some(out)
}

/// Fullbright that works live: the lightmap sample is pulled towards
/// white by `strength` (0..1). Patched from the game's own shader:
/// `minecraft_sample_lightmap` (light.glsl, up to 1.21.x) or
/// `sample_lightmap` (sample_lightmap.glsl, 26.x).
pub fn patch_light_glsl(src: &str, strength: f32) -> Option<String> {
    let k = strength.clamp(0.0, 1.0);
    ["minecraft_sample_lightmap", "sample_lightmap"].iter().find_map(|name| {
        wrap_function(src, "vec4", name, |p, orig| {
            format!("return mix({orig}({}), vec4(1.0), {k:.3});", p.join(", "))
        })
    })
}

/// Shader includes that hold the lightmap sampler, newest layout last.
const LIGHT_SHADERS: [&str; 2] = [
    "assets/minecraft/shaders/include/light.glsl",
    "assets/minecraft/shaders/include/sample_lightmap.glsl",
];

/// Fog off (`disable`) or pushed `scale` times farther, live. Covers air,
/// water and lava fog — the vanilla renderer draws all three through
/// `linear_fog` / `linear_fog_fade`.
pub fn patch_fog_glsl(src: &str, disable: bool, scale: f32) -> Option<String> {
    let scale = scale.clamp(1.0, 64.0);
    let scaled_args = |p: &[String]| -> String {
        p.iter()
            .map(|n| {
                let l = n.to_ascii_lowercase();
                if l.contains("start") || l.contains("end") {
                    format!("{n} * {scale:.2}")
                } else {
                    n.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    // 1.21.6+ / 26.x: every fog (environment, render distance, sky,
    // clouds) is one `linear_fog_value` → 0.0 means no fog.
    if let Some(out) = wrap_function(src, "float", "linear_fog_value", |p, orig| {
        if disable {
            "return 0.0;".to_string()
        } else {
            format!("return {orig}({});", scaled_args(p))
        }
    }) {
        return Some(out);
    }
    let step = wrap_function(src, "vec4", "linear_fog", |p, orig| {
        if disable {
            format!("return {};", p.first().map(|s| s.as_str()).unwrap_or("vec4(0.0)"))
        } else {
            format!("return {orig}({});", scaled_args(p))
        }
    })?;
    // Clouds and sky fade use the second function (absent in some versions).
    Some(
        wrap_function(&step, "float", "linear_fog_fade", |p, orig| {
            if disable {
                "return 1.0;".to_string()
            } else {
                format!("return {orig}({});", scaled_args(p))
            }
        })
        .unwrap_or(step),
    )
}

/// A text file from the game's client jar.
fn jar_text(jar: &Path, name: &str) -> Option<String> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(jar).ok()?).ok()?;
    let mut entry = zip.by_name(name).ok()?;
    let mut text = String::new();
    std::io::Read::read_to_string(&mut entry, &mut text).ok()?;
    Some(text)
}

/// Where downloaded set packs are kept (shared by all builds).
pub fn rp_cache_dir() -> PathBuf {
    crate::config::data_dir().join("meta").join("rp-cache")
}

/// Cached zip of a set pack for this Minecraft version.
pub fn rp_cached(slug: &str, mc: &str) -> PathBuf {
    let mc = mc.split('-').next().unwrap_or(mc);
    rp_cache_dir().join(format!("{slug}-{mc}.zip"))
}

/// Downloads the set packs that are switched on but not cached yet:
/// the version made for this Minecraft, or the newest one.
pub async fn fetch_set_packs(client: &reqwest::Client, mc: &str, slugs: &[String]) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct V {
        #[serde(default)]
        files: Vec<F>,
    }
    #[derive(Deserialize)]
    struct F {
        url: String,
        #[serde(default)]
        primary: bool,
    }
    let base = mc.split('-').next().unwrap_or(mc).to_string();
    let mut fetched = Vec::new();
    for slug in slugs {
        let dest = rp_cached(slug, &base);
        if dest.is_file() {
            continue;
        }
        let mut url = None;
        for filter in [format!("?game_versions=[\"{base}\"]"), String::new()] {
            let list: Vec<V> = client
                .get(format!("https://api.modrinth.com/v2/project/{slug}/version{filter}"))
                .send()
                .await
                .context("Modrinth не отвечает")?
                .json()
                .await
                .unwrap_or_default();
            if let Some(v) = list.into_iter().find(|v| !v.files.is_empty()) {
                let f = v.files.iter().find(|f| f.primary).unwrap_or(&v.files[0]);
                url = Some(f.url.clone());
                break;
            }
        }
        let Some(url) = url.filter(|u| u.starts_with("https://")) else {
            continue;
        };
        std::fs::create_dir_all(rp_cache_dir())?;
        crate::minecraft::download_file(client, &url, &dest, None).await?;
        fetched.push(slug.clone());
    }
    Ok(fetched)
}

/// `assets/…` files of a downloaded pack (never outside `assets/`, no
/// absolute paths or `..`; at most 64 МБ unpacked).
fn set_pack_assets(zip_path: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(file) = std::fs::File::open(zip_path) else {
        return Vec::new();
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else { continue };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if !name.starts_with("assets/") || name.contains("..") || name.contains(':') {
            continue;
        }
        total += entry.size();
        if total > 64 * 1024 * 1024 {
            break;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        if std::io::Read::read_to_end(&mut entry, &mut bytes).is_ok() {
            out.push((name, bytes));
        }
    }
    out
}

/// Everything the SadEffects pack holds for `cfg`. Set packs go first so
/// the launcher's own textures win where both change the same file.
fn pack_files(mc_version: &str, cfg: &EffectsConfig, client_jar: Option<&Path>) -> Result<PackFiles> {
    let mut files: PackFiles = Vec::new();
    let put = |files: &mut PackFiles, name: String, bytes: Vec<u8>| {
        files.retain(|(n, _)| *n != name);
        files.push((name, bytes));
    };
    let format = client_jar
        .and_then(client_pack_format)
        .unwrap_or_else(|| pack_format_for_mc(mc_version));
    put(&mut files, "pack.mcmeta".into(), pack_mcmeta(format).into_bytes());

    for slug in &cfg.rp_set {
        for (name, bytes) in set_pack_assets(&rp_cached(slug, mc_version)) {
            put(&mut files, name, bytes);
        }
    }

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
            put(&mut files, name.into(), bytes.clone());
            put(&mut files, format!("{name}.mcmeta"), b"{\"animation\":{\"frametime\":2}}".to_vec());
        }
    }
    if cfg.no_fluid && cfg.no_fluid_water {
        put(&mut files, "assets/minecraft/textures/misc/underwater.png".into(), blank.clone());
    }
    if cfg.no_overlay {
        if cfg.ov_pumpkin {
            put(&mut files, "assets/minecraft/textures/misc/pumpkinblur.png".into(), blank.clone());
            put(&mut files, "assets/minecraft/textures/misc/vignette.png".into(), blank.clone());
        }
        if cfg.ov_snow {
            put(&mut files, "assets/minecraft/textures/misc/powder_snow_outline.png".into(), blank.clone());
        }
        if cfg.ov_totem {
            put(&mut files, "assets/minecraft/textures/item/totem_of_undying.png".into(), blank.clone());
        }
    }
    if cfg.no_weather {
        for p in ["assets/minecraft/textures/environment/rain.png", "assets/minecraft/textures/environment/snow.png"] {
            put(&mut files, p.into(), blank.clone());
        }
    }
    if cfg.no_particles {
        if cfg.part_explosions {
            for i in 0..16 {
                put(&mut files, format!("assets/minecraft/textures/particle/explosion_{i}.png"), blank.clone());
            }
            put(&mut files, "assets/minecraft/textures/particle/flash.png".into(), blank.clone());
        }
        if cfg.part_smoke {
            for i in 0..8 {
                put(&mut files, format!("assets/minecraft/textures/particle/generic_{i}.png"), blank.clone());
            }
            for i in 0..12 {
                put(&mut files, format!("assets/minecraft/textures/particle/big_smoke_{i}.png"), blank.clone());
            }
        }
        if cfg.part_potions {
            for i in 0..8 {
                put(&mut files, format!("assets/minecraft/textures/particle/effect_{i}.png"), blank.clone());
                put(&mut files, format!("assets/minecraft/textures/particle/spell_{i}.png"), blank.clone());
            }
            put(&mut files, "assets/minecraft/textures/particle/glint.png".into(), blank.clone());
        }
        if cfg.part_blocks {
            for name in ["crit.png", "enchanted_hit.png"] {
                put(&mut files, format!("assets/minecraft/textures/particle/{name}"), blank.clone());
            }
        }
    }

    // Live lighting / fog through the game's own core shaders.
    if let Some(jar) = client_jar {
        if cfg.fullbright {
            let strength = ((cfg.fullbright_gamma - 1.0) / 14.0).clamp(0.15, 1.0);
            for path in LIGHT_SHADERS {
                if let Some(patched) = jar_text(jar, path).and_then(|src| patch_light_glsl(&src, strength)) {
                    put(&mut files, path.into(), patched.into_bytes());
                    break;
                }
            }
        }
        let fog_off = (cfg.custom_fog && cfg.fog_disable) || cfg.no_fluid;
        let fog_scale = if cfg.custom_fog && !cfg.fog_disable { 1.0 / cfg.fog_density.max(0.05) } else { 1.0 };
        if fog_off || fog_scale > 1.01 {
            if let Some(patched) = jar_text(jar, "assets/minecraft/shaders/include/fog.glsl")
                .and_then(|src| patch_fog_glsl(&src, fog_off, fog_scale))
            {
                put(&mut files, "assets/minecraft/shaders/include/fog.glsl".into(), patched.into_bytes());
            }
        }
    }
    Ok(files)
}

/// Writes `files` into the pack folder: only what changed is written,
/// files that are no longer part of the pack are removed. True if anything
/// on disk changed (then the running game should reload).
fn write_pack_dir(dir: &Path, files: &PackFiles) -> Result<bool> {
    std::fs::create_dir_all(dir)?;
    let mut changed = false;
    let wanted: std::collections::HashSet<PathBuf> = files.iter().map(|(n, _)| dir.join(n)).collect();
    for (name, bytes) in files {
        let path = dir.join(name);
        if std::fs::read(&path).map(|old| old == *bytes).unwrap_or(false) {
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, bytes).with_context(|| format!("Не записать {name}"))?;
        changed = true;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if !wanted.contains(&p) {
                let _ = std::fs::remove_file(&p);
                changed = true;
            }
        }
    }
    Ok(changed)
}

/// The pack folder inside `resourcepacks/` and its options.txt entry.
pub const PACK_DIR: &str = "SadEffects";
const PACK_ENTRY: &str = "file/SadEffects";
/// Before 0.7.7 the pack was a zip — removed on the next apply.
const LEGACY_ENTRY: &str = "file/SadEffects.zip";

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

    // The pack is always on: switching effects only rewrites its files and
    // the running game reloads them (F3+T) — no restart, no options edit.
    let want_pack = true;
    let mut found_packs = false;
    for line in &mut lines {
        if let Some(raw) = line.strip_prefix("resourcePacks:") {
            found_packs = true;
            let mut list: Vec<String> = serde_json::from_str(raw.trim()).unwrap_or_else(|_| vec!["vanilla".into()]);
            if !list.iter().any(|x| x == "vanilla") {
                list.insert(0, "vanilla".into());
            }
            list.retain(|x| x != PACK_ENTRY && x != LEGACY_ENTRY);
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
    // Never in the «incompatible» list: a pack that is listed there and is
    // compatible again gets dropped from the list AND skipped for that
    // start (seen live on 26.3) — 0.7.8 put it there, take it out.
    for line in &mut lines {
        if let Some(raw) = line.strip_prefix("incompatibleResourcePacks:") {
            let mut list: Vec<String> = serde_json::from_str(raw.trim()).unwrap_or_default();
            if list.iter().any(|x| x == PACK_ENTRY) {
                list.retain(|x| x != PACK_ENTRY);
                *line = format!("incompatibleResourcePacks:{}", serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()));
            }
            break;
        }
    }

    let mut out = lines.join("\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Applies `cfg` to `instance`: rewrites `resourcepacks/SadEffects/`
/// (`pack_written` = something changed → reload a running game) and
/// updates `options.txt`. `client_jar` feeds the live shader effects.
pub fn apply_to_instance(
    instance: &Path,
    mc_version: &str,
    cfg: &EffectsConfig,
    client_jar: Option<&Path>,
) -> Result<ApplyReport> {
    std::fs::create_dir_all(instance).context("Не создать папку сборки")?;
    let rp_dir = instance.join("resourcepacks");
    let legacy = rp_dir.join("SadEffects.zip");
    if legacy.exists() {
        let _ = std::fs::remove_file(&legacy);
    }
    let files = pack_files(mc_version, cfg, client_jar)?;
    let pack_written = write_pack_dir(&rp_dir.join(PACK_DIR), &files).context("Не записать ресурспак SadEffects")?;

    let options_path = instance.join("options.txt");
    let existing = std::fs::read_to_string(&options_path).unwrap_or_default();
    let updated = patch_options_text(&existing, cfg);
    let options_updated = updated != existing;
    if options_updated {
        std::fs::write(&options_path, updated).context("Не обновить options.txt")?;
    }
    Ok(ApplyReport { pack_written, options_updated })
}

/// The vanilla client jar of `mc` in the shared game folder.
pub fn client_jar(cfg: &Config, mc: &str) -> Option<PathBuf> {
    let mc = mc.split('-').next().unwrap_or(mc);
    let jar = cfg.game_dir().join("versions").join(mc).join(format!("{mc}.jar"));
    jar.is_file().then_some(jar)
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
        assert_eq!(pack_format_for_mc("1.21.5"), 55);
        assert_eq!(pack_format_for_mc("1.21.11"), 75);
        // 26.x used to fall through to 3 → the game dropped the pack.
        assert_eq!(pack_format_for_mc("26.3"), NEW_PACK_FORMATS);
        assert_eq!(pack_format_for_mc("26.3-forge-66.1.9"), NEW_PACK_FORMATS);
    }

    /// The game's own rules: old clients read `pack_format`, 1.21.9+ only
    /// `min_format`/`max_format` (as in the jar's built-in packs).
    #[test]
    fn pack_mcmeta_matches_client_scheme() {
        let parse = |s: String| serde_json::from_str::<serde_json::Value>(&s).unwrap()["pack"].clone();
        let old = parse(pack_mcmeta(15));
        assert_eq!(old["pack_format"], 15);
        assert!(old.get("min_format").is_none());
        let new = parse(pack_mcmeta(97));
        assert_eq!(new["min_format"], 97);
        assert_eq!(new["max_format"], 97);
        assert!(new.get("pack_format").is_none(), "26.x rejects old-style packs");
        let unknown = parse(pack_mcmeta(NEW_PACK_FORMATS));
        assert_eq!(unknown["min_format"], 65);
        assert!(unknown["max_format"].as_u64().unwrap() > 1000);
    }

    #[test]
    fn pack_is_taken_out_of_incompatible_list() {
        let cfg = EffectsConfig::default();
        let out = patch_options_text(
            "resourcePacks:[\"vanilla\",\"file/SadEffects\"]\nincompatibleResourcePacks:[\"file/Old.zip\",\"file/SadEffects\"]\n",
            &cfg,
        );
        assert!(out.contains("incompatibleResourcePacks:[\"file/Old.zip\"]"));
        assert!(out.contains("resourcePacks:[\"vanilla\",\"file/SadEffects\"]"));
        assert_eq!(patch_options_text(&out, &cfg), out, "second apply changes nothing");
        assert!(!patch_options_text("fov:70.0\n", &cfg).contains("incompatibleResourcePacks"));
    }

    const LIGHT_26: &str = "#ifndef MINECRAFT_SAMPLE_LIGHTMAP_GLSL\n#define MINECRAFT_SAMPLE_LIGHTMAP_GLSL\n\nvec4 sample_lightmap(sampler2D lightMap, ivec2 uv) {\n    return texture(lightMap, clamp((uv / 256.0) + 0.5 / 16.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0)));\n}\n\n#endif\n";
    const FOG_26: &str = "float linear_fog_value(float vertexDistance, float fogStart, float fogEnd) {\n    if (vertexDistance <= fogStart) {\n        return 0.0;\n    }\n    return (vertexDistance - fogStart) / (fogEnd - fogStart);\n}\n\nfloat total_fog_value(float a, float b) {\n    return max(linear_fog_value(a, 1.0, 2.0), b);\n}\n";

    #[test]
    fn shaders_of_26_x_are_patched() {
        let light = patch_light_glsl(LIGHT_26, 1.0).unwrap();
        assert!(light.contains("vec4 sad_orig_sample_lightmap(sampler2D lightMap, ivec2 uv)"));
        assert!(light.contains("return mix(sad_orig_sample_lightmap(lightMap, uv), vec4(1.0), 1.000);"));
        let off = patch_fog_glsl(FOG_26, true, 1.0).unwrap();
        assert!(off.contains("float linear_fog_value(float vertexDistance, float fogStart, float fogEnd) {\n    return 0.0;"));
        assert!(off.contains("float total_fog_value("), "rest of fog.glsl kept");
        // Declared before its caller in the same file (live 26.3: «no
        // matching overloaded function» when appended at the end).
        assert!(off.find("float linear_fog_value(").unwrap() < off.find("float total_fog_value(").unwrap());
        assert!(light.find("vec4 sample_lightmap(").unwrap() < light.find("#endif").unwrap(), "inside the include guard");
        let far = patch_fog_glsl(FOG_26, false, 4.0).unwrap();
        assert!(far.contains("return sad_orig_linear_fog_value(vertexDistance, fogStart * 4.00, fogEnd * 4.00);"));
    }

    /// The installed 26.3 client: its real format and shaders.
    #[test]
    fn real_26_client_pack_and_shaders() {
        let jar = crate::config::data_dir().join("game/versions/26.3/26.3.jar");
        if !jar.is_file() {
            return;
        }
        assert_eq!(client_pack_format(&jar), Some(97));
        let light = jar_text(&jar, LIGHT_SHADERS[1]).unwrap();
        assert!(patch_light_glsl(&light, 0.8).is_some());
        let fog = jar_text(&jar, "assets/minecraft/shaders/include/fog.glsl").unwrap();
        assert!(patch_fog_glsl(&fog, true, 1.0).is_some());
        let mut cfg = EffectsConfig::default();
        cfg.fullbright = true;
        cfg.custom_fog = true;
        cfg.fog_disable = true;
        let files = pack_files("26.3", &cfg, Some(&jar)).unwrap();
        let has = |n: &str| files.iter().any(|(f, _)| f == n);
        assert!(has(LIGHT_SHADERS[1]) && !has(LIGHT_SHADERS[0]));
        assert!(has("assets/minecraft/shaders/include/fog.glsl"));
        let meta = &files.iter().find(|(f, _)| f == "pack.mcmeta").unwrap().1;
        assert!(String::from_utf8_lossy(meta).contains("\"min_format\":97"));
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
        assert!(patched.contains("\"file/SadEffects\""));

        cfg.fullbright = false;
        cfg.no_hurt_cam = false;
        cfg.no_fire = false;
        let restored = patch_options_text(&patched, &cfg);
        assert!(restored.contains("gamma:1.0"));
        assert!(restored.contains("damageTiltStrength:1.0"));
        // The pack stays on (effects switch live inside it); the old zip goes.
        assert!(restored.contains("\"file/SadEffects\""));
        let legacy = patch_options_text("resourcePacks:[\"vanilla\",\"file/SadEffects.zip\"]\n", &cfg);
        assert!(!legacy.contains("SadEffects.zip"));
    }

    #[test]
    fn pack_holds_requested_textures_and_folder_follows_it() {
        let mut cfg = EffectsConfig::default();
        cfg.no_fire = true;
        cfg.no_overlay = true;
        cfg.no_weather = true;
        cfg.no_particles = true;
        let files = pack_files("1.20.1", &cfg, None).unwrap();
        let has = |n: &str| files.iter().any(|(f, _)| f == n);
        assert!(has("pack.mcmeta"));
        assert!(has("assets/minecraft/textures/block/fire_0.png"));
        assert!(has("assets/minecraft/textures/misc/pumpkinblur.png"));
        assert!(has("assets/minecraft/textures/environment/rain.png"));
        assert!(has("assets/minecraft/textures/particle/explosion_0.png"));

        let dir = std::env::temp_dir().join(format!("sad-fx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(write_pack_dir(&dir, &files).unwrap());
        assert!(!write_pack_dir(&dir, &files).unwrap(), "same files — nothing to reload");
        cfg.no_fire = false;
        let fewer = pack_files("1.20.1", &cfg, None).unwrap();
        assert!(write_pack_dir(&dir, &fewer).unwrap());
        assert!(!dir.join("assets/minecraft/textures/block/fire_0.png").exists(), "fire file removed live");
        let _ = std::fs::remove_dir_all(&dir);
    }

    const LIGHT_1_20: &str = "#version 150\n\nvec4 minecraft_mix_light(vec3 a, vec3 b, vec3 n, vec4 color) {\n    return color;\n}\n\nvec4 minecraft_sample_lightmap(sampler2D lightMap, ivec2 uv) {\n    return texture(lightMap, clamp(uv / 256.0, vec2(0.5 / 16.0), vec2(15.5 / 16.0)));\n}\n";
    const FOG_1_20: &str = "#version 150\n\nvec4 linear_fog(vec4 inColor, float vertexDistance, float fogStart, float fogEnd, vec4 fogColor) {\n    return inColor;\n}\n\nfloat linear_fog_fade(float vertexDistance, float fogStart, float fogEnd) {\n    return 1.0;\n}\n";

    #[test]
    fn light_shader_keeps_original_and_adds_wrapper() {
        let out = patch_light_glsl(LIGHT_1_20, 1.0).unwrap();
        assert!(out.contains("vec4 sad_orig_minecraft_sample_lightmap(sampler2D lightMap, ivec2 uv)"));
        assert!(out.contains("vec4 minecraft_sample_lightmap(sampler2D lightMap, ivec2 uv) {"));
        assert!(out.contains("return mix(sad_orig_minecraft_sample_lightmap(lightMap, uv), vec4(1.0), 1.000);"));
        assert!(out.contains("vec4 minecraft_mix_light("), "other functions untouched");
        assert!(patch_light_glsl("#version 150\n", 1.0).is_none(), "unknown shader → no patch");
    }

    #[test]
    fn fog_shader_off_or_farther() {
        let off = patch_fog_glsl(FOG_1_20, true, 1.0).unwrap();
        assert!(off.contains("vec4 linear_fog(vec4 inColor, float vertexDistance, float fogStart, float fogEnd, vec4 fogColor) {\n    return inColor;"));
        assert!(off.contains("float linear_fog_fade(float vertexDistance, float fogStart, float fogEnd) {\n    return 1.0;"));
        let far = patch_fog_glsl(FOG_1_20, false, 4.0).unwrap();
        assert!(far.contains("return sad_orig_linear_fog(inColor, vertexDistance, fogStart * 4.00, fogEnd * 4.00, fogColor);"));
    }

    /// The real 1.20.1 shaders from the installed client, when present.
    #[test]
    fn real_client_shaders_patch() {
        let jar = crate::config::data_dir().join("game/versions/1.20.1/1.20.1.jar");
        if !jar.is_file() {
            return;
        }
        let light = jar_text(&jar, "assets/minecraft/shaders/include/light.glsl").unwrap();
        assert!(patch_light_glsl(&light, 0.8).is_some());
        let fog = jar_text(&jar, "assets/minecraft/shaders/include/fog.glsl").unwrap();
        let out = patch_fog_glsl(&fog, true, 1.0).unwrap();
        assert!(out.contains("float fog_distance("), "rest of fog.glsl kept");
        assert_eq!(client_pack_format(&jar), Some(15));
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

    /// Live: a real Minecraft in a singleplayer world. Checks that the
    /// effects pack is on after start, that switching effects reloads the
    /// running game (window NOT focused, pause menu open — like when the
    /// player is in the launcher) without shader errors, and that the
    /// autoclicker clicks the game window: the real cursor parked on the
    /// window's × must not close the game.
    /// `cargo test --bin sad-launcher live_effects_in_game -- --ignored --nocapture`
    /// (`MADRID_LIVE_MC=26.3`, `MADRID_DEV_WORLD=SadTest` by default)
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn live_effects_in_game() {
        use std::time::{Duration, Instant};
        type H = *mut std::ffi::c_void;
        #[allow(clashing_extern_declarations)]
        #[link(name = "user32")]
        extern "system" {
            fn FindWindowW(class: *const u16, title: *const u16) -> H;
            fn SetForegroundWindow(h: H) -> i32;
            fn GetForegroundWindow() -> H;
            fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
            fn GetWindowRect(h: H, r: *mut [i32; 4]) -> i32;
            fn SetCursorPos(x: i32, y: i32) -> i32;
            fn GetCursorPos(p: *mut [i32; 2]) -> i32;
            fn PostMessageW(h: H, msg: u32, w: usize, l: isize) -> i32;
            fn MapVirtualKeyW(code: u32, kind: u32) -> u32;
            fn IsWindow(h: H) -> i32;
        }
        let post_key = |h: H, vk: u16| unsafe {
            let sc = (MapVirtualKeyW(vk as u32, 0) as isize) << 16;
            PostMessageW(h, 0x0100, vk as usize, 1 | sc);
            PostMessageW(h, 0x0101, vk as usize, 1 | sc | (3 << 30));
        };

        let mc = std::env::var("MADRID_LIVE_MC").unwrap_or_else(|_| "26.3".into());
        if std::env::var("MADRID_DEV_WORLD").is_err() {
            std::env::set_var("MADRID_DEV_WORLD", "SadTest");
        }
        let mut cfg = crate::config::load();
        cfg.active_profile = "default".into();
        let instance = cfg.instance_dir();
        let options_path = instance.join("options.txt");
        let saved_options = std::fs::read_to_string(&options_path).ok();
        let jar = client_jar(&cfg, &mc).expect("client jar of this version is installed");
        let shared = crate::app::new_shared(cfg.clone());
        let log = |n: usize| shared.game_log_tail(n);
        let shader_error = |l: &str| {
            let low = l.to_ascii_lowercase();
            low.contains("shadercompileexception")
                || ((low.contains("/error") || low.contains("/warn"))
                    && (low.contains("shader") || low.contains("glsl") || low.contains("pipeline") || low.contains("sadeffects")))
        };
        let reloads = |lines: &[String]| lines.iter().filter(|l| l.contains("Reloading ResourceManager")).count();

        crate::launch::launch(&shared, &cfg, &mc, None).expect("launch");
        let t = Instant::now();
        let mut in_world = false;
        while t.elapsed() < Duration::from_secs(240) && shared.is_game_running() {
            std::thread::sleep(Duration::from_millis(500));
            if log(600).iter().any(|l| l.contains("logged in with entity id")) {
                in_world = true;
                break;
            }
        }
        let finish = |shared: &std::sync::Arc<crate::shared::Shared>| {
            crate::launch::stop(shared);
            std::thread::sleep(Duration::from_secs(2));
            if let Some(o) = &saved_options {
                let _ = std::fs::write(&options_path, o);
            }
            let _ = apply_to_instance(&instance, &mc, &load(), Some(&jar));
        };
        let boot = log(600);
        if !in_world {
            println!("{}", boot.iter().rev().take(40).rev().cloned().collect::<Vec<_>>().join("\n"));
            finish(&shared);
            panic!("the game did not reach the world in {:?}", t.elapsed());
        }
        println!("в мире через {:?}", t.elapsed());
        let removed = boot.iter().any(|l| l.contains("Removed resource pack file/SadEffects"));
        let loaded = boot.iter().any(|l| l.contains("Reloading ResourceManager") && l.contains("file/SadEffects"));
        let boot_errors = boot.iter().filter(|l| shader_error(l)).count();
        println!("пак при старте: включён={loaded}, выкинут={removed}, ошибок шейдеров={boot_errors}");

        std::thread::sleep(Duration::from_secs(4));
        let pid = shared.game_pid();
        let hwnd_n = crate::automation::game_window(pid).expect("game window");
        let hwnd = hwnd_n as H;
        let captured = || crate::automation::game_cursor_captured(hwnd_n);
        let tray: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
        // The player goes to the launcher: the game loses focus (and opens
        // its pause menu ~0.5 s later).
        let leave_game = || {
            unsafe {
                let taskbar = FindWindowW(tray.as_ptr(), std::ptr::null());
                keybd_event(0x12, 0, 0, 0); // Alt lets SetForegroundWindow through
                SetForegroundWindow(taskbar);
                keybd_event(0x12, 0, 2, 0);
            }
            std::thread::sleep(Duration::from_secs(2));
            let fg = unsafe { GetForegroundWindow() };
            fg != hwnd
        };
        // The player comes back and closes the pause menu (Esc) — until the
        // game holds the cursor again. Never a click: the middle of the
        // pause menu is «Save and quit».
        let back_to_world = || {
            unsafe {
                keybd_event(0x12, 0, 0, 0);
                SetForegroundWindow(hwnd);
                keybd_event(0x12, 0, 2, 0);
            }
            std::thread::sleep(Duration::from_millis(800));
            for _ in 0..3 {
                if captured() {
                    break;
                }
                post_key(hwnd, 0x1B);
                std::thread::sleep(Duration::from_millis(800));
            }
            captured()
        };

        let mut ac = load();
        ac.autoclicker = false;
        ac.autoclicker_ms = 50;
        ac.autoclicker_rmb = false;
        ac.autoclicker_bind = String::new();
        ac.anti_afk = false;
        ac.anti_afk_bind = String::new();
        let handle = crate::automation::AutomationHandle::start(shared.clone(), egui::Context::default(), &ac);

        let mut steps = Vec::new();
        let mut c = EffectsConfig::default();
        c.fullbright = true;
        c.fullbright_gamma = 15.0;
        steps.push(("яркость", c.clone()));
        c.custom_fog = true;
        c.fog_disable = true;
        steps.push(("яркость + без тумана", c.clone()));
        c.fullbright = false;
        c.custom_fog = false;
        c.no_fire = true;
        steps.push(("всё выкл, низкий огонь", c.clone()));
        let mut results = Vec::new();
        for (name, step) in &steps {
            let unfocused = leave_game();
            let before = log(600);
            let n0 = reloads(&before);
            let rep = apply_to_instance(&instance, &mc, step, Some(&jar)).expect("apply");
            handle.request_reload(&instance);
            std::thread::sleep(Duration::from_secs(2));
            let waited = reloads(&log(600)) == n0 && handle.snapshot().reload_pending;
            let back = back_to_world();
            let t = Instant::now();
            let mut ok = false;
            while t.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(300));
                let now = log(600);
                if reloads(&now) > n0 {
                    ok = now.iter().rev().any(|l| l.contains("Reloading ResourceManager") && l.contains("file/SadEffects"));
                    // Let shaders compile before looking for errors.
                    std::thread::sleep(Duration::from_secs(3));
                    break;
                }
            }
            let now = log(600);
            let errors: Vec<&String> = now.iter().filter(|l| !before.contains(l) && shader_error(l)).collect();
            println!(
                "{name}: пак записан={}, игра не в фокусе={unfocused}, F3+T ждал возвращения={waited}, вернулся в мир={back}, перезагрузка={ok}, ошибок шейдеров={}",
                rep.pack_written,
                errors.len()
            );
            for e in &errors {
                println!("    {e}");
            }
            results.push((unfocused && waited && ok && errors.is_empty(), name.to_string()));
        }

        // Autoclicker in the world (captured cursor → real clicks, inside
        // the game window only).
        let in_world = back_to_world();
        ac.autoclicker = true;
        handle.update_from_config(&ac);
        std::thread::sleep(Duration::from_secs(3));
        let snap_world = handle.snapshot();
        // Inventory open → the cursor is free; park the REAL cursor on the
        // window's ×: clicks must go to the game, not close it.
        post_key(hwnd, 0x45); // E
        std::thread::sleep(Duration::from_millis(800));
        let free = !captured();
        let mut cur = [0i32; 2];
        let mut r = [0i32; 4];
        unsafe {
            GetCursorPos(&mut cur);
            GetWindowRect(hwnd, &mut r);
            SetCursorPos(r[2] - 22, r[1] + 12);
        }
        std::thread::sleep(Duration::from_millis(2500));
        let snap_inv = handle.snapshot();
        ac.autoclicker = false;
        handle.update_from_config(&ac);
        unsafe { SetCursorPos(cur[0], cur[1]) };
        std::thread::sleep(Duration::from_millis(300));
        post_key(hwnd, 0x1B);
        std::thread::sleep(Duration::from_secs(1));
        drop(handle);
        let alive = shared.is_game_running() && unsafe { IsWindow(hwnd) } != 0;
        println!(
            "автокликер: в мире (курсор захвачен={in_world}) кликов={}, в инвентаре (курсор свободен={free}) с курсором на × кликов={}, игра жива={alive}",
            snap_world.click_count,
            snap_inv.click_count - snap_world.click_count
        );

        finish(&shared);
        assert!(loaded && !removed, "пак эффектов должен быть включён при старте");
        assert_eq!(boot_errors, 0, "шейдеры пака не собрались при старте");
        for (ok, name) in &results {
            assert!(ok, "эффект «{name}» не применился в запущенной игре");
        }
        assert!(in_world && snap_world.click_count >= 30, "автокликер почти не кликал в мире");
        assert!(free && snap_inv.click_count > snap_world.click_count + 20, "автокликер не кликал в инвентаре");
        assert!(alive, "игра закрылась во время автокликера");
    }

    /// Live helper: starts the game (as «Играть» does) and keeps it until
    /// it is closed, for checks by hand. `MADRID_DEV_WORLD` opens a world.
    /// `cargo test --bin sad-launcher live_game_hold -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_game_hold() {
        let mc = std::env::var("MADRID_LIVE_MC").unwrap_or_else(|_| "26.3".into());
        let mut cfg = crate::config::load();
        cfg.active_profile = "default".into();
        let shared = crate::app::new_shared(cfg.clone());
        crate::launch::launch(&shared, &cfg, &mc, None).expect("launch");
        let t = std::time::Instant::now();
        while shared.is_game_running() && t.elapsed() < std::time::Duration::from_secs(1200) {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        crate::launch::stop(&shared);
    }
}
