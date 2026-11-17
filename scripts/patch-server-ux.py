import io, re

# ============ mods.rs: drop unused Ordering imports
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("    use std::sync::atomic::Ordering;\n", "")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ config.rs: auto_server default false
p = "src/config.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    /// Host the Minecraft server as soon as the launcher opens.
    pub auto_server: bool,''', '''    /// Host the Minecraft server as soon as the launcher opens.
    /// Off by default: the server starts only by its button.
    pub auto_server: bool,''')
src = src.replace("            auto_server: true,", "            auto_server: false,")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ launch.rs: connect param instead of implicit auto-join
p = "src/launch.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("pub fn launch(shared: &Arc<Shared>, cfg: &Config, id: &str) -> Result<PathBuf> {",
                  "pub fn launch(shared: &Arc<Shared>, cfg: &Config, id: &str, connect: bool) -> Result<PathBuf> {")
src = src.replace("    let args = build_args(shared, cfg, &resolved)?;",
                  "    let args = build_args(shared, cfg, &resolved, connect)?;")
src = src.replace("fn build_args(shared: &Arc<Shared>, cfg: &Config, resolved: &Resolved) -> Result<Vec<String>> {",
                  "fn build_args(shared: &Arc<Shared>, cfg: &Config, resolved: &Resolved, connect: bool) -> Result<Vec<String>> {")
src = src.replace('''    // Madrid flow: when the hosted server is up, the client joins it directly.
    if shared.is_mc_running() && mc_release(&resolved.id) >= (1, 20) {
        let port = cfg.server_port;
        out.push("--quickPlayMultiplayer".into());
        out.push(format!("127.0.0.1:{port}"));
    }
    Ok(out)''', '''    // The game joins the hosted server only when asked explicitly
    // (the «На сервер» button), never silently.
    if connect && shared.is_mc_running() && mc_release(&resolved.id) >= (1, 20) {
        let port = cfg.server_port;
        out.push("--quickPlayMultiplayer".into());
        out.push(format!("127.0.0.1:{port}"));
    }
    Ok(out)''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ app.rs: connect flag, cancel helper, build rejects packs
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    pub fn launch_game(&mut self) {
        let id = self.cfg.selected_version.clone();
        if id.is_empty() {
            self.toast("Сначала выберите версию");
            self.tab = Tab::Versions;
            return;
        }
        match launch::launch(&self.shared, &self.cfg, &id) {
            Ok(_) => {
                self.toast("Minecraft запускается");
                self.was_running = true;
            }
            Err(e) => self.toast(format!("{e:#}")),
        }
    }''', '''    pub fn launch_game(&mut self, connect: bool) {
        let id = self.cfg.selected_version.clone();
        if id.is_empty() {
            self.toast("Сначала выберите версию");
            self.tab = Tab::Versions;
            return;
        }
        match launch::launch(&self.shared, &self.cfg, &id, connect) {
            Ok(_) => {
                if connect {
                    self.toast("Запускаю и подключаю к серверу Мадрид");
                } else {
                    self.toast("Minecraft запускается");
                }
                self.was_running = true;
            }
            Err(e) => self.toast(format!("{e:#}")),
        }
    }

    /// Cancels every in-flight download the user asked to stop.
    pub fn cancel_active_downloads(&mut self) {
        let mut any = false;
        if let Some(t) = &self.install {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if let Some(t) = &self.mod_task {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if let Some(t) = &self.build_task {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if let Some(t) = &self.java_task {
            if !t.finished {
                t.cancel.store(true, Ordering::SeqCst);
                any = true;
            }
        }
        if any {
            self.toast("Отменяю…");
        }
    }''')

# build_add rejects modpacks
src = src.replace('''    pub fn build_add(&mut self, hit: &ModHit, mc: String, loader: String) {
        if self.build.iter().any(|e| e.slug == hit.slug && e.source == hit.source) {
            self.toast("Уже в сборке");
            return;
        }''', '''    pub fn build_add(&mut self, hit: &ModHit, mc: String, loader: String) {
        if hit.kind == mods::KIND_MODPACK {
            self.toast("Модпак ставится кнопкой «Скачать»");
            return;
        }
        if self.build.iter().any(|e| e.slug == hit.slug && e.source == hit.source) {
            self.toast("Уже в сборке");
            return;
        }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ play.rs: launch_game(false) + На сервер button
p = "src/views/play.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("        if launch_now {\n            self.launch_game();\n        }",
                  "        if launch_now {\n            self.launch_game(false);\n        }")
src = src.replace('''        let mut launch_now = false;
        let mut stop_now = false;''', '''        let mut launch_now = false;
        let mut launch_server = false;
        let mut stop_now = false;''')
src = src.replace('''                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if play_button(ui, skin, running).clicked() {
                                    if running {
                                        stop_now = true;
                                    } else {
                                        launch_now = true;
                                    }
                                }
                            });''', '''                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if play_button(ui, skin, running).clicked() {
                                    if running {
                                        stop_now = true;
                                    } else {
                                        launch_now = true;
                                    }
                                }
                                if widgets::button(ui, skin, "На сервер", Btn::Outline, mc_port > 0)
                                    .clicked()
                                {
                                    launch_server = true;
                                }
                            });''')
src = src.replace('''        if stop_now {
            self.stop_game();
        }''', '''        if launch_server {
            self.launch_game(true);
        }
        if stop_now {
            self.stop_game();
        }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ versions.rs: play without connect + cancel button
p = "src/views/versions.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''        if let Some(id) = play_req {
            self.cfg.selected_version = id;
            self.save_cfg();
            self.launch_game();
        }''', '''        if let Some(id) = play_req {
            self.cfg.selected_version = id;
            self.save_cfg();
            self.launch_game(false);
        }''')
src = src.replace('''                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let label = if task.finished {''', '''                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !task.finished && widgets::button_sized(ui, skin, "Отменить", Btn::Ghost, true, 24.0, 10.5, 9.5).clicked() {
                            self.cancel_active_downloads();
                        }
                        let label = if task.finished {''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ server.rs view: connect button
p = "src/views/server.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''        let mut start = false;
        let mut stop = false;''', '''        let mut start = false;
        let mut stop = false;
        let mut connect = false;''')
src = src.replace('''                } else if widgets::button(
                    ui,
                    skin,
                    if busy { "Занято…" } else { "Запустить" },
                    Btn::Primary,
                    !busy,
                )
                .clicked()
                {
                    start = true;
                }''', '''                } else if widgets::button(
                    ui,
                    skin,
                    if busy { "Занято…" } else { "Запустить" },
                    Btn::Primary,
                    !busy,
                )
                .clicked()
                {
                    start = true;
                }
                if widgets::button(ui, skin, "Подключиться", Btn::Outline, running).clicked() {
                    connect = true;
                }''')
src = src.replace('''        if start {
            self.start_server();
        }''', '''        if connect {
            self.launch_game(true);
        }
        if start {
            self.start_server();
        }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# ============ rustmc: connect only when server running
p = "src/rustmc.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    let port = cfg.server_port;
    let mut cmd = Command::new(&exe);
    cmd.args([
        "--server".to_string(),
        format!("127.0.0.1:{port}"),
        "--username".to_string(),
        cfg.nickname_or_default(),
    ]);''', '''    let mut cmd = Command::new(&exe);
    cmd.arg("--username".to_string()).arg(cfg.nickname_or_default());
    if shared.is_mc_running() {
        cmd.arg("--server")
            .arg(format!("127.0.0.1:{}", cfg.server_port));
    }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

print("server ux ok")
