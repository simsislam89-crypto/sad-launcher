//! Missing mod dependencies: read them from the game log, fetch them.
//!
//! Forge/NeoForge print
//! `Mod ID: 'moogs_structures', Requested by: 'mns', … Actual version: '[MISSING]'`
//! and Fabric prints
//! `- Mod 'A' (a) requires version 1.0 or later of 'B' (b), which is missing!`.
//! The launcher stops the game on such an error screen, finds each mod for
//! the build's Minecraft + loader (Modrinth by id/slug, then search, then
//! CurseForge), puts it into the build and starts the game again.

use crate::config::Config;
use crate::loader::{Loader, Spec};
use crate::mods::ModEvent;
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Ids that are the platform itself, never downloadable mods.
const PLATFORM: &[&str] = &["minecraft", "forge", "neoforge", "fabricloader", "fabric-loader", "quilt_loader", "java", "fml", "javafml"];

/// Incremental parser fed with game log lines.
#[derive(Default)]
pub struct MissingParser {
    in_forge_block: bool,
    pub found: Vec<String>,
    /// (missing id, mod id that asked for it)
    pub pairs: Vec<(String, String)>,
}

impl MissingParser {
    /// Feeds one line; returns true when a new missing id was recorded.
    pub fn feed(&mut self, line: &str) -> bool {
        if line.contains("Missing or unsupported mandatory dependencies") {
            self.in_forge_block = true;
            return false;
        }
        if self.in_forge_block {
            if let Some((id, by)) = forge_line(line) {
                return self.push(id, by);
            }
            if !line.trim_start().starts_with("Mod ID:") {
                self.in_forge_block = false;
            }
        }
        if let Some((id, by)) = fabric_line(line) {
            return self.push(id, by);
        }
        false
    }

    fn push(&mut self, id: String, by: String) -> bool {
        if PLATFORM.contains(&id.as_str()) {
            return false;
        }
        if !by.is_empty() && !self.pairs.contains(&(id.clone(), by.clone())) {
            self.pairs.push((id.clone(), by));
        }
        if self.found.contains(&id) {
            return false;
        }
        self.found.push(id);
        true
    }
}

/// `Mod ID: 'x', Requested by: 'y', Expected range: '[1,)', Actual version: '[MISSING]'`
/// — also too-old versions: the newest file replaces them.
fn forge_line(line: &str) -> Option<(String, String)> {
    let rest = line.trim_start().strip_prefix("Mod ID: '")?;
    let end = rest.find('\'')?;
    let id = rest[..end].trim();
    let by = rest
        .find("Requested by: '")
        .map(|i| &rest[i + 15..])
        .and_then(|r| r.find('\'').map(|e| r[..e].trim().to_string()))
        .unwrap_or_default();
    (!id.is_empty()).then(|| (id.to_string(), by))
}

/// `- Mod 'A' (a) requires … of 'B' (b), which is missing!`
fn fabric_line(line: &str) -> Option<(String, String)> {
    if !line.contains(" requires ") || !line.contains("which is missing") {
        return None;
    }
    let cut = line.find(" requires ")?;
    let tail = &line[cut..];
    let open = tail.rfind('(')?;
    let close = tail[open..].find(')')? + open;
    let id = tail[open + 1..close].trim();
    // Requester: the first «(id)» before « requires ».
    let head = &line[..cut];
    let by = head
        .find('(')
        .and_then(|o| head[o + 1..].find(')').map(|c| head[o + 1..o + 1 + c].trim().to_string()))
        .unwrap_or_default();
    (!id.is_empty()).then(|| (id.to_string(), by))
}

#[derive(Deserialize)]
struct MrVersion {
    #[serde(default)]
    files: Vec<MrFile>,
}

#[derive(Deserialize)]
struct MrFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
}

fn loaders_param(spec: &Spec) -> String {
    let list: &[&str] = match spec.loader {
        Loader::Forge => &["forge"],
        Loader::NeoForge if spec.mc == "1.20.1" => &["neoforge", "forge"],
        Loader::NeoForge => &["neoforge"],
        Loader::Fabric => &["fabric"],
        Loader::Quilt => &["quilt", "fabric"],
        Loader::Vanilla => &[],
    };
    let quoted: Vec<String> = list.iter().map(|l| format!("\"{l}\"")).collect();
    format!("[{}]", quoted.join(","))
}

