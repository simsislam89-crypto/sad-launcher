//! Старт — the play surface: profile, version, and the hosted server.

use super::{console, play_button, truncate};
use crate::app::Launcher;
use crate::icons::Icon;
use crate::theme::{self, Skin};
use crate::util;
use crate::widgets::{self, Btn};
use egui::{
    Align, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2,
};

impl Launcher {
    pub fn view_play(&mut self, ui: &mut Ui, skin: &Skin) {
        let running = self.shared.is_game_running();
        let installed = self.installed();
        let nickname = self.cfg.nickname_or_default();
        let uuid = crate::minecraft::offline_uuid(&nickname);
        let spec = self.active_spec();
        let compatible = self.compatible_version();
        let selected_installed = compatible
            .as_ref()
            .and_then(|id| installed.iter().find(|v| &v.id == id).cloned());
        // Cheap: derived from the release, no JSON read per frame.
        let required = crate::java::required_for(&spec.mc);
        // Show the Java that will actually run this build, not merely the
        // newest one on the machine («Java 25 · нужно 17» was misleading).
        let java = crate::java::fitting_cached(required, spec.loader).or_else(|| self.shared.java());
        let java_ok = java
            .as_ref()
            .map(|j| crate::loader::java_fits(j.major, required, spec.loader))
            .unwrap_or(false);
        let mods_count = self.mods_cache.len();
        let build_count = self.build.len();
        let mc_port = self.shared.mc_port();
        let mc_busy = self.shared.is_mc_busy();
        let mc_stage = self.shared.mc_stage();
        let java_busy = self.java_task.as_ref().map(|t| !t.finished).unwrap_or(false);
        let java_task = self.java_task.as_ref().map(|t| (t.major, t.frac()));
        let total = ui.available_width();
        // 274 right column + 18 gap + 18 right gutter (lines up with the header).
        let left_w = (total - 310.0).max(360.0);
        let face = self.skin_face(ui.ctx()).cloned();

        let mut launch_now = false;
        let mut launch_server = false;
        let mut stop_now = false;
        let mut go_versions = false;
        let mut go_mods = false;
        let mut go_settings = false;
        let mut go_server = false;
        let mut go_build = false;
        let mut switch_profile_id: Option<String> = None;
        let mut open_game = false;
        let mut download_java = false;

        let all_profiles = crate::profiles::load_profiles();
        let active_profile = crate::profiles::get_active_profile(&self.cfg);

        widgets::header(
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
            ui.set_width(ui.available_width());

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 18.0;
            ui.allocate_ui_with_layout(
                Vec2::new(left_w, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(left_w);
                    let frame = egui::Frame::new()
                        .fill(skin.fill())
                        .stroke(Stroke::new(1.0, skin.line_soft()))
                        .corner_radius(CornerRadius::same(16))
                        .inner_margin(egui::Margin::symmetric(20, 18));
                    frame.show(ui, |ui| {
                        ui.set_width(left_w - 42.0);
                        ui.horizontal(|ui| {
                            avatar_face(ui, skin, &nickname, &face, 66.0);
                            ui.add_space(12.0);
                            let info_w = (ui.available_width() - 262.0).max(120.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(info_w, 66.0),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.set_max_width(info_w);
                                    ui.spacing_mut().item_spacing.y = 3.0;
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&nickname)
                                                .font(theme::display(24.0))
                                                .color(skin.ink()),
                                        )
                                        .truncate(),
                                    );
                                    ui.label(
                                        RichText::new(format!("Офлайн-профиль · {}", &uuid[..8]))
                                            .font(theme::mono(9.5))
                                            .color(skin.faint()),
                                    );
                                    ui.add_space(4.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                                        super::loader_badge(ui, skin, spec.loader.id());
                                        widgets::badge(ui, skin, &spec.mc, false);
                                        if selected_installed.is_none() {
                                            widgets::badge(ui, skin, "СКАЧАЕТСЯ", false);
                                        }
                                    });
                                },
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
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
                            });
                        });
                        ui.add_space(16.0);
                        widgets::hairline(ui, skin.line_soft());
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            let w = ui.available_width();
                            let col = w / 3.0;
                            let loader = if spec.version.is_empty() {
                                spec.loader.label().to_string()
                            } else {
                                format!("{} {}", spec.loader.label(), spec.version)
                            };
                            for (label, value) in [
                                ("ВЕРСИЯ", spec.mc.clone()),
                                ("ЗАГРУЗЧИК", loader),
                                (
                                    "JAVA",
                                    java.as_ref()
                                        .map(|j| format!("Java {}", j.major))
                                        .unwrap_or_else(|| "нет".into()),
                                ),
                            ] {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(col, 46.0),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        ui.set_width(col);
                                        widgets::stat(ui, skin, label, &value);
                                    },
                                );
                            }
                        });
                        ui.add_space(8.0);
                        widgets::kv_row(
                            ui,
                            skin,
                            "Память",
                            &format!("{} МБ", self.cfg.memory_mb),
                            false,
                        );
                        widgets::kv_row(
                            ui,
                            skin,
                            "Папка игры",
                            &truncate(&self.cfg.instance_dir().display().to_string(), 58),
                            true,
                        );
                    });

                    ui.add_space(14.0);
                    widgets::section_label(ui, skin, "СБОРКА / МОДПАК");
                    ui.add_space(4.0);

                    // Active profile card
                    let p_frame = egui::Frame::new()
                        .fill(skin.fill())
                        .stroke(Stroke::new(1.0, skin.line_soft()))
                        .corner_radius(CornerRadius::same(14))
                        .inner_margin(egui::Margin::symmetric(16, 12));

                    p_frame.show(ui, |ui| {
                        ui.set_width(left_w - 42.0);
                        ui.horizontal(|ui| {
                            let icon = match active_profile.kind {
                                crate::profiles::ProfileKind::Default => Icon::Cube,
                                crate::profiles::ProfileKind::Custom => Icon::Layers,
                                crate::profiles::ProfileKind::Modpack => Icon::Globe,
                            };
                            let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
                            crate::icons::paint(ui.painter(), icon_rect, icon, skin.ink(), 1.6);
                            ui.add_space(6.0);

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    let kind_badge = match active_profile.kind {
                                        crate::profiles::ProfileKind::Default => "ОСНОВНАЯ",
                                        crate::profiles::ProfileKind::Custom => "СВОЯ СБОРКА",
                                        crate::profiles::ProfileKind::Modpack => "МОДПАК",
                                    };
                                    widgets::badge(ui, skin, kind_badge, false);
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new(&active_profile.name)
                                            .font(theme::display(16.0))
                                            .color(skin.ink()),
                                    );
                                });
                                let mod_count = crate::profiles::list_profile_mods(&self.cfg, &active_profile.id).len();
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {} модов",
                                        active_profile.spec().title(),
                                        mod_count
                                    ))
                                    .font(theme::mono(10.0))
                                    .color(skin.faint()),
                                );
                            });

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::button_sized(ui, skin, "Настроить сборку →", Btn::Outline, true, 28.0, 11.5, 12.0).clicked() {
                                    go_build = true;
                                }
                            });
                        });
                    });
                    ui.add_space(8.0);

                    // Switcher chips row
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                        for p in &all_profiles {
                            let is_active = p.id == active_profile.id;
                            let icon = match p.kind {
                                crate::profiles::ProfileKind::Default => Icon::Cube,
                                crate::profiles::ProfileKind::Custom => Icon::Layers,
                                crate::profiles::ProfileKind::Modpack => Icon::Globe,
                            };
                            let label = truncate(&p.name, 22);
                            if widgets::chip_icon(ui, skin, icon, &label, is_active).clicked() {
                                if !is_active {
                                    switch_profile_id = Some(p.id.clone());
                                }
                            }
                        }
                        if widgets::chip(ui, skin, "+ Создать сборку", false).clicked() {
                            self.build_creating = true;
                            go_build = true;
                        }
                    });

                    if let Some(t) = self.install.as_ref() {
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
                                t.spec.title(),
                                t.total > 0,
                            );
                            let speed = self.speed_label();
                            frame.show(ui, |ui| {
                                ui.set_width(left_w - 30.0);
                                ui.horizontal(|ui| {
                                    widgets::lead_text(
                                        ui,
                                        230.0,
                                        RichText::new(format!("Установка клиента · {stage}"))
                                            .font(theme::body(12.5))
                                            .color(skin.dim()),
                                        false,
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
                                            .font(theme::mono(11.0))
                                            .color(skin.dim()),
                                        );
                                        if !speed.is_empty() {
                                            ui.label(RichText::new(format!("{speed} ·")).font(theme::mono(11.0)).color(skin.ink()));
                                        }
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
                    widgets::section_label(ui, skin, "ВЕРСИЯ ДЛЯ СБОРКИ");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        widgets::status_dot(
                            ui,
                            if selected_installed.is_some() { skin.ink() } else { skin.faint() },
                            selected_installed.is_some(),
                        );
                        ui.label(
                            RichText::new(match &selected_installed {
                                Some(v) => format!("{} · установлена ({})", spec.title(), v.id),
                                None => format!("{} — скачается сама при «Играть»", spec.title()),
                            })
                            .font(theme::body(12.0))
                            .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::button_sized(ui, skin, "Все версии", Btn::Ghost, true, 26.0, 11.5, 10.0).clicked() {
                                go_versions = true;
                            }
                        });
                    });

                    if !java_ok || java_busy {
                        ui.add_space(12.0);
                        let warn = egui::Frame::new()
                            .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 6))
                            .stroke(Stroke::new(1.0, skin.line()))
                            .corner_radius(CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(14, 10));
                        warn.show(ui, |ui| {
                            ui.set_width(left_w - 30.0);
                            if java_busy {
                                if let Some((major, frac)) = java_task {
                                    let speed = self.speed_label();
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(if speed.is_empty() {
                                                format!("Качаю Java {major} — сама установится в лаунчер")
                                            } else {
                                                format!("Качаю Java {major} · {speed} — сама установится в лаунчер")
                                            })
                                            .font(theme::body(12.5))
                                            .color(skin.dim()),
                                        )
                                        .wrap(),
                                    );
                                    ui.add_space(6.0);
                                    widgets::progress_bar(ui, skin, frac);
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    widgets::lead_text(
                                        ui,
                                        150.0,
                                        RichText::new(match &java {
                                            Some(j) => format!(
                                                "Нужна Java {required}, найдена {} — нужная скачается сама при запуске.",
                                                j.version
                                            ),
                                            None => format!("Java {required} скачается сама при запуске."),
                                        })
                                        .font(theme::body(12.5))
                                        .color(skin.dim()),
                                        true,
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if widgets::button_sized(
                                            ui,
                                            skin,
                                            "Скачать Java",
                                            Btn::Outline,
                                            !java_busy,
                                            26.0,
                                            11.5,
                                            10.0,
                                        )
                                        .clicked()
                                        {
                                            download_java = true;
                                        }
                                        if widgets::button_sized(
                                            ui,
                                            skin,
                                            "Настройки",
                                            Btn::Ghost,
                                            true,
                                            26.0,
                                            11.5,
                                            10.0,
                                        )
                                        .clicked()
                                        {
                                            go_settings = true;
                                        }
                                    });
                                });
                            }
                        });
                    }
                },
            );

            // Right column: one card that stretches to the bottom of the view.
            let right_h = ui.available_height().max(220.0);
            ui.allocate_ui_with_layout(
                Vec2::new(274.0, right_h),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(274.0);
                    let frame = theme::card_frame(skin);
                    frame.show(ui, |ui| {
                        ui.set_width(274.0 - 52.0);
                        ui.set_min_height(right_h - 44.0);
                        widgets::section_label(ui, skin, "СОСТОЯНИЕ");
                        ui.add_space(6.0);
                        status_row(
                            ui,
                            skin,
                            "Java",
                            &match &java {
                                Some(j) if java_ok => j.version.clone(),
                                Some(j) => format!("{} · нужна {}", j.version, required),
                                None => format!("скачается {required}"),
                            },
                            java_ok,
                        );
                        status_row(
                            ui,
                            skin,
                            "Версий",
                            &installed.len().to_string(),
                            !installed.is_empty(),
                        );
                        status_row(ui, skin, "Модов", &mods_count.to_string(), mods_count > 0);
                        status_row(
                            ui,
                            skin,
                            "Сборка",
                            &format!("{build_count} в списке"),
                            build_count > 0,
                        );
                        status_row(
                            ui,
                            skin,
                            "Память",
                            &format!("{} МБ", self.cfg.memory_mb),
                            true,
                        );
                        ui.add_space(8.0);
                        widgets::hairline(ui, skin.line_soft());
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            widgets::status_dot(
                                ui,
                                if mc_port > 0 { skin.green() } else { skin.faint() },
                                mc_port > 0,
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(if mc_port > 0 {
                                        format!("127.0.0.1:{mc_port}")
                                    } else if mc_busy {
                                        format!("{mc_stage}")
                                    } else {
                                        "127.0.0.1:—".to_string()
                                    })
                                    .font(theme::mono(11.0))
                                    .color(if mc_port > 0 { skin.green() } else { skin.ink() }),
                                )
                                .truncate(),
                            );
                        });
                        ui.label(
                            RichText::new("локальный сервер Мадрид")
                                .font(theme::mono(8.5))
                                .color(skin.faint()),
                        );
                    });
                    ui.add_space(12.0);
                    if widgets::button(ui, skin, "Все версии", Btn::Outline, true).clicked() {
                        go_versions = true;
                    }
                    ui.add_space(6.0);
                    if widgets::button(ui, skin, "Моды", Btn::Ghost, true).clicked() {
                        go_mods = true;
                    }
                    ui.add_space(6.0);
                    if widgets::button(ui, skin, "Сервер", Btn::Ghost, true).clicked() {
                        go_server = true;
                    }
                },
            );
        });

        ui.add_space(14.0);
        let lines = self.shared.game_log_tail(240);
        let height = (ui.available_height() - 64.0).clamp(48.0, 220.0);
            if console(
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
        });

        if download_java {
            self.start_java_download(ui.ctx(), required);
        }
        if open_game {
            util::open_path(&self.cfg.game_dir());
        }
        if let Some(target) = switch_profile_id {
            match crate::profiles::switch_profile(&mut self.cfg, &target) {
                Ok(_) => {
                    self.toast("Сборка активирована");
                    self.touch_mods();
                }
                Err(e) => {
                    self.toast(format!("Ошибка переключения: {e}"));
                }
            }
        }
        if go_build {
            self.go(crate::app::Tab::Build);
        }
        if go_versions {
            self.go(crate::app::Tab::Versions);
        }
        if go_mods {
            self.go(crate::app::Tab::Mods);
        }
        if go_server {
            self.go(crate::app::Tab::Server);
        }
        if go_settings {
            self.go(crate::app::Tab::Settings);
        }
        if launch_now {
            self.launch_game(false);
        }
        if launch_server {
            self.launch_game(true);
        }
        if stop_now {
            self.stop_game();
        }
    }
}

