import io, re

# ================= views/server.rs: scrollable + mods sync info =================
p = "src/views/server.rs"
src = io.open(p, encoding="utf-8").read()

# wrap whole content in ScrollArea: after header() call we need a scroll container.
# The function draws: status card, invite card, friends card, console.
# Wrap everything from the first frame to the console into ScrollArea.
src = src.replace('''        widgets::header(
            ui,
            skin,
            "Сервер",
            "Лаунчер сам хостит сервер Мадрид — пока окно открыто",
            |ui| {
                if running {
                    if widgets::button(ui, skin, "Остановить", Btn::Outline, true).clicked() {
                        stop = true;
                    }
                } else if widgets::button(
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
                }
            },
        );''', '''        widgets::header(
            ui,
            skin,
            "Сервер",
            "Лаунчер сам хостит сервер Мадрид — пока окно открыто",
            |ui| {
                if running {
                    if widgets::button(ui, skin, "Остановить", Btn::Outline, true).clicked() {
                        stop = true;
                    }
                } else if widgets::button(
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
                }
            },
        );

        egui::ScrollArea::vertical()
            .id_salt("server-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
            ui.set_width(ui.available_width());''')

# close the ScrollArea before handlers block
src = src.replace('''        // ---------------------------------------------------------- console''',
'''        // ---------------------------------------------------------- console''')

# find the console call and close scroll after it
src = src.replace('''        if crate::views::console(
            ui,
            skin,
            "КОНСОЛЬ СЕРВЕРА",
            state,
            &logs,
            running,
            height,
            "server-console",
        ) {
            self.shared.clear_mc_log();
        }''', '''            if crate::views::console(
                ui,
                skin,
                "КОНСОЛЬ СЕРВЕРА",
                state,
                &logs,
                running,
                height,
                "server-console",
            ) {
                self.shared.clear_mc_log();
            }
        });''')

# add mods-sync row into friends card
src = src.replace('''            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Друг вставляет этот код у себя и жмёт «Зайти к другу». Работает через проброс порта или Radmin VPN / Hamachi — там в коде IP вашей виртуальной сети.",
                )
                .font(theme::body(11.0))
                .color(skin.faint()),
            );
            ui.add_space(10.0);''', '''            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Друг вставляет этот код у себя и жмёт «Зайти к другу». Работает через проброс порта или Radmin VPN / Hamachi — там в коде IP вашей виртуальной сети.",
                )
                .font(theme::body(11.0))
                .color(skin.faint()),
            );
            ui.add_space(8.0);
            let mods_n = self.mods_cache.len();
            ui.horizontal(|ui| {
                widgets::kv_row(
                    ui,
                    skin,
                    "Модов в сборке сервера",
                    &mods_n.to_string(),
                    true,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_sized(
                        ui,
                        skin,
                        "Скопировать список",
                        Btn::Ghost,
                        mods_n > 0,
                        24.0,
                        10.0,
                        9.0,
                    )
                    .clicked()
                    {
                        copy_list = true;
                    }
                });
            });
            ui.label(
                RichText::new(
                    "У друга должна быть та же сборка модов — иначе Forge не пустит на сервер.",
                )
                .font(theme::body(11.0))
                .color(skin.faint()),
            );
            ui.add_space(10.0);''')

src = src.replace('''        let mut copy_code = false;
        let mut join_friend = false;''', '''        let mut copy_code = false;
        let mut copy_list = false;
        let mut join_friend = false;''')

src = src.replace('''        if copy_code {''', '''        if copy_list {
            let names: Vec<String> = self
                .mods_cache
                .iter()
                .map(|m| m.file.clone())
                .collect();
            ui.ctx().copy_text(names.join("\\n"));
            self.toast("Список модов скопирован — передай другу");
        }
        if copy_code {''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("server scroll ok")

# ================= views/versions.rs: loader switch resets catalog; rust delete =================
p = "src/views/versions.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''            for (key, label) in [("forge", "Forge"), ("vanilla", "Vanilla")] {
                if widgets::chip(ui, skin, label, self.cfg.loader == key).clicked() {
                    self.cfg.loader = key.into();
                    self.save_cfg();
                }
            }''', '''            for (key, label) in [("forge", "Forge"), ("vanilla", "Vanilla")] {
                if widgets::chip(ui, skin, label, self.cfg.loader == key).clicked() {
                    self.cfg.loader = key.into();
                    self.save_cfg();
                    // Catalog instantly retargets when the loader changes.
                    self.search.results.clear();
                    self.search.searched = false;
                    self.search.offset = 0;
                }
            }''')

# rust card: add delete button
src = src.replace('''                                } else if ready {
                                    if widgets::button_sized(ui, skin, "Играть", Btn::Primary, true, 28.0, 11.5, 11.0).clicked() {
                                        play_rust = true;
                                    }
                                } else {''', '''                                } else if ready {
                                    if widgets::button_sized(ui, skin, "Играть", Btn::Primary, true, 28.0, 11.5, 11.0).clicked() {
                                        play_rust = true;
                                    }
                                    if widgets::button_sized(ui, skin, "Удалить", Btn::Ghost, true, 28.0, 11.5, 10.0).clicked() {
                                        delete_rust = true;
                                    }
                                } else {''')

src = src.replace("        let mut install_rust = false;\n        let mut play_rust = false;",
                  "        let mut install_rust = false;\n        let mut play_rust = false;\n        let mut delete_rust = false;")

src = src.replace('''        if play_rust {
            self.launch_rustmc();
        }''', '''        if play_rust {
            self.launch_rustmc();
        }
        if delete_rust {
            let dir = crate::config::data_dir().join("rustmc");
            match std::fs::remove_dir_all(&dir) {
                Ok(_) => self.toast("Rust-издание удалено"),
                Err(e) => self.toast(format!("Не удалить: {e}")),
            }
        }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("versions ok")

# ================= minecraft.rs: protection settings =================
p = "src/minecraft.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    let props = format!(
        "# managed by LLM Launcher Madrid\\nonline-mode=false\\nserver-port={port}\\nquery.port={port}\\nmotd=Madrid · localhost\\nwhite-list=false\\nspawn-protection=0\\nview-distance=8\\nmax-players=20\\n"
    );''', '''    // autosave every 20 s (in ticks): a sudden power cut loses seconds, not the world
    let props = format!(
        "# managed by LLM Launcher Madrid\\nonline-mode=false\\nserver-port={port}\\nquery.port={port}\\nmotd=Madrid · localhost\\nwhite-list=false\\nspawn-protection=0\\nview-distance=8\\nmax-players=20\\nautosave-interval=400\\nsync-chunk-writes=true\\n"
    );''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("protections ok")

# ================= config.rs: atomic save =================
p = "src/config.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''pub fn save(cfg: &Config) {
    ensure_dirs();
    if let Ok(text) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(config_path(), text);
    }
}''', '''pub fn save(cfg: &Config) {
    ensure_dirs();
    if let Ok(text) = serde_json::to_string_pretty(cfg) {
        // Temp file + rename: a power cut never leaves a half-written config.
        let path = config_path();
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("config atomic ok")

# ================= launch.rs: crash diagnostics for JVM =================
p = "src/launch.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''    out.extend(crate::launch::perf_gc_flags(mem as u64));''', '''    out.extend(crate::launch::perf_gc_flags(mem as u64));
    out.push("-XX:+HeapDumpOnOutOfMemoryError".into());
    out.push(format!(
        "-XX:ErrorFile={}",
        config::data_dir().join("logs").join("jvm-crash-%p.log").display()
    ));''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("jvm crash flags ok")
