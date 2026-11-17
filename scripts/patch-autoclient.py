import io

# ================= app.rs: ctx field + auto-install client on Играть =================
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("    pub nick_buf: String,", "    pub nick_buf: String,\n    pub ctx: Option<egui::Context>,")
src = src.replace("            nick_buf: String::new(),", "            nick_buf: String::new(),\n            ctx: None,")
src = src.replace('''        Self {
            cfg,''', '''        let ctx = cc.egui_ctx.clone();
        Self {
            ctx: Some(ctx),
            cfg,''')

# auto-install on Играть when the Madrid client is missing
src = src.replace('''    pub fn launch_game(&mut self, connect: bool) {
        let id = self.cfg.selected_version.clone();
        if id.is_empty() {
            self.toast("Сначала выберите версию");
            self.tab = Tab::Versions;
            return;
        }''', '''    pub fn launch_game(&mut self, connect: bool) {
        // Madrid client missing? Install it right here, then auto-launch.
        let (mc, loader) = self.target_version();
        let have_client = self
            .installed()
            .iter()
            .any(|v| v.mc == mc && v.loader == loader);
        if !have_client {
            let ctx = match &self.ctx {
                Some(c) => c.clone(),
                None => return,
            };
            self.toast("Ставлю клиент Мадрида — после установки игра откроется сама");
            self.start_install(&ctx, mc, "forge".to_string(), true);
            return;
        }
        let id = self.cfg.selected_version.clone();
        if id.is_empty() {
            self.cfg.selected_version = self
                .installed()
                .iter()
                .find(|v| v.loader == "forge")
                .map(|v| v.id.clone())
                .unwrap_or_default();
            if self.cfg.selected_version.is_empty() {
                self.toast("Клиент ещё ставится — секунду");
                return;
            }
        }''')

# start_install gains auto_launch param
src = src.replace("    pub fn start_install(&mut self, ctx: &egui::Context, mc: String, loader: String) {",
                  "    pub fn start_install(&mut self, ctx: &egui::Context, mc: String, loader: String, auto_launch: bool) {")
src = src.replace('''        self.install = Some(InstallTask {
            version: mc,
            loader,
            stage: "Подготовка…".into(),
            done: 0,
            total: 0,
            finished: false,
            ok: false,
            error: None,
            cancel,
            rx,
        });''', '''        self.install = Some(InstallTask {
            version: mc,
            loader,
            stage: "Подготовка…".into(),
            done: 0,
            total: 0,
            finished: false,
            ok: false,
            error: None,
            cancel,
            auto_launch,
            rx,
        });''')

src = src.replace('''    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<InstallEvent>,
}''', '''    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub auto_launch: bool,
    pub rx: Receiver<InstallEvent>,
}''')

# poll: auto launch after install
src = src.replace('''                if task.ok {
                    self.cfg.selected_version = task.version.clone();
                    self.save_cfg();
                    self.refresh_installed();
                    self.toast(format!("Установлено: {}", task.version));
                } else if task.error''', '''                if task.ok {
                    self.cfg.selected_version = task.version.clone();
                    self.save_cfg();
                    self.refresh_installed();
                    self.toast(format!("Установлено: {}", task.version));
                    if task.auto_launch {
                        let connect = self.shared.is_mc_running();
                        self.launch_game(connect);
                    }
                } else if task.error''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("auto-install ok")

# ================= views: update start_install call sites =================
p = "src/views/versions.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("            self.start_install(ui.ctx(), mc, loader);",
                  "            self.start_install(ui.ctx(), mc, loader, false);")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("call sites ok")

# ================= mods.rs: CF packs without key + honest messages =================
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''        .context("CurseForge отклонил запрос (ключ?)")?''',
                  '''        .context("CurseForge не ответил — проверь соединение")?''')
src = src.replace('''            .context("CurseForge отклонил запрос (проверь ключ)")?''',
                  '''            .context("CurseForge не ответил — зеркало могло отхлынуть, попробуй ещё раз")?''')
src = src.replace('''    if cfg.cf_api_key.trim().is_empty() {
        bail!("CurseForge-пак требует ключ API в настройках");
    }''', '''    // No key needed: files resolve through cf_base() (mirror by default).''')

# resolve each CF pack file via /files/{fid} -> downloadUrl, fallback public download
src = src.replace('''    for (i, file) in manifest.files.iter().enumerate() {
        #[derive(Deserialize)]
        struct UrlResp {
            data: String,
        }
        let resolved: UrlResp = client
            .get(format!(
                "{}/mods/{}/files/{}/download-url",
                cf_base(),
                file.project_id,
                file.file_id
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
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Отменено");
        }
        on(ModEvent::File(name.clone()));
        installed_names.push(name);
        if !dest.exists() {
            minecraft_dl(client, &resolved.data, &dest).await?;
        }''', '''    for (i, file) in manifest.files.iter().enumerate() {
        #[derive(Deserialize)]
        struct FileResp {
            data: FileData,
        }
        #[derive(Deserialize)]
        struct FileData {
            #[serde(default, rename = "fileName")]
            file_name: String,
            #[serde(default, rename = "downloadUrl")]
            download_url: Option<String>,
        }
        let file_url = match client
            .get(format!(
                "{}/mods/{}/files/{}",
                cf_base(),
                file.project_id,
                file.file_id
            ))
            .send()
            .await
        {
            Ok(r) => r
                .json::<FileResp>()
                .await
                .ok()
                .and_then(|d| d.data.download_url),
            Err(_) => None,
        };
        // Public CDN redirect — always works, no key.
        let url = file_url.unwrap_or_else(|| {
            format!(
                "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
                file.project_id, file.file_id
            )
        });
        let name = if file_url.is_some() {
            String::new() // resolved below from the url tail
        } else {
            String::new()
        };
        let _ = &name;
        let tail = url.rsplit('/').next().unwrap_or("mod.jar").to_string();
        let dest = mods_dir.join(sanitize(&tail));
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Отменено");
        }
        on(ModEvent::File(tail.clone()));
        installed_names.push(tail);
        if !dest.exists() {
            minecraft_dl(client, &url, &dest).await?;
        }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("cf pack no-key ok")
