//! FTB mods for Modrinth packs.
//!
//! FTB's license keeps their mods off Modrinth, so Modrinth ports of packs
//! built around FTB Quests/Teams/Library arrive without them and stop on
//! «Modrinth does not have the FTB mods!». The pack still carries their
//! data (`config/ftbquests/…`) or links to their CurseForge pages, which
//! tells exactly what is missing. The launcher fetches those mods from
//! CurseForge for the pack's own Minecraft + loader, with their required
//! FTB dependencies.

use crate::config::Config;
use crate::loader::{Loader, Spec};
use crate::mods::ModEvent;
use anyhow::Result;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct Known {
    name: &'static str,
    /// Folders/files under config/ or defaultconfigs/ that only exist when
    /// the pack uses this mod.
    dirs: &'static [&'static str],
    /// CurseForge page slugs a pack's menus link to.
    slugs: &'static [&'static str],
    /// Lowercase fragments of the jar name.
    jar: &'static [&'static str],
    forge: u64,
    fabric: u64,
}

const KNOWN: &[Known] = &[
    Known { name: "FTB Quests", dirs: &["ftbquests"], slugs: &["ftb-quests"], jar: &["ftb-quests", "ftbquests"], forge: 289412, fabric: 438496 },
    Known { name: "FTB Library", dirs: &["ftblibrary"], slugs: &["ftb-library"], jar: &["ftb-library", "ftblibrary"], forge: 404465, fabric: 438495 },
    Known { name: "FTB Teams", dirs: &["ftbteams"], slugs: &["ftb-teams"], jar: &["ftb-teams", "ftbteams"], forge: 404468, fabric: 438497 },
    Known { name: "FTB XMod Compat", dirs: &["ftbxmodcompat"], slugs: &["ftb-xmod"], jar: &["ftb-xmod", "ftbxmod"], forge: 889915, fabric: 889915 },
    Known { name: "FTB Chunks", dirs: &["ftbchunks"], slugs: &["ftb-chunks"], jar: &["ftb-chunks", "ftbchunks"], forge: 314906, fabric: 472657 },
    Known { name: "FTB Ranks", dirs: &["ftbranks"], slugs: &["ftb-ranks"], jar: &["ftb-ranks", "ftbranks"], forge: 314905, fabric: 472659 },
    Known { name: "FTB Ultimine", dirs: &["ftbultimine"], slugs: &["ftb-ultimine"], jar: &["ftb-ultimine", "ftbultimine"], forge: 386134, fabric: 448231 },
    Known { name: "FTB Essentials", dirs: &["ftbessentials"], slugs: &["ftb-essentials"], jar: &["ftb-essentials", "ftbessentials"], forge: 410811, fabric: 410811 },
    Known { name: "FTB Filter System", dirs: &["ftbfiltersystem"], slugs: &["ftb-filter-system"], jar: &["ftb-filter", "ftbfilter"], forge: 943925, fabric: 943925 },
];

/// Required non-FTB libraries the FTB mods pull in (Modrinth has them, but
/// a pack may not): (project, jar fragments).
const LIBS: &[(u64, &[&str])] = &[(419699, &["architectury"]), (306612, &["fabric-api", "fabricapi"])];

fn project_for(k: &Known, loader: Loader) -> u64 {
    match loader {
        Loader::Fabric | Loader::Quilt => k.fabric,
        _ => k.forge,
    }
}

fn jar_names(root: &Path) -> Vec<String> {
    std::fs::read_dir(root.join("mods"))
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .map(|n| n.to_lowercase())
                .filter(|n| n.ends_with(".jar"))
                .collect()
        })
        .unwrap_or_default()
}

fn has_jar(jars: &[String], fragments: &[&str]) -> bool {
    jars.iter().any(|j| fragments.iter().any(|f| j.contains(f)))
}

/// Text of the pack's menu/config files that may link to CurseForge pages
/// (FancyMenu layouts, Drippy, custom notices). Bounded: ≤ 3000 files,
/// ≤ 512 KB each.
fn config_text(root: &Path) -> String {
    let mut out = String::new();
    let mut budget = 3000usize;
    let mut stack = vec![root.join("config")];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            if budget == 0 {
                return out;
            }
            budget -= 1;
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if e.metadata().map(|m| m.len() > 512 * 1024).unwrap_or(true) {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if text.contains("curseforge.com") {
                    out.push_str(&text.to_lowercase());
                    out.push('\n');
                }
            }
        }
    }
    out
}

/// Which FTB mods the pack needs but does not have.
fn missing(root: &Path) -> Vec<&'static Known> {
    let jars = jar_names(root);
    let text = config_text(root);
    let mut need: Vec<&'static Known> = KNOWN
        .iter()
        .filter(|k| !has_jar(&jars, k.jar))
        .filter(|k| {
            let by_dir = k.dirs.iter().any(|d| {
                root.join("config").join(d).exists() || root.join("defaultconfigs").join(d).exists()
            });
            let by_link = k.slugs.iter().any(|s| text.contains(&format!("mc-mods/{s}")));
            by_dir || by_link
        })
        .collect();
    // Packs that ask for FTB Quests by link also ask for its compat layer.
    if need.iter().any(|k| k.name == "FTB Quests") && text.contains("xmod") {
        if let Some(x) = KNOWN.iter().find(|k| k.name == "FTB XMod Compat") {
            if !has_jar(&jars, x.jar) && !need.iter().any(|k| k.name == x.name) {
                need.push(x);
            }
        }
    }
    need
}

