import io

# ================= mods.rs: record real file names for CF packs =================
p = "src/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''        struct FileData {
            #[serde(default, rename = "downloadUrl")]
            download_url: Option<String>,
        }''', '''        struct FileData {
            #[serde(default, rename = "fileName")]
            file_name: String,
            #[serde(default, rename = "downloadUrl")]
            download_url: Option<String>,
        }''')
src = src.replace('''        let url = match &file_url {
            Some(u) => u.clone(),
            None => format!(
                "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
                file.project_id, file.file_id
            ),
        };
        let tail = url
            .rsplit('/')
            .next()
            .unwrap_or("mod.jar")
            .trim_start_matches("download")
            .to_string();
        let tail = if tail.is_empty() { format!("{}.jar", file.file_id) } else { tail };''', '''        let (url, nice_name) = match &file_url {
            Some(u) => {
                let tail = u.rsplit('/').next().unwrap_or("mod.jar").to_string();
                (u.clone(), tail)
            }
            None => (
                format!(
                    "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
                    file.project_id, file.file_id
                ),
                String::new(),
            ),
        };
        let tail = if nice_name.is_empty() { format!("cf-file-{}.jar", file.file_id) } else { nice_name };''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("cf names ok")

# ================= views/mods.rs: body fallback + video note =================
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''            if detail.body.is_empty() {
                panel.label(
                    RichText::new("У проекта нет описания.")
                        .font(theme::body(12.0))
                        .color(skin.faint()),
                );
            }''', '''            if detail.body.is_empty() {
                panel.label(
                    RichText::new(if detail.summary.is_empty() {
                        "У проекта нет описания."
                    } else {
                        &detail.summary
                    })
                    .font(theme::body(12.0))
                    .color(skin.dim()),
                );
            }
            if hit.source == "curseforge" && !detail.body.is_empty() {
                panel.label(
                    RichText::new("Видео и медиа — на сайте проекта.")
                        .font(theme::mono(9.0))
                        .color(skin.faint()),
                );
            }''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("detail fallback ok")

# ================= views/server.rs: explicit copy button + LAN fallback =================
p = "src/views/server.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''        let my_code = self
            .public_ip(ui.ctx())
            .map(|ip| format!("{}:{}", ip, port_buf.trim()));''', '''        let my_code = match self.public_ip(ui.ctx()) {
            Some(ip) => Some(format!("{}:{}", ip, port_buf.trim())),
            None => self
                .lan_ip
                .clone()
                .map(|ip| format!("{}:{}", ip, port_buf.trim())),
        };''')
src = src.replace('''            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Готовый адрес сервера")
                        .font(theme::body(12.5))
                        .color(skin.dim()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let code = match &my_code {
                        Some(c) => c.clone(),
                        None => "определяю IP…".to_string(),
                    };
                    if widgets::chip(ui, skin, &code, running).clicked() {
                        copy_code = true;
                    }
                });
            });''', '''            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Готовый адрес сервера")
                        .font(theme::body(12.5))
                        .color(skin.dim()),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_sized(
                        ui,
                        skin,
                        "Скопировать адрес",
                        Btn::Primary,
                        my_code.is_some(),
                        26.0,
                        11.0,
                        10.0,
                    )
                    .clicked()
                    {
                        copy_code = true;
                    }
                    let code = match &my_code {
                        Some(c) => c.clone(),
                        None => "определяю IP…".to_string(),
                    };
                    ui.label(
                        RichText::new(code)
                            .font(theme::mono(12.5))
                            .color(skin.ink()),
                    );
                });
            });''')
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("copy button ok")

# ================= views/play.rs: ScrollArea + install progress on Старт =================
p = "src/views/play.rs"
src = io.open(p, encoding="utf-8").read()

src = src.replace('''        widgets::header(
            ui,
            skin,
            "Старт",
            "Профиль, версия и запуск — в одном окне",
            |ui| {
                if widgets::button(ui, skin, "Папка игры", Btn::Ghost, true).clicked() {
                    open_game = true;
                }
            },
        );''', '''        widgets::header(
            ui,
            skin,
            "Старт",
            "Профиль, версия и запуск — в одном окне",
            |ui| {
                if widgets::button(ui, skin, "Папка игры", Btn::Ghost, true).clicked() {
                    open_game = true;
                }
            },
        );

        egui::ScrollArea::vertical()
            .id_salt("play-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
            ui.set_width(ui.available_width());''')

# close the scroll area after the console block
src = src.replace('''        if console(
            ui,
            skin,
            "КОНСОЛЬ ИГРЫ",
            if running { "запущена" } else { "ожидание" },
            &lines,
            running,
            height,
            "game-console",
        ) {
            self.shared.clear_game_log();
        }''', '''            if console(
                ui,
                skin,
                "КОНСОЛЬ ИГРЫ",
                if running { "запущена" } else { "ожидание" },
                &lines,
                running,
                height,
                "game-console",
            ) {
                self.shared.clear_game_log();
            }
        });''')

# install progress card on Старт (after profile card)
src = src.replace('''                    ui.add_space(14.0);
                    widgets::section_label(ui, skin, "ВЕРСИИ УСТАНОВЛЕНЫ");''', '''                    if let Some(t) = self.install.as_ref() {
                        if !t.finished {
                            ui.add_space(12.0);
                            let frame = egui::Frame::new()
                                .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8))
                                .stroke(Stroke::new(1.0, skin.line()))
                                .corner_radius(CornerRadius::same(12))
                                .inner_margin(egui::Margin::symmetric(14, 10));
                            let (stage, frac, ver, has_total) = (
                                t.stage.clone(),
                                t.frac(),
                                t.version.clone(),
                                t.total > 0,
                            );
                            frame.show(ui, |ui| {
                                ui.set_width(left_w - 30.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!(
                                            "Установка клиента · {}",
                                            truncate(&stage, 44)
                                        ))
                                        .font(theme::body(12.5))
                                        .color(skin.dim()),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::button_sized(
                                            ui,
                                            skin,
                                            "Отменить",
                                            Btn::Ghost,
                                            true,
                                            24.0,
                                            10.5,
                                            9.5,
                                        )
                                        .clicked()
                                        {
                                            self.cancel_active_downloads();
                                        }
                                        ui.label(
                                            RichText::new(if has_total {
                                                format!("{}%", (frac * 100.0).round() as u32)
                                            } else {
                                                "…".to_string()
                                            })
                                            .font(theme::mono(10.0))
                                            .color(skin.faint()),
                                        );
                                    });
                                });
                                ui.add_space(6.0);
                                widgets::progress_bar(ui, skin, frac);
                                ui.label(
                                    RichText::new(format!("цель: {}", truncate(&ver, 40)))
                                        .font(theme::mono(9.0))
                                        .color(skin.faint()),
                                );
                            });
                        }
                    }

                    ui.add_space(14.0);
                    widgets::section_label(ui, skin, "ВЕРСИИ УСТАНОВЛЕНЫ");''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("play scroll + progress ok")
