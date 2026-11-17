import io

# ================= app.rs: BuildEvent enum with live file names =================
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()

src = src.replace('''/// Installs every mod of the user's build, one by one.
pub struct BuildTask {
    pub total: usize,
    pub done: usize,
    pub finished: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub rx: Receiver<Result<PathBuf, String>>,
}''', '''pub enum BuildEvent {
    File(String),
    Ok(PathBuf),
    Err(String),
}

/// Installs every mod of the user's build, one by one.
pub struct BuildTask {
    pub total: usize,
    pub done: usize,
    pub finished: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
    pub last_file: String,
    pub rx: Receiver<BuildEvent>,
}''')

src = src.replace('''    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(Ok(_path)) => {
                    self.done += 1;
                    if self.done >= self.total {
                        self.finished = true;
                        return false;
                    }
                }
                Ok(Err(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                    }
                    return false;
                }
            }
        }
    }
}

/// Builds the Rust Minecraft client (stevenarella) from source.''', '''    pub fn pump(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(BuildEvent::File(name)) => {
                    self.last_file = name;
                }
                Ok(BuildEvent::Ok(_path)) => {
                    self.done += 1;
                    if self.done >= self.total {
                        self.finished = true;
                        return false;
                    }
                }
                Ok(BuildEvent::Err(e)) => {
                    self.error = Some(e);
                    self.finished = true;
                    return false;
                }
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => {
                    if !self.finished {
                        self.finished = true;
                    }
                    return false;
                }
            }
        }
    }
}

/// Builds the Rust Minecraft client (stevenarella) from source.''')

src = src.replace('''        let (tx, rx) = std::sync::mpsc::channel::<Result<PathBuf, String>>();
        let ctx = ctx.clone();
        let total = entries.len();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();''', '''        let (tx, rx) = std::sync::mpsc::channel::<BuildEvent>();
        let ctx = ctx.clone();
        let total = entries.len();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = cancel.clone();''')

src = src.replace('''                    for entry in &entries {
                        if cancel_task.load(Ordering::Relaxed) {
                            break;
                        }
                        let hit = ModHit {''', '''                    for entry in &entries {
                        if cancel_task.load(Ordering::Relaxed) {
                            break;
                        }
                        let _ = tx.send(BuildEvent::File(entry.title.clone()));
                        let hit = ModHit {''')

src = src.replace('''                        let res = mods::install(&client, &cfg, &hit, mc_eff, loader_eff, &entry.kind, &cancel_task, &mut noop)
                            .await
                            .map_err(|e| format!("{}: {e:#}", entry.title));
                        if tx.send(res).is_err() {
                            break;
                        }''', '''                        let res = mods::install(&client, &cfg, &hit, mc_eff, loader_eff, &entry.kind, &cancel_task, &mut noop)
                            .await
                            .map_err(|e| format!("{}: {e:#}", entry.title));
                        let ev = res.map(BuildEvent::Ok).unwrap_or(BuildEvent::Err);
                        if tx.send(ev).is_err() {
                            break;
                        }''')

