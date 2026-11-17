import io

# ================= mods.rs: refuse fabric packs on the Forge Madrid profile =================
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()

# mrpack: check dependencies — fabric packs must not enter a Forge profile
src = src.replace('''    let index: MrpackIndex =
        serde_json::from_str(&text).context("Индекс пака не читается")?;
    let total = index.files.len() as u64;''', '''    let index: MrpackIndex =
        serde_json::from_str(&text).context("Индекс пака не читается")?;
    // A Fabric pack cannot run on the Forge Madrid profile — refuse early
    // instead of shipping 300 jars that crash ModLauncher.
    if index
        .dependencies
        .keys()
        .any(|d| d.contains("fabric"))
    {
        bail!("Этот пак под Fabric — Мадрид работает на Forge. Ищи Forge-версию пака или поставь Fabric-профиль.");
    }
    let total = index.files.len() as u64;''')

src = src.replace('''#[derive(Deserialize)]
struct MrpackIndex {
    files: Vec<MrpackFile>,
    #[serde(default)]
    overrides: Option<String>,
}''', '''#[derive(Deserialize)]
struct MrpackIndex {
    files: Vec<MrpackFile>,
    #[serde(default)]
    overrides: Option<String>,
    #[serde(default)]
    dependencies: std::collections::HashMap<String, String>,
}''')

# CF pack manifest: modLoader 4 = Fabric — refuse too
src = src.replace('''#[derive(Deserialize)]
struct CfManifest {
    files: Vec<CfFileRef>,
    #[serde(default = "default_overrides")]
    overrides: String,
}''', '''#[derive(Deserialize)]
struct CfManifest {
    files: Vec<CfFileRef>,
    #[serde(default = "default_overrides")]
    overrides: String,
    /// CF modLoader: 0 = any, 1 = Forge, 4 = Fabric.
    #[serde(default)]
    mod_loader: i32,
}''')
src = src.replace('''    #[serde(default, rename = "downloadUrl")]
            download_url: Option<String>,
        }''', '''    #[serde(default, rename = "downloadUrl")]
            download_url: Option<String>,
        }''')
src = src.replace('''    let text = std::fs::read_to_string(&manifest_path)
        .context("В паке нет manifest.json — это не CurseForge-пак")?;
    let manifest: CfManifest =
        serde_json::from_str(&text).context("Манифест пака не читается")?;''', '''    let text = std::fs::read_to_string(&manifest_path)
        .context("В паке нет manifest.json — это не CurseForge-пак")?;
    let manifest: CfManifest =
        serde_json::from_str(&text).context("Манифест пака не читается")?;
    if manifest.mod_loader == 4 {
        bail!("Этот пак под Fabric — Мадрид работает на Forge. Ищи Forge-версию пака.");
    }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("pack compatibility ok")

# ================= launch.rs: self-heal broken mod on ServiceConfigurationError =================
p = "src/launch.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''                Ok(Some(status)) => {
                    drop(guard);
                    shared.game_running.store(false, Ordering::SeqCst);
                    shared.push_game(format!("[Madrid] Игра завершилась · {status}"));
                    if let Ok(mut g) = shared.game.lock() {
                        *g = None;
                    }
                    break;
                }''', '''                Ok(Some(status)) => {
                    drop(guard);
                    shared.game_running.store(false, Ordering::SeqCst);
                    shared.push_game(format!("[Madrid] Игра завершилась · {status}"));
                    if status.code() == Some(1) {
                        heal_broken_mod(shared, &log_path);
                    }
                    if let Ok(mut g) = shared.game.lock() {
                        *g = None;
                    }
                    break;
                }''')

src += '''
/// When ModLauncher dies with `Provider ... not found`, the jar that carries
/// that provider is broken or from another loader — disable it automatically
/// so the next Играть has a chance to boot.
fn heal_broken_mod(shared: &Arc<Shared>, log_path: &Path) {
    use std::io::BufRead;
    let Ok(file) = std::fs::File::open(log_path) else {
        return;
    };
    let reader = std::io::BufReader::new(file);
    let mut provider = String::new();
    for line in std::io::BufReader::new(std::fs::File::open(log_path).unwrap()).lines().flatten() {
        if let Some(idx) = line.find("Provider ") {
            let rest = &line[idx + 9..];
            if let Some(end) = rest.find(" not found") {
                provider = rest[..end].to_string();
                break;
            }
        }
    }
    if provider.is_empty() {
        return;
    }
    let needle = provider.replace('.', "/");
    let mods_dir = crate::config::load().mods_dir();
    let mut culprit = String::new();
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || !path.extension().map(|e| e == "jar").unwrap_or(false) {
                continue;
            }
            if let Ok(f) = std::fs::File::open(&path) {
                if let Ok(mut zip) = zip::ZipArchive::new(f) {
                    for i in 0..zip.len() {
                        if let Ok(name) = zip.by_index(i).map(|e| e.name().to_string()) {
                            if name.starts_with(&needle) {
                                culprit = path
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or_default()
                                    .to_string();
                                break;
                            }
                        }
                    }
                }
            }
            if !culprit.is_empty() {
                break;
            }
        }
    }
    if culprit.is_empty() {
        return;
    }
    let from = mods_dir.join(&culprit);
    let to = mods_dir.join(format!("{culprit}.disabled"));
    if std::fs::rename(&from, &to).is_ok() {
        shared.push_game(format!(
            "[Madrid] Сборка не запускалась из-за битого мода {culprit} — он отключён. Нажми «Играть» снова."
        ));
        shared.log(
            crate::shared::LogKind::Warn,
            format!("Самолечение: отключён битый мод {culprit}"),
        );
    }
}
'''
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("self-heal ok")
