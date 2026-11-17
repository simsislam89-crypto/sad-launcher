//! Versions — the version that fits the active build, Pomme, manual installs.

use super::{search_field, short_date, truncate};
use crate::app::Launcher;
use crate::icons::Icon;
use crate::loader::{Loader, Spec};
use crate::minecraft::{self, InstalledVersion, VersionSummary};
use crate::theme::{self, Skin};
use crate::util;
use crate::widgets::{self, Btn};
use egui::{Align, Align2, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::path::PathBuf;

const LOADERS: [Loader; 5] = [Loader::Forge, Loader::NeoForge, Loader::Fabric, Loader::Quilt, Loader::Vanilla];

impl Launcher {
    pub fn view_versions(&mut self, ui: &mut Ui, skin: &Skin) {
        let installed = self.installed();
        let versions = self.shared.versions();
        let loading = self.shared.versions_loading();
        let error = self.shared.versions_error();
        let profile = crate::profiles::get_active_profile(&self.cfg);
        let spec = profile.spec();
        let compatible = self.compatible_version();
        let mut refresh = false;
        let mut play_compatible = false;
        let mut fetch_compatible = false;
        let mut install_pomme = false;
        let mut play_pomme = false;
        let mut delete_pomme = false;

        widgets::header(
            ui,
            skin,
            "Версии",
            "Версия под сборку ставится сама — или выберите любую вручную",
            |ui| {
                if widgets::button(ui, skin, "Обновить список", Btn::Ghost, true).clicked() {
                    refresh = true;
                }
            },
        );
        if refresh {
            minecraft::spawn_manifest(self.shared.clone());
        }

        let install_view = self.install.as_ref().map(|t| {
            (t.spec.title(), t.stage.clone(), t.finished, t.frac(), t.total, t.error.clone(), t.spec == spec)
        });
        let installing_compatible = install_view.as_ref().map(|v| v.6 && !v.2).unwrap_or(false);

        // ---- the version this build needs
        let hero = egui::Frame::new()
            .fill(skin.fill())
            .stroke(Stroke::new(1.0, skin.line()))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin::symmetric(18, 16));
        hero.show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
                ui.painter().rect_filled(r, CornerRadius::same(10), skin.hover());
                crate::icons::paint(ui.painter(), r.shrink(8.0), Icon::Layers, skin.ink(), 1.5);
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.label(
                        RichText::new(format!("Для сборки «{}»", truncate(&profile.name, 32)))
                            .font(theme::body(12.0))
                            .color(skin.faint()),
                    );
                    ui.label(RichText::new(spec.title()).font(theme::display(18.0)).color(skin.ink()));
                    ui.label(
                        RichText::new(match (&compatible, installing_compatible) {
                            (_, true) => "ставится сейчас — игра откроется сама".to_string(),
                            (Some(id), _) => format!("установлена · {id}"),
                            (None, _) => "не установлена — скачается при запуске вместе с Java".to_string(),
                        })
                        .font(theme::mono(9.5))
                        .color(skin.faint()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button(ui, skin, "Играть", Btn::Primary, !installing_compatible).clicked() {
                        play_compatible = true;
                    }
                    if compatible.is_none()
                        && !installing_compatible
                        && widgets::button(ui, skin, "Только скачать", Btn::Outline, true).clicked()
                    {
                        fetch_compatible = true;
                    }
                });
            });
        });
        ui.add_space(10.0);

        if let Some((title, stage, finished, frac, total, err, _)) = &install_view {
            theme::card_frame(skin).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{title} · {}", truncate(stage, 60)))
                            .font(theme::body(12.5))
                            .color(skin.dim()),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if !*finished && widgets::button_sized(ui, skin, "Отменить", Btn::Ghost, true, 24.0, 10.5, 9.5).clicked() {
                            self.cancel_active_downloads();
                        }
                        let label = if *finished {
                            "готово".to_string()
                        } else if *total > 0 {
                            format!("{}%", (frac * 100.0).round() as u32)
                        } else {
                            "…".to_string()
                        };
                        ui.label(RichText::new(label).font(theme::mono(10.0)).color(skin.faint()));
                    });
                });
                ui.add_space(6.0);
                widgets::progress_bar(ui, skin, if *finished { 1.0 } else { *frac });
                if let Some(err) = err {
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(RichText::new(truncate(err, 300)).font(theme::body(11.5)).color(skin.red())).wrap());
                }
            });
            ui.add_space(10.0);
        }

        // ---- Pomme
        let pomme_ready = crate::pomme::installed();
        let pomme_task = self.pomme_task.as_ref().map(|t| (t.stage.clone(), t.frac(), t.finished));
        theme::card_frame(skin).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::status_dot(ui, if pomme_ready { skin.green() } else { skin.faint() }, pomme_ready);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(RichText::new("Pomme · Minecraft на Rust + Vulkan").font(theme::body(13.0)).color(skin.ink()));
                    ui.label(
                        RichText::new(if pomme_ready {
                            "без Java и без сервера: открывает последний мир сразу"
                        } else {
                            "один exe ~30 МБ, встроенный одиночный мир"
                        })
                        .font(theme::mono(9.0))
                        .color(skin.faint()),
                    );
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let downloading = pomme_task.as_ref().map(|t| !t.2).unwrap_or(false);
                    if pomme_ready {
                        if widgets::button_sized(ui, skin, "Играть", Btn::Primary, true, 28.0, 11.5, 13.0).clicked() {
                            play_pomme = true;
                        }
                        if widgets::button_sized(ui, skin, "Удалить", Btn::Ghost, true, 28.0, 11.0, 10.0).clicked() {
                            delete_pomme = true;
                        }
                    } else if widgets::button_sized(ui, skin, if downloading { "Скачиваю…" } else { "Скачать" }, Btn::Outline, !downloading, 28.0, 11.5, 12.0).clicked() {
                        install_pomme = true;
                    }
                });
            });
            if let Some((stage, frac, false)) = &pomme_task {
                ui.add_space(6.0);
                ui.label(RichText::new(stage).font(theme::mono(9.5)).color(skin.faint()));
                widgets::progress_bar(ui, skin, *frac);
            }
            if pomme_ready {
                ui.add_space(4.0);
                ui.add(egui::Label::new(
                    RichText::new("Официальный Pomme один раз просит войти в аккаунт Microsoft — это требование самого клиента, лаунчер его не обходит. Вход запоминается.")
                        .font(theme::body(10.5))
                        .color(skin.faint()),
                ).wrap());
            }
        });
        ui.add_space(12.0);

        // ---- manual installs
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 28.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                ui.label(RichText::new("ЗАГРУЗЧИК").font(theme::mono(9.5)).color(skin.faint()));
                let current = Loader::parse(&self.cfg.loader);
                for l in LOADERS {
                    if widgets::chip(ui, skin, l.label(), current == l).clicked() && current != l {
                        self.cfg.loader = l.id().into();
                        self.save_cfg();
                    }
                }
            },
        );
        ui.add_space(6.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 32.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                let field_w = (ui.available_width() - 120.0).clamp(180.0, 320.0);
                let resp = search_field(ui, skin, "version-search", &mut self.versions_filter, "Поиск версии…", field_w);
                if self.focus_search {
                    self.focus_search = false;
                    resp.request_focus();
                }
                if widgets::chip(ui, skin, "Снапшоты", self.versions_all).clicked() {
                    self.versions_all = !self.versions_all;
                }
            },
        );
        ui.add_space(10.0);

        let mut confirm = self.confirm_delete.clone();
        let mut install_req: Option<Spec> = None;
        let mut delete_req: Option<String> = None;
        let mut play_req: Option<String> = None;
        let mut folder_req: Option<PathBuf> = None;
        let loader = Loader::parse(&self.cfg.loader);

        egui::ScrollArea::vertical()
            .id_salt("versions-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                widgets::section_label(ui, skin, &format!("УСТАНОВЛЕНЫ · {}", installed.len()));
                ui.add_space(4.0);
                if installed.is_empty() {
                    widgets::empty_state(
                        ui,
                        skin,
                        Icon::Cube,
                        "Нет установленных версий",
                        "Нажмите «Играть» выше — нужная версия скачается сама.",
                    );
                } else {
                    for v in &installed {
                        let is_confirm = confirm.as_deref() == Some(v.id.as_str());
                        let fits = compatible.as_deref() == Some(v.id.as_str());
                        let (play, folder, delete) = installed_row(ui, skin, v, is_confirm, fits);
                        if play {
                            play_req = Some(v.id.clone());
                        }
                        if folder {
                            folder_req = Some(self.cfg.game_dir().join("versions").join(&v.id));
                        }
                        if delete {
                            if is_confirm {
                                delete_req = Some(v.id.clone());
                                confirm = None;
                            } else {
                                confirm = Some(v.id.clone());
                            }
                        }
                    }
                }

                ui.add_space(16.0);
                widgets::section_label(ui, skin, &format!("ДОСТУПНЫ · {}", loader.label().to_uppercase()));
                ui.add_space(4.0);
                if versions.is_empty() && loading {
                    ui.add_space(10.0);
                    ui.label(RichText::new("Загружаю список Mojang…").font(theme::body(12.5)).color(skin.faint()));
                } else if versions.is_empty() && !error.is_empty() {
                    theme::card_frame(skin).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(format!("Список версий недоступен: {}", truncate(&error, 160)))
                                .font(theme::body(12.0))
                                .color(skin.dim()),
                        );
                        ui.label(
                            RichText::new("Проверьте интернет. Уже скачанное продолжает работать.")
                                .font(theme::body(11.0))
                                .color(skin.faint()),
                        );
                    });
                } else {
                    let filter = self.versions_filter.trim().to_lowercase();
                    let list: Vec<&VersionSummary> = versions
                        .iter()
                        .filter(|v| self.versions_all || v.kind == "release")
                        .filter(|v| filter.is_empty() || v.id.to_lowercase().contains(&filter))
                        .take(120)
                        .collect();
                    let shown = list.len();
                    let installed_specs = minecraft::as_installed(&installed);
                    for v in list {
                        let s = Spec::new(&v.id, loader, "");
                        let installed_here = crate::loader::pick_installed(&installed_specs, &s).is_some();
                        let installing = self
                            .install
                            .as_ref()
                            .map(|t| t.spec == s && !t.finished)
                            .unwrap_or(false);
                        if available_row(ui, skin, v, installed_here, installing) {
                            install_req = Some(s);
                        }
                    }
                    if shown == 0 {
                        ui.add_space(8.0);
                        ui.label(RichText::new("Ничего не найдено.").font(theme::body(12.0)).color(skin.faint()));
                    } else if versions.len() > shown && filter.is_empty() {
                        ui.add_space(8.0);
                        ui.label(RichText::new("Показаны первые 120 — уточните поиск.").font(theme::mono(9.0)).color(skin.faint()));
                    }
                }
                ui.add_space(12.0);
            });

        self.confirm_delete = confirm;

        if play_compatible {
            self.launch_game(false);
        }
        if fetch_compatible {
            self.start_install(ui.ctx(), spec, false);
        }
        if install_pomme {
            self.start_pomme_install(ui.ctx());
        }
        if play_pomme {
            self.launch_pomme();
        }
        if delete_pomme {
            match crate::pomme::remove() {
                Ok(_) => self.toast("Pomme удалён"),
                Err(e) => self.toast(format!("Не удалить: {e}")),
            }
        }
        if let Some(s) = install_req {
            self.start_install(ui.ctx(), s, false);
        }
        if let Some(id) = play_req {
            self.cfg.selected_version = id;
            self.save_cfg();
            self.launch_to(None);
        }
        if let Some(dir) = folder_req {
            util::open_path(&dir);
        }
        if let Some(id) = delete_req {
            let dir = self.cfg.game_dir().join("versions").join(&id);
            // Instant: the folder is renamed away, files go in the background.
            crate::remove::queue(format!("Версия {id}"), &dir);
            if self.cfg.selected_version == id {
                self.cfg.selected_version.clear();
            }
            self.save_cfg();
            self.refresh_installed();
        }
    }
}

