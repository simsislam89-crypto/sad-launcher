import io

p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()

# 1. constants + KINDS
src = src.replace('''pub const KIND_MAP: &str = "map";

/// Content kinds shown as chips; values feed search facets and dest dirs.
pub const KINDS: [(&str, &str); 4] = [
    (KIND_MOD, "Моды"),
    (KIND_SHADER, "Шейдеры"),
    (KIND_RESOURCEPACK, "Ресурспаки"),
    (KIND_MAP, "Карты"),
];''', '''pub const KIND_MAP: &str = "map";
pub const KIND_MODPACK: &str = "modpack";

/// Content kinds shown as chips; values feed search facets and dest dirs.
pub const KINDS: [(&str, &str); 5] = [
    (KIND_MOD, "Моды"),
    (KIND_SHADER, "Шейдеры"),
    (KIND_RESOURCEPACK, "Ресурспаки"),
    (KIND_MAP, "Карты"),
    (KIND_MODPACK, "Модпаки"),
];''')

# 2. pick_source: modpack routes by key
src = src.replace('''pub fn pick_source(source: &str, kind: &str, has_key: bool) -> &'static str {
    if kind == KIND_SHADER || kind == KIND_RESOURCEPACK {
        return "modrinth";
    }''', '''pub fn pick_source(source: &str, kind: &str, has_key: bool) -> &'static str {
    if kind == KIND_SHADER || kind == KIND_RESOURCEPACK {
        return "modrinth";
    }
    if kind == KIND_MODPACK {
        return if has_key { "curseforge" } else { "modrinth" };
    }''')

# 3. modrinth facets
src = src.replace('''    let project_type = match kind {
        KIND_SHADER => "shader",
        KIND_RESOURCEPACK => "resourcepack",
        _ => "mod",
    };
    let facets = format!(
        r#"[["project_type:{}"],["versions:{}"],["categories:{}"]]"#,
        project_type, mc, loader
    );''', '''    let project_type = match kind {
        KIND_SHADER => "shader",
        KIND_RESOURCEPACK => "resourcepack",
        KIND_MODPACK => "modpack",
        _ => "mod",
    };
    let facets = if kind == KIND_MODPACK {
        format!(r#"[["project_type:modpack"],["versions:{}"]]"#, mc)
    } else {
        format!(
            r#"[["project_type:{}"],["versions:{}"],["categories:{}"]]"#,
            project_type, mc, loader
        )
    };''')

# 4. curse classId
src = src.replace('''    // classId: 6 = mods, 12 = resource packs, 17 = worlds.
    let class_id = match kind {
        KIND_MAP => 17,
        KIND_RESOURCEPACK => 12,
        _ => 6,
    };''', '''    // classId: 6 = mods, 12 = resource packs, 17 = worlds, 4471 = modpacks.
    let class_id = match kind {
        KIND_MAP => 17,
        KIND_RESOURCEPACK => 12,
        KIND_MODPACK => 4471,
        _ => 6,
    };''')

# 5. route modpack install
old = '''pub async fn install<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    mc: &str,
    loader: &str,
    kind: &str,
    on: &mut F,
) -> Result<PathBuf> {
    let (url, filename) = if hit.source == "curseforge" {'''
new = '''pub async fn install<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    mc: &str,
    loader: &str,
    kind: &str,
    on: &mut F,
) -> Result<PathBuf> {
    if kind == KIND_MODPACK {
        return install_modpack(client, cfg, hit, mc, on).await;
    }
    let (url, filename) = if hit.source == "curseforge" {'''
assert old in src
src = src.replace(old, new, 1)

