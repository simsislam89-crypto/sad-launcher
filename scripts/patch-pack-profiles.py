import io

# ================= profiles.rs: modpack profile helpers =================
p = "src/profiles.rs"
src = io.open(p, encoding="utf-8").read()
src += '''
/// Creates a profile for an installed modpack: isolated mods, its own MC/loader.
pub fn create_modpack_profile(title: &str, mc_version: &str, loader: &str) -> Result<Profile> {
    let mut profiles = load_profiles();
    let stamp = crate::util::file_stamp();
    let base_id = format!("modpack-{}", sanitize_id(title));
    let mut id = base_id.clone();
    let mut counter = 1;
    while profiles.iter().any(|p| p.id == id) {
        id = format!("{base_id}-{counter}");
        counter += 1;
    }
    let profile = Profile {
        id: id.clone(),
        name: title.trim().to_string(),
        kind: ProfileKind::Modpack,
        mc_version: if mc_version.is_empty() {
            crate::config::MADRID_MC.into()
        } else {
            mc_version.into()
        },
        loader: if loader.is_empty() { "forge".into() } else { loader.into() },
        description: format!("Модпак · установлен {}", stamp),
        created_at: crate::util::file_stamp(),
    };
    profiles.push(profile.clone());
    save_profiles(&profiles)?;
    Ok(profile)
}

/// Updates a profile's MC version / loader after the pack metadata is parsed.
pub fn update_profile_meta(profile_id: &str, mc_version: &str, loader: &str) -> Result<()> {
    let mut profiles = load_profiles();
    if let Some(p) = profiles.iter_mut().find(|p| p.id == profile_id) {
        if !mc_version.is_empty() {
            p.mc_version = mc_version.to_string();
        }
        if !loader.is_empty() {
            p.loader = loader.to_string();
        }
    }
    save_profiles(&profiles)?;
    Ok(())
}
'''
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("profile helpers ok")

# ================= mods.rs: ModEvent::PackMeta + metadata extraction =================
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    /// A concrete file started/completed — shown live in the UI.
    File(String),''', '''    /// A concrete file started/completed — shown live in the UI.
    File(String),
    /// The pack's target MC version + loader (parsed from its manifest).
    PackMeta(String, String),''')

# mrpack: emit meta after parsing dependencies
src = src.replace('''    if index
        .dependencies
        .keys()
        .any(|d| d.contains("fabric"))
    {
        bail!("Этот пак под Fabric — Мадрид работает на Forge. Ищи Forge-версию пака или поставь Fabric-профиль.");
    }
    let total = index.files.len() as u64;''', '''    if index
        .dependencies
        .keys()
        .any(|d| d.contains("fabric"))
    {
        bail!("Этот пак под Fabric — Мадрид работает на Forge. Ищи Forge-версию пака или поставь Fabric-профиль.");
    }
    let pack_mc = index
        .dependencies
        .get("minecraft")
        .cloned()
        .unwrap_or_default();
    let pack_loader = if index.dependencies.keys().any(|d| d.contains("forge")) {
        "forge".to_string()
    } else {
        "forge".to_string()
    };
    on(ModEvent::PackMeta(pack_mc, pack_loader));
    let total = index.files.len() as u64;''')

# CF pack: emit meta from manifest (mcVersion field) + don't filter loader for packs
src = src.replace('''#[derive(Deserialize)]
struct CfManifest {
    files: Vec<CfFileRef>,
    #[serde(default = "default_overrides")]
    overrides: String,
    /// CF modLoader: 0 = any, 1 = Forge, 4 = Fabric.
    #[serde(default)]
    mod_loader: i32,
}''', '''#[derive(Deserialize)]
struct CfManifest {
    files: Vec<CfFileRef>,
    #[serde(default = "default_overrides")]
    overrides: String,
    /// CF modLoader: 0 = any, 1 = Forge, 4 = Fabric.
    #[serde(default)]
    mod_loader: i32,
    #[serde(default, rename = "mcVersion")]
    mc_version: String,
}''')
src = src.replace('''    if manifest.mod_loader == 4 {
        bail!("Этот пак под Fabric — Мадрид работает на Forge. Ищи Forge-версию пака.");
    }''', '''    if manifest.mod_loader == 4 {
        bail!("Этот пак под Fabric — Мадрид работает на Forge. Ищи Forge-версию пака.");
    }
    on(ModEvent::PackMeta(manifest.mc_version.clone(), "forge".to_string()));''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("packmeta ok")