/// Quick check without network: does this pack folder lack FTB mods?
pub fn needs_fill(root: &Path) -> bool {
    !missing(root).is_empty()
}

/// Downloads missing FTB mods (and their required FTB/library deps) into
/// `root/mods`. Returns the names that were added.
pub async fn fill_missing<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    root: &Path,
    spec: &Spec,
    cancel: &Arc<AtomicBool>,
    on: &mut F,
) -> Result<Vec<String>> {
    let need = missing(root);
    if need.is_empty() {
        return Ok(Vec::new());
    }
    let mut queue: Vec<u64> = need.iter().map(|k| project_for(k, spec.loader)).collect();
    let mut done: Vec<u64> = Vec::new();
    let mut added: Vec<String> = Vec::new();
    let jars = jar_names(root);
    let mods_dir = root.join("mods");
    std::fs::create_dir_all(&mods_dir)?;
    while let Some(project) = queue.pop() {
        if cancel.load(Ordering::Relaxed) {
            anyhow::bail!("Отменено");
        }
        if done.contains(&project) {
            continue;
        }
        done.push(project);
        let Some(file) = crate::mods::curse_latest_file(client, cfg, project, spec).await? else {
            continue;
        };
        for dep in &file.requires {
            let ftb = KNOWN.iter().find(|k| project_for(k, spec.loader) == *dep);
            let lib = LIBS.iter().find(|(id, _)| id == dep);
            let wanted = match (ftb, lib) {
                (Some(k), _) => !has_jar(&jars, k.jar),
                (None, Some((_, frag))) => !has_jar(&jars, frag),
                _ => false,
            };
            if wanted && !done.contains(dep) {
                queue.push(*dep);
            }
        }
        let clean = crate::mods::sanitize(&file.name);
        let name = if clean.is_empty() || (!clean.ends_with(".jar") && !clean.ends_with(".zip")) {
            format!("cf-{}.jar", file.id)
        } else {
            clean
        };
        on(ModEvent::File(name.clone()));
        crate::minecraft::download_file(client, &file.url, &mods_dir.join(&name), Some(cancel.as_ref())).await?;
        let pretty = KNOWN
            .iter()
            .find(|k| project_for(k, spec.loader) == project)
            .map(|k| k.name.to_string())
            .unwrap_or(name);
        added.push(pretty);
    }
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("madrid-ftb-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        std::fs::create_dir_all(dir.join("config")).unwrap();
        dir
    }

    #[test]
    fn quests_folder_means_ftb_quests() {
        let root = temp("dir");
        std::fs::create_dir_all(root.join("config/ftbquests/quests")).unwrap();
        let names: Vec<&str> = missing(&root).iter().map(|k| k.name).collect();
        assert_eq!(names, vec!["FTB Quests"]);
        std::fs::write(root.join("mods/ftb-quests-fabric-2001.4.22.jar"), b"x").unwrap();
        assert!(missing(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn menu_links_are_read() {
        let root = temp("link");
        std::fs::create_dir_all(root.join("config/fancymenu")).unwrap();
        std::fs::write(
            root.join("config/fancymenu/layout.txt"),
            "url=https://www.curseforge.com/minecraft/mc-mods/ftb-teams-forge\nurl=https://www.curseforge.com/minecraft/mc-mods/ftb-xmod-compat",
        )
        .unwrap();
        let names: Vec<&str> = missing(&root).iter().map(|k| k.name).collect();
        assert_eq!(names, vec!["FTB Teams", "FTB XMod Compat"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn plain_pack_needs_nothing() {
        let root = temp("none");
        assert!(!needs_fill(&root));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `cargo test live_ftb -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_ftb_fill_for_fabric_pack() {
        let root = temp("live");
        std::fs::create_dir_all(root.join("config/ftbquests/quests")).unwrap();
        std::fs::write(root.join("mods/architectury-9.2.14-fabric.jar"), b"x").unwrap();
        let cfg = Config::default();
        let spec = Spec::new("1.20.1", Loader::Fabric, "");
        let cancel = Arc::new(AtomicBool::new(false));
        let mut events = Vec::new();
        let added = crate::net::rt()
            .block_on(fill_missing(&crate::net::client(), &cfg, &root, &spec, &cancel, &mut |e| {
                if let ModEvent::File(f) = e {
                    events.push(f)
                }
            }))
            .unwrap();
        println!("added {added:?} files {events:?}");
        let jars = jar_names(&root);
        assert!(has_jar(&jars, &["ftb-quests"]), "{jars:?}");
        assert!(has_jar(&jars, &["ftb-library"]), "{jars:?}");
        assert!(has_jar(&jars, &["ftb-teams"]), "{jars:?}");
        assert!(!needs_fill(&root));
        let _ = std::fs::remove_dir_all(&root);
    }
}