fn installed_row(ui: &mut Ui, skin: &Skin, version: &InstalledVersion, confirm: bool, fits: bool) -> (bool, bool, bool) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 50.0), Sense::hover());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.14);
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(10),
        theme::lerp_color(skin.hover().gamma_multiply(0.45), skin.hover(), hover),
    );
    let loader = Loader::parse(&version.loader);
    let painter = ui.painter().with_clip_rect(Rect::from_min_max(rect.min, Pos2::new(rect.right() - 300.0, rect.bottom())));
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        truncate(&version.id, 46),
        theme::body(13.0),
        skin.ink(),
    );
    let meta = format!(
        "{} · {}{} · {}{}",
        version.mc,
        loader.label(),
        if version.loader_version.is_empty() { String::new() } else { format!(" {}", version.loader_version) },
        util::human_bytes(version.size),
        if fits { " · для текущей сборки" } else { "" }
    );
    painter.text(Pos2::new(rect.left() + 14.0, rect.center().y + 9.0), Align2::LEFT_CENTER, meta, theme::mono(9.0), skin.faint());
    let mut play = false;
    let mut folder = false;
    let mut delete = false;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(Pos2::new(rect.right() - 300.0, rect.center().y - 15.0), Vec2::new(288.0, 30.0)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    if widgets::button_sized(&mut child, skin, "Играть", Btn::Primary, true, 28.0, 11.5, 12.0).clicked() {
        play = true;
    }
    if widgets::button_sized(&mut child, skin, "Папка", Btn::Ghost, true, 28.0, 11.5, 10.0).clicked() {
        folder = true;
    }
    if widgets::button_sized(&mut child, skin, if confirm { "Точно?" } else { "Удалить" }, if confirm { Btn::Outline } else { Btn::Ghost }, true, 28.0, 11.5, 10.0).clicked() {
        delete = true;
    }
    ui.add_space(4.0);
    (play, folder, delete)
}

