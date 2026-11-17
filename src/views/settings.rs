//! Настройки — profile, Java, memory, folders, CurseForge key, window.

use crate::app::Launcher;
use crate::config::{self, DEFAULT_MEMORY_MB};
use crate::java;
use crate::theme::{self, Skin};
use crate::util;
use crate::widgets::{self, Btn, FieldStyle};
use egui::{Align, Layout, RichText, Ui, Vec2};

impl Launcher {
    pub fn view_settings(&mut self, ui: &mut Ui, skin: &Skin) {
        let mut save = false;
        let mut detect = false;
        let mut download_java = false;
        let mut open_data = false;
        let mut reset = false;
        let mut restart_server = false;
        let mut pick_wallpaper = false;
        let mut clear_wallpaper = false;
        let mut pick_skin = false;
        let mut make_shortcut = false;
        let mut uninstall = false;
        // Java of the ACTIVE build, from its release — no file reads per frame.
        let required = java::required_for(&self.active_spec().mc);

        widgets::header(
            ui,
            skin,
            "Настройки",
            crate::platform::text::DATA_LOCAL,
            |ui| {
                if widgets::button(ui, skin, "Сохранить", Btn::Primary, true).clicked() {
                    save = true;
                }
            },
        );

        egui::ScrollArea::vertical()
            .id_salt("settings-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // ------------------------------------------------------ profile
                widgets::section_label(ui, skin, "ПРОФИЛЬ");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    field_row(
                        ui,
                        skin,
                        "Никнейм",
                        "Офлайн-профиль · 3–16 символов: буквы, цифры, _",
                        &mut self.cfg.nickname,
                        false,
                    );
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("UUID")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(crate::minecraft::offline_uuid(
                                    &self.cfg.nickname_or_default(),
                                ))
                                .font(theme::mono(10.0))
                                .color(skin.faint()),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Microsoft-вход не используется: профиль полностью офлайн.")
                            .font(theme::body(11.0))
                            .color(skin.faint()),
                    );
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "JAVA");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let spec = self.active_spec();
                    let java_now = crate::java::fitting_cached(required, spec.loader).or_else(|| self.shared.java());
                    let java_busy = self.java_task.as_ref().map(|t| !t.finished).unwrap_or(false);
                    ui.horizontal(|ui| {
                        widgets::status_dot(
                            ui,
                            if java_now.is_some() { skin.ink() } else { skin.faint() },
                            java_now.is_some(),
                        );
                        ui.label(
                            RichText::new(crate::app::java_label(&java_now))
                                .font(theme::body(12.5))
                                .color(skin.ink()),
                        );
                        let btn_label = if java_busy {
                            "Качаю…".to_string()
                        } else {
                            format!("Скачать Java {required}")
                        };
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::button_sized(
                                ui,
                                skin,
                                &btn_label,
                                Btn::Outline,
                                !java_busy,
                                26.0,
                                11.0,
                                10.0,
                            )
                            .clicked()
                            {
                                download_java = true;
                            }
                        });
                    });
                    if java_busy {
                        ui.add_space(8.0);
                        let frac = self
                            .java_task
                            .as_ref()
                            .map(|t| t.frac())
                            .unwrap_or(0.05);
                        widgets::progress_bar(ui, skin, frac);
                    }
                    ui.add_space(10.0);
                    field_row(
                        ui,
                        skin,
                        crate::platform::text::JAVA_PATH,
                        crate::platform::text::JAVA_AUTO,
                        &mut self.cfg.java_path,
                        false,
                    );
                    ui.add_space(8.0);
                    if widgets::button_sized(
                        ui,
                        skin,
                        "Найти Java заново",
                        Btn::Ghost,
                        true,
                        26.0,
                        11.5,
                        11.0,
                    )
                    .clicked()
                    {
                        detect = true;
                    }
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "ПРОИЗВОДИТЕЛЬНОСТЬ");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Память")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{} МБ", self.cfg.memory_mb))
                                    .font(theme::mono(12.0))
                                    .color(skin.ink()),
                            );
                        });
                    });
                    let mut mem = self.cfg.memory_mb as f32;
                    if widgets::slider(ui, skin, &mut mem, 1024.0, 16384.0).changed() {
                        self.cfg.memory_mb = ((mem / 512.0).round() as u32 * 512).max(1024);
                        save = true;
                    }
                    ui.horizontal(|ui| {
                        if widgets::button_sized(
                            ui,
                            skin,
                            "4 ГБ",
                            Btn::Ghost,
                            true,
                            24.0,
                            11.0,
                            10.0,
                        )
                        .clicked()
                        {
                            self.cfg.memory_mb = DEFAULT_MEMORY_MB;
                            save = true;
                        }
                        if widgets::button_sized(
                            ui,
                            skin,
                            "8 ГБ",
                            Btn::Ghost,
                            true,
                            24.0,
                            11.0,
                            10.0,
                        )
                        .clicked()
                        {
                            self.cfg.memory_mb = 8192;
                            save = true;
                        }
                        ui.label(
                            RichText::new("Xms = половина Xmx")
                                .font(theme::mono(9.0))
                                .color(skin.faint()),
                        );
                    });
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "ПАПКА ИГРЫ");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    field_row(
                        ui,
                        skin,
                        "Путь",
                        crate::platform::text::GAME_DIR_EMPTY,
                        &mut self.cfg.game_dir,
                        false,
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if widgets::button_sized(
                            ui,
                            skin,
                            "Открыть папку",
                            Btn::Ghost,
                            true,
                            26.0,
                            11.0,
                            10.0,
                        )
                        .clicked()
                        {
                            open_data = true;
                        }
                        ui.label(
                            RichText::new(self.cfg.game_dir().display().to_string())
                                .font(theme::mono(9.0))
                                .color(skin.faint()),
                        );
                    });
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "CURSEFORGE");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    field_row(
                        ui,
                        skin,
                        "API-ключ",
                        "Ключ не нужен: CurseForge идёт через публичное зеркало",
                        &mut self.cfg.cf_api_key,
                        true,
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "Без ключа всё работает через api.curse.tools (публичное зеркало). Ключ нужен только для прямых вызовов api.curseforge.com.",
                        )
                        .font(theme::body(11.0))
                        .color(skin.faint()),
                    );
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "ОКНО И СЕРВЕР");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Прозрачность фона")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{}%",
                                    (self.cfg.transparency * 100.0).round() as u32
                                ))
                                .font(theme::mono(12.0))
                                .color(skin.ink()),
                            );
                        });
                    });
                    let mut alpha = self.cfg.transparency;
                    if widgets::slider(ui, skin, &mut alpha, 0.20, 0.99).changed() {
                        self.cfg.transparency = alpha;
                        save = true;
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Порт сервера")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut port_text = self.cfg.server_port.to_string();
                            let resp = widgets::compact_input(
                                ui,
                                skin,
                                "port-field",
                                &mut port_text,
                                "25565",
                                74.0,
                                28.0,
                                theme::mono(12.0),
                            );
                            if resp.changed() {
                                if let Ok(p) = port_text.trim().parse::<u16>() {
                                    if p >= 1024 {
                                        self.cfg.server_port = p;
                                        save = true;
                                    }
                                }
                            }
                            if widgets::button_sized(
                                ui,
                                skin,
                                "Применить",
                                Btn::Ghost,
                                true,
                                24.0,
                                10.5,
                                9.0,
                            )
                            .clicked()
                            {
                                restart_server = true;
                            }
                        });
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Сервер стартует с лаунчером")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut on = self.cfg.auto_server;
                            if widgets::toggle(ui, skin, &mut on).changed() {
                                self.cfg.auto_server = on;
                                save = true;
                            }
                        });
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Память сервера")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!("{} МБ", self.cfg.server_memory_mb))
                                    .font(theme::mono(12.0))
                                    .color(skin.ink()),
                            );
                        });
                    });
                    let smem_before = self.cfg.server_memory_mb;
                    let mut smem = self.cfg.server_memory_mb as f32;
                    let smem_resp = widgets::slider(ui, skin, &mut smem, 1024.0, 8192.0);
                    if smem_resp.changed() {
                        self.cfg.server_memory_mb =
                            ((smem / 512.0).round() as u32 * 512).max(1024);
                        save = true;
                    }
                    // New RAM applies as soon as the slider is released.
                    if smem_resp.drag_stopped() && self.cfg.server_memory_mb != smem_before {
                        save = false;
                        self.save_cfg();
                        crate::server::restart(&self.shared);
                        self.toast("Память сервера применена — перезапускаю");
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(crate::platform::text::AUTOSTART)
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut on = self.cfg.autostart;
                            if widgets::toggle(ui, skin, &mut on).changed() {
                                self.cfg.autostart = on;
                                util::set_autostart(on);
                                save = true;
                            }
                        });
                    });
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Ярлык на рабочем столе")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::button_sized(
                                ui,
                                skin,
                                "Создать",
                                Btn::Ghost,
                                true,
                                24.0,
                                10.5,
                                9.0,
                            )
                            .clicked()
                            {
                                make_shortcut = true;
                            }
                        });
                    });
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "ОБОИ И СКИН");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Обои лаунчера")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::button_sized(
                                ui,
                                skin,
                                "Выбрать",
                                Btn::Outline,
                                true,
                                26.0,
                                11.0,
                                10.0,
                            )
                            .clicked()
                            {
                                pick_wallpaper = true;
                            }
                            if widgets::button_sized(
                                ui,
                                skin,
                                "Убрать",
                                Btn::Ghost,
                                !self.cfg.wallpaper.is_empty(),
                                26.0,
                                11.0,
                                10.0,
                            )
                            .clicked()
                            {
                                clear_wallpaper = true;
                            }
                        });
                    });
                    if !self.cfg.wallpaper.is_empty() {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(super::truncate(&self.cfg.wallpaper, 64))
                                .font(theme::mono(9.0))
                                .color(skin.faint()),
                        );
                    }
                    ui.add_space(10.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Скин персонажа")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::button_sized(
                                ui,
                                skin,
                                "Выбрать PNG",
                                Btn::Outline,
                                true,
                                26.0,
                                11.0,
                                10.0,
                            )
                            .clicked()
                            {
                                pick_skin = true;
                            }
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "Скин попадает в папку игры → skins. В игре: Настройки → Скины → выбрать файл.",
                        )
                        .font(theme::body(11.0))
                        .color(skin.faint()),
                    );
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "ХРАНИЛИЩЕ");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let root = config::data_dir();
                    let total = self.storage_size();
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Весит всё вместе")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(util::human_bytes(total))
                                    .font(theme::mono(12.0))
                                    .color(skin.ink()),
                            );
                        });
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "{}",
                            super::truncate(&root.display().to_string(), 66)
                        ))
                        .font(theme::mono(9.0))
                        .color(skin.faint()),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if widgets::button_sized(
                            ui,
                            skin,
                            "Очистить кэш",
                            Btn::Outline,
                            true,
                            26.0,
                            11.0,
                            10.0,
                        )
                        .clicked()
                        {
                            let freed = util::clear_cache(&root);
                            self.toast(format!("Освобождено {}", util::human_bytes(freed)));
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new("установщики Forge и старые логи; игра докачается сама")
                                    .font(theme::mono(9.0))
                                    .color(skin.faint()),
                            )
                            .truncate(),
                        );
                    });
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "Всё лишнее не хранится: версии, библиотеки и ресурсы лежат на серверах Mojang и Forge и качаются по требованию.",
                        )
                        .font(theme::body(11.0))
                        .color(skin.faint()),
                    );
                });

                ui.add_space(16.0);
                widgets::section_label(ui, skin, "СБРОС");
                ui.add_space(4.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Вернуть настройки по умолчанию")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let label = if self.confirm_reset {
                                "Точно сбросить?"
                            } else {
                                "Сбросить"
                            };
                            if widgets::button_sized(
                                ui,
                                skin,
                                label,
                                if self.confirm_reset {
                                    Btn::Primary
                                } else {
                                    Btn::Ghost
                                },
                                true,
                                26.0,
                                11.0,
                                10.0,
                            )
                            .clicked()
                            {
                                if self.confirm_reset {
                                    reset = true;
                                    self.confirm_reset = false;
                                } else {
                                    self.confirm_reset = true;
                                }
                            }
                        });
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Игровые файлы, версии и моды не удаляются.")
                            .font(theme::body(11.0))
                            .color(skin.faint()),
                    );
                    ui.add_space(10.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Удалить лаунчер и все его файлы")
                                .font(theme::body(13.0))
                                .color(skin.dim()),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let label = if self.confirm_uninstall {
                                "Точно удалить всё?"
                            } else {
                                "Удалить"
                            };
                            if widgets::button_sized(
                                ui,
                                skin,
                                label,
                                if self.confirm_uninstall {
                                    Btn::Primary
                                } else {
                                    Btn::Ghost
                                },
                                true,
                                26.0,
                                11.0,
                                10.0,
                            )
                            .clicked()
                            {
                                if self.confirm_uninstall {
                                    uninstall = true;
                                    self.confirm_uninstall = false;
                                } else {
                                    self.confirm_uninstall = true;
                                }
                            }
                        });
                    });
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(
                            "Сотрёт игру, сервер, Java, кэш, ярлык и автозапуск — без следов.",
                        )
                        .font(theme::body(11.0))
                        .color(skin.faint()),
                    );
                });
                ui.add_space(14.0);
            });

        if save {
            if !crate::util::nickname_valid(&self.cfg.nickname) {
                self.toast("Ник должен быть 3–16 символов: буквы, цифры, _");
            } else {
                self.save_cfg();
            }
        }
        if detect {
            self.reprobe_java();
            if self.shared.java().is_some() {
                self.toast("Java найдена");
            } else {
                self.toast("Java не найдена — укажите путь вручную");
            }
        }
        if download_java {
            self.start_java_download(ui.ctx(), required);
        }
        if open_data {
            util::open_path(&self.cfg.game_dir());
        }
        if restart_server {
            self.save_cfg();
            crate::server::restart(&self.shared);
            self.toast("Сервер перезапускается на новом порту");
        }
        if pick_wallpaper {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Обои лаунчера")
                .add_filter("Изображения", &["png", "jpg", "jpeg"])
                .pick_file()
            {
                self.cfg.wallpaper = path.display().to_string();
                self.wallpaper_key.clear();
                self.save_cfg();
            }
        }
        if clear_wallpaper {
            self.cfg.wallpaper.clear();
            self.save_cfg();
        }
        if pick_skin {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Скин персонажа")
                .add_filter("Скин Minecraft", &["png"])
                .pick_file()
            {
                let skins = self.cfg.game_dir().join("skins");
                let _ = std::fs::create_dir_all(&skins);
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "skin".into());
                let dest = skins.join(format!("{name}.png"));
                match std::fs::copy(&path, &dest) {
                    Ok(_) => {
                        self.cfg.skin_file = dest.display().to_string();
                        self.skin_key.clear();
                        self.save_cfg();
                        self.toast(format!("Скин установлен: {name}.png"));
                    }
                    Err(e) => self.toast(format!("Скин не скопировался: {e}")),
                }
            }
        }
        if make_shortcut {
            if util::create_desktop_shortcut() {
                self.cfg.shortcut_done = true;
                self.save_cfg();
                self.toast("Ярлык создан на рабочем столе");
            } else {
                self.toast("Не удалось создать ярлык");
            }
        }
        if reset {
            self.cfg = config::Config::default();
            self.save_cfg();
            self.reprobe_java();
            self.refresh_installed();
            self.toast("Настройки сброшены");
        }
        if uninstall {
            crate::util::uninstall_everything(&self.shared);
        }
    }
}

fn field_row(
    ui: &mut Ui,
    skin: &Skin,
    label: &str,
    hint: &str,
    buf: &mut String,
    secret: bool,
) {
    let field_w = (ui.available_width() - 180.0).clamp(240.0, 460.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .font(theme::body(13.0))
                .color(skin.dim()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(field_w, 34.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    widgets::text_field(
                        ui,
                        skin,
                        &format!("field-{label}"),
                        buf,
                        hint,
                        FieldStyle {
                            rows: 1,
                            font: 12.5,
                            lead: 8.0,
                            secret,
                        },
                    );
                },
            );
        });
    });
}
