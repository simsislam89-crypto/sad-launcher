//! Content catalog: Modrinth (no key) and CurseForge (public mirror or own
//! key) search, installs per content kind, modpacks, and the user's build list.

use crate::config::Config;
use crate::loader::{Loader, Spec};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

/// CurseForge host: the official API needs a key; the public mirror
/// (api.curse.tools) serves the same v1 schema without one.
/// The mirror moved from `/v1/cf/…` to `/v1/…` (the old path now
/// redirects to plain http) — talk to the new path directly over https.
fn cf_base(cfg: &Config) -> &'static str {
    if cfg.cf_api_key.trim().is_empty() {
        "https://api.curse.tools/v1"
    } else {
        "https://api.curseforge.com/v1"
    }
}

/// Text as the UI can draw it: the bundled fonts have Latin, Cyrillic and
/// emoji but no CJK/Hangul — those runs would show as empty boxes, so they
/// are dropped (loading a CJK font would cost ~20 MB of RAM).
pub fn displayable(text: &str) -> String {
    let kept: String = text
        .chars()
        .map(|c| {
            let cp = c as u32;
            let cjk = (0x2E80..=0x9FFF).contains(&cp)
                || (0xAC00..=0xD7AF).contains(&cp)
                || (0xF900..=0xFAFF).contains(&cp)
                || (0xFF00..=0xFFEF).contains(&cp)
                || (0x20000..=0x2FFFF).contains(&cp);
            if cjk { ' ' } else { c }
        })
        .collect();
    let mut out = String::with_capacity(kept.len());
    for word in kept.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    out.trim_matches(|c: char| c == ':' || c == '-' || c == '|' || c.is_whitespace()).to_string()
}

/// Public CDN redirect for a CurseForge file — works without any key.
fn cf_public_url(project: &str, file: u64) -> String {
    format!("https://www.curseforge.com/api/v1/mods/{project}/files/{file}/download")
}

/// Best link for a CurseForge file. The API's direct `edge.forgecdn.net`
/// link answers 404 when the file name has spaces (`RLCraft%201.12.2…`),
/// so those go through the public redirect, which always resolves.
fn cf_file_url(project: &str, file: u64, direct: Option<String>) -> String {
    match direct {
        Some(u) if !u.is_empty() && !u.contains("%20") && !u.contains(' ') => u,
        _ => cf_public_url(project, file),
    }
}

pub const KIND_MOD: &str = "mod";
pub const KIND_SHADER: &str = "shader";
pub const KIND_RESOURCEPACK: &str = "resourcepack";
pub const KIND_MAP: &str = "map";
pub const KIND_MODPACK: &str = "modpack";

/// Content kinds shown as chips; values feed search facets and dest dirs.
pub const KINDS: [(&str, &str); 5] = [
    (KIND_MOD, "Моды"),
    (KIND_SHADER, "Шейдеры"),
    (KIND_RESOURCEPACK, "Ресурспаки"),
    (KIND_MAP, "Карты"),
    (KIND_MODPACK, "Модпаки"),
];

/// Results per page for each source — also the «no more pages» threshold.
pub const MODRINTH_PAGE: usize = 40;
pub const CF_PAGE: usize = 40;

pub fn page_size(source: &str) -> usize {
    if source == "curseforge" {
        CF_PAGE
    } else {
        MODRINTH_PAGE
    }
}

/// Which source actually serves a kind: Modrinth has no maps.
pub fn effective_source(kind: &str, wanted: &str) -> &'static str {
    match (kind, wanted) {
        (KIND_MAP, _) => "curseforge",
        (_, "curseforge") => "curseforge",
        _ => "modrinth",
    }
}

