//! Minecraft: version manifest, installation pipeline, and offline UUID.
//! Launching lives in `launch.rs`.

use crate::config::{self, Config};
use crate::loader::{Loader, Spec};
use crate::shared::{LogKind, Shared};
use anyhow::{anyhow, bail, Context, Result};

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::Semaphore;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub const RESOURCES: &str = "https://resources.download.minecraft.net";
pub const LIBRARIES: &str = "https://libraries.minecraft.net";
pub const FORGE_PROMOS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";

pub fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(format!(
            "MinecraftLauncherMadrid/{} (Windows)",
            config::VERSION
        ))
        // No total timeout: big jars on slow links must finish. A stalled
        // read is what we cut off.
        .connect_timeout(std::time::Duration::from_secs(12))
        .read_timeout(std::time::Duration::from_secs(40))
        .build()
        .context("HTTP-клиент не создался")
}

// ---------------------------------------------------------------- manifest

#[derive(Clone, Serialize, Deserialize)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VersionSummary {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<VersionSummary>,
}

fn manifest_cache() -> PathBuf {
    let mut p = config::data_dir();
    p.push("meta");
    p.push("manifest.json");
    p
}

pub fn cached_manifest() -> Option<Manifest> {
    let text = std::fs::read_to_string(manifest_cache()).ok()?;
    serde_json::from_str(&text).ok()
}

pub async fn fetch_manifest(client: &reqwest::Client) -> Result<Manifest> {
    let manifest: Manifest = client
        .get(MANIFEST_URL)
        .send()
        .await
        .context("Манифест Mojang недоступен")?
        .error_for_status()?
        .json()
        .await
        .context("Манифест Mojang не читается")?;
    if let Ok(text) = serde_json::to_string(&manifest) {
        let _ = std::fs::write(manifest_cache(), text);
    }
    Ok(manifest)
}

pub fn spawn_manifest(shared: Arc<Shared>) {
    if shared.versions_loading.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::Builder::new()
        .name("mc-manifest".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build();
            let result = match rt {
                Ok(rt) => rt.block_on(async {
                    let client = http_client()?;
                    fetch_manifest(&client).await
                }),
                Err(e) => Err(anyhow!("{e}")),
            };
            match result {
                Ok(m) => {
                    let count = m.versions.len();
                    if let Ok(mut guard) = shared.versions.lock() {
                        *guard = m.versions;
                    }
                    if let Ok(mut err) = shared.versions_error.lock() {
                        err.clear();
                    }
                    shared.log(
                        LogKind::Info,
                        format!("Манифест Minecraft: {count} версий"),
                    );
                }
                Err(e) => {
                    if let Some(m) = cached_manifest() {
                        if let Ok(mut guard) = shared.versions.lock() {
                            *guard = m.versions;
                        }
                        shared.log(LogKind::Warn, "Манифест взят из кэша");
                    } else if let Ok(mut err) = shared.versions_error.lock() {
                        *err = e.to_string();
                    }
                }
            }
            shared.versions_loading.store(false, Ordering::SeqCst);
        })
        .ok();
}

// ---------------------------------------------------------------- installed

#[derive(Clone, Serialize)]
pub struct InstalledVersion {
    pub id: String,
    pub loader: String,
    pub mc: String,
    /// Exact loader build (Forge 47.4.28, Fabric 0.15.3); empty for vanilla.
    pub loader_version: String,
    pub size: u64,
    pub has_jar: bool,
}

#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Marker {
    pub loader: String,
    pub mc: String,
    #[serde(default)]
    pub loader_version: String,
    pub installed_at: String,
}

