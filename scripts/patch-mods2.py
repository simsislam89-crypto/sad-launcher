import io

p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()

# 1) ModEvent::File for the live "mods flying by" list
src = src.replace('''#[derive(Clone)]
pub enum ModEvent {
    Progress { done: u64, total: u64 },
    Done(PathBuf),
    Error(String),
}''', '''#[derive(Clone)]
pub enum ModEvent {
    Progress { done: u64, total: u64 },
    /// A concrete file started/completed — shown live in the UI.
    File(String),
    Done(PathBuf),
    Error(String),
}''')

# 2) modrinth_file: no loaders filter for non-mod kinds (modpacks/shaders/packs)
src = src.replace('''    let project_type = match kind {
        KIND_SHADER => "shader",
        KIND_RESOURCEPACK => "resourcepack",
        _ => "mod",
    };
    let loaders = if kind == KIND_MOD {
        format!(r#"["{loader}"]"#)
    } else {
        r#"["minecraft"]"#.to_string()
    };
    let url = format!(
        "https://api.modrinth.com/v2/project/{}/version?game_versions={}&loaders={}",
        hit.slug,
        enc(&format!(r#"["{mc}"]"#)),
        enc(&loaders)
    );''', '''    let _ = loader;
    let mut url = format!(
        "https://api.modrinth.com/v2/project/{}/version?game_versions={}",
        hit.slug,
        enc(&format!(r#"["{mc}"]"#))
    );
    if kind == KIND_MOD {
        url.push_str(&format!("&loaders={}", enc(&format!(r#"["forge"]"#))));
    }''')

# 3) modpack: File events per entry
src = src.replace('''    let game = cfg.game_dir();
    for (i, file) in index.files.iter().enumerate() {
        let Some(url) = file.downloads.first() else {
            continue;
        };
        let dest = game.join(&file.path);
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Отменено");
        }
        minecraft_dl(client, url, &dest).await?;''', '''    let game = cfg.game_dir();
    for (i, file) in index.files.iter().enumerate() {
        let Some(url) = file.downloads.first() else {
            continue;
        };
        let dest = game.join(&file.path);
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Отменено");
        }
        on(ModEvent::File(file.path.clone()));
        minecraft_dl(client, url, &dest).await?;''')

# 4) CF pack: File events too
src = src.replace('''        let dest = mods_dir.join(sanitize(&name));
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Отменено");
        }
        if !dest.exists() {
            minecraft_dl(client, &resolved.data, &dest).await?;
        }''', '''        let dest = mods_dir.join(sanitize(&name));
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Отменено");
        }
        on(ModEvent::File(name.clone()));
        if !dest.exists() {
            minecraft_dl(client, &resolved.data, &dest).await?;
        }''')

# 5) CurseForge direct public download URL (no key needed for the CDN)
src = src.replace('''async fn curse_file(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    mc: &str,
    loader: &str,
    kind: &str,
) -> Result<(String, String)> {
    if let Some(url) = &hit.file_url {
        if !url.is_empty() {
            return Ok((
                url.clone(),
                hit.file_name.clone().unwrap_or_else(|| format!("{}.zip", hit.slug)),
            ));
        }
    }''', '''async fn curse_file(
    client: &reqwest::Client,
    cfg: &Config,
    hit: &ModHit,
    mc: &str,
    loader: &str,
    kind: &str,
) -> Result<(String, String)> {
    // The file CDN is public: search needs a key, downloads do not.
    if let Some(fid) = hit.file_id {
        let name = hit
            .file_name
            .clone()
            .unwrap_or_else(|| format!("{}-{}.jar", hit.slug, fid));
        return Ok((
            format!(
                "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
                hit.id, fid
            ),
            name,
        ));
    }
    if let Some(url) = &hit.file_url {
        if !url.is_empty() {
            return Ok((
                url.clone(),
                hit.file_name.clone().unwrap_or_else(|| format!("{}.zip", hit.slug)),
            ));
        }
    }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("mods fixes ok")

# ================= app.rs: ModTask last_file + File events; rust synthetic entry
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()

src = src.replace('''pub struct ModTask {
    pub title: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub path: Option<PathBuf>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<ModEvent>,
}''', '''pub struct ModTask {
    pub title: String,
    pub done: u64,
    pub total: u64,
    pub finished: bool,
    pub error: Option<String>,
    pub path: Option<PathBuf>,
    pub cancel: Arc<AtomicBool>,
    pub last_file: String,
    pub rx: Receiver<ModEvent>,
}''')

src = src.replace('''                Ok(ModEvent::Progress { done, total }) => {
                    self.done = done;
                    self.total = total;
                }
                Ok(ModEvent::Done(path)) => {''', '''                Ok(ModEvent::Progress { done, total }) => {
                    self.done = done;
                    self.total = total;
                }
                Ok(ModEvent::File(name)) => {
                    self.last_file = name;
                }
                Ok(ModEvent::Done(path)) => {''')

src = src.replace('''        self.mod_task = Some(ModTask {
            title,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            path: None,
            cancel,
            rx,
        });''', '''        self.mod_task = Some(ModTask {
            title,
            done: 0,
            total: 0,
            finished: false,
            error: None,
            path: None,
            cancel,
            last_file: String::new(),
            rx,
        });''')

# build install: emit File per entry
src = src.replace('''                    for entry in &entries {
                        if cancel_task.load(Ordering::Relaxed) {
                            break;
                        }
                        let hit = ModHit {''', '''                    for entry in &entries {
                        if cancel_task.load(Ordering::Relaxed) {
                            break;
                        }
                        let hit = ModHit {''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("app modtask ok")