/// Pixel head, or the real face of the chosen skin when there is one.
fn avatar_face(
    ui: &mut Ui,
    skin: &Skin,
    name: &str,
    face: &Option<egui::TextureHandle>,
    side: f32,
) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
    if let Some(tex) = face {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), skin.fill());
        let inner = rect.shrink(side * 0.06);
        ui.painter().image(
            tex.id(),
            inner,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        let mut hash = 0u64;
        for b in name.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(b as u64);
        }
        ui.painter()
            .rect_filled(rect, CornerRadius::same(14), skin.fill());
        let inner = rect.shrink(side * 0.10);
        let cell = inner.width() / 8.0;
        let hair = Color32::from_gray(64 + (hash % 56) as u8);
        let face_c = Color32::from_gray(198);
        let dark = Color32::from_gray(16);
        for row in 0..8u32 {
            for col in 0..8u32 {
                let color = match (row, col) {
                    (0..=1, _) => hair,
                    (_, 0) | (_, 7) => hair,
                    (4, 2) | (4, 5) => dark,
                    (6, 3) | (6, 4) => dark,
                    _ => face_c,
                };
                let cell_rect = Rect::from_min_size(
                    inner.min + Vec2::new(col as f32 * cell, row as f32 * cell),
                    Vec2::splat(cell),
                );
                ui.painter().rect_filled(cell_rect, 0, color);
            }
        }
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(14),
        Stroke::new(1.0, skin.line()),
        egui::StrokeKind::Inside,
    );
}

fn status_row(ui: &mut Ui, skin: &Skin, label: &str, value: &str, ok: bool) {
    ui.horizontal(|ui| {
        widgets::status_dot(ui, if ok { skin.green() } else { skin.faint() }, ok);
        ui.label(
            RichText::new(label)
                .font(theme::body(12.0))
                .color(skin.dim()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .font(theme::mono(10.0))
                    .color(skin.ink()),
            );
        });
    });
}