fn enc(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Newest Modrinth file of project `slug` for the build, if any.
async fn modrinth_file(client: &reqwest::Client, slug: &str, spec: &Spec) -> Option<(String, String)> {
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?include_changelog=false&game_versions={}&loaders={}",
        enc(slug),
        enc(&format!("[\"{}\"]", spec.mc)),
        enc(&loaders_param(spec))
    );
    let list: Vec<MrVersion> = client.get(url).send().await.ok()?.error_for_status().ok()?.json().await.ok()?;
    list.into_iter().find_map(|v| {
        let mut files = v.files;
        if files.is_empty() {
            return None;
        }
        let i = files.iter().position(|f| f.primary).unwrap_or(0);
        let f = files.swap_remove(i);
        Some((f.url, f.filename))
    })
}

/// Mod ids a jar declares (Forge/NeoForge toml, Fabric/Quilt json).
pub fn jar_mod_ids(path: &Path) -> Vec<String> {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(path) else { return Vec::new() };
    let Ok(mut zip) = zip::ZipArchive::new(std::io::BufReader::new(file)) else { return Vec::new() };
    let mut ids: Vec<String> = Vec::new();
    for toml in ["META-INF/mods.toml", "META-INF/neoforge.mods.toml"] {
        if let Ok(mut e) = zip.by_name(toml) {
            let mut text = String::new();
            if e.read_to_string(&mut text).is_ok() {
                // Only `[[mods]]` tables declare mods; `[[dependencies.x]]`
                // tables also carry `modId` — of what they depend on.
                let mut in_mods = false;
                for line in text.lines() {
                    // `[[mods]] #mandatory` — comments share the line.
                    let line = line.split('#').next().unwrap_or("").trim();
                    if line.starts_with('[') {
                        in_mods = line.replace(' ', "") == "[[mods]]";
                        continue;
                    }
                    if !in_mods {
                        continue;
                    }
                    let Some(rest) = line.strip_prefix("modId") else { continue };
                    let Some(eq) = rest.find('=') else { continue };
                    let value = rest[eq + 1..].trim();
                    let value = value.trim_start_matches(['"', '\'']);
                    let value = value.split(['"', '\'']).next().unwrap_or("").trim();
                    if !value.is_empty() && !ids.iter().any(|i| i == value) {
                        ids.push(value.to_string());
                    }
                }
            }
        }
    }
    for json in ["fabric.mod.json", "quilt.mod.json"] {
        if let Ok(mut e) = zip.by_name(json) {
            let mut text = String::new();
            if e.read_to_string(&mut text).is_ok() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    let id = v["id"].as_str().or_else(|| v["quilt_loader"]["id"].as_str());
                    if let Some(id) = id {
                        if !ids.iter().any(|i| i == id) {
                            ids.push(id.to_string());
                        }
                    }
                }
            }
        }
    }
    ids
}

fn sha1_hex(path: &Path) -> Option<String> {
    use sha1::{Digest, Sha1};
    let bytes = std::fs::read(path).ok()?;
    let digest = Sha1::digest(&bytes);
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

/// Every jar in the folder (enabled or switched off) with its mod ids.
fn index_jars(mods_dir: &Path) -> Vec<(std::path::PathBuf, Vec<String>, bool)> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(mods_dir) else { return out };
    for e in rd.flatten() {
        let path = e.path();
        let name = e.file_name().to_string_lossy().to_lowercase();
        let enabled = name.ends_with(".jar");
        if !enabled && !name.ends_with(".jar.disabled") {
            continue;
        }
        let ids = jar_mod_ids(&path);
        out.push((path, ids, enabled));
    }
    out
}

/// Modrinth versions for jar hashes: hash → (project id, required deps).
async fn versions_by_hash(client: &reqwest::Client, hashes: &[String]) -> std::collections::HashMap<String, (String, Vec<String>)> {
    #[derive(Deserialize)]
    struct V {
        project_id: String,
        #[serde(default)]
        dependencies: Vec<D>,
    }
    #[derive(Deserialize)]
    struct D {
        #[serde(default)]
        project_id: Option<String>,
        #[serde(default)]
        dependency_type: String,
    }
    let mut out = std::collections::HashMap::new();
    if hashes.is_empty() {
        return out;
    }
    let resp = client
        .post("https://api.modrinth.com/v2/version_files")
        .json(&serde_json::json!({ "hashes": hashes, "algorithm": "sha1" }))
        .send()
        .await;
    if let Ok(r) = resp {
        if let Ok(map) = r.json::<std::collections::HashMap<String, V>>().await {
            for (hash, v) in map {
                let deps = v
                    .dependencies
                    .into_iter()
                    .filter(|d| d.dependency_type == "required")
                    .filter_map(|d| d.project_id)
                    .collect();
                out.insert(hash, (v.project_id, deps));
            }
        }
    }
    out
}