#[derive(Clone, Serialize)]
pub struct InstalledMod {
    pub name: String,
    pub file: String,
    pub size: u64,
    pub enabled: bool,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ModHit {
    pub source: String,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub downloads: u64,
    pub author: String,
    pub kind: String,
    pub icon_url: Option<String>,
    pub file_id: Option<u64>,
    pub file_url: Option<String>,
    pub file_name: Option<String>,
    /// «1.20.1 · Forge» — newest target, shown on the card.
    #[serde(default)]
    pub tags: String,
    /// Localised categories («RPG и приключения», «Магия»), at most three.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Last update of the project (ISO 8601), for «обновлено 3 дн. назад».
    #[serde(default)]
    pub updated: String,
}

/// One item of the user's own build (the плюсик list).
#[derive(Clone, Serialize, Deserialize)]
pub struct BuildEntry {
    pub source: String,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub kind: String,
    pub mc: String,
    pub loader: String,
}

fn build_path() -> PathBuf {
    crate::config::data_dir().join("build.json")
}

pub fn load_build() -> Vec<BuildEntry> {
    std::fs::read_to_string(build_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_build(entries: &[BuildEntry]) {
    if let Ok(text) = serde_json::to_string_pretty(entries) {
        let _ = std::fs::write(build_path(), text);
    }
}

#[derive(Clone)]
pub enum ModEvent {
    Progress { done: u64, total: u64 },
    /// Bytes of the files being fetched (the bar; `Progress` counts files).
    Bytes { done: u64, total: u64 },
    /// A concrete file started/completed — shown live in the UI.
    File(String),
    /// The pack's own build was created: switch to it, fetch its client.
    PackMeta { profile_id: String, spec: Spec },
    /// Finished with something the user should know (skipped files).
    Warn(String),
    Done(PathBuf),
    Error(String),
}

pub fn dest_dir(cfg: &Config, kind: &str) -> PathBuf {
    let root = cfg.instance_dir();
    match kind {
        KIND_SHADER => root.join("shaderpacks"),
        KIND_RESOURCEPACK => root.join("resourcepacks"),
        KIND_MAP => root.join("saves"),
        _ => root.join("mods"),
    }
}

pub fn installed(cfg: &Config) -> Vec<InstalledMod> {
    let dir = cfg.mods_dir();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        // On Windows the listing already carries type and size: no extra
        // stat per jar (was two: `is_file` + `metadata`).
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let Ok(file) = entry.file_name().into_string() else { continue };
        let enabled = file.ends_with(".jar");
        if !enabled && !file.ends_with(".jar.disabled") {
            continue;
        }
        let name = file
            .trim_end_matches(".disabled")
            .trim_end_matches(".jar")
            .to_string();
        out.push(InstalledMod {
            name,
            file,
            size: meta.len(),
            enabled,
        });
    }
    out.sort_by_cached_key(|m| m.name.to_lowercase());
    out
}

pub fn set_enabled(cfg: &Config, file: &str, enabled: bool) -> Result<()> {
    let dir = cfg.mods_dir();
    let current = dir.join(file);
    if !current.exists() {
        bail!("Файл не найден");
    }
    let target = if enabled {
        dir.join(file.trim_end_matches(".disabled"))
    } else {
        dir.join(format!("{file}.disabled"))
    };
    if current != target {
        std::fs::rename(&current, &target)?;
    }
    Ok(())
}

pub fn delete(cfg: &Config, file: &str) -> Result<()> {
    let path = cfg.mods_dir().join(file);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

// ---------------------------------------------------------------- search

/// CurseForge sortField codes: 2 popularity, 3 last updated, 6 total
/// downloads, 11 release date.
pub fn cf_sort_field(sort: &str) -> u32 {
    match sort {
        "follows" | "popularity" => 2,
        "updated" => 3,
        "newest" => 11,
        _ => 6,
    }
}

/// CurseForge modLoaderType codes.
pub fn cf_loader_type(loader: Loader) -> Option<u32> {
    match loader {
        Loader::Forge => Some(1),
        Loader::Fabric => Some(4),
        Loader::Quilt => Some(5),
        Loader::NeoForge => Some(6),
        Loader::Vanilla => None,
    }
}

fn cf_loader_name(code: Option<u32>) -> &'static str {
    match code {
        Some(1) => "Forge",
        Some(4) => "Fabric",
        Some(5) => "Quilt",
        Some(6) => "NeoForge",
        _ => "",
    }
}

/// Modrinth `loaders` filter for mod files of a build.
fn modrinth_loaders(spec: &Spec) -> Option<String> {
    let list: &[&str] = match spec.loader {
        Loader::Forge => &["forge"],
        Loader::NeoForge if spec.mc == "1.20.1" => &["neoforge", "forge"],
        Loader::NeoForge => &["neoforge"],
        Loader::Fabric => &["fabric"],
        Loader::Quilt => &["quilt", "fabric"],
        Loader::Vanilla => return None,
    };
    let quoted: Vec<String> = list.iter().map(|l| format!("\"{l}\"")).collect();
    Some(format!("[{}]", quoted.join(",")))
}

#[allow(clippy::too_many_arguments)]
pub async fn search(
    client: &reqwest::Client,
    cfg: &Config,
    query: &str,
    source: &str,
    spec: &Spec,
    kind: &str,
    sort: &str,
    category: &str,
    offset: usize,
) -> Result<Vec<ModHit>> {
    let q = query.trim();
    // Later pages and empty queries are one plain request.
    if q.is_empty() || offset > 0 {
        let primary = crate::smart::query_plan(q).into_iter().next().unwrap_or_default();
        return search_once(client, cfg, &primary, source, spec, kind, sort, category, offset).await;
    }
    // Smart first page: Latin forms + typo prefix in parallel, merged and
    // re-ranked so «проминсе» or «promince» still put Prominence on top.
    let plan = crate::smart::query_plan(q);
    let requests = plan
        .iter()
        .map(|text| search_once(client, cfg, text, source, spec, kind, sort, category, 0));
    let answers = futures_util::future::join_all(requests).await;
    let mut merged: Vec<ModHit> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut first_err = None;
    let mut primary_len = 0usize;
    for (i, answer) in answers.into_iter().enumerate() {
        match answer {
            Ok(list) => {
                if i == 0 {
                    primary_len = list.len();
                }
                for hit in list {
                    if seen.insert(hit.id.clone()) {
                        merged.push(hit);
                    }
                }
            }
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    if merged.is_empty() {
        if let Some(e) = first_err {
            return Err(e);
        }
    }
    let mut ranked = crate::smart::rerank(merged, q, |h| (h.title.clone(), h.slug.clone()));
    // A full first page keeps «load more» aligned with the primary request.
    let page = page_size(source);
    if primary_len >= page && ranked.len() > page + page / 2 {
        ranked.truncate(page + page / 2);
    }
    Ok(ranked)
}

#[allow(clippy::too_many_arguments)]
async fn search_once(
    client: &reqwest::Client,
    cfg: &Config,
    query: &str,
    source: &str,
    spec: &Spec,
    kind: &str,
    sort: &str,
    category: &str,
    offset: usize,
) -> Result<Vec<ModHit>> {
    if source == "curseforge" {
        curse_search(client, cfg, query, spec, kind, sort, category, offset).await
    } else {
        // With words typed, relevance answers better than raw downloads.
        let sort = if !query.trim().is_empty() && sort == "downloads" { "relevance" } else { sort };
        modrinth_search(client, query, spec, kind, sort, category, offset).await
    }
}

/// Search URL for CurseForge; `index` is an item offset, not a page.
pub fn cf_search_url(base: &str, query: &str, spec: &Spec, kind: &str, sort: &str, category: &str, offset: usize) -> String {
    // classId: 6 mods, 12 resource packs, 17 worlds, 4471 modpacks, 6552 shaders.
    let class_id = match kind {
        KIND_MAP => 17,
        KIND_RESOURCEPACK => 12,
        KIND_MODPACK => 4471,
        KIND_SHADER => 6552,
        _ => 6,
    };
    let mut url = format!(
        "{base}/mods/search?gameId=432&classId={class_id}&sortField={}&sortOrder=desc&pageSize={CF_PAGE}&index={offset}",
        cf_sort_field(sort),
    );
    // A modpack brings its own Minecraft — show every version of packs.
    if kind != KIND_MODPACK && !spec.mc.is_empty() {
        url.push_str(&format!("&gameVersion={}", enc(&spec.mc)));
    }
    if kind == KIND_MOD {
        if let Some(t) = cf_loader_type(spec.loader) {
            url.push_str(&format!("&modLoaderType={t}"));
        }
    }
    if let Ok(id) = category.parse::<u64>() {
        url.push_str(&format!("&categoryId={id}"));
    }
    if !query.trim().is_empty() {
        url.push_str(&format!("&searchFilter={}", enc(query.trim())));
    }
    url
}

#[allow(clippy::too_many_arguments)]
async fn curse_search(
    client: &reqwest::Client,
    cfg: &Config,
    query: &str,
    spec: &Spec,
    kind: &str,
    sort: &str,
    category: &str,
    offset: usize,
) -> Result<Vec<ModHit>> {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<Mod>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mod {
        id: u64,
        name: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        download_count: f64,
        #[serde(default)]
        authors: Vec<Author>,
        #[serde(default)]
        logo: Option<Logo>,
        #[serde(default)]
        main_file_id: Option<u64>,
        #[serde(default)]
        latest_files_indexes: Vec<FileIndex>,
        #[serde(default)]
        categories: Vec<Cat>,
        /// Newest file release — what «обновлено» means for players.
        #[serde(default)]
        date_released: String,
        #[serde(default)]
        date_modified: String,
    }
    #[derive(Deserialize)]
    struct Cat {
        #[serde(default)]
        id: u64,
    }
    #[derive(Deserialize)]
    struct Author {
        name: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Logo {
        #[serde(default)]
        thumbnail_url: String,
        #[serde(default)]
        url: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FileIndex {
        #[serde(default)]
        game_version: String,
        #[serde(default)]
        mod_loader: Option<u32>,
    }

    let url = cf_search_url(cf_base(cfg), query, spec, kind, sort, category, offset);
    let resp: Resp = client
        .get(url)
        .header("x-api-key", cfg.cf_api_key.trim())
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(25))
        .send()
        .await
        .context("CurseForge не отвечает")?
        .error_for_status()
        .context("CurseForge не ответил — проверь соединение")?
        .json()
        .await
        .context("CurseForge вернул не JSON")?;

    Ok(resp
        .data
        .into_iter()
        .map(|m| {
            // Thumbnails are ~256px — plenty for a 126px tile, 10× lighter.
            let icon = m
                .logo
                .map(|l| if l.thumbnail_url.is_empty() { l.url } else { l.thumbnail_url });
            let tags = m
                .latest_files_indexes
                .iter()
                .find(|f| f.game_version == spec.mc && cf_loader_name(f.mod_loader) == spec.loader.label())
                .or_else(|| m.latest_files_indexes.first())
                .map(|f| {
                    let loader = cf_loader_name(f.mod_loader);
                    if loader.is_empty() {
                        f.game_version.clone()
                    } else {
                        format!("{} · {loader}", f.game_version)
                    }
                })
                .unwrap_or_default();
            ModHit {
                source: "curseforge".into(),
                id: m.id.to_string(),
                slug: m.id.to_string(),
                title: {
                    let t = displayable(&m.name);
                    if t.is_empty() { m.name.clone() } else { t }
                },
                summary: displayable(&m.summary),
                downloads: m.download_count.max(0.0) as u64,
                author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
                kind: kind.into(),
                icon_url: icon.filter(|s| !s.is_empty()),
                file_id: m.main_file_id,
                file_url: None,
                file_name: None,
                tags,
                categories: crate::categories::labels("curseforge", kind, m.categories.iter().map(|c| c.id.to_string())),
                updated: if m.date_released.is_empty() { m.date_modified } else { m.date_released },
            }
        })
        .collect())
}

/// Modrinth facets: loader categories only for mods; versions for all but packs.
pub fn modrinth_facets(spec: &Spec, kind: &str, category: &str) -> String {
    let project_type = match kind {
        KIND_SHADER => "shader",
        KIND_RESOURCEPACK => "resourcepack",
        KIND_MODPACK => "modpack",
        _ => "mod",
    };
    let mut groups = vec![format!(r#"["project_type:{project_type}"]"#)];
    if kind != KIND_MODPACK && !spec.mc.is_empty() {
        groups.push(format!(r#"["versions:{}"]"#, spec.mc));
    }
    if kind == KIND_MOD && spec.loader != Loader::Vanilla {
        let mut cats = vec![format!(r#""categories:{}""#, spec.loader.id())];
        if spec.loader == Loader::Quilt {
            cats.push(r#""categories:fabric""#.into());
        }
        if spec.loader == Loader::NeoForge && spec.mc == "1.20.1" {
            cats.push(r#""categories:forge""#.into());
        }
        groups.push(format!("[{}]", cats.join(",")));
    }
    if !category.is_empty() && !category.chars().all(|c| c.is_ascii_digit()) {
        groups.push(format!(r#"["categories:{category}"]"#));
    }
    format!("[{}]", groups.join(","))
}

async fn modrinth_search(
    client: &reqwest::Client,
    query: &str,
    spec: &Spec,
    kind: &str,
    sort: &str,
    category: &str,
    offset: usize,
) -> Result<Vec<ModHit>> {
    #[derive(Deserialize)]
    struct Resp {
        hits: Vec<Hit>,
    }
    #[derive(Deserialize)]
    struct Hit {
        project_id: String,
        slug: String,
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        downloads: u64,
        #[serde(default)]
        author: String,
        #[serde(default)]
        icon_url: Option<String>,
        #[serde(default)]
        versions: Vec<String>,
        #[serde(default)]
        categories: Vec<String>,
        #[serde(default)]
        date_modified: String,
    }

    let sort_q = match sort {
        "follows" | "popularity" => "follows",
        "updated" => "updated",
        "newest" => "newest",
        "relevance" => "relevance",
        _ => "downloads",
    };
    let url = format!(
        "https://api.modrinth.com/v2/search?limit={MODRINTH_PAGE}&offset={}&index={}&query={}&facets={}",
        offset,
        sort_q,
        enc(query.trim()),
        enc(&modrinth_facets(spec, kind, category))
    );
    let resp: Resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(25))
        .send()
        .await
        .context("Modrinth не отвечает")?
        .error_for_status()?
        .json()
        .await
        .context("Modrinth вернул не JSON")?;

    Ok(resp
        .hits
        .into_iter()
        .map(|h| {
            // The badge names what fits the active build when the project
            // supports it (results are filtered by it): «1.20.1 · Forge»,
            // not the project's newest «26.3 · Fabric».
            let loaders: Vec<Loader> = h
                .categories
                .iter()
                .map(|c| Loader::parse(c))
                .filter(|l| *l != Loader::Vanilla)
                .collect();
            let loader = if loaders.contains(&spec.loader) {
                Some(spec.loader)
            } else {
                loaders.first().copied()
            };
            let newest = if h.versions.iter().any(|v| *v == spec.mc) {
                spec.mc.clone()
            } else {
                h.versions
                    .iter()
                    .rev()
                    .find(|v| v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) && !v.contains('w'))
                    .cloned()
                    .unwrap_or_default()
            };
            let tags = match loader {
                Some(l) if !newest.is_empty() => format!("{newest} · {}", l.label()),
                Some(l) => l.label().to_string(),
                None => newest,
            };
            ModHit {
                source: "modrinth".into(),
                id: h.project_id,
                title: {
                    let t = displayable(&h.title);
                    if t.is_empty() { h.slug.clone() } else { t }
                },
                slug: h.slug,
                summary: displayable(&h.description),
                downloads: h.downloads,
                author: h.author,
                kind: kind.into(),
                icon_url: h.icon_url.filter(|s| !s.is_empty()),
                file_id: None,
                file_url: None,
                file_name: None,
                tags,
                categories: crate::categories::labels("modrinth", kind, h.categories.iter().cloned()),
                updated: h.date_modified,
            }
        })
        .collect())
}

// ---------------------------------------------------------------- single files

#[allow(clippy::too_many_arguments)]
pub async fn install<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    spec: &Spec,
    kind: &str,
    cancel: &Arc<AtomicBool>,
    on: &mut F,
) -> Result<PathBuf> {
    if kind == KIND_MODPACK {
        return install_modpack(client, cfg, hit, cancel, on).await;
    }
    let (url, filename) = if hit.source == "curseforge" {
        curse_file(client, cfg, hit, spec, kind).await?
    } else {
        modrinth_file(client, hit, spec, kind).await?
    };
    let safe = sanitize(&filename);
    if safe.is_empty() {
        bail!("Не удалось определить имя файла");
    }
    let dir = dest_dir(cfg, kind);
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(&safe);

    {
        let mut report = |done: u64, total: u64| on(ModEvent::Progress { done, total });
        if dest.exists() {
            let _ = std::fs::remove_file(&dest);
        }
        crate::minecraft::download_file_progress(client, &url, &dest, Some(cancel.as_ref()), &mut report)
            .await
            .context("Файл недоступен")?;
    }

    // Maps arrive zipped: unpack them straight into saves/.
    if kind == KIND_MAP && safe.to_lowercase().ends_with(".zip") {
        let target = dir.join(safe.trim_end_matches(".zip"));
        if unzip(&dest, &target).is_ok() {
            let _ = std::fs::remove_file(&dest);
            on(ModEvent::Done(target.clone()));
            return Ok(target);
        }
    }
    on(ModEvent::Done(dest.clone()));
    Ok(dest)
}

async fn curse_file(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    spec: &Spec,
    kind: &str,
) -> Result<(String, String)> {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<File>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct File {
        id: u64,
        #[serde(default)]
        file_name: String,
        #[serde(default)]
        download_url: Option<String>,
    }
    // Newest file that fits this build; the project's main file otherwise.
    let mut url = format!("{}/mods/{}/files?pageSize=1", cf_base(cfg), hit.id);
    if kind != KIND_MODPACK && !spec.mc.is_empty() {
        url.push_str(&format!("&gameVersion={}", enc(&spec.mc)));
    }
    if kind == KIND_MOD {
        if let Some(t) = cf_loader_type(spec.loader) {
            url.push_str(&format!("&modLoaderType={t}"));
        }
    }
    let fitting = match client
        .get(url)
        .header("x-api-key", cfg.cf_api_key.trim())
        .send()
        .await
    {
        Ok(r) => r.json::<Resp>().await.ok().and_then(|r| r.data.into_iter().next()),
        Err(_) => None,
    };
    let file = match fitting {
        Some(f) => f,
        None if kind == KIND_MODPACK || kind == KIND_MAP => match hit.file_id {
            Some(fid) => File {
                id: fid,
                file_name: hit.file_name.clone().unwrap_or_default(),
                download_url: None,
            },
            None => bail!("У «{}» нет файлов", hit.title),
        },
        None => bail!("У «{}» нет файла для {}", hit.title, spec.title()),
    };
    let name = if file.file_name.is_empty() {
        format!("{}-{}.jar", hit.slug, file.id)
    } else {
        file.file_name
    };
    let url = cf_file_url(&hit.id, file.id, file.download_url);
    Ok((url, name))
}

async fn modrinth_file(
    client: &reqwest::Client,
    hit: &ModHit,
    spec: &Spec,
    kind: &str,
) -> Result<(String, String)> {
    #[derive(Deserialize)]
    struct Version {
        #[serde(default)]
        files: Vec<File>,
    }
    #[derive(Deserialize)]
    struct File {
        url: String,
        filename: String,
        #[serde(default)]
        primary: bool,
    }
    let mut query: Vec<String> = Vec::new();
    if kind != KIND_MODPACK && !spec.mc.is_empty() {
        query.push(format!("game_versions={}", enc(&format!(r#"["{}"]"#, spec.mc))));
    }
    if kind == KIND_MOD {
        if let Some(l) = modrinth_loaders(spec) {
            query.push(format!("loaders={}", enc(&l)));
        }
    }
    // Changelogs make the version list megabytes long for big packs.
    query.push("include_changelog=false".into());
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?{}",
        enc(&hit.slug),
        query.join("&")
    );
    let list: Vec<Version> = client
        .get(url)
        .send()
        .await
        .context("Modrinth не отвечает")?
        .error_for_status()?
        .json()
        .await
        .context("Список версий не читается")?;
    let file = list
        .into_iter()
        .find_map(|v| {
            let mut files = v.files;
            if files.is_empty() {
                return None;
            }
            let i = files.iter().position(|f| f.primary).unwrap_or(0);
            Some(files.swap_remove(i))
        })
        .ok_or_else(|| anyhow!("У «{}» нет файла для {}", hit.title, spec.title()))?;
    Ok((file.url, file.filename))
}

// ---------------------------------------------------------------- modpacks

/// Where one pack file comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackSource {
    /// Modrinth: mirrors in order, relative target path inside the build,
    /// size and SHA-1 from the index (0 / empty when the pack omits them).
    Direct { urls: Vec<String>, path: String, size: u64, sha1: String },
    /// CurseForge: project + file; the name is learnt from the batch file
    /// list (when the API answers) or from the CDN redirect.
    Curse { project: u64, file: u64 },
}

/// A parsed pack: what to run and what to fetch.
#[derive(Clone, Debug)]
pub struct PackPlan {
    pub spec: Spec,
    pub files: Vec<PackSource>,
    /// Folder names inside the archive copied over the build as-is.
    pub overrides: Vec<String>,
}

/// Reads `modrinth.index.json`. Server-only files are skipped.
pub fn mrpack_plan(text: &str) -> Result<PackPlan> {
    #[derive(Deserialize)]
    struct Index {
        #[serde(default)]
        files: Vec<IndexFile>,
        #[serde(default)]
        dependencies: HashMap<String, String>,
    }
    #[derive(Deserialize)]
    struct IndexFile {
        path: String,
        #[serde(default)]
        downloads: Vec<String>,
        #[serde(default)]
        env: Option<Env>,
        #[serde(default, rename = "fileSize")]
        file_size: u64,
        #[serde(default)]
        hashes: HashMap<String, String>,
    }
    #[derive(Deserialize)]
    struct Env {
        #[serde(default)]
        client: String,
    }
    let index: Index = serde_json::from_str(text).context("Индекс пака не читается")?;
    let spec = crate::loader::mrpack_spec(&index.dependencies);
    if spec.mc.is_empty() {
        bail!("В паке не указана версия Minecraft");
    }
    let files = index
        .files
        .into_iter()
        .filter(|f| f.env.as_ref().map(|e| e.client != "unsupported").unwrap_or(true))
        .filter(|f| !f.downloads.is_empty())
        .filter_map(|f| {
            safe_rel(&f.path).map(|path| PackSource::Direct {
                urls: f.downloads,
                path,
                size: f.file_size,
                sha1: f.hashes.get("sha1").cloned().unwrap_or_default().to_lowercase(),
            })
        })
        .collect();
    Ok(PackPlan {
        spec,
        files,
        overrides: vec!["overrides".into(), "client-overrides".into()],
    })
}

/// Reads a CurseForge `manifest.json` (minecraft.version + modLoaders).
pub fn cf_plan(text: &str) -> Result<PackPlan> {
    #[derive(Deserialize)]
    struct Manifest {
        #[serde(default)]
        minecraft: Mc,
        #[serde(default)]
        files: Vec<CfFile>,
        #[serde(default)]
        overrides: Option<String>,
    }
    #[derive(Deserialize, Default)]
    struct Mc {
        #[serde(default)]
        version: String,
        #[serde(default, rename = "modLoaders")]
        mod_loaders: Vec<ModLoader>,
    }
    #[derive(Deserialize)]
    struct ModLoader {
        id: String,
        #[serde(default)]
        primary: bool,
    }
    #[derive(Deserialize)]
    struct CfFile {
        #[serde(rename = "projectID")]
        project_id: u64,
        #[serde(rename = "fileID")]
        file_id: u64,
        #[serde(default = "yes")]
        required: bool,
    }
    fn yes() -> bool {
        true
    }
    let m: Manifest = serde_json::from_str(text).context("Манифест пака не читается")?;
    if m.minecraft.version.is_empty() {
        bail!("В паке не указана версия Minecraft");
    }
    let loaders: Vec<(String, bool)> = m
        .minecraft
        .mod_loaders
        .iter()
        .map(|l| (l.id.clone(), l.primary))
        .collect();
    let spec = crate::loader::cf_spec(&m.minecraft.version, &loaders);
    let files = m
        .files
        .into_iter()
        .filter(|f| f.required)
        .map(|f| PackSource::Curse {
            project: f.project_id,
            file: f.file_id,
        })
        .collect();
    Ok(PackPlan {
        spec,
        files,
        overrides: vec![m.overrides.unwrap_or_else(|| "overrides".into())],
    })
}

/// A pack path is only accepted when it stays inside the build folder.
pub fn safe_rel(path: &str) -> Option<String> {
    let norm = path.replace('\\', "/");
    if norm.is_empty() || norm.starts_with('/') || norm.contains(':') {
        return None;
    }
    if norm.split('/').any(|seg| seg == ".." || seg.is_empty()) {
        return None;
    }
    Some(norm)
}

/// A modpack is its own build: own folder, own Minecraft + loader.
/// The client for it starts downloading the moment the manifest is read.
async fn install_modpack<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    cancel: &Arc<AtomicBool>,
    on: &mut F,
) -> Result<PathBuf> {
    on(ModEvent::Progress { done: 0, total: 0 });
    on(ModEvent::File("архив пака".into()));
    let pack = latest_pack_file(client, cfg, hit).await?;
    let (url, filename) = (pack.url.clone(), pack.filename.clone());
    let work = crate::config::data_dir().join("meta").join("packs");
    std::fs::create_dir_all(&work)?;
    let tag = sanitize(&format!("{}-{}", hit.source, hit.id));
    let archive = work.join(format!("{tag}.pack"));
    let unpacked = work.join(format!("{tag}-unpacked"));
    let _ = std::fs::remove_file(&archive);
    {
        let mut report = |done: u64, total: u64| on(ModEvent::Progress { done, total });
        crate::minecraft::download_file_progress(client, &url, &archive, Some(cancel.as_ref()), &mut report)
            .await
            .with_context(|| format!("Архив пака {filename} не скачался"))?;
    }
    // Leftover of older versions that unpacked the whole archive first.
    let _ = std::fs::remove_dir_all(&unpacked);
    let result = async {
        // Only the index is read; nothing is unpacked to a temp folder.
        let plan = {
            let archive = archive.clone();
            tokio::task::spawn_blocking(move || -> Result<PackPlan> {
                if let Some(text) = crate::unpack::read_text(&archive, "modrinth.index.json") {
                    mrpack_plan(&text)
                } else if let Some(text) = crate::unpack::read_text(&archive, "manifest.json") {
                    cf_plan(&text)
                } else {
                    bail!("Это не модпак: внутри нет modrinth.index.json или manifest.json");
                }
            })
            .await??
        };
        let source = format!("{}:{}", hit.source, hit.id);
        let profile = crate::profiles::upsert_modpack_profile(cfg, &hit.title, &source, &plan.spec, &pack.stamp())?;
        on(ModEvent::PackMeta {
            profile_id: profile.id.clone(),
            spec: plan.spec.clone(),
        });
        let root = cfg.instance_dir_for(&profile.id);
        std::fs::create_dir_all(root.join("mods"))?;
        // Overrides (configs, scripts, bundled jars) are written straight
        // into the build on worker threads while the mods download.
        let jobs = {
            let (archive, root, prefixes) = (archive.clone(), root.clone(), plan.overrides.clone());
            tokio::task::spawn_blocking(move || crate::unpack::prefixed_jobs(&archive, &prefixes, &root))
                .await??
        };
        let overridden: std::collections::HashSet<PathBuf> = jobs.iter().map(|j| j.target.clone()).collect();
        let files: Vec<PackSource> = plan
            .files
            .into_iter()
            .filter(|f| match f {
                PackSource::Direct { path, .. } => !overridden.contains(&root.join(path)),
                PackSource::Curse { .. } => true,
            })
            .collect();
        let override_count = jobs.len();
        let extracting = {
            let (archive, cancel) = (archive.clone(), cancel.clone());
            tokio::task::spawn_blocking(move || {
                crate::unpack::extract(&archive, jobs, Some(cancel.as_ref()), &std::sync::atomic::AtomicU64::new(0))
            })
        };
        let fetched = fetch_pack_files(client, cfg, &root, files, cancel, on).await;
        let extracted = extracting.await?;
        let mut failed = fetched?;
        extracted.context("Файлы настроек пака не распаковались")?;
        if override_count > 0 {
            on(ModEvent::File(format!("настройки пака: {override_count} файлов")));
        }
        // Modrinth ports of FTB packs ship without the FTB mods (license):
        // the game then stops on «Modrinth does not have the FTB mods!».
        // Fetch them from CurseForge for the pack's exact Minecraft/loader.
        if hit.source == "modrinth" {
            match crate::ftb::fill_missing(client, cfg, &root, &plan.spec, cancel, on).await {
                Ok(added) if !added.is_empty() => {
                    on(ModEvent::File(format!("FTB: {}", added.join(", "))));
                }
                Ok(_) => {}
                Err(e) => failed.push(format!("FTB-моды ({e:#})")),
            }
        }
        if !failed.is_empty() {
            let shown: Vec<&str> = failed.iter().take(3).map(|s| s.as_str()).collect();
            on(ModEvent::Warn(format!(
                "не скачалось файлов: {} ({}{})",
                failed.len(),
                shown.join(", "),
                if failed.len() > 3 { ", …" } else { "" }
            )));
        }
        Ok(root)
    }
    .await;
    // The archive and its unpacked index are disposable.
    let _ = std::fs::remove_dir_all(&unpacked);
    let _ = std::fs::remove_file(&archive);
    if let Ok(root) = &result {
        on(ModEvent::Done(root.clone()));
    }
    result
}

/// The pack archive: newest version of the pack, whatever Minecraft it targets.
pub async fn pack_archive(client: &reqwest::Client, cfg: &Config, hit: &ModHit) -> Result<(String, String)> {
    let p = latest_pack_file(client, cfg, hit).await?;
    Ok((p.url, p.filename))
}

/// The newest version of a pack at its source, asked live every time (no
/// cache): what gets installed and what an installed build compares with.
#[derive(Clone, Debug, Default)]
pub struct PackFile {
    pub url: String,
    pub filename: String,
    /// Modrinth version id / CurseForge file id.
    pub id: String,
    /// «1.8.0», «All the Mods 10-8.6».
    pub name: String,
    /// Publication date (ISO 8601).
    pub date: String,
}

impl PackFile {
    pub fn stamp(&self) -> crate::profiles::PackStamp {
        crate::profiles::PackStamp { file: self.id.clone(), version: self.name.clone(), date: self.date.clone() }
    }
}

pub async fn latest_pack_file(client: &reqwest::Client, cfg: &Config, hit: &ModHit) -> Result<PackFile> {
    if hit.source == "curseforge" {
        #[derive(Deserialize)]
        struct Resp {
            data: Vec<File>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct File {
            id: u64,
            #[serde(default)]
            display_name: String,
            #[serde(default)]
            file_name: String,
            #[serde(default)]
            file_date: String,
            #[serde(default)]
            download_url: Option<String>,
        }
        let url = format!("{}/mods/{}/files?pageSize=1", cf_base(cfg), hit.id);
        let newest = client
            .get(url)
            .header("x-api-key", cfg.cf_api_key.trim())
            .send()
            .await
            .context("CurseForge не отвечает")?
            .error_for_status()?
            .json::<Resp>()
            .await
            .ok()
            .and_then(|r| r.data.into_iter().next());
        let Some(f) = newest else {
            // The search result still knows the main file.
            let (url, filename) = curse_file(client, cfg, hit, &Spec::new("", Loader::Vanilla, ""), KIND_MODPACK).await?;
            let id = hit.file_id.map(|i| i.to_string()).unwrap_or_default();
            return Ok(PackFile { url, filename: filename.clone(), id, name: filename, date: String::new() });
        };
        let filename = if f.file_name.is_empty() { format!("{}-{}.zip", hit.slug, f.id) } else { f.file_name };
        return Ok(PackFile {
            url: cf_file_url(&hit.id, f.id, f.download_url),
            name: if f.display_name.is_empty() { filename.clone() } else { f.display_name },
            filename,
            id: f.id.to_string(),
            date: f.file_date,
        });
    }
    #[derive(Deserialize)]
    struct Version {
        id: String,
        #[serde(default)]
        version_number: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        date_published: String,
        #[serde(default)]
        files: Vec<VFile>,
    }
    #[derive(Deserialize)]
    struct VFile {
        url: String,
        filename: String,
        #[serde(default)]
        primary: bool,
    }
    // The project id works where the slug does; packs installed from a
    // build only remember the id.
    let key = if hit.slug.is_empty() { &hit.id } else { &hit.slug };
    let list: Vec<Version> = client
        .get(format!("https://api.modrinth.com/v2/project/{}/version?include_changelog=false", enc(key)))
        .send()
        .await
        .context("Modrinth не отвечает")?
        .error_for_status()?
        .json()
        .await
        .context("Список версий не читается")?;
    // Newest by date, not by list position.
    let v = list
        .into_iter()
        .filter(|v| !v.files.is_empty())
        .max_by(|a, b| a.date_published.cmp(&b.date_published))
        .ok_or_else(|| anyhow!("У «{}» нет файлов", hit.title))?;
    let mut files = v.files;
    let i = files.iter().position(|f| f.primary).unwrap_or(0);
    let f = files.swap_remove(i);
    let name = if v.version_number.is_empty() { v.name } else { v.version_number };
    Ok(PackFile { url: f.url, filename: f.filename, id: v.id, name, date: v.date_published })
}

/// A newer pack version than the build holds, or None when it is current.
/// Builds installed before versions were remembered (empty `pack_file`)
/// get the newest version as an update too — reinstall stamps them.
pub async fn pack_update(
    client: &reqwest::Client,
    cfg: &Config,
    profile: &crate::profiles::Profile,
) -> Result<Option<PackFile>> {
    let Some(hit) = profile_hit(profile) else {
        return Ok(None);
    };
    let newest = latest_pack_file(client, cfg, &hit).await?;
    Ok((newest.id != profile.pack_file).then_some(newest))
}

/// The catalog entry a pack build came from (`modrinth:ID`, `curseforge:ID`).
pub fn profile_hit(profile: &crate::profiles::Profile) -> Option<ModHit> {
    let (source, id) = profile.source.split_once(':')?;
    if id.is_empty() || !(source == "modrinth" || source == "curseforge") {
        return None;
    }
    Some(ModHit {
        source: source.to_string(),
        id: id.to_string(),
        slug: if source == "curseforge" { id.to_string() } else { String::new() },
        title: profile.name.clone(),
        kind: KIND_MODPACK.to_string(),
        ..Default::default()
    })
}

/// Downloads every pack file into the build, 32 at a time. Files that
/// already exist in another build (same name and size, SHA-1 when known)
/// are hard-linked instead of downloaded — reinstalls and packs sharing
/// popular mods finish almost instantly.
/// Returns the names of files that could not be fetched.
async fn fetch_pack_files<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    root: &Path,
    files: Vec<PackSource>,
    cancel: &Arc<AtomicBool>,
    on: &mut F,
) -> Result<Vec<String>> {
    let total = files.len() as u64;
    let local = {
        let (cfg, root) = (cfg.clone(), root.to_path_buf());
        Arc::new(tokio::task::spawn_blocking(move || LocalFiles::scan(&cfg, &root)).await.unwrap_or_default())
    };
    let curse_names = Arc::new(curse_file_names(client, cfg, &files).await);
    // Biggest first: the 100+ МБ resource packs of a pack start while the
    // small mods keep the other slots busy, instead of crawling alone at
    // the end of the bar.
    let mut files = files;
    files.sort_by_cached_key(|f| std::cmp::Reverse(match f {
        PackSource::Direct { size, .. } => *size,
        PackSource::Curse { file, .. } => curse_names.get(file).map(|(_, s)| *s).unwrap_or(0),
    }));
    let bytes_total: u64 = files
        .iter()
        .map(|f| match f {
            PackSource::Direct { size, .. } => *size,
            PackSource::Curse { file, .. } => curse_names.get(file).map(|(_, s)| *s).unwrap_or(0),
        })
        .sum();
    // Received bytes of every pack file, live: the bar moves while a
    // 130 МБ resource pack downloads instead of jumping when it ends.
    let live = Arc::new(AtomicU64::new(0));
    let mut last_bytes = std::time::Instant::now();
    let mut report_bytes = |on: &mut F, force: bool| {
        if bytes_total > 0 && (force || last_bytes.elapsed() >= std::time::Duration::from_millis(200)) {
            last_bytes = std::time::Instant::now();
            on(ModEvent::Bytes { done: live.load(Ordering::Relaxed).min(bytes_total), total: bytes_total });
        }
    };
    let sem = Arc::new(tokio::sync::Semaphore::new(32));
    let mut set = tokio::task::JoinSet::new();
    let mut done = 0u64;
    let mut failed: Vec<String> = Vec::new();
    on(ModEvent::Progress { done: 0, total });
    report_bytes(on, true);
    let handle = |res: std::result::Result<(String, bool), tokio::task::JoinError>,
                      done: &mut u64,
                      failed: &mut Vec<String>,
                      on: &mut F| {
        *done += 1;
        match res {
            Ok((name, true)) => on(ModEvent::File(name)),
            Ok((name, false)) => failed.push(name),
            Err(_) => failed.push("?".into()),
        }
        on(ModEvent::Progress { done: *done, total });
    };
    for file in files {
        if cancel.load(Ordering::Relaxed) {
            set.abort_all();
            bail!("Отменено");
        }
        let permit = sem.clone().acquire_owned().await?;
        let client = client.clone();
        let root = root.to_path_buf();
        let cancel = cancel.clone();
        let (local, curse_names, live) = (local.clone(), curse_names.clone(), live.clone());
        set.spawn(async move {
            let _permit = permit;
            fetch_pack_file(&client, &root, &file, &cancel, &local, &curse_names, &live).await
        });
        while let Some(res) = set.try_join_next() {
            handle(res, &mut done, &mut failed, on);
        }
        report_bytes(on, false);
    }
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        tokio::select! {
            res = set.join_next() => match res {
                Some(res) => handle(res, &mut done, &mut failed, on),
                None => break,
            },
            _ = ticker.tick() => {}
        }
        report_bytes(on, false);
    }
    report_bytes(on, true);
    if cancel.load(Ordering::Relaxed) {
        bail!("Отменено");
    }
    Ok(failed)
}

/// One pack file → (shown name, ok).
async fn fetch_pack_file(
    client: &reqwest::Client,
    root: &Path,
    file: &PackSource,
    cancel: &Arc<AtomicBool>,
    local: &LocalFiles,
    curse_names: &HashMap<u64, (String, u64)>,
    live: &AtomicU64,
) -> (String, bool) {
    match file {
        PackSource::Direct { urls, path, size, sha1 } => {
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            let dest = root.join(path);
            if std::fs::metadata(&dest).map(|m| m.len() > 0).unwrap_or(false)
                || local.link(&name, *size, sha1, &dest)
            {
                live.fetch_add(*size, Ordering::Relaxed);
                return (name, true);
            }
            for url in urls {
                // Bytes count as they arrive; a retry starts from zero, so
                // the counter follows `done` down as well as up.
                let mut counted = 0u64;
                let mut report = |done: u64, _total: u64| {
                    if done >= counted {
                        live.fetch_add(done - counted, Ordering::Relaxed);
                    } else {
                        live.fetch_sub(counted - done, Ordering::Relaxed);
                    }
                    counted = done;
                };
                let res = crate::minecraft::download_file_progress(
                    client,
                    url,
                    &dest,
                    Some(cancel.as_ref()),
                    &mut report,
                )
                .await;
                if res.is_ok() {
                    // Exactly the file's size, whatever the server reported.
                    let real = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(*size);
                    live.fetch_add(real.saturating_sub(counted), Ordering::Relaxed);
                    live.fetch_sub(counted.saturating_sub(real), Ordering::Relaxed);
                    return (name, true);
                }
                live.fetch_sub(counted, Ordering::Relaxed);
            }
            (name, false)
        }
        PackSource::Curse { project, file } => {
            if let Some((name, size)) = curse_names.get(file) {
                let sub = if name.to_lowercase().ends_with(".zip") { "resourcepacks" } else { "mods" };
                let dest = root.join(sub).join(name);
                if std::fs::metadata(&dest).map(|m| m.len() > 0).unwrap_or(false)
                    || local.link(name, *size, "", &dest)
                {
                    live.fetch_add(*size, Ordering::Relaxed);
                    return (name.clone(), true);
                }
            }
            for attempt in 1..=3u64 {
                match curse_public_file(client, root, *project, *file, cancel).await {
                    Ok(name) => {
                        let size = curse_names.get(file).map(|(_, s)| *s).unwrap_or(0);
                        live.fetch_add(size, Ordering::Relaxed);
                        return (name, true);
                    }
                    Err(e) if e.to_string().contains("Отменено") || attempt == 3 => break,
                    Err(_) => tokio::time::sleep(std::time::Duration::from_millis(500 * attempt)).await,
                }
            }
            (format!("CurseForge {project}/{file}"), false)
        }
    }
}

/// Fetches a CurseForge file through the public redirect; the final CDN
/// URL carries the real file name. Jars go to mods/, zips to resourcepacks/.
async fn curse_public_file(
    client: &reqwest::Client,
    root: &Path,
    project: u64,
    file: u64,
    cancel: &Arc<AtomicBool>,
) -> Result<String> {
    use futures_util::StreamExt;
    use std::io::Write;
    let resp = client
        .get(cf_public_url(&project.to_string(), file))
        .send()
        .await?
        .error_for_status()?;
    let name = resp
        .url()
        .path_segments()
        .and_then(|mut s| s.next_back())
        .map(percent_decode)
        .map(|n| sanitize(&n))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("cf-{file}.jar"));
    let sub = if name.to_lowercase().ends_with(".zip") {
        "resourcepacks"
    } else {
        "mods"
    };
    let dest = root.join(sub).join(&name);
    if std::fs::metadata(&dest).map(|m| m.len() > 0).unwrap_or(false) {
        return Ok(name);
    }
    std::fs::create_dir_all(root.join(sub))?;
    let tmp = crate::minecraft::part_path(&dest);
    {
        let mut out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    drop(out);
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e.into());
                }
            };
            crate::net::count(chunk.len());
            out.write_all(&chunk)?;
            if cancel.load(Ordering::Relaxed) {
                drop(out);
                let _ = std::fs::remove_file(&tmp);
                bail!("Отменено");
            }
        }
        out.flush()?;
    }
    std::fs::rename(&tmp, &dest)?;
    Ok(name)
}

fn unzip(archive: &Path, dest: &Path) -> Result<()> {
    crate::unpack::unzip(archive, dest).map(|_| ())
}

/// Files of every other build, by lowercase file name — the local cache a
/// pack install borrows from before going to the network.
#[derive(Default)]
struct LocalFiles {
    by_name: HashMap<String, Vec<PathBuf>>,
}

impl LocalFiles {
    fn scan(cfg: &Config, skip: &Path) -> Self {
        let mut roots = vec![cfg.game_dir()];
        if let Ok(rd) = std::fs::read_dir(crate::config::data_dir().join("meta").join("profiles")) {
            roots.extend(rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
        }
        let mut by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for root in roots.iter().filter(|r| r.as_path() != skip) {
            for sub in ["mods", "resourcepacks", "shaderpacks"] {
                let Ok(rd) = std::fs::read_dir(root.join(sub)) else { continue };
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_lowercase();
                    if name.ends_with(".jar") || name.ends_with(".zip") {
                        by_name.entry(name).or_default().push(e.path());
                    }
                }
            }
        }
        Self { by_name }
    }

    /// Hard-links (or copies across drives) a matching local file to `dest`.
    fn link(&self, name: &str, size: u64, sha1: &str, dest: &Path) -> bool {
        if size == 0 {
            return false;
        }
        let Some(candidates) = self.by_name.get(&name.to_lowercase()) else { return false };
        for src in candidates {
            if std::fs::metadata(src).map(|m| m.len()).ok() != Some(size) {
                continue;
            }
            if !sha1.is_empty() && file_sha1(src).as_deref() != Some(sha1) {
                continue;
            }
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::remove_file(dest);
            if std::fs::hard_link(src, dest).is_ok() || std::fs::copy(src, dest).is_ok() {
                return true;
            }
        }
        false
    }
}

fn file_sha1(path: &Path) -> Option<String> {
    use sha1::{Digest, Sha1};
    let mut f = std::fs::File::open(path).ok()?;
    let mut h = Sha1::new();
    let mut buf = vec![0u8; 128 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// CurseForge file id → (file name, size) in one batch request per 500
/// files: lets a reinstall skip present files without touching the CDN.
/// Empty when the API does not answer — files then resolve one by one.
async fn curse_file_names(client: &reqwest::Client, cfg: &Config, files: &[PackSource]) -> HashMap<u64, (String, u64)> {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<F>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct F {
        id: u64,
        #[serde(default)]
        file_name: String,
        #[serde(default)]
        file_length: u64,
    }
    let ids: Vec<u64> = files
        .iter()
        .filter_map(|f| match f {
            PackSource::Curse { file, .. } => Some(*file),
            _ => None,
        })
        .collect();
    let mut out = HashMap::new();
    for chunk in ids.chunks(500) {
        let resp = client
            .post(format!("{}/mods/files", cf_base(cfg)))
            .header("x-api-key", cfg.cf_api_key.trim())
            .json(&serde_json::json!({ "fileIds": chunk }))
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;
        let Ok(resp) = resp.and_then(|r| r.error_for_status()) else { break };
        let Ok(body) = resp.json::<Resp>().await else { break };
        for f in body.data {
            let name = sanitize(&f.file_name);
            if !name.is_empty() {
                out.insert(f.id, (name, f.file_length));
            }
        }
    }
    out
}

/// Sum of CurseForge file sizes, one batch request per 500 ids.
pub async fn curse_file_sizes(client: &reqwest::Client, cfg: &Config, ids: &[u64]) -> Result<u64> {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<F>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct F {
        #[serde(default)]
        file_length: u64,
    }
    let mut sum = 0u64;
    for chunk in ids.chunks(500) {
        let resp: Resp = client
            .post(format!("{}/mods/files", cf_base(cfg)))
            .header("x-api-key", cfg.cf_api_key.trim())
            .json(&serde_json::json!({ "fileIds": chunk }))
            .timeout(std::time::Duration::from_secs(20))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        sum += resp.data.iter().map(|f| f.file_length).sum::<u64>();
    }
    Ok(sum)
}

/// CurseForge's file fingerprint: MurmurHash2 (seed 1) over the bytes with
/// whitespace (tab, LF, CR, space) removed.
pub fn curse_fingerprint(data: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    let bytes: Vec<u8> = data.iter().copied().filter(|b| !matches!(b, 9 | 10 | 13 | 32)).collect();
    let len = bytes.len();
    let mut h: u32 = 1 ^ (len as u32);
    let mut chunks = bytes.chunks_exact(4);
    for c in &mut chunks {
        let mut k = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        k = k.wrapping_mul(M);
        k ^= k >> 24;
        k = k.wrapping_mul(M);
        h = h.wrapping_mul(M);
        h ^= k;
    }
    let rest = chunks.remainder();
    if rest.len() >= 3 {
        h ^= (rest[2] as u32) << 16;
    }
    if rest.len() >= 2 {
        h ^= (rest[1] as u32) << 8;
    }
    if !rest.is_empty() {
        h ^= rest[0] as u32;
        h = h.wrapping_mul(M);
    }
    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;
    h
}

/// Exact CurseForge matches for fingerprints: fp → (project, required deps).
pub async fn curse_by_fingerprint(client: &reqwest::Client, cfg: &Config, fps: &[u32]) -> HashMap<u32, (u64, Vec<u64>)> {
    #[derive(Deserialize)]
    struct Resp {
        data: Data,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Data {
        #[serde(default)]
        exact_matches: Vec<Match>,
    }
    #[derive(Deserialize)]
    struct Match {
        file: F,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct F {
        mod_id: u64,
        #[serde(default)]
        file_fingerprint: u64,
        #[serde(default)]
        dependencies: Vec<Dep>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Dep {
        mod_id: u64,
        relation_type: u32,
    }
    let mut out = HashMap::new();
    if fps.is_empty() {
        return out;
    }
    let resp = client
        .post(format!("{}/fingerprints/432", cf_base(cfg)))
        .header("x-api-key", cfg.cf_api_key.trim())
        .json(&serde_json::json!({ "fingerprints": fps }))
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await;
    if let Ok(r) = resp {
        if let Ok(body) = r.json::<Resp>().await {
            for m in body.data.exact_matches {
                let deps = m.file.dependencies.iter().filter(|d| d.relation_type == 3).map(|d| d.mod_id).collect();
                out.insert(m.file.file_fingerprint as u32, (m.file.mod_id, deps));
            }
        }
    }
    out
}

/// One CurseForge file picked for a build.
pub struct CurseFile {
    pub id: u64,
    pub name: String,
    pub url: String,
    /// Projects this file requires (relationType 3).
    pub requires: Vec<u64>,
}

/// Newest CurseForge file of a project for the build's Minecraft + loader.
pub async fn curse_latest_file(client: &reqwest::Client, cfg: &Config, project: u64, spec: &Spec) -> Result<Option<CurseFile>> {
    #[derive(Deserialize)]
    struct Resp {
        data: Vec<F>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct F {
        id: u64,
        #[serde(default)]
        file_name: String,
        #[serde(default)]
        download_url: Option<String>,
        #[serde(default)]
        dependencies: Vec<Dep>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Dep {
        mod_id: u64,
        relation_type: u32,
    }
    let mut url = format!("{}/mods/{project}/files?pageSize=1&gameVersion={}", cf_base(cfg), enc(&spec.mc));
    if let Some(t) = cf_loader_type(spec.loader) {
        url.push_str(&format!("&modLoaderType={t}"));
    }
    let resp: Resp = client
        .get(url)
        .header("x-api-key", cfg.cf_api_key.trim())
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(resp.data.into_iter().next().map(|f| CurseFile {
        url: cf_file_url(&project.to_string(), f.id, f.download_url),
        requires: f
            .dependencies
            .iter()
            .filter(|d| d.relation_type == 3)
            .map(|d| d.mod_id)
            .collect(),
        id: f.id,
        name: f.file_name,
    }))
}

// ---------------------------------------------------------------- project card

#[derive(Clone, Serialize)]
pub struct ProjectVersion {
    pub name: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    /// Publication date (ISO 8601); empty when the source omits it.
    pub date: String,
}

#[derive(Clone, Serialize)]
pub struct ProjectDetail {
    pub title: String,
    pub summary: String,
    pub body: String,
    pub downloads: u64,
    pub author: String,
    pub page_url: String,
    pub icon_url: Option<String>,
    /// Latest versions for the «версии» section.
    pub versions: Vec<ProjectVersion>,
    /// Whether some version runs on the active build (None = not applicable).
    pub compatible: Option<bool>,
    /// Screenshot URLs for the gallery strip.
    pub gallery: Vec<String>,
    /// Last update (ISO 8601): newest file/version.
    pub updated: String,
    /// YouTube video ids found in the description (embeds and links).
    pub videos: Vec<String>,
}

fn fits_build(v: &ProjectVersion, spec: &Spec, kind: &str) -> bool {
    let mc_ok = spec.mc.is_empty() || v.game_versions.iter().any(|g| g == &spec.mc);
    if kind != KIND_MOD || spec.loader == Loader::Vanilla {
        return mc_ok;
    }
    mc_ok
        && v.loaders.iter().any(|l| {
            let l = Loader::parse(l);
            l == spec.loader
                || (spec.loader == Loader::Quilt && l == Loader::Fabric)
                || (spec.loader == Loader::NeoForge && spec.mc == "1.20.1" && l == Loader::Forge)
        })
}

/// Loads the full project card: description, gallery, versions, site link.
pub async fn detail(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    spec: &Spec,
) -> Result<ProjectDetail> {
    let mut d = if hit.source == "modrinth" {
        modrinth_detail(client, hit).await?
    } else {
        curse_detail(client, cfg, hit).await?
    };
    if hit.kind != KIND_MODPACK && !d.versions.is_empty() {
        d.compatible = Some(d.versions.iter().any(|v| fits_build(v, spec, &hit.kind)));
    }
    d.versions.truncate(10);
    Ok(d)
}

async fn modrinth_detail(client: &reqwest::Client, hit: &ModHit) -> Result<ProjectDetail> {
    #[derive(Deserialize)]
    struct Proj {
        title: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        body: String,
        #[serde(default)]
        downloads: u64,
        #[serde(default)]
        icon_url: Option<String>,
        #[serde(default)]
        project_type: String,
        #[serde(default)]
        gallery: Vec<Shot>,
    }
    #[derive(Deserialize)]
    struct Shot {
        url: String,
        #[serde(default)]
        featured: bool,
    }
    #[derive(Deserialize)]
    struct V {
        name: String,
        #[serde(default)]
        game_versions: Vec<String>,
        #[serde(default)]
        loaders: Vec<String>,
        #[serde(default)]
        date_published: String,
    }
    let slug = enc(&hit.slug);
    let proj_req = client
        .get(format!("https://api.modrinth.com/v2/project/{slug}"))
        .send();
    let vers_req = client
        .get(format!("https://api.modrinth.com/v2/project/{slug}/version?include_changelog=false"))
        .send();
    let (proj, vers) = tokio::join!(proj_req, vers_req);
    let proj: Proj = proj
        .context("Modrinth не отвечает")?
        .error_for_status()?
        .json()
        .await
        .context("Проект не читается")?;
    let versions: Vec<ProjectVersion> = match vers {
        Ok(r) => r
            .json::<Vec<V>>()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|v| ProjectVersion {
                name: v.name,
                game_versions: v.game_versions,
                loaders: v.loaders,
                date: v.date_published,
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    let mut gallery = proj.gallery;
    gallery.sort_by_key(|s| !s.featured);
    let kind = if proj.project_type.is_empty() { "mod".to_string() } else { proj.project_type };
    let updated = versions.iter().map(|v| v.date.as_str()).max().unwrap_or("").to_string();
    let videos = youtube_ids(&proj.body);
    Ok(ProjectDetail {
        updated,
        videos,
        title: proj.title,
        summary: proj.description,
        body: clean_md(&proj.body),
        downloads: proj.downloads,
        author: hit.author.clone(),
        page_url: format!("https://modrinth.com/{kind}/{}", hit.slug),
        icon_url: proj.icon_url,
        versions,
        compatible: None,
        gallery: gallery.into_iter().take(8).map(|s| s.url).collect(),
    })
}

async fn curse_detail(client: &reqwest::Client, cfg: &Config, hit: &ModHit) -> Result<ProjectDetail> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Mod {
        name: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        download_count: f64,
        #[serde(default)]
        logo: Option<Logo>,
        #[serde(default)]
        authors: Vec<Author>,
        #[serde(default)]
        screenshots: Vec<Shot>,
        #[serde(default)]
        latest_files_indexes: Vec<FileIndex>,
        #[serde(default)]
        links: Option<Links>,
        #[serde(default)]
        latest_files: Vec<LatestFile>,
        #[serde(default)]
        date_released: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct LatestFile {
        #[serde(default)]
        file_name: String,
        #[serde(default)]
        file_date: String,
    }
    #[derive(Deserialize)]
    struct Logo {
        #[serde(default)]
        url: String,
    }
    #[derive(Deserialize)]
    struct Author {
        name: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Shot {
        #[serde(default)]
        url: String,
        #[serde(default)]
        thumbnail_url: String,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FileIndex {
        #[serde(default)]
        game_version: String,
        #[serde(default)]
        filename: String,
        #[serde(default)]
        mod_loader: Option<u32>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Links {
        #[serde(default)]
        website_url: String,
    }
    #[derive(Deserialize)]
    struct Resp {
        data: Mod,
    }
    #[derive(Deserialize)]
    struct DescResp {
        data: String,
    }
    let key = cfg.cf_api_key.trim().to_string();
    let base = cf_base(cfg);
    let mod_req = client
        .get(format!("{base}/mods/{}", hit.id))
        .header("x-api-key", key.clone())
        .send();
    let desc_req = client
        .get(format!("{base}/mods/{}/description", hit.id))
        .header("x-api-key", key)
        .send();
    let (m, desc) = tokio::join!(mod_req, desc_req);
    let m = m
        .context("CurseForge не отвечает")?
        .error_for_status()
        .context("CurseForge не ответил — попробуй ещё раз")?
        .json::<Resp>()
        .await
        .context("Проект не читается")?
        .data;
    let raw_desc = match desc {
        Ok(r) => r.json::<DescResp>().await.map(|d| d.data).unwrap_or_default(),
        Err(_) => String::new(),
    };
    let body = clean_html(&raw_desc);
    let videos = youtube_ids(&raw_desc);
    let file_dates: HashMap<&str, &str> =
        m.latest_files.iter().map(|f| (f.file_name.as_str(), f.file_date.as_str())).collect();
    let mut seen = std::collections::HashSet::new();
    let versions = m
        .latest_files_indexes
        .iter()
        .filter(|f| seen.insert((f.game_version.clone(), f.mod_loader)))
        .map(|f| ProjectVersion {
            name: f.filename.clone(),
            game_versions: vec![f.game_version.clone()],
            loaders: match cf_loader_name(f.mod_loader) {
                "" => Vec::new(),
                l => vec![l.to_lowercase()],
            },
            date: file_dates.get(f.filename.as_str()).map(|d| d.to_string()).unwrap_or_default(),
        })
        .collect();
    let updated = m
        .latest_files
        .iter()
        .map(|f| f.file_date.as_str())
        .max()
        .filter(|d| !d.is_empty())
        .unwrap_or(m.date_released.as_str())
        .to_string();
    let page_url = m
        .links
        .map(|l| l.website_url)
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| format!("https://www.curseforge.com/projects/{}", hit.id));
    Ok(ProjectDetail {
        updated,
        videos,
        title: m.name,
        summary: m.summary,
        body,
        downloads: m.download_count.max(0.0) as u64,
        author: m.authors.first().map(|a| a.name.clone()).unwrap_or_default(),
        page_url,
        icon_url: m.logo.map(|l| l.url).filter(|s| !s.is_empty()),
        versions,
        compatible: None,
        gallery: m
            .screenshots
            .into_iter()
            .take(8)
            .map(|s| if s.thumbnail_url.is_empty() { s.url } else { s.thumbnail_url })
            .filter(|u| !u.is_empty())
            .collect(),
    })
}

/// YouTube video ids in a description (markdown or HTML): embeds
/// (`/embed/ID`, nocookie), watch links (`?v=ID`), `youtu.be/ID`,
/// shorts. Order kept, duplicates dropped, at most six.
pub fn youtube_ids(text: &str) -> Vec<String> {
    const MARKERS: [&str; 6] = [
        "youtube.com/embed/",
        "youtube-nocookie.com/embed/",
        "youtube.com/watch?v=",
        "youtube.com/shorts/",
        "youtu.be/",
        "youtube.com/v/",
    ];
    let mut found: Vec<(usize, String)> = Vec::new();
    for marker in MARKERS {
        let mut from = 0;
        while let Some(at) = text[from..].find(marker) {
            let start = from + at + marker.len();
            let id: String = text[start..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if id.len() == 11 {
                found.push((from + at, id));
            }
            from = start;
        }
    }
    // A video both linked and embedded is shown once, in text order.
    found.sort_by_key(|(at, _)| *at);
    let mut seen = std::collections::HashSet::new();
    found
        .into_iter()
        .filter(|(_, id)| seen.insert(id.clone()))
        .map(|(_, id)| id)
        .take(6)
        .collect()
}

/// «3 дн. назад», «2 мес. назад» from an ISO 8601 date; empty if unreadable.
pub fn ago(iso: &str, now_unix: i64) -> String {
    let Some(then) = crate::util::parse_iso_unix(iso) else {
        return String::new();
    };
    let secs = (now_unix - then).max(0);
    let (hour, day) = (3600, 86_400);
    match secs {
        s if s < hour => "только что".into(),
        s if s < day => format!("{} ч. назад", s / hour),
        s if s < 2 * day => "вчера".into(),
        s if s < 31 * day => format!("{} дн. назад", s / day),
        s if s < 365 * day => format!("{} мес. назад", s / (30 * day)),
        s => format!("{} г. назад", s / (365 * day)),
    }
}

/// HTML → readable plain text: block tags become line breaks, list items
/// get bullets, entities are decoded, blank runs collapse.
pub fn clean_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('>') else {
            rest = "";
            break;
        };
        let tag = rest[start + 1..start + end].trim().to_ascii_lowercase();
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        match name {
            "br" | "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "tr" | "ul" | "ol" | "table" => {
                out.push('\n')
            }
            "li" if !tag.starts_with('/') => out.push_str("\n• "),
            _ => {}
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let mut text = String::with_capacity(decoded.len());
    let mut blank = 0;
    for line in decoded.lines() {
        let line = line.trim();
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        text.push_str(line);
        text.push('\n');
    }
    let text = text.trim().to_string();
    if text.chars().count() > 24000 {
        let cut: String = text.chars().take(24000).collect();
        format!("{cut}…\n(полное описание — на сайте проекта)")
    } else {
        text
    }
}

/// Strips markdown noise so the body reads as plain text.
pub fn clean_md(md: &str) -> String {
    // Modrinth bodies often embed HTML too.
    let md = if md.contains("</") || md.contains("<br") {
        clean_html(md)
    } else {
        md.to_string()
    };
    let mut out = String::with_capacity(md.len());
    let mut in_code = false;
    for line in md.lines() {
        let mut line = line.to_string();
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            out.push_str(line.trim_start());
            out.push('\n');
            continue;
        }
        // images: ![alt](url) -> drop; links: [text](url) -> text
        while let Some(start) = line.find("![") {
            let Some(end) = line[start..].find(')') else { break };
            line.replace_range(start..start + end + 1, "");
        }
        while let Some(start) = line.find('[') {
            let Some(mid) = line[start..].find("](") else { break };
            let Some(end) = line[start + mid..].find(')') else { break };
            let text = line[start + 1..start + mid].to_string();
            line.replace_range(start..start + mid + end + 1, &text);
        }
        let t = line.trim();
        let t = t.trim_start_matches('#').trim();
        let t = t.replace("**", "").replace("__", "");
        let t = if let Some(item) = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")) {
            format!("• {item}")
        } else {
            t
        };
        out.push_str(&t);
        out.push('\n');
        if out.len() > 24000 {
            out.push_str("…\n(полное описание — на сайте проекта)");
            break;
        }
    }
    out.trim().to_string()
}

/// `%D0%9F` → `П`; invalid sequences are kept as-is.
pub fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sanitize(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .to_string()
}

fn enc(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn youtube_ids_from_markdown_and_html() {
        let md = "Trailer: https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10 and                   <iframe src=\"https://www.youtube-nocookie.com/embed/abcdefghijk\"></iframe>                   again https://youtu.be/dQw4w9WgXcQ, short https://youtube.com/shorts/ZYXWVUTSRQP                   broken https://youtu.be/short";
        assert_eq!(youtube_ids(md), vec!["dQw4w9WgXcQ", "abcdefghijk", "ZYXWVUTSRQP"]);
        assert!(youtube_ids("no videos here").is_empty());
    }

    #[test]
    fn iso_dates_and_ago() {
        let t = crate::util::parse_iso_unix("2026-11-17T23:14:40.617931Z").unwrap();
        assert_eq!(t, 1_794_957_280);
        // Same moment with an offset.
        assert_eq!(crate::util::parse_iso_unix("2026-11-18T02:14:40+03:00"), Some(t));
        assert_eq!(crate::util::parse_iso_unix("2026-11-17"), Some(t - (23 * 3600 + 14 * 60 + 40)));
        assert_eq!(crate::util::parse_iso_unix("вчера"), None);
        assert_eq!(crate::util::ru_date("2026-11-17T23:14:40Z"), "17 нояб. 2026");
        assert_eq!(ago("2026-11-17T23:14:40Z", t + 3 * 86_400 + 5), "3 дн. назад");
        assert_eq!(ago("2026-11-17T23:14:40Z", t + 600), "только что");
        assert_eq!(ago("2026-11-17T23:14:40Z", t + 90 * 86_400), "3 мес. назад");
        assert_eq!(ago("", t), "");
    }

    /// Newest pack versions and card data straight from both sources.
    /// `cargo test live_pack_versions -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_pack_versions() {
        crate::net::rt().block_on(async {
            let client = crate::net::client();
            let cfg = Config::default();
            for (source, id, slug) in [("modrinth", "Jkb29YJU", "cobbleverse"), ("curseforge", "925200", "925200")] {
                let hit = ModHit { source: source.into(), id: id.into(), slug: slug.into(), title: slug.into(), kind: KIND_MODPACK.into(), ..Default::default() };
                let f = latest_pack_file(&client, &cfg, &hit).await.unwrap();
                let d = detail(&client, &cfg, &hit, &Spec::new("", Loader::Vanilla, "")).await.unwrap();
                println!("{source}: newest {} ({}) {} · downloads {} · updated {} · videos {:?}", f.name, f.id, f.date, d.downloads, d.updated, d.videos);
                assert!(!f.id.is_empty() && !f.date.is_empty() && !d.updated.is_empty());
            }
        });
    }

    #[test]
    fn pack_build_maps_back_to_its_catalog_entry() {
        let mut p = crate::profiles::Profile {
            id: "pack-x".into(),
            name: "Cobbleverse".into(),
            kind: crate::profiles::ProfileKind::Modpack,
            mc_version: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: String::new(),
            description: String::new(),
            created_at: String::new(),
            source: "modrinth:Jkb29YJU".into(),
            pack_file: String::new(),
            pack_version: String::new(),
            pack_date: String::new(),
        };
        let hit = profile_hit(&p).unwrap();
        assert_eq!((hit.source.as_str(), hit.id.as_str(), hit.kind.as_str()), ("modrinth", "Jkb29YJU", KIND_MODPACK));
        p.source = "curseforge:925200".into();
        assert_eq!(profile_hit(&p).unwrap().slug, "925200");
        p.source = String::new();
        assert!(profile_hit(&p).is_none());
    }

    #[test]
    fn cf_sort_codes_match_api() {
        assert_eq!(cf_sort_field("downloads"), 6);
        assert_eq!(cf_sort_field("follows"), 2);
        assert_eq!(cf_sort_field("updated"), 3);
        assert_eq!(cf_sort_field("newest"), 11);
    }

    #[test]
    fn cf_search_url_uses_item_offset_and_build_filters() {
        let spec = Spec::new("1.20.1", Loader::Fabric, "");
        let url = cf_search_url("B", "", &spec, KIND_MOD, "downloads", "", 80);
        assert!(url.contains("index=80"), "{url}");
        assert!(url.contains("pageSize=40"));
        assert!(url.contains("gameVersion=1.20.1"));
        assert!(url.contains("modLoaderType=4"));
        assert!(!url.contains("searchFilter"));
        let packs = cf_search_url("B", "rl craft", &spec, KIND_MODPACK, "newest", "4475", 0);
        assert!(packs.contains("categoryId=4475"));
        assert!(!packs.contains("gameVersion"), "packs show every version: {packs}");
        assert!(!packs.contains("modLoaderType"));
        assert!(packs.contains("classId=4471"));
        assert!(packs.contains("sortField=11"));
        assert!(packs.contains("searchFilter=rl%20craft"));
    }

    #[test]
    fn modrinth_facets_per_kind() {
        let spec = Spec::new("1.20.1", Loader::Forge, "");
        assert_eq!(
            modrinth_facets(&spec, KIND_MOD, ""),
            r#"[["project_type:mod"],["versions:1.20.1"],["categories:forge"]]"#
        );
        assert_eq!(modrinth_facets(&spec, KIND_MODPACK, ""), r#"[["project_type:modpack"]]"#);
        assert_eq!(
            modrinth_facets(&spec, KIND_SHADER, ""),
            r#"[["project_type:shader"],["versions:1.20.1"]]"#
        );
        assert_eq!(
            modrinth_facets(&spec, KIND_MODPACK, "adventure"),
            r#"[["project_type:modpack"],["categories:adventure"]]"#
        );
        let quilt = Spec::new("1.20.1", Loader::Quilt, "");
        assert!(modrinth_facets(&quilt, KIND_MOD, "").contains(r#""categories:fabric""#));
    }

    #[test]
    fn mrpack_plan_reads_loader_and_skips_server_files() {
        let text = r#"{
            "formatVersion": 1, "game": "minecraft", "name": "P",
            "dependencies": {"minecraft": "1.20.1", "fabric-loader": "0.15.3"},
            "files": [
                {"path": "mods/a.jar", "downloads": ["https://x/a.jar"], "env": {"client": "required", "server": "required"}},
                {"path": "mods/server.jar", "downloads": ["https://x/s.jar"], "env": {"client": "unsupported", "server": "required"}},
                {"path": "../evil.jar", "downloads": ["https://x/e.jar"]}
            ]
        }"#;
        let plan = mrpack_plan(text).unwrap();
        assert_eq!(plan.spec, Spec::new("1.20.1", Loader::Fabric, "0.15.3"));
        assert_eq!(
            plan.files,
            vec![PackSource::Direct {
                urls: vec!["https://x/a.jar".into()],
                path: "mods/a.jar".into(),
                size: 0,
                sha1: String::new(),
            }]
        );
    }

    #[test]
    fn cf_plan_reads_real_manifest_shape() {
        let text = r#"{
            "minecraft": {"version": "1.12.2", "modLoaders": [{"id": "forge-14.23.5.2860", "primary": true}]},
            "manifestType": "minecraftModpack", "manifestVersion": 1, "name": "RLCraft",
            "files": [
                {"projectID": 1, "fileID": 10, "required": true},
                {"projectID": 2, "fileID": 20, "required": false}
            ],
            "overrides": "overrides"
        }"#;
        let plan = cf_plan(text).unwrap();
        assert_eq!(plan.spec, Spec::new("1.12.2", Loader::Forge, "14.23.5.2860"));
        assert_eq!(plan.files, vec![PackSource::Curse { project: 1, file: 10 }]);
        assert_eq!(plan.overrides, vec!["overrides".to_string()]);
    }

    #[test]
    fn safe_rel_blocks_escapes() {
        assert_eq!(safe_rel("mods/a.jar").as_deref(), Some("mods/a.jar"));
        assert_eq!(safe_rel("config\\x.toml").as_deref(), Some("config/x.toml"));
        assert_eq!(safe_rel("../a.jar"), None);
        assert_eq!(safe_rel("/etc/a"), None);
        assert_eq!(safe_rel("C:/Windows/a"), None);
        assert_eq!(safe_rel("mods//a.jar"), None);
    }

    #[test]
    fn clean_html_keeps_structure() {
        let html = "<h2>Title</h2><p>Hello&nbsp;<b>world</b> &amp; friends</p><ul><li>one</li><li>two</li></ul><br>end";
        assert_eq!(clean_html(html), "Title\n\nHello world & friends\n\n• one\n• two\n\nend");
    }

    #[test]
    fn percent_decode_handles_utf8() {
        assert_eq!(percent_decode("jei%201.20.jar"), "jei 1.20.jar");
        assert_eq!(percent_decode("%D0%9F%D0%B0%D0%BA.zip"), "Пак.zip");
        assert_eq!(percent_decode("100%"), "100%");
    }

    #[test]
    fn cf_links_with_spaces_use_public_redirect() {
        assert_eq!(cf_file_url("1", 2, Some("https://edge.forgecdn.net/files/a/b/x.jar".into())), "https://edge.forgecdn.net/files/a/b/x.jar");
        assert_eq!(
            cf_file_url("285109", 4612979, Some("https://edge.forgecdn.net/files/4612/979/RLCraft%201.12.2.zip".into())),
            "https://www.curseforge.com/api/v1/mods/285109/files/4612979/download"
        );
        assert_eq!(cf_file_url("1", 2, None), "https://www.curseforge.com/api/v1/mods/1/files/2/download");
    }

    #[test]
    fn curse_fingerprint_matches_reference() {
        // Reference values from CurseForge's MurmurHash2 (whitespace-stripped).
        assert_eq!(curse_fingerprint(b""), 1540447798);
        assert_eq!(curse_fingerprint(b"hello world"), 2824650221);
        assert_eq!(curse_fingerprint(b"Minecraft mods!"), 1727433954);
        assert_eq!(curse_fingerprint(b"a b\tc\n"), curse_fingerprint(b"abc"));
    }

    #[test]
    fn cjk_runs_are_dropped() {
        assert_eq!(displayable("Prominence II: 哈斯图尔纪元"), "Prominence II");
        assert_eq!(displayable("Зомби 100 дней 僵尸"), "Зомби 100 дней");
        assert_eq!(displayable("全中文"), "");
        assert_eq!(displayable("JEI  —  рецепты"), "JEI — рецепты");
    }

    #[test]
    fn sources_per_kind() {
        assert_eq!(effective_source(KIND_MAP, "modrinth"), "curseforge");
        assert_eq!(effective_source(KIND_MOD, "curseforge"), "curseforge");
        assert_eq!(effective_source(KIND_SHADER, "modrinth"), "modrinth");
    }

    /// Real pack, temp folder: download + plan + parallel overrides + files.
    /// `cargo test live_pack_speed -- --ignored --nocapture`
    /// (MADRID_PACK=slug to pick another Modrinth pack)
    #[test]
    #[ignore]
    fn live_pack_speed() {
        let slug = std::env::var("MADRID_PACK").unwrap_or_else(|_| "fabulously-optimized".into());
        crate::net::rt().block_on(async {
            let client = crate::net::client();
            let versions: serde_json::Value = client
                .get(format!("https://api.modrinth.com/v2/project/{slug}/version"))
                .send().await.unwrap().json().await.unwrap();
            let url = versions[0]["files"][0]["url"].as_str().unwrap().to_string();
            let dir = std::env::temp_dir().join(format!("sad-pack-speed-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let archive = dir.join("p.mrpack");
            let t0 = std::time::Instant::now();
            crate::minecraft::download_file(&client, &url, &archive, None).await.unwrap();
            let t_dl = t0.elapsed();
            let text = crate::unpack::read_text(&archive, "modrinth.index.json").unwrap();
            let plan = mrpack_plan(&text).unwrap();
            let root = dir.join("root");
            let t1 = std::time::Instant::now();
            let b0 = crate::net::bytes_total();
            let jobs = crate::unpack::prefixed_jobs(&archive, &plan.overrides, &root).unwrap();
            let n_over = jobs.len();
            let a2 = archive.clone();
            let ex = tokio::task::spawn_blocking(move || {
                let t = std::time::Instant::now();
                let n = crate::unpack::extract(&a2, jobs, None, &std::sync::atomic::AtomicU64::new(0)).unwrap();
                (n, t.elapsed())
            });
            let cancel = Arc::new(AtomicBool::new(false));
            let n_files = plan.files.len();
            let mut sizes: Vec<u64> = plan.files.iter().filter_map(|f| match f {
                PackSource::Direct { size, .. } => Some(*size),
                _ => None,
            }).collect();
            sizes.sort_unstable_by(|a, b| b.cmp(a));
            println!("files {n_files}, {:.1} MB, biggest MB: {:?}", sizes.iter().sum::<u64>() as f64 / 1048576.0,
                sizes.iter().take(6).map(|s| s / 1048576).collect::<Vec<_>>());
            // Timeline: MB/s and files done every 2 s.
            let done = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let stop = Arc::new(AtomicBool::new(false));
            let sampler = {
                let (done, stop) = (done.clone(), stop.clone());
                std::thread::spawn(move || {
                    let mut prev = crate::net::bytes_total();
                    let mut out = Vec::new();
                    while !stop.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        let now = crate::net::bytes_total();
                        out.push(format!("{:.1}/{}", (now - prev) as f64 / 2.0 / 1048576.0, done.load(Ordering::Relaxed)));
                        prev = now;
                    }
                    out
                })
            };
            let d2 = done.clone();
            let failed = fetch_pack_files(&client, &Config::default(), &root, plan.files, &cancel, &mut |ev| {
                if let ModEvent::Progress { done, .. } = ev { d2.store(done, Ordering::Relaxed); }
            })
                .await.unwrap();
            stop.store(true, Ordering::Relaxed);
            println!("МБ/с / файлов по 2 с: {}", sampler.join().unwrap().join(" "));
            let (n_ex, t_ex) = ex.await.unwrap();
            println!(
                "pack {slug}: archive {:?}, overrides {n_over} ({n_ex} written in {:?}), files {n_files} failed {} — total {:?}",
                t_dl, t_ex, failed.len(), t1.elapsed()
            );
            println!("downloaded {:.1} MB", (crate::net::bytes_total() - b0) as f64 / 1048576.0);
            assert!(failed.is_empty());
            let _ = std::fs::remove_dir_all(&dir);
        });
    }
}