pub fn read_marker(game: &Path, id: &str) -> Option<Marker> {
    std::fs::read_to_string(game.join("versions").join(id).join("madrid.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
}

/// Loader, release and loader build of a version folder: the madrid.json
/// marker wins, the folder name is the fallback.
pub fn describe_version(game: &Path, id: &str) -> (Loader, String, String) {
    let parsed = crate::loader::parse_version_id(id);
    match read_marker(game, id) {
        Some(m) if !m.mc.is_empty() => {
            let loader = Loader::parse(&m.loader);
            let lv = if m.loader_version.is_empty() && loader == parsed.0 {
                parsed.2
            } else {
                m.loader_version
            };
            (loader, m.mc, lv)
        }
        _ => parsed,
    }
}

pub fn installed_versions(cfg: &Config) -> Vec<InstalledVersion> {
    let game = cfg.game_dir();
    let dir = game.join("versions");
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(id) = path.file_name().and_then(|s| s.to_str()).map(String::from) else {
            continue;
        };
        let json = path.join(format!("{id}.json"));
        if !json.exists() {
            continue;
        }
        let (loader, mc, loader_version) = describe_version(&game, &id);
        let jar = path.join(format!("{id}.jar"));
        let has_jar = jar.exists();
        let mut size = std::fs::metadata(&json).map(|m| m.len()).unwrap_or(0);
        if let Ok(m) = std::fs::metadata(&jar) {
            size += m.len();
        }
        out.push(InstalledVersion {
            id,
            loader: loader.id().to_string(),
            mc,
            loader_version,
            size,
            has_jar,
        });
    }
    out.sort_by(|a, b| b.mc.cmp(&a.mc).then(b.id.cmp(&a.id)));
    out
}

/// The installed list in the loader module's terms.
pub fn as_installed(list: &[InstalledVersion]) -> Vec<crate::loader::Installed> {
    list.iter()
        .map(|v| crate::loader::Installed {
            id: v.id.clone(),
            mc: v.mc.clone(),
            loader: Loader::parse(&v.loader),
            version: v.loader_version.clone(),
        })
        .collect()
}

// ---------------------------------------------------------------- version json

#[derive(Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub main_class: String,
    pub minecraft_arguments: Option<String>,
    pub arguments: Option<Arguments>,
    pub libraries: Vec<Library>,
    pub asset_index: Option<AssetIndexRef>,
    pub assets: Option<String>,
    pub downloads: Option<Downloads>,
    pub java_version: Option<JavaVersion>,
    pub inherits_from: Option<String>,
    pub jar: Option<String>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct Arguments {
    pub game: Vec<ArgItem>,
    pub jvm: Vec<ArgItem>,
}

#[derive(Clone, Deserialize)]
#[serde(untagged)]
pub enum ArgItem {
    Plain(String),
    Conditional {
        #[serde(default)]
        rules: Vec<Rule>,
        value: serde_json::Value,
    },
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct Rule {
    pub action: String,
    pub os: Option<OsRule>,
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
    pub version: Option<String>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct Library {
    pub name: String,
    pub url: Option<String>,
    pub rules: Vec<Rule>,
    pub downloads: Option<LibDownloads>,
    pub natives: Option<HashMap<String, String>>,
    pub extract: Option<Extract>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub url: String,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct Extract {
    pub exclude: Vec<String>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct Downloads {
    pub client: Option<Artifact>,
    pub server: Option<Artifact>,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: String,
    pub major_version: u32,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct AssetIndex {
    pub objects: HashMap<String, AssetObject>,
    #[serde(rename = "virtual")]
    pub virtual_: bool,
    pub map_to_resources: bool,
}

#[derive(Clone, Deserialize, Default)]
#[serde(default)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

pub fn rules_allow(rules: &[Rule]) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule in rules {
        if !rule_matches(rule) {
            continue;
        }
        allow = rule.action == "allow";
    }
    allow
}

fn rule_matches(rule: &Rule) -> bool {
    if let Some(os) = &rule.os {
        if let Some(name) = &os.name {
            if name != crate::platform::MOJANG_OS {
                return false;
            }
        }
        if let Some(arch) = &os.arch {
            if arch != crate::platform::MOJANG_ARCH {
                return false;
            }
        }
        if os.version.is_some() {
            return false;
        }
    }
    if let Some(features) = &rule.features {
        for wanted in features.values() {
            if *wanted {
                return false;
            }
        }
    }
    true
}

pub fn maven_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let (group, artifact, version) = (parts[0], parts[1], parts[2]);
    let classifier = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Some(format!(
        "{}/{}/{}/{}-{}{}.jar",
        group.replace('.', "/"),
        artifact,
        version,
        artifact,
        version,
        classifier
    ))
}

pub fn ensure_layout(game: &Path) {
    for sub in ["versions", "libraries", "assets/indexes", "assets/objects", "mods"] {
        let _ = std::fs::create_dir_all(game.join(sub));
    }
}

pub fn offline_uuid(name: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{name}").as_bytes());
    let mut d = hasher.finalize();
    d[6] = (d[6] & 0x0f) | 0x30;
    d[8] = (d[8] & 0x3f) | 0x80;
    let h: Vec<String> = d.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        h[0..4].concat(),
        h[4..6].concat(),
        h[6..8].concat(),
        h[8..10].concat(),
        h[10..16].concat()
    )
}

// ---------------------------------------------------------------- install

#[derive(Clone)]
pub enum InstallEvent {
    Stage(String),
    Progress {
        done: u64,
        total: u64,
        label: String,
    },
    Done {
        version: String,
    },
    Error(String),
}

/// Installs everything a build needs: the vanilla release, a Java that
/// can run it (downloaded when missing) and the loader the build asks for.
/// Idempotent — files already on disk are skipped.
pub async fn install<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    spec: &Spec,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<String> {
    let game = cfg.game_dir();
    ensure_layout(&game);
    let mc = spec.mc.trim();
    if mc.is_empty() {
        bail!("Не указана версия Minecraft");
    }
    let version = install_vanilla(client, &game, mc, cancel, on).await?;
    let java = ensure_java(client, cfg, java_major_for(&version), spec.loader, cancel, on).await?;
    let (final_id, loader_version) = match spec.loader {
        Loader::Vanilla => (mc.to_string(), String::new()),
        Loader::Forge => install_forge(client, &game, mc, &spec.version, &java, cancel, on).await?,
        Loader::NeoForge => {
            install_neoforge(client, &game, mc, &spec.version, &java, cancel, on).await?
        }
        Loader::Fabric | Loader::Quilt => install_meta_loader(client, &game, spec, cancel, on).await?,
    };

    let marker = Marker {
        loader: spec.loader.id().to_string(),
        mc: mc.to_string(),
        loader_version,
        installed_at: crate::util::now_stamp(),
    };
    let final_dir = game.join("versions").join(&final_id);
    std::fs::create_dir_all(&final_dir)?;
    if let Ok(text) = serde_json::to_string_pretty(&marker) {
        let _ = std::fs::write(final_dir.join("madrid.json"), text);
    }

    on(InstallEvent::Done {
        version: final_id.clone(),
    });
    Ok(final_id)
}

async fn install_vanilla<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    game: &Path,
    mc: &str,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<VersionJson> {
    on(InstallEvent::Stage(format!("Minecraft {mc}")));
    let version_dir = game.join("versions").join(mc);
    std::fs::create_dir_all(&version_dir)?;
    let json_path = version_dir.join(format!("{mc}.json"));
    let cached = std::fs::read_to_string(&json_path)
        .ok()
        .filter(|t| serde_json::from_str::<VersionJson>(t).is_ok());
    let text = match cached {
        Some(t) => t,
        None => {
            let url = version_url(client, mc).await?;
            let text = client
                .get(&url)
                .send()
                .await
                .context("Файл версии недоступен")?
                .error_for_status()?
                .text()
                .await?;
            write_atomic(&json_path, text.as_bytes())?;
            text
        }
    };
    let version: VersionJson = serde_json::from_str(&text).context("Файл версии не читается")?;

    if let Some(dl) = version.downloads.as_ref().and_then(|d| d.client.as_ref()) {
        let jar = version_dir.join(format!("{mc}.jar"));
        if !jar.exists() {
            on(InstallEvent::Stage("Клиент Minecraft".into()));
            let mut report = |done: u64, total: u64| {
                on(InstallEvent::Progress {
                    done,
                    total,
                    label: format!("Клиент Minecraft · {}", crate::util::human_bytes(done)),
                })
            };
            download_file_progress(client, &dl.url, &jar, Some(cancel), &mut report).await?;
        }
    }

    install_libraries(client, game, &version, mc, cancel, on).await?;
    install_assets(client, game, &version, cancel, on).await?;
    Ok(version)
}

/// A Java the build can run on; Temurin is downloaded when none fits.
async fn ensure_java<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    required: u32,
    loader: Loader,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<crate::java::JavaInfo> {
    if let Some(j) = crate::java::pick_fitting(&cfg.java_path, required, loader) {
        return Ok(j);
    }
    on(InstallEvent::Stage(format!("Java {required} — скачиваю сам")));
    // Some majors have no Windows build any more (Java 16): the next LTS
    // the loader accepts runs the same game.
    let mut majors = vec![required];
    for lts in [17u32, 21, 25] {
        if lts > required && crate::loader::java_fits(lts, required, loader) {
            majors.push(lts);
            break;
        }
    }
    let mut last_err = None;
    for major in majors {
        let mut forward = |ev: crate::java::JavaEvent| {
            if let crate::java::JavaEvent::Progress { done, total } = ev {
                on(InstallEvent::Progress {
                    done,
                    total,
                    label: format!("Java {major} · {}", crate::util::human_bytes(done)),
                });
            }
        };
        match crate::java::download_runtime(client, major, cancel, &mut forward).await {
            Ok(_) => {
                last_err = None;
                break;
            }
            Err(e) if e.to_string().contains("Отменено") => return Err(e),
            Err(e) => last_err = Some(e),
        }
    }
    if let Some(e) = last_err {
        return Err(e);
    }
    crate::java::pick_fitting(&cfg.java_path, required, loader)
        .ok_or_else(|| anyhow!("Java {required} не нашлась после загрузки"))
}

async fn version_url(client: &reqwest::Client, id: &str) -> Result<String> {
    if let Some(m) = cached_manifest() {
        if let Some(v) = m.versions.iter().find(|v| v.id == id) {
            return Ok(v.url.clone());
        }
    }
    // Cache may predate a new release — ask Mojang directly.
    let manifest = fetch_manifest(client).await?;
    manifest
        .versions
        .iter()
        .find(|v| v.id == id)
        .map(|v| v.url.clone())
        .ok_or_else(|| anyhow!("Версия {id} не найдена в манифесте"))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------- forge

/// Latest Forge build for a Minecraft release, via the official promotions list.
pub async fn forge_build(client: &reqwest::Client, mc: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Promos {
        promos: HashMap<String, String>,
    }
    let p: Promos = client
        .get(FORGE_PROMOS)
        .send()
        .await
        .context("Список сборок Forge недоступен")?
        .error_for_status()?
        .json()
        .await
        .context("Список сборок Forge не читается")?;
    let rec = format!("{mc}-recommended");
    let lat = format!("{mc}-latest");
    if let Some(b) = p.promos.get(&rec) {
        return Ok(b.clone());
    }
    if let Some(b) = p.promos.get(&lat) {
        return Ok(b.clone());
    }
    bail!("Для {mc} нет сборки Forge");
}

/// Cache path for a loader installer jar.
pub fn installer_path(name: &str) -> PathBuf {
    config::data_dir().join("meta").join("forge").join(name)
}

/// Downloads the first installer URL that answers; cached between runs.
async fn fetch_installer(
    client: &reqwest::Client,
    name: &str,
    urls: &[String],
) -> Result<PathBuf> {
    let dest = installer_path(name);
    if std::fs::metadata(&dest).map(|m| m.len() > 1000).unwrap_or(false) {
        return Ok(dest);
    }
    let mut last = None;
    for url in urls {
        match download_file(client, url, &dest, None).await {
            Ok(()) => return Ok(dest),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("нет адреса")))
        .with_context(|| format!("Установщик {name} не скачался"))
}

/// The official Forge installer for an exact build (shared with the server).
pub async fn forge_installer(client: &reqwest::Client, mc: &str, build: &str) -> Result<PathBuf> {
    fetch_installer(
        client,
        &format!("forge-{mc}-{build}-installer.jar"),
        &[
            format!("{FORGE_MAVEN}/{mc}-{build}/forge-{mc}-{build}-installer.jar"),
            format!("{FORGE_MAVEN}/{mc}-{build}-{mc}/forge-{mc}-{build}-{mc}-installer.jar"),
        ],
    )
    .await
}

fn ensure_launcher_profiles(game: &Path) {
    let path = game.join("launcher_profiles.json");
    if !path.exists() {
        let _ = std::fs::write(path, "{\"profiles\":{}}");
    }
}

/// Version folder already holding this loader build, if any.
fn find_installed_id(game: &Path, loader: Loader, mc: &str, version: &str) -> Option<String> {
    let entries = std::fs::read_dir(game.join("versions")).ok()?;
    entries.flatten().find_map(|e| {
        let id = e.file_name().into_string().ok()?;
        if !e.path().join(format!("{id}.json")).exists() {
            return None;
        }
        let (l, m, v) = describe_version(game, &id);
        (l == loader && m == mc && v == version).then_some(id)
    })
}

fn version_dirs(game: &Path) -> std::collections::HashSet<String> {
    std::fs::read_dir(game.join("versions"))
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Installs the Forge client by driving the official installer headless
/// (`--installClient`). Legacy installers (≤ 1.12.2) are unpacked by hand.
async fn install_forge<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    game: &Path,
    mc: &str,
    version: &str,
    java: &crate::java::JavaInfo,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<(String, String)> {
    let build = if version.is_empty() {
        forge_build(client, mc).await?
    } else {
        version.to_string()
    };
    if let Some(id) = find_installed_id(game, Loader::Forge, mc, &build) {
        on(InstallEvent::Stage(format!("Forge {build} уже установлен")));
        return Ok((id, build));
    }
    let expected = format!("{mc}-forge-{build}");
    if game.join("versions").join(&expected).join(format!("{expected}.json")).exists() {
        return Ok((expected, build));
    }
    on(InstallEvent::Stage(format!("Установщик Forge {mc}-{build}")));
    let installer = forge_installer(client, mc, &build).await?;
    let id = run_installer(client, game, &installer, java, &["--installClient"], &expected, cancel, on).await?;
    Ok((id, build))
}

/// NeoForge: its own maven; MC 1.20.1 builds live under the `forge` artifact.
async fn install_neoforge<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    game: &Path,
    mc: &str,
    version: &str,
    java: &crate::java::JavaInfo,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<(String, String)> {
    const NEO: &str = "https://maven.neoforged.net/releases/net/neoforged";
    let legacy = crate::loader::neoforge_prefix(mc).is_none();
    let v = if version.is_empty() {
        neoforge_latest(client, mc).await?
    } else {
        version.to_string()
    };
    if let Some(id) = find_installed_id(game, Loader::NeoForge, mc, &v) {
        on(InstallEvent::Stage(format!("NeoForge {v} уже установлен")));
        return Ok((id, v));
    }
    let (name, url, expected) = if legacy {
        (
            format!("neoforge-{mc}-{v}-installer.jar"),
            format!("{NEO}/forge/{mc}-{v}/forge-{mc}-{v}-installer.jar"),
            format!("{mc}-forge-{v}"),
        )
    } else {
        (
            format!("neoforge-{v}-installer.jar"),
            format!("{NEO}/neoforge/{v}/neoforge-{v}-installer.jar"),
            format!("neoforge-{v}"),
        )
    };
    on(InstallEvent::Stage(format!("Установщик NeoForge {v}")));
    let installer = fetch_installer(client, &name, &[url]).await?;
    let id = run_installer(
        client,
        game,
        &installer,
        java,
        &["--install-client", "--installClient"],
        &expected,
        cancel,
        on,
    )
    .await?;
    Ok((id, v))
}

async fn neoforge_latest(client: &reqwest::Client, mc: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Versions {
        versions: Vec<String>,
    }
    let api = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged";
    let (url, prefix, strip) = match crate::loader::neoforge_prefix(mc) {
        Some(p) => (format!("{api}/neoforge"), p, String::new()),
        None => (format!("{api}/forge"), format!("{mc}-"), format!("{mc}-")),
    };
    let list: Versions = client
        .get(url)
        .send()
        .await
        .context("NeoForge не отвечает")?
        .error_for_status()?
        .json()
        .await
        .context("Список NeoForge не читается")?;
    let matching: Vec<&String> = list.versions.iter().filter(|v| v.starts_with(&prefix)).collect();
    let best = matching
        .iter()
        .rev()
        .find(|v| !v.contains("beta") && !v.contains("alpha"))
        .or_else(|| matching.last())
        .ok_or_else(|| anyhow!("Для {mc} нет сборки NeoForge"))?;
    Ok(best.strip_prefix(strip.as_str()).unwrap_or(best).to_string())
}

/// Runs a Forge-family installer headless and returns the version id it
/// created. Tries each CLI flag spelling until one succeeds.
#[allow(clippy::too_many_arguments)]
async fn run_installer<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    game: &Path,
    installer: &Path,
    java: &crate::java::JavaInfo,
    flags: &[&str],
    expected: &str,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<String> {
    if let Some(id) = legacy_forge_install(game, installer)? {
        let text = std::fs::read_to_string(game.join("versions").join(&id).join(format!("{id}.json")))?;
        let version: VersionJson = serde_json::from_str(&text).context("Профиль Forge не читается")?;
        install_libraries(client, game, &version, &id, cancel, on).await?;
        let _ = std::fs::remove_file(installer);
        return Ok(id);
    }

    ensure_launcher_profiles(game);
    let before = version_dirs(game);
    on(InstallEvent::Stage("Установщик загрузчика работает (до пары минут)…".into()));
    let mut last_log = String::new();
    for flag in flags {
        let java_path = java.path.clone();
        let args = vec![
            "-jar".to_string(),
            installer.display().to_string(),
            flag.to_string(),
            game.display().to_string(),
        ];
        let out = tokio::task::spawn_blocking(move || crate::java::run_logged(&java_path, &args))
            .await
            .context("Установщик прервался")??;
        if !out.status.success() {
            last_log = out.log;
            continue;
        }
        let after = version_dirs(game);
        let created = after
            .difference(&before)
            .find(|id| game.join("versions").join(id).join(format!("{id}.json")).exists())
            .cloned();
        let id = match created {
            Some(id) => id,
            None if game.join("versions").join(expected).join(format!("{expected}.json")).exists() => {
                expected.to_string()
            }
            None => bail!("Установщик не создал профиль версии"),
        };
        // The installer jar is re-downloadable — keep the disk lean.
        let _ = std::fs::remove_file(installer);
        return Ok(id);
    }
    let tail: Vec<&str> = last_log.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = tail.iter().rev().take(6).rev().cloned().collect::<Vec<_>>();
    bail!("Установщик загрузчика завершился ошибкой:\n{}", tail.join("\n"))
}

/// Pre-1.13 Forge installers carry the whole version JSON inside
/// install_profile.json; unpack it and the universal jar directly.
fn legacy_forge_install(game: &Path, installer: &Path) -> Result<Option<String>> {
    let file = File::open(installer)?;
    let mut zip = zip::ZipArchive::new(file).context("Установщик Forge битый")?;
    let profile: serde_json::Value = {
        let Ok(mut entry) = zip.by_name("install_profile.json") else {
            return Ok(None);
        };
        let mut text = String::new();
        entry.read_to_string(&mut text)?;
        serde_json::from_str(&text).context("install_profile.json не читается")?
    };
    let Some(info) = profile.get("versionInfo") else {
        return Ok(None);
    };
    let id = info
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("В профиле Forge нет id"))?
        .to_string();
    let dir = game.join("versions").join(&id);
    std::fs::create_dir_all(&dir)?;
    write_atomic(&dir.join(format!("{id}.json")), serde_json::to_string_pretty(info)?.as_bytes())?;
    let install = profile.get("install");
    let coords = install.and_then(|i| i.get("path")).and_then(|v| v.as_str());
    let inner = install.and_then(|i| i.get("filePath")).and_then(|v| v.as_str());
    if let (Some(coords), Some(inner)) = (coords, inner) {
        if let Some(rel) = maven_path(coords) {
            let dest = game.join("libraries").join(rel);
            if !dest.exists() {
                let mut entry = zip.by_name(inner).context("В установщике нет universal-jar")?;
                let mut buf = Vec::with_capacity(entry.size() as usize);
                entry.read_to_end(&mut buf)?;
                write_atomic(&dest, &buf)?;
            }
        }
    }
    Ok(Some(id))
}

/// Fabric and Quilt publish ready launcher profiles on their meta servers.
async fn install_meta_loader<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    game: &Path,
    spec: &Spec,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<(String, String)> {
    let (meta, prefix) = match spec.loader {
        Loader::Quilt => ("https://meta.quiltmc.org/v3", "quilt-loader"),
        _ => ("https://meta.fabricmc.net/v2", "fabric-loader"),
    };
    let label = spec.loader.label();
    let mc = spec.mc.as_str();
    let lv = if spec.version.is_empty() {
        #[derive(Deserialize)]
        struct Entry {
            loader: LoaderRef,
        }
        #[derive(Deserialize)]
        struct LoaderRef {
            version: String,
            #[serde(default)]
            stable: Option<bool>,
        }
        let list: Vec<Entry> = client
            .get(format!("{meta}/versions/loader/{mc}"))
            .send()
            .await
            .with_context(|| format!("{label} не отвечает"))?
            .error_for_status()?
            .json()
            .await
            .with_context(|| format!("Список {label} не читается"))?;
        list.iter()
            .find(|e| e.loader.stable.unwrap_or(true))
            .or_else(|| list.first())
            .map(|e| e.loader.version.clone())
            .ok_or_else(|| anyhow!("{label} не поддерживает Minecraft {mc}"))?
    } else {
        spec.version.clone()
    };
    on(InstallEvent::Stage(format!("{label} {lv}")));
    let guess = format!("{prefix}-{lv}-{mc}");
    let guess_path = game.join("versions").join(&guess).join(format!("{guess}.json"));
    let text = match std::fs::read_to_string(&guess_path) {
        Ok(t) => t,
        Err(_) => client
            .get(format!("{meta}/versions/loader/{mc}/{lv}/profile/json"))
            .send()
            .await
            .with_context(|| format!("{label} не отвечает"))?
            .error_for_status()
            .with_context(|| format!("{label} {lv} не поддерживает Minecraft {mc}"))?
            .text()
            .await?,
    };
    let version: VersionJson = serde_json::from_str(&text)
        .with_context(|| format!("Профиль {label} не читается"))?;
    let id = if version.id.is_empty() { guess } else { version.id.clone() };
    write_atomic(
        &game.join("versions").join(&id).join(format!("{id}.json")),
        text.as_bytes(),
    )?;
    install_libraries(client, game, &version, &id, cancel, on).await?;
    Ok((id, lv))
}

// Server files: eula accepted, offline mode on, our port, no GUI surprises.
pub fn prepare_server_dir(server: &Path, port: u16) {
    let _ = std::fs::create_dir_all(server.join("mods"));
    let _ = std::fs::write(server.join("eula.txt"), "# accepted by Sad Launcher\neula=true\n");
    // autosave every 20 s (in ticks): a sudden power cut loses seconds, not the world
    let props = format!(
        "# managed by Sad Launcher\nonline-mode=false\nserver-port={port}\nquery.port={port}\nmotd=Madrid · localhost\nwhite-list=false\nspawn-protection=0\nview-distance=8\nmax-players=20\nautosave-interval=400\nsync-chunk-writes=true\n"
    );
    let path = server.join("server.properties");
    if !path.exists() {
        let _ = std::fs::write(path, props);
    }
}

/// Patches the two lines we own in server.properties, leaving the rest untouched.
pub fn patch_server_props(server: &Path, port: u16) {
    let path = server.join("server.properties");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let line = if line.starts_with("server-port=") || line.starts_with("query.port=") {
            let key = line.split('=').next().unwrap_or("server-port");
            format!("{key}={port}")
        } else if line.starts_with("online-mode=") {
            "online-mode=false".to_string()
        } else {
            line.to_string()
        };
        out.push_str(&line);
        out.push('\n');
    }
    let _ = std::fs::write(path, out);
}

/// Copies Madrid mod jars from the client mods dir into the server.
pub fn sync_server_mods(server: &Path, client_mods: &Path) -> u32 {
    let dest = server.join("mods");
    let _ = std::fs::create_dir_all(&dest);
    let mut count = 0u32;
    if let Ok(entries) = std::fs::read_dir(client_mods) {
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !name.to_lowercase().ends_with(".jar") {
                continue;
            }
            let to = dest.join(name);
            let fresh = match (entry.metadata(), std::fs::metadata(&to)) {
                (Ok(a), Ok(b)) => {
                    let a: std::time::SystemTime = a.modified().unwrap_or(std::time::UNIX_EPOCH);
                    let b: std::time::SystemTime = b.modified().unwrap_or(std::time::UNIX_EPOCH);
                    a != b
                }
                (Ok(_), Err(_)) => true,
                _ => false,
            };
            if fresh {
                let _ = std::fs::copy(&path, &to);
            }
            count += 1;
        }
    }
    // Mods switched off in the client are switched off on the server too.
    if let Ok(entries) = std::fs::read_dir(&dest) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.to_lowercase().ends_with(".jar") && client_mods.join(format!("{name}.disabled")).exists() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    count
}


async fn install_libraries<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    game: &Path,
    version: &VersionJson,
    id: &str,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<()> {
    let natives_dir = game.join("versions").join(id).join("natives");
    std::fs::create_dir_all(&natives_dir)?;

    // Collect every missing file first, then fetch them in parallel.
    let mut jobs: Vec<(String, PathBuf)> = Vec::new();
    let mut natives: Vec<(PathBuf, Option<Extract>)> = Vec::new();
    for lib in version.libraries.iter().filter(|l| rules_allow(&l.rules)) {
        let artifact = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref());
        let rel = artifact
            .and_then(|a| a.path.clone())
            .or_else(|| maven_path(&lib.name));
        if let Some(path) = rel {
            let dest = game.join("libraries").join(&path);
            if !dest.exists() {
                let url = artifact
                    .map(|a| a.url.clone())
                    .filter(|u| !u.is_empty())
                    .or_else(|| {
                        lib.url
                            .as_ref()
                            .filter(|b| !b.is_empty())
                            .map(|base| format!("{}/{}", base.trim_end_matches('/'), path))
                    })
                    .unwrap_or_else(|| format!("{LIBRARIES}/{path}"));
                jobs.push((url, dest));
            }
        }
        if let Some(template) = lib.natives.as_ref().and_then(|n| n.get(crate::platform::MOJANG_OS)) {
            let key = template.replace("${arch}", "64");
            let art = lib
                .downloads
                .as_ref()
                .and_then(|d| d.classifiers.as_ref())
                .and_then(|c| c.get(&key))
                .cloned();
            let path = art
                .as_ref()
                .and_then(|a| a.path.clone())
                .or_else(|| maven_path(&format!("{}:{key}", lib.name)))
                .unwrap_or_default();
            if !path.is_empty() {
                let dest = game.join("libraries").join(&path);
                if !dest.exists() {
                    let url = art
                        .map(|a| a.url)
                        .filter(|u| !u.is_empty())
                        .unwrap_or_else(|| format!("{LIBRARIES}/{path}"));
                    jobs.push((url, dest.clone()));
                }
                natives.push((dest, lib.extract.clone()));
            }
        }
    }

    let total = jobs.len() as u64;
    if total > 0 {
        on(InstallEvent::Stage(format!("Библиотеки · {total}")));
        let sem = Arc::new(Semaphore::new(32));
        let mut set = tokio::task::JoinSet::new();
        for (url, dest) in jobs {
            if cancel.load(Ordering::Relaxed) {
                bail!("Отменено");
            }
            let client = client.clone();
            let permit = sem.clone().acquire_owned().await?;
            set.spawn(async move {
                let _permit = permit;
                // Some loader profiles list jars their installer generates
                // locally; a 404 there is expected, not fatal.
                let _ = download_file(&client, &url, &dest, None).await;
            });
        }
        let mut done = 0u64;
        while set.join_next().await.is_some() {
            done += 1;
            on(InstallEvent::Progress {
                done,
                total,
                label: format!("Библиотеки · {done}/{total}"),
            });
        }
    }
    for (jar, extract) in natives {
        if jar.exists() {
            let _ = extract_natives(&jar, &natives_dir, extract.as_ref());
        }
    }
    Ok(())
}

async fn install_assets<F: FnMut(InstallEvent) + Send>(
    client: &reqwest::Client,
    game: &Path,
    version: &VersionJson,
    cancel: &std::sync::atomic::AtomicBool,
    on: &mut F,
) -> Result<()> {
    let Some(index_ref) = &version.asset_index else {
        return Ok(());
    };
    let index_path = game
        .join("assets")
        .join("indexes")
        .join(format!("{}.json", index_ref.id));
    if !index_path.exists() {
        on(InstallEvent::Stage("Индекс ресурсов".into()));
        download_file(client, &index_ref.url, &index_path, Some(cancel)).await?;
    }
    let text = std::fs::read_to_string(&index_path)?;
    let index: AssetIndex = serde_json::from_str(&text).context("Индекс ресурсов не читается")?;
    let total = index.objects.len() as u64;
    if total == 0 {
        return Ok(());
    }
    on(InstallEvent::Stage(format!("Ресурсы · {total}")));

    let objects_dir = game.join("assets").join("objects");
    // Thousands of tiny files: latency, not bandwidth, is the limit —
    // many requests in flight on pooled connections hide it.
    // 128 in flight measured best (277 → 323 files/s vs 64; 192 adds nothing).
    let sem = Arc::new(Semaphore::new(128));
    let mut set = tokio::task::JoinSet::new();
    let mut done = 0u64;
    for object in index.objects.values() {
        if object.hash.len() < 2 {
            done += 1;
            continue;
        }
        let sub = object.hash[..2].to_string();
        let dest = objects_dir.join(&sub).join(&object.hash);
        if dest.exists() {
            done += 1;
            continue;
        }
        if cancel.load(Ordering::Relaxed) {
            bail!("Отменено");
        }
        let url = format!("{RESOURCES}/{sub}/{}", object.hash);
        let client = client.clone();
        let permit = sem.clone().acquire_owned().await?;
        set.spawn(async move {
            let _permit = permit;
            let _ = download_file(&client, &url, &dest, None).await;
        });
        if set.len() >= 256 {
            set.join_next().await;
            done += 1;
            if done % 32 == 0 {
                on(InstallEvent::Progress {
                    done,
                    total,
                    label: format!("Ресурсы · {done}/{total}"),
                });
            }
        }
    }
    // The tail of the queue reports too (the bar used to freeze here for
    // the last ~128 files).
    while set.join_next().await.is_some() {
        done += 1;
        if done % 32 == 0 {
            on(InstallEvent::Progress {
                done,
                total,
                label: format!("Ресурсы · {done}/{total}"),
            });
        }
    }
    on(InstallEvent::Progress {
        done: total,
        total,
        label: "Ресурсы готовы".into(),
    });

    if index.virtual_ || index.map_to_resources {
        let target = if index.map_to_resources {
            game.join("resources")
        } else {
            game.join("assets").join("virtual").join(&index_ref.id)
        };
        for (name, object) in &index.objects {
            if object.hash.len() < 2 {
                continue;
            }
            let src = objects_dir.join(&object.hash[..2]).join(&object.hash);
            let dst = target.join(name);
            if dst.exists() {
                continue;
            }
            if let Some(parent) = dst.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(src, dst);
        }
    }
    Ok(())
}

pub async fn download_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<()> {
    if let Ok(meta) = std::fs::metadata(dest) {
        if meta.len() > 0 {
            return Ok(());
        }
    }
    // Flaky links drop a request now and then; a 4xx is final, the rest retry.
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match download_once(client, url, dest, cancel).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let fatal = e.to_string().contains("Отменено")
                    || e
                        .downcast_ref::<reqwest::Error>()
                        .and_then(|re| re.status())
                        .map(|s| s.is_client_error())
                        .unwrap_or(false);
                if fatal || attempt >= 3 {
                    return Err(e);
                }
                tokio::time::sleep(std::time::Duration::from_millis(400 * attempt as u64)).await;
            }
        }
    }
}