src = src.replace('''        self.build_task = Some(BuildTask {
            total,
            done: 0,
            finished: false,
            error: None,
            cancel,
            rx,
        });''', '''        self.build_task = Some(BuildTask {
            total,
            done: 0,
            finished: false,
            error: None,
            cancel,
            last_file: String::new(),
            rx,
        });''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("build events ok")

# ================= views/mods.rs: show live file names =================
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

# download_card: show last_file line
src = src.replace('''            let (title, frac, percent) = {
                let t = self.mod_task.as_ref().unwrap();
                (t.title.clone(), t.frac(), t.total > 0)
            };
            let frame = theme::card_frame(skin);
            frame.show(ui, |ui| {
                ui.set_width(ui.available_width() - 32.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Скачивание · {}", truncate(&title, 40)))
                            .font(theme::body(12.5))
                            .color(skin.dim()),
                    );''', '''            let (title, frac, percent, last) = {
                let t = self.mod_task.as_ref().unwrap();
                (t.title.clone(), t.frac(), t.total > 0, t.last_file.clone())
            };
            let frame = theme::card_frame(skin);
            frame.show(ui, |ui| {
                ui.set_width(ui.available_width() - 32.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("Скачивание · {}", truncate(&title, 40)))
                            .font(theme::body(12.5))
                            .color(skin.dim()),
                    );''')

src = src.replace('''                ui.add_space(6.0);
                widgets::progress_bar(ui, skin, frac);
            });
            ui.add_space(10.0);
        }
        cancel
    }''', '''                if !last.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!("→ {}", truncate(&last, 70)))
                            .font(theme::mono(9.5))
                            .color(skin.faint()),
                    );
                }
                ui.add_space(6.0);
                widgets::progress_bar(ui, skin, frac);
            });
            ui.add_space(10.0);
        }
        cancel
    }''')

# build panel: live file line
src = src.replace('''                        let progress = self
                            .build_task
                            .as_ref()
                            .map(|t| (t.done, t.total, t.frac(), t.error.clone()));
                        if building {
                            if let Some((done, total, _, _)) = &progress {
                                ui.label(
                                    RichText::new(format!("Ставлю сборку · {done}/{total}"))
                                        .font(theme::mono(10.0))
                                        .color(skin.dim()),
                                );
                            }''', '''                        let progress = self
                            .build_task
                            .as_ref()
                            .map(|t| (t.done, t.total, t.frac(), t.error.clone(), t.last_file.clone()));
                        if building {
                            if let Some((done, total, _, _, last)) = &progress {
                                ui.label(
                                    RichText::new(format!(
                                        "Ставлю сборку · {done}/{total} · {}",
                                        truncate(last, 34)
                                    ))
                                    .font(theme::mono(10.0))
                                    .color(skin.dim()),
                                );
                            }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("live files ok")

# ================= views/versions.rs: rust entry in installed list =================
p = "src/views/versions.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    let mut confirm = self.confirm_delete.clone();
    let mut install_req: Option<(String, String)> = None;''', '''    let mut confirm = self.confirm_delete.clone();
    let mut install_req: Option<(String, String)> = None;''')

src = src.replace('''                widgets::section_label(ui, skin, "УСТАНОВЛЕНЫ");
                ui.add_space(4.0);
                if installed.is_empty() {''', '''                widgets::section_label(ui, skin, "УСТАНОВЛЕНЫ");
                ui.add_space(4.0);
                let rust_ready = crate::rustmc::installed();
                let mut installed = installed.clone();
                if rust_ready {
                    let size = crate::util::dir_size(&crate::config::data_dir().join("rustmc"));
                    installed.push(crate::minecraft::InstalledVersion {
                        id: "stevenarella-rust".into(),
                        loader: "rust".into(),
                        mc: "Rust".into(),
                        size,
                        has_jar: true,
                    });
                }
                if installed.is_empty() {''')

# installed_row label for rust
src = src.replace('''            if version.loader == "vanilla" { "Vanilla" } else { "Forge" },''',
'''            match version.loader.as_str() {
                "vanilla" => "Vanilla",
                "forge" => "Forge",
                _ => "Rust",
            },''')

# play handler: route rust
src = src.replace('''        if let Some(id) = play_req {
            self.cfg.selected_version = id;
            self.save_cfg();
            self.launch_game(false);
        }''', '''        if let Some(id) = play_req {
            if id == "stevenarella-rust" {
                self.launch_rustmc();
            } else {
                self.cfg.selected_version = id;
                self.save_cfg();
                self.launch_game(false);
            }
        }''')

# delete handler: route rust
src = src.replace('''        if let Some(id) = delete_req {
            let dir = self.cfg.game_dir().join("versions").join(&id);''', '''        if let Some(id) = delete_req {
            if id == "stevenarella-rust" {
                match std::fs::remove_dir_all(crate::config::data_dir().join("rustmc")) {
                    Ok(_) => self.toast("Rust-издание удалено"),
                    Err(e) => self.toast(format!("Не удалить: {e}")),
                }
                self.refresh_installed();
                return;
            }
            let dir = self.cfg.game_dir().join("versions").join(&id);''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("rust in installed ok")