/// Downloads `url` to a temp name, keeps it only if it declares `id`.
async fn try_candidate(client: &reqwest::Client, url: &str, name: &str, id: &str, mods_dir: &Path, cancel: &Arc<AtomicBool>) -> Option<String> {
    let clean: String = name.chars().filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')).collect();
    if clean.is_empty() {
        return None;
    }
    let tmp = mods_dir.join(format!("{clean}.check"));
    let _ = std::fs::remove_file(&tmp);
    crate::minecraft::download_file(client, url, &tmp, Some(cancel.as_ref())).await.ok()?;
    if jar_mod_ids(&tmp).iter().any(|m| m == id) {
        let dest = mods_dir.join(&clean);
        let _ = std::fs::remove_file(mods_dir.join(format!("{clean}.disabled")));
        if std::fs::rename(&tmp, &dest).is_ok() {
            return Some(clean);
        }
    }
    let _ = std::fs::remove_file(&tmp);
    None
}

/// Downloads every missing mod into `mods_dir`. Returns (added, not found).
/// Candidates per id, most precise first: required dependencies of the
/// mod that asked for it (looked up on Modrinth by jar hash), a switched-off
/// copy for another loader (same project, right loader), slug guesses,
/// catalog search. A candidate is kept only if its jar declares the id —
/// no wrong mods, no duplicates.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_missing<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    ids: &[String],
    pairs: &[(String, String)],
    spec: &Spec,
    mods_dir: &Path,
    cancel: &Arc<AtomicBool>,
    on: &mut F,
) -> Result<(Vec<String>, Vec<String>)> {
    std::fs::create_dir_all(mods_dir)?;
    let jars = index_jars(mods_dir);
    // Jars worth a hash lookup: requesters of missing ids, and switched-off
    // jars that declare a missing id.
    let mut wanted: Vec<(std::path::PathBuf, String)> = Vec::new();
    for (path, mids, enabled) in &jars {
        let requester = *enabled && pairs.iter().any(|(_, by)| mids.contains(by));
        let disabled_twin = !*enabled && ids.iter().any(|i| mids.contains(i));
        if requester || disabled_twin {
            if let Some(h) = sha1_hex(path) {
                wanted.push((path.clone(), h));
            }
        }
    }
    let hashes: Vec<String> = wanted.iter().map(|(_, h)| h.clone()).collect();
    let by_hash = versions_by_hash(client, &hashes).await;
    // The same jars on CurseForge (fingerprint), for mods Modrinth lacks.
    let mut fps: Vec<(std::path::PathBuf, u32)> = Vec::new();
    for (path, _) in &wanted {
        if let Ok(bytes) = std::fs::read(path) {
            fps.push((path.clone(), crate::mods::curse_fingerprint(&bytes)));
        }
    }
    let fp_list: Vec<u32> = fps.iter().map(|(_, f)| *f).collect();
    let by_fp = crate::mods::curse_by_fingerprint(client, cfg, &fp_list).await;

    let mut added = Vec::new();
    let mut missing = Vec::new();
    let total = ids.len() as u64;
    on(ModEvent::Progress { done: 0, total });
    for (i, id) in ids.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            anyhow::bail!("Отменено");
        }
        on(ModEvent::File(id.clone()));
        let mut projects: Vec<String> = Vec::new();
        for (path, hash) in &wanted {
            let Some((project, deps)) = by_hash.get(hash) else { continue };
            let mids = jars.iter().find(|(p, _, _)| p == path).map(|(_, m, _)| m.clone()).unwrap_or_default();
            if mids.contains(id) {
                projects.insert(0, project.clone()); // switched-off twin: same project
            } else if pairs.iter().any(|(m, by)| m == id && mids.contains(by)) {
                projects.extend(deps.iter().cloned());
            }
        }
        projects.push(id.clone());
        projects.push(id.replace('_', "-"));
        if let Ok(hits) = crate::mods::search(client, cfg, &id.replace('_', " "), "modrinth", spec, crate::mods::KIND_MOD, "relevance", "", 0).await {
            projects.extend(hits.into_iter().take(4).map(|h| h.id));
        }
        let mut seen = std::collections::HashSet::new();
        projects.retain(|p| seen.insert(p.clone()));
        let mut got = None;
        let mut tried = 0;
        for project in &projects {
            if tried >= 10 {
                break;
            }
            if let Some((url, name)) = modrinth_file(client, project, spec).await {
                tried += 1;
                if let Some(file) = try_candidate(client, &url, &name, id, mods_dir, cancel).await {
                    got = Some(file);
                    break;
                }
            }
        }
        if got.is_none() {
            // CurseForge: twin/requester matches by fingerprint first, then search.
            let mut cf_projects: Vec<u64> = Vec::new();
            for (path, fp) in &fps {
                let Some((project, deps)) = by_fp.get(fp) else { continue };
                let mids = jars.iter().find(|(p, _, _)| p == path).map(|(_, m, _)| m.clone()).unwrap_or_default();
                if mids.contains(id) {
                    cf_projects.insert(0, *project);
                } else if pairs.iter().any(|(m, by)| m == id && mids.contains(by)) {
                    cf_projects.extend(deps.iter().copied());
                }
            }
            if let Ok(hits) = crate::mods::search(client, cfg, &id.replace('_', " "), "curseforge", spec, crate::mods::KIND_MOD, "downloads", "", 0).await {
                cf_projects.extend(hits.into_iter().take(4).filter_map(|h| h.id.parse::<u64>().ok()));
            }
            let mut seen = std::collections::HashSet::new();
            cf_projects.retain(|p| seen.insert(*p));
            for project in cf_projects.into_iter().take(10) {
                if let Ok(Some(file)) = crate::mods::curse_latest_file(client, cfg, project, spec).await {
                    if let Some(name) = try_candidate(client, &file.url, &file.name, id, mods_dir, cancel).await {
                        got = Some(name);
                        break;
                    }
                }
            }
        }
        match got {
            Some(name) => added.push(name),
            None => missing.push(id.clone()),
        }
        on(ModEvent::Progress { done: i as u64 + 1, total });
    }
    Ok((added, missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_block_is_parsed() {
        let log = "[00:24:34] [main/ERROR] [ne.mi.fm.lo.ModSorter/LOADING]: Missing or unsupported mandatory dependencies:\n\
\tMod ID: 'moogs_structures', Requested by: 'mns', Expected range: '[1.0,)', Actual version: '[MISSING]'\n\
\tMod ID: 'formations', Requested by: 'formationsnether', Expected range: '[1.0.0,)', Actual version: '[MISSING]'\n\
\tMod ID: 'moogs_structures', Requested by: 'mvs', Expected range: '[1.0,)', Actual version: '[MISSING]'\n\
\tMod ID: 'minecraft', Requested by: 'x', Expected range: '[1.21,)', Actual version: '1.20.1'\n\
[00:24:34] [main/INFO] Launching target 'forgeclient'\n\
\tMod ID: 'late', Requested by: 'x'";
        let mut p = MissingParser::default();
        for line in log.lines() {
            p.feed(line);
        }
        assert_eq!(p.found, vec!["moogs_structures", "formations"]);
        assert_eq!(p.pairs[0], ("moogs_structures".to_string(), "mns".to_string()));
    }

    #[test]
    fn fabric_lines_are_parsed_but_not_recommends() {
        let mut p = MissingParser::default();
        p.feed(" - Mod 'Alloy Forgery' (alloy_forgery) 2.1.2+1.20 requires version 12.0.0 or later of 'Roughly Enough Items' (roughlyenoughitems), which is missing!");
        p.feed(" - Mod 'Colormatic' (colormatic) 3.1.2+mc.1.19 recommends any version of quilt_loader, which is missing!");
        p.feed(" - Mod 'X' (x) 1.0 requires any version of 'Fabric API' (fabric-api), which is missing!");
        assert_eq!(p.found, vec!["roughlyenoughitems", "fabric-api"]);
        assert_eq!(p.pairs[0], ("roughlyenoughitems".to_string(), "alloy_forgery".to_string()));
    }

    /// Resolves real missing deps of the Madrid build into a temp copy.
    /// `cargo test live_deps -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_deps_are_found() {
        let cfg = crate::config::load();
        let src = cfg.game_dir().join("mods");
        let dir = std::env::temp_dir().join("madrid-live-deps");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for e in std::fs::read_dir(&src).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            let l = n.to_lowercase();
            if l.starts_with("mns") || l.starts_with("moogs") || l.starts_with("mvs") || l.starts_with("formations") || l.starts_with("stony") || l.starts_with("lithostitched") {
                std::fs::copy(e.path(), dir.join(&n)).unwrap();
            }
        }
        let ids = vec!["moogs_structures".to_string(), "formations".to_string(), "lithostitched".to_string()];
        let pairs = vec![
            ("moogs_structures".to_string(), "mns".to_string()),
            ("formations".to_string(), "formationsnether".to_string()),
            ("lithostitched".to_string(), "stonycliffs".to_string()),
        ];
        let spec = Spec::new("1.20.1", Loader::Forge, "");
        let cancel = Arc::new(AtomicBool::new(false));
        let (added, missing) = crate::net::rt()
            .block_on(fetch_missing(&crate::net::client(), &cfg, &ids, &pairs, &spec, &dir, &cancel, &mut |_| {}))
            .unwrap();
        println!("added {added:?} missing {missing:?}");
        assert!(missing.is_empty(), "{missing:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

}