fn available_row(ui: &mut Ui, skin: &Skin, version: &VersionSummary, installed: bool, installing: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::hover());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.14);
    if hover > 0.01 {
        ui.painter().rect_filled(rect, CornerRadius::same(8), skin.hover().gamma_multiply(hover * 0.6));
    }
    widgets::text_fit(
        ui.painter(),
        Pos2::new(rect.left() + 10.0, rect.center().y),
        Align2::LEFT_CENTER,
        &version.id,
        theme::mono(11.5),
        if installed { skin.dim() } else { skin.ink() },
        222.0,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 240.0, rect.center().y),
        Align2::LEFT_CENTER,
        short_date(&version.release_time),
        theme::mono(9.5),
        skin.faint(),
    );
    let kind = match version.kind.as_str() {
        "release" => "RELEASE",
        "snapshot" => "SNAPSHOT",
        _ => "OLD",
    };
    let badge_rect = Rect::from_min_size(Pos2::new(rect.left() + 330.0, rect.center().y - 9.0), Vec2::new(76.0, 18.0));
    ui.painter().rect_stroke(badge_rect, CornerRadius::same(9), Stroke::new(1.0, skin.line_soft()), egui::StrokeKind::Inside);
    ui.painter().text(badge_rect.center(), Align2::CENTER_CENTER, kind, theme::mono(8.0), skin.faint());

    let mut clicked = false;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(Pos2::new(rect.right() - 124.0, rect.center().y - 15.0), Vec2::new(118.0, 30.0)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    if installed {
        child.label(RichText::new("Установлена").font(theme::mono(9.0)).color(skin.faint()));
    } else if installing {
        child.label(RichText::new("Загрузка…").font(theme::mono(9.0)).color(skin.dim()));
    } else if widgets::button_sized(&mut child, skin, "Установить", Btn::Outline, true, 28.0, 11.5, 11.0).clicked() {
        clicked = true;
    }
    ui.painter().line_segment(
        [Pos2::new(rect.left(), rect.bottom()), Pos2::new(rect.right(), rect.bottom())],
        Stroke::new(1.0, skin.line_soft()),
    );
    clicked
}