installer = '''// ---------------------------------------------------------------- modpacks

/// A modpack is not a single file: unpack its index and fetch every entry.
async fn install_modpack<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    mc: &str,
    on: &mut F,
) -> Result<PathBuf> {
    let (url, filename) = if hit.source == "curseforge" {
        curse_file(client, cfg, hit, mc, "", KIND_MODPACK).await?
    } else {
        modrinth_file(client, hit, mc, "", KIND_MODPACK).await?
    };
    let safe = sanitize(&filename);
    if safe.is_empty() {
        bail!("Не удалось определить имя пака");
    }
    let work = crate::config::data_dir().join("meta").join("packs");
    std::fs::create_dir_all(&work)?;
    let pack_file = work.join(&safe);
    on(ModEvent::Progress { done: 0, total: 0 });
    minecraft_dl(client, &url, &pack_file).await?;

    let extract_dir = work.join("unpacked");
    let _ = std::fs::remove_dir_all(&extract_dir);
    unzip(&pack_file, &extract_dir)?;

    let result = if filename.to_lowercase().ends_with(".mrpack") {
        install_mrpack(client, cfg, &extract_dir, on).await
    } else {
        install_cf_pack(client, cfg, &extract_dir, on).await
    };

    // The pack archive and its unpacked index are disposable.
    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::remove_file(&pack_file);
    result
}

async fn minecraft_dl(
    client: &reqwest::Client,
    url: &str,
    dest: &std::path::Path,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let resp = client
        .get(url)
        .send()
        .await
        .context("Файл пака недоступен")?
        .error_for_status()?;
    use futures_util::StreamExt;
    use std::io::Write;
    let mut file = std::fs::File::create(dest)?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        file.write_all(&chunk?)?;
    }
    Ok(())
}

fn unzip(archive: &std::path::Path, dest: &std::path::Path) -> Result<()> {
    let file = std::fs::File::open(archive)?;
    let mut zip = zip::ZipArchive::new(file).context("Архив пака не читается")?;
    std::fs::create_dir_all(dest)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf)?;
        std::fs::write(&out, buf)?;
    }
    Ok(())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> Result<u64> {
    let mut count = 0u64;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            count += copy_dir(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&from, &to)?;
            count += 1;
        }
    }
    Ok(count)
}

#[derive(Deserialize)]
struct MrpackIndex {
    files: Vec<MrpackFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
struct MrpackFile {
    path: String,
    downloads: Vec<String>,
}

async fn install_mrpack<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    unpacked: &std::path::Path,
    on: &mut F,
) -> Result<PathBuf> {
    let index_path = unpacked.join("modrinth.index.json");
    let text = std::fs::read_to_string(&index_path)
        .context("В паке нет modrinth.index.json — это не Modrinth-пак")?;
    let index: MrpackIndex =
        serde_json::from_str(&text).context("Индекс пака не читается")?;
    let total = index.files.len() as u64;

    let game = cfg.game_dir();
    for (i, file) in index.files.iter().enumerate() {
        let Some(url) = file.downloads.first() else {
            continue;
        };
        let dest = game.join(&file.path);
        minecraft_dl(client, url, &dest).await?;
        on(ModEvent::Progress {
            done: i as u64 + 1,
            total,
        });
    }

    let overrides = unpacked.join(index.overrides.as_deref().unwrap_or("overrides"));
    if overrides.is_dir() {
        let _ = copy_dir(&overrides, &game)?;
    }
    on(ModEvent::Progress {
        done: total,
        total,
    });
    Ok(game.join("mods"))
}

#[derive(Deserialize)]
struct CfManifest {
    files: Vec<CfFileRef>,
    #[serde(default = "default_overrides")]
    overrides: String,
}

fn default_overrides() -> String {
    "overrides".into()
}

#[derive(Deserialize)]
struct CfFileRef {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
}

async fn install_cf_pack<F: FnMut(ModEvent) + Send>(
    client: &reqwest::Client,
    cfg: &Config,
    unpacked: &std::path::Path,
    on: &mut F,
) -> Result<PathBuf> {
    if cfg.cf_api_key.trim().is_empty() {
        bail!("CurseForge-пак требует ключ API в настройках");
    }
    let manifest_path = unpacked.join("manifest.json");
    let text = std::fs::read_to_string(&manifest_path)
        .context("В паке нет manifest.json — это не CurseForge-пак")?;
    let manifest: CfManifest =
        serde_json::from_str(&text).context("Манифест пака не читается")?;
    let total = manifest.files.len() as u64;
    let mods_dir = cfg.mods_dir();
    std::fs::create_dir_all(&mods_dir)?;

    for (i, file) in manifest.files.iter().enumerate() {
        #[derive(Deserialize)]
        struct UrlResp {
            data: String,
        }
        let resolved: UrlResp = client
            .get(format!(
                "https://api.curseforge.com/v1/mods/{}/files/{}/download-url",
                file.project_id, file.file_id
            ))
            .header("x-api-key", cfg.cf_api_key.trim())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let name = resolved
            .data
            .rsplit('/')
            .next()
            .unwrap_or("mod.jar")
            .to_string();
        let dest = mods_dir.join(sanitize(&name));
        if !dest.exists() {
            minecraft_dl(client, &resolved.data, &dest).await?;
        }
        on(ModEvent::Progress {
            done: i as u64 + 1,
            total,
        });
    }

    let overrides = unpacked.join(&manifest.overrides);
    if overrides.is_dir() {
        copy_dir(&overrides, &cfg.game_dir())?;
    }
    on(ModEvent::Progress {
        done: total,
        total,
    });
    Ok(mods_dir)
}
'''

anchor = "fn sanitize(name: &str) -> String {"
assert anchor in src
src = src.replace(anchor, installer + "\n" + anchor, 1)

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("modpack installer ok")
