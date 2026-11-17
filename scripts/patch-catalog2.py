import io

# ============ app.rs: Madrid content target, nickname validation
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()

# Content target: Madrid profile (forge 1.20.1) first — the catalog must
# always have a meaningful target, even when the selected version is vanilla.
src = src.replace('''    /// Minecraft version + loader that mods should target.
    pub fn target_version(&self) -> (String, String) {
        let installed = self.installed();
        if let Some(v) = installed.iter().find(|v| v.id == self.cfg.selected_version) {
            return (
                v.mc.clone(),
                if v.loader == "vanilla" {
                    "vanilla".into()
                } else {
                    "forge".into()
                },
            );
        }
        if let Some(v) = installed.iter().find(|v| v.loader == "forge") {
            return (v.mc.clone(), "forge".into());
        }
        if let Some(v) = installed.first() {
            return (v.mc.clone(), "forge".into());
        }
        let versions = self.shared.versions();
        if let Some(v) = versions.iter().find(|v| v.kind == "release") {
            return (v.id.clone(), "forge".into());
        }
        (config::MADRID_MC.to_string(), "forge".into())
    }''', '''    /// Minecraft version + loader that mods should target. The Madrid pack
    /// is Forge 1.20.1, so the catalog targets that profile first —
    /// a vanilla selection must not leave the catalog empty.
    pub fn target_version(&self) -> (String, String) {
        let installed = self.installed();
        if let Some(v) = installed.iter().find(|v| v.loader == "forge") {
            return (v.mc.clone(), "forge".into());
        }
        (config::MADRID_MC.to_string(), "forge".into())
    }''')

# save_cfg: sanitize nickname (3..16, safe chars)
src = src.replace('''    pub fn save_cfg(&self) {
        config::save(&self.cfg);
        self.shared.set_cfg(self.cfg.clone());
    }''', '''    pub fn save_cfg(&self) {
        let mut cfg = self.cfg.clone();
        let nick = crate::util::sanitize_nickname(&cfg.nickname);
        if !nick.is_empty() {
            cfg.nickname = nick;
        }
        config::save(&cfg);
        self.shared.set_cfg(cfg);
    }''')

# catalog auto refresh: 5 minutes re-search
src = src.replace('''    /// 0 = каталог, 1 = установленные
    pub mods_page: usize,''', '''    /// 0 = каталог, 1 = установленные
    pub mods_page: usize,
    pub catalog_at: Instant,''')

src = src.replace("            mods_page: 0,", "            mods_page: 0,\n            catalog_at: Instant::now(),")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# util: sanitize_nickname
p = "src/util.rs"
src = io.open(p, encoding="utf-8").read()
src += '''
/// Nickname rules: 3..16 chars, letters/digits/underscore only.
pub fn sanitize_nickname(raw: &str) -> String {
    let filtered: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .take(16)
        .collect();
    filtered
}

pub fn nickname_valid(raw: &str) -> bool {
    let n = sanitize_nickname(raw);
    n.len() >= 3 && n.len() <= 16
}
'''
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("app+util ok")