/// `name.jar` → `name.jar.part`: unique per target, never shared by two files.
pub fn part_path(dest: &Path) -> PathBuf {
    crate::net::part_path(dest)
}

async fn download_once(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<()> {
    crate::net::fetch_to(client, url, dest, cancel, &mut |_, _| {}).await
}

/// Like [`download_file`], reporting `(done, total)` bytes while it runs —
/// used for the big single files (client jar, pack archive, Java).
pub async fn download_file_progress(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    progress: &mut (dyn FnMut(u64, u64) + Send),
) -> Result<()> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match crate::net::fetch_to(client, url, dest, cancel, progress).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                let fatal = e.to_string().contains("Отменено")
                    || e
                        .downcast_ref::<reqwest::Error>()
                        .and_then(|re| re.status())
                        .map(|s| s.is_client_error())
                        .unwrap_or(false);
                if fatal || attempt >= 3 {
                    return Err(e);
                }
                tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
            }
        }
    }
}

pub fn extract_natives(jar: &Path, dest: &Path, extract: Option<&Extract>) -> Result<()> {
    let file = File::open(jar)?;
    let mut zip = zip::ZipArchive::new(file)?;
    std::fs::create_dir_all(dest)?;
    let excludes = extract.map(|e| e.exclude.clone()).unwrap_or_default();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.starts_with("META-INF/") {
            continue;
        }
        if excludes.iter().any(|e| name.starts_with(e)) {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        std::fs::write(&out, buf)?;
    }
    Ok(())
}

pub fn java_major_for(version: &VersionJson) -> u32 {
    version
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .filter(|m| *m > 0)
        .unwrap_or_else(|| crate::java::required_for(&version.id))
}

/// Java major needed by an installed build, read from its version JSON.
pub fn required_java(cfg: &Config, id: &str) -> u32 {
    let path = cfg
        .game_dir()
        .join("versions")
        .join(id)
        .join(format!("{id}.json"));
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(version) = serde_json::from_str::<VersionJson>(&text) {
            if let Some(java) = version.java_version {
                if java.major_version > 0 {
                    return java.major_version;
                }
            }
        }
    }
    let mc = installed_versions(cfg)
        .iter()
        .find(|v| v.id == id)
        .map(|v| v.mc.clone())
        .unwrap_or_else(|| id.to_string());
    crate::java::required_for(&mc)
}