# ================= app.rs: profile flow for modpack installs =================
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''                Ok(ModEvent::File(name)) => {
                    self.last_file = name;
                }
                Ok(ModEvent::Done(path)) => {''', '''                Ok(ModEvent::File(name)) => {
                    self.last_file = name;
                }
                Ok(ModEvent::PackMeta(mc, loader)) => {
                    // The pack dictates the profile's version: create or update it.
                    let title = self
                        .mod_task
                        .as_ref()
                        .map(|t| t.title.clone())
                        .unwrap_or_else(|| "Модпак".into());
                    let profile = match crate::profiles::create_modpack_profile(
                        &title, &mc, &loader,
                    ) {
                        Ok(p) => p,
                        Err(e) => {
                            self.toast(format!("Профиль пака: {e:#}"));
                            return;
                        }
                    };
                    let mut cfg = self.cfg.clone();
                    if crate::profiles::switch_profile(&mut cfg, &profile.id).is_ok() {
                        self.cfg = cfg;
                        config::save(&self.cfg);
                        self.shared.set_cfg(self.cfg.clone());
                        self.toast(format!(
                            "Профиль «{}» · Minecraft {} · {}",
                            profile.name, profile.mc_version, profile.loader
                        ));
                    }
                }
                Ok(ModEvent::Done(path)) => {''')

# start_mod_install: modpack installs after profile switch — use live mods dir
src = src.replace('''        let (mc, loader) = self.target_version();
        let cfg = self.cfg.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let title = hit.title.clone();
        let kind = hit.kind.clone();''', '''        let (mc, loader) = self.target_version();
        let cfg = self.cfg.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        let title = hit.title.clone();
        let kind = hit.kind.clone();
        if kind == mods::KIND_MODPACK {
            self.toast(format!("Ставлю модпак «{}» — отдельная сборка", hit.title));
        }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("app modpack profile ok")

# ================= app.rs: target_version follows the ACTIVE profile =================
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    /// Minecraft version + loader that mods should target. The Madrid pack
    /// is Forge 1.20.1, so the catalog targets that profile first —
    /// a vanilla selection must not leave the catalog empty.
    pub fn target_version(&self) -> (String, String) {
        let installed = self.installed();
        if let Some(v) = installed.iter().find(|v| v.loader == "forge") {
            return (v.mc.clone(), "forge".into());
        }
        (config::MADRID_MC.to_string(), "forge".into())
    }''', '''    /// Minecraft version + loader of the ACTIVE profile. A modpack profile
    /// targets its own version — so «Играть» uses the pack's MC, not Madrid's.
    pub fn target_version(&self) -> (String, String) {
        let profile = crate::profiles::get_active_profile(&self.cfg);
        (
            profile.mc_version.clone(),
            if profile.loader.is_empty() {
                "forge".into()
            } else {
                profile.loader.clone()
            },
        )
    }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("target follows profile ok")

# ================= mods.rs: CF modpacks — all loaders visible =================
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    if kind == KIND_MOD || kind == KIND_MODPACK {
        url.push_str(&format!("&modLoaderType=1")); // Forge — the Madrid loader
    }
    let _ = loader;''', '''    // Mods are Forge-only (Madrid). Packs: show ALL loaders — compatibility
    // is checked at install time, not by hiding content.
    if kind == KIND_MOD {
        url.push_str(&format!("&modLoaderType=1")); // Forge — the Madrid loader
    }
    let _ = loader;''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("cf all loaders ok")
