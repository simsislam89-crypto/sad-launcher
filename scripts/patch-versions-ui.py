import io, re

# app.rs: detail stores the full ProjectDetail (versions, compatibility)
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("    pub detail_body: Option<Result<String, String>>,\n    pub detail_rx: Option<Receiver<Result<String, String>>>,",
                  "    pub detail_body: Option<Result<crate::mods::ProjectDetail, String>>,\n    pub detail_rx: Option<Receiver<Result<crate::mods::ProjectDetail, String>>>,")
src = src.replace("self.detail_body = Some(res);", "self.detail_body = Some(res);")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# views/mods.rs: fetch full detail, render versions + compatibility
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace('''                        let res: Result<String, String> = match rt {
                            Ok(rt) => rt
                                .block_on(async {
                                    let client = match crate::minecraft::http_client() {
                                        Ok(c) => c,
                                        Err(e) => return Err(e.to_string()),
                                    };
                                    crate::mods::detail(&client, &hit)
                                        .await
                                        .map(|d| d.body)
                                        .map_err(|e| format!("{e:#}"))
                                }),
                            Err(_) => Err("Не удалось запустить загрузчик".into()),
                        };
                        let _ = tx.send(res);''', '''                        let res: Result<crate::mods::ProjectDetail, String> = match rt {
                            Ok(rt) => rt
                                .block_on(async {
                                    let client = match crate::minecraft::http_client() {
                                        Ok(c) => c,
                                        Err(e) => return Err(e.to_string()),
                                    };
                                    crate::mods::detail(&client, &hit)
                                        .await
                                        .map_err(|e| format!("{e:#}"))
                                }),
                            Err(_) => Err("Не удалось запустить загрузчик".into()),
                        };
                        let _ = tx.send(res);''')

# body rendering: detail_full fields
src = src.replace('''        Some(Ok(body)) => {
            if body.is_empty() {
                panel.label(
                    RichText::new("У проекта нет описания.")
                        .font(theme::body(12.0))
                        .color(skin.faint()),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("mod-detail-body")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(&body)
                            .font(theme::body(12.5))
                            .color(skin.dim()),
                    );
                });
        }''', '''        Some(Ok(detail)) => {
            if let Some(compatible) = detail.compatible {
                panel.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    if compatible {
                        widgets::badge(ui, skin, "✓ совместимо с Мадрид 1.20.1 Forge", true);
                    } else {
                        widgets::badge(ui, skin, "⚠ нет версии под Мадрид 1.20.1 Forge", false);
                    }
                    if !detail.versions.is_empty() {
                        widgets::badge(ui, skin, "версии ↓", false);
                    }
                });
                panel.add_space(8.0);
            }
            if detail.body.is_empty() {
                panel.label(
                    RichText::new("У проекта нет описания.")
                        .font(theme::body(12.0))
                        .color(skin.faint()),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt("mod-detail-body")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(
                        RichText::new(&detail.body)
                            .font(theme::body(12.5))
                            .color(skin.dim()),
                    );
                    if !detail.versions.is_empty() {
                        ui.add_space(12.0);
                        widgets::section_label(ui, skin, "ВЕРСИИ ПРОЕКТА");
                        ui.add_space(4.0);
                        for v in &detail.versions {
                            let targets = format!(
                                "{} · {}",
                                v.game_versions.join(", "),
                                v.loaders.join("/")
                            );
                            let fits = v.game_versions.iter().any(|g| g == crate::config::MADRID_MC)
                                && v.loaders.iter().any(|l| l == "forge");
                            ui.horizontal(|ui| {
                                widgets::status_dot(
                                    ui,
                                    if fits { skin.ink() } else { skin.faint() },
                                    fits,
                                );
                                ui.label(
                                    RichText::new(truncate(&v.name, 44))
                                        .font(theme::mono(10.0))
                                        .color(if fits { skin.ink() } else { skin.dim() }),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.label(
                                        RichText::new(truncate(&targets, 60))
                                            .font(theme::mono(8.5))
                                            .color(skin.faint()),
                                    );
                                });
                            });
                        }
                    }
                });
        }''')

io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("detail versions ok")
