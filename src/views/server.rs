//! Игра с другом и локальный сервер Мадрид.

use crate::app::{FriendState, Launcher};
use crate::theme::{self, Skin};
use crate::util;
use crate::widgets::{self, Btn};
use egui::{Align, CornerRadius, Layout, RichText, Stroke, Ui, Vec2};

impl Launcher {
    pub fn view_server(&mut self, ui: &mut Ui, skin: &Skin) {
        let running = self.shared.is_mc_running();
        let busy = self.shared.is_mc_busy();
        let stage = self.shared.mc_stage();
        let port = self.shared.mc_port();
        let cfg_port = self.cfg.server_port;
        let uptime = util::human_uptime(self.shared.mc_uptime_secs());
        let error = self.shared.mc_error();
        let logs = self.shared.mc_log_tail(240);
        let mods_count = crate::profiles::list_profile_mods(&self.cfg, "default").len();
        let server_dir = self.cfg.server_dir();

        let mut start = false;
        let mut stop = false;
        let mut connect = false;
        let mut copy_list = false;
        let mut open_folder = false;
        let mut copy_local_server = false;

        widgets::header(
            ui,
            skin,
            "Игра с другом",
            "Комната по коду, прямое подключение по адресу и локальный сервер ниже",
            |_| {},
        );

        egui::ScrollArea::vertical()
            .id_salt("server-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // ------------------------------------------------ Игра с другом (сверху)
                self.friends_play_section(ui, skin);
                ui.add_space(16.0);

                // ------------------------------------------------ Локальный сервер (ниже)
                widgets::section_label(ui, skin, "ЛОКАЛЬНЫЙ СЕРВЕР МАДРИД (ЕСЛИ НУЖЕН СВОЙ ХОСТ)");
                ui.add_space(6.0);
                let frame = theme::card_frame(skin);
                frame.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 32.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            widgets::status_dot(
                                ui,
                                if running {
                                    skin.green()
                                } else if error.is_some() {
                                    skin.red()
                                } else {
                                    skin.faint()
                                },
                                running,
                            );
                            ui.label(
                                RichText::new(if running {
                                    "СЕРВЕР РАБОТАЕТ"
                                } else if busy {
                                    "СЕРВЕР ГОТОВИТСЯ"
                                } else {
                                    "СЕРВЕР ОСТАНОВЛЕН"
                                })
                                .font(theme::mono(10.0))
                                .color(if running { skin.green() } else { skin.faint() }),
                            );
                            if busy && !stage.is_empty() {
                                ui.label(
                                    RichText::new(format!("· {stage}"))
                                        .font(theme::mono(9.5))
                                        .color(skin.dim()),
                                );
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if running {
                                    if widgets::button_sized(ui, skin, "Остановить", Btn::Outline, true, 28.0, 11.5, 11.0).clicked() {
                                        stop = true;
                                    }
                                    if widgets::button_sized(ui, skin, "Подключиться", Btn::Primary, true, 28.0, 11.5, 11.0).clicked() {
                                        connect = true;
                                    }
                                } else if widgets::button_sized(
                                    ui,
                                    skin,
                                    if busy { "Запускается…" } else { "Запустить сервер" },
                                    Btn::Outline,
                                    !busy,
                                    28.0,
                                    11.5,
                                    11.0,
                                )
                                .clicked()
                                {
                                    start = true;
                                }
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(if running {
                                        format!("аптайм · {uptime}")
                                    } else {
                                        "—".to_string()
                                    })
                                    .font(theme::mono(9.5))
                                    .color(skin.faint()),
                                );
                            });
                        },
                    );

                    ui.add_space(8.0);
                    let local_srv = format!("127.0.0.1:{}", if running { port } else { cfg_port });
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 28.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new("Локальный адрес:")
                                    .font(theme::body(12.0))
                                    .color(skin.dim()),
                            );
                            widgets::copyable_pill(ui, skin, "srv-local-addr", &local_srv, 160.0, 26.0, None);
                            if widgets::button_icon(ui, skin, crate::icons::Icon::Copy, "Копировать", Btn::Outline, true, 26.0, 10.5, 9.0).clicked() {
                                copy_local_server = true;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::button_sized(ui, skin, "Папка сервера", Btn::Ghost, true, 26.0, 10.5, 9.0).clicked() {
                                    open_folder = true;
                                }
                            });
                        },
                    );

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let gap = ui.spacing().item_spacing.x;
                        let w = ((ui.available_width() - 2.0 * gap) / 3.0).max(40.0);
                        for (label, value) in [
                            ("ПОРТ", cfg_port.to_string()),
                            ("МОДОВ МАДРИД", mods_count.to_string()),
                            ("ПАМЯТЬ", format!("{} МБ", self.cfg.server_memory_mb)),
                        ] {
                            ui.allocate_ui_with_layout(
                                Vec2::new(w, 44.0),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    ui.set_width(w);
                                    widgets::stat(ui, skin, label, &value);
                                },
                            );
                        }
                    });
                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), 26.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            let mods_n = self.mods_cache.len();
                            ui.label(
                                RichText::new(format!(
                                    "Автозапуск: {}  ·  Модов в активной сборке: {mods_n}",
                                    if self.cfg.auto_server { "вкл" } else { "выкл" }
                                ))
                                .font(theme::body(12.0))
                                .color(skin.dim()),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::button_sized(
                                    ui,
                                    skin,
                                    "Скопировать список модов",
                                    Btn::Ghost,
                                    mods_n > 0,
                                    26.0,
                                    11.0,
                                    10.0,
                                )
                                .clicked()
                                {
                                    copy_list = true;
                                }
                            });
                        },
                    );
                    ui.label(
                        RichText::new(super::truncate(&server_dir.display().to_string(), 78))
                            .font(theme::mono(9.5))
                            .color(skin.faint()),
                    );
                    if let Some(e) = &error {
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("Ошибка сервера: {}", super::truncate(e, 220)))
                                    .font(theme::body(12.0))
                                    .color(skin.red()),
                            )
                            .wrap(),
                        );
                    }
                });

                ui.add_space(12.0);
                let height = (ui.available_height() - 28.0).clamp(130.0, 260.0);
                let state = if running {
                    "работает"
                } else if busy {
                    &stage
                } else {
                    "остановлен"
                };
                if crate::views::console(
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
            });

        if connect {
            self.launch_game(true);
        }
        if start {
            self.start_server();
        }
        if stop {
            self.stop_server();
        }
        if open_folder {
            util::open_path(&server_dir);
        }
        if copy_local_server {
            let addr = format!("127.0.0.1:{}", if running { port } else { cfg_port });
            util::copy_text(ui.ctx(), &addr);
            self.toast(format!("Скопирован {addr}"));
        }
        if copy_list {
            let names: Vec<String> = self.mods_cache.iter().map(|m| m.file.clone()).collect();
            util::copy_text(ui.ctx(), &names.join("\n"));
            self.toast("Список модов скопирован — передай другу");
        }
    }

    /// Единый блок «Игра с другом»: комната + оба адреса сразу при вводе порта + вход в одну полосу.
    fn friends_play_section(&mut self, ui: &mut Ui, skin: &Skin) {
        let ctx = ui.ctx().clone();
        let my_public = self.public_ip(&ctx);
        let my_public_str = my_public.clone().unwrap_or_default();
        let lan_ip = self.lan_ip.clone();

        let mut create = false;
        let mut join_code: Option<String> = None;
        let mut leave = false;
        let mut copy_code = false;
        let mut toggle_host: Option<bool> = None;
        let mut host_port_changed: Option<u16> = None;
        let mut join_addr: Option<String> = None;
        let mut copy_str: Option<(String, &'static str)> = None;

        let frame = egui::Frame::new()
            .fill(skin.fill())
            .stroke(Stroke::new(1.0, skin.line()))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin::symmetric(18, 16));

        frame.show(ui, |ui| {
            let w = ui.available_width();
            ui.set_width(w);

            // ---------------------------------------- ПОЛОСА 1: Комната по коду (строго в 1 полосу)
            ui.allocate_ui_with_layout(
                Vec2::new(w, 34.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    if let Some(room) = self.room.as_ref() {
                        let code = room.code.clone();
                        let status = room.status();
                        let members = room.members();
                        let is_err = status.contains("нет связи")
                            || status.contains("недоступ")
                            || status.contains("не удалось");
                        let is_ok = status == "в комнате";

                        ui.label(
                            RichText::new("КОМНАТА")
                                .font(theme::mono(9.5))
                                .color(skin.faint()),
                        );
                        widgets::copyable_pill(ui, skin, "room-code-pill", &code, 84.0, 26.0, None);
                        widgets::status_dot(
                            ui,
                            if is_err {
                                skin.red()
                            } else if is_ok {
                                skin.green()
                            } else {
                                skin.ink()
                            },
                            is_ok,
                        );
                        ui.label(
                            RichText::new(format!("{status} · игроков: {}", members.len()))
                                .font(theme::mono(10.0))
                                .color(if is_err {
                                    skin.red()
                                } else if is_ok {
                                    skin.green()
                                } else {
                                    skin.dim()
                                }),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::button_sized(ui, skin, "Выйти", Btn::Ghost, true, 30.0, 11.5, 10.0).clicked() {
                                leave = true;
                            }
                            if widgets::button_sized(ui, skin, "Скопировать код", Btn::Outline, true, 30.0, 11.5, 11.0).clicked() {
                                copy_code = true;
                            }
                        });
                    } else {
                        if widgets::button_sized(ui, skin, "Создать комнату", Btn::Primary, true, 28.0, 12.0, 12.0).clicked() {
                            create = true;
                        }
                        ui.add_space(6.0);
                        let resp = widgets::compact_input(
                            ui,
                            skin,
                            "room-code",
                            &mut self.room_code_buf,
                            "код 5 цифр",
                            104.0,
                            28.0,
                            theme::mono(12.0),
                        );
                        let valid = crate::room::parse_code(&self.room_code_buf).is_some();
                        let enter = (resp.lost_focus() || resp.has_focus())
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let clicked_join = widgets::button_sized(ui, skin, "Войти", Btn::Outline, true, 28.0, 11.5, 11.0).clicked();
                        if (clicked_join || enter) && valid {
                            join_code = Some(self.room_code_buf.clone());
                        } else if clicked_join && !valid {
                            resp.request_focus();
                            self.friend_state = Some(FriendState::Err("Введи 5-значный код комнаты".into()));
                        }
                        let last = self.cfg.last_room.clone();
                        if !last.is_empty() {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if widgets::button_sized(
                                    ui,
                                    skin,
                                    &format!("Вернуться в {last}"),
                                    Btn::Ghost,
                                    true,
                                    30.0,
                                    11.5,
                                    10.0,
                                )
                                .clicked()
                                {
                                    join_code = Some(last);
                                }
                            });
                        }
                    }
                },
            );

            // ---------------------------------------- ПОЛОСА 2: В комнате — раздача порта и участники
            let my_mc = self.active_spec().title();
            let game_running = self.shared.is_game_running();
            let room_snap = self.room.as_ref().map(|room| {
                let me = room.me();
                let members = room.members();
                let reaches: Vec<(String, String, crate::room::Reach)> = members
                    .iter()
                    .filter(|m| m.presence.host && m.presence.id != me.id)
                    .map(|m| {
                        let target = crate::room::best_address(&my_public_str, &m.presence);
                        let reach = if target.is_empty() {
                            crate::room::Reach::Closed
                        } else {
                            room.reach(&m.presence.id, &target, &ctx)
                        };
                        (m.presence.id.clone(), target, reach)
                    })
                    .collect();
                (me, members, reaches)
            });

            if let Some((me, members, reaches)) = room_snap {
                ui.add_space(10.0);
                ui.allocate_ui_with_layout(
                    Vec2::new(w, 32.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        let mut on = me.host;
                        if widgets::toggle(ui, skin, &mut on).changed() || on != me.host {
                            toggle_host = Some(on);
                        }
                        ui.label(
                            RichText::new("Раздаю игру · порт")
                                .font(theme::body(12.5))
                                .color(skin.dim()),
                        );
                        let resp = widgets::compact_input(
                            ui,
                            skin,
                            "host-port",
                            &mut self.host_port_buf,
                            "25565",
                            74.0,
                            30.0,
                            theme::mono(12.5),
                        );
                        if resp.changed() {
                            if let Ok(p) = self.host_port_buf.trim().parse::<u16>() {
                                if p > 0 {
                                    host_port_changed = Some(p);
                                }
                            }
                        }
                        if me.host {
                            ui.add_space(6.0);
                            let (utext, ucol) = match (&self.upnp_rx, &self.upnp_state) {
                                (Some(_), _) => ("Открываю порт (UPnP)…".to_string(), skin.dim()),
                                (None, Some((p, Ok(o)))) => match &o.external_ip {
                                    Some(ip) if crate::upnp::is_unreachable_ip(ip) => (
                                        format!("CGNAT ({ip}) — играйте в одной сети / Radmin VPN"),
                                        skin.red(),
                                    ),
                                    Some(ip) if !my_public_str.is_empty() && ip != &my_public_str => (
                                        format!("Двойной NAT ({ip} ≠ {my_public_str}) — лучше одна сеть / Radmin"),
                                        skin.red(),
                                    ),
                                    _ => (format!("Порт {p} открыт по UPnP"), skin.green()),
                                },
                                (None, Some((p, Err(e)))) => (format!("UPnP {p}: {e}"), skin.red()),
                                (None, None) => (String::new(), skin.faint()),
                            };
                            if !utext.is_empty() {
                                ui.add(
                                    egui::Label::new(RichText::new(utext).font(theme::body(11.0)).color(ucol))
                                        .truncate(),
                                );
                            }
                        }
                    },
                );

                ui.add_space(8.0);
                widgets::hairline(ui, skin.line_soft());
                ui.add_space(6.0);

                for m in &members {
                    let p = &m.presence;
                    let is_me = p.id == me.id;
                    let same_net = !is_me && crate::room::same_network(&my_public_str, p);
                    let lan_addr = if p.lan.is_empty() {
                        String::new()
                    } else {
                        format!("{}:{}", p.lan, p.port)
                    };
                    let pub_addr = if p.public.is_empty() {
                        String::new()
                    } else {
                        format!("{}:{}", p.public, p.port)
                    };

                    ui.allocate_ui_with_layout(
                        Vec2::new(w, 32.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            widgets::status_dot(
                                ui,
                                if same_net { skin.green() } else { skin.ink() },
                                true,
                            );
                            ui.label(RichText::new(&p.nick).font(theme::display(13.5)).color(skin.ink()));
                            if is_me {
                                widgets::badge(ui, skin, "ВЫ", false);
                            }
                            if p.host {
                                widgets::badge(ui, skin, "ХОСТ", true);
                            }
                            if same_net {
                                widgets::semantic_badge(ui, "В ОДНОЙ СЕТИ", skin.green());
                            }
                            ui.label(
                                RichText::new(super::truncate(&p.mc, 22))
                                    .font(theme::mono(9.5))
                                    .color(skin.faint()),
                            );
                            if p.host && !is_me {
                                if let Some((_, target, reach)) = reaches.iter().find(|(id, _, _)| id == &p.id) {
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if !target.is_empty() {
                                            if widgets::button_sized(
                                                ui,
                                                skin,
                                                "Зайти",
                                                Btn::Primary,
                                                !game_running,
                                                28.0,
                                                11.5,
                                                13.0,
                                            )
                                            .clicked()
                                            {
                                                join_addr = Some(target.clone());
                                            }
                                            let (rtext, rcol, rok) = match reach {
                                                crate::room::Reach::Open => ("доступен", skin.green(), true),
                                                crate::room::Reach::Closed => ("не отвечает", skin.red(), false),
                                                crate::room::Reach::Checking => ("проверяю…", skin.dim(), false),
                                            };
                                            ui.label(RichText::new(rtext).font(theme::mono(9.5)).color(rcol));
                                            widgets::status_dot(ui, rcol, rok);
                                        }
                                    });
                                }
                            }
                        },
                    );

                    if p.host {
                        let pill_w = ((w - 410.0) / 2.0).clamp(112.0, 166.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(w, 30.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                ui.add_space(12.0);
                                let lan_accent = if same_net || is_me { Some(skin.green()) } else { None };
                                ui.label(
                                    RichText::new("Сеть:")
                                        .font(theme::body(11.5))
                                        .color(if same_net { skin.green() } else { skin.faint() }),
                                );
                                let lan_show = if lan_addr.is_empty() { "не найден" } else { &lan_addr };
                                widgets::copyable_pill(
                                    ui,
                                    skin,
                                    &format!("rm-lan-{}", p.id),
                                    lan_show,
                                    pill_w,
                                    28.0,
                                    if lan_addr.is_empty() { None } else { lan_accent },
                                );
                                if widgets::button_sized(
                                    ui,
                                    skin,
                                    "Копировать",
                                    Btn::Outline,
                                    !lan_addr.is_empty(),
                                    28.0,
                                    10.5,
                                    8.0,
                                )
                                .clicked()
                                    && !lan_addr.is_empty()
                                {
                                    copy_str = Some((lan_addr.clone(), "Локальный адрес скопирован"));
                                }

                                ui.add_space(4.0);
                                ui.label(RichText::new("Интернет:").font(theme::body(11.5)).color(skin.faint()));
                                let pub_show = if pub_addr.is_empty() { "определяю…" } else { &pub_addr };
                                widgets::copyable_pill(
                                    ui,
                                    skin,
                                    &format!("rm-pub-{}", p.id),
                                    pub_show,
                                    pill_w,
                                    28.0,
                                    None,
                                );
                                if widgets::button_sized(
                                    ui,
                                    skin,
                                    "Копировать",
                                    Btn::Outline,
                                    !pub_addr.is_empty(),
                                    28.0,
                                    10.5,
                                    8.0,
                                )
                                .clicked()
                                    && !pub_addr.is_empty()
                                {
                                    copy_str = Some((pub_addr.clone(), "Внешний адрес скопирован"));
                                }
                            },
                        );
                        if !is_me && !p.mc.is_empty() && p.mc != my_mc {
                            ui.label(
                                RichText::new(format!(
                                    "Несовпадение сборки: у хоста {}, у тебя {} — выбери такую же, иначе сервер не пустит.",
                                    p.mc, my_mc
                                ))
                                .font(theme::body(11.0))
                                .color(skin.red()),
                            );
                        }
                    }
                }
                if members.len() <= 1 {
                    ui.label(
                        RichText::new("Отправь код комнаты другу — он появится здесь, как только зайдёт.")
                            .font(theme::body(11.5))
                            .color(skin.faint()),
                    );
                }
            }

            ui.add_space(10.0);
            widgets::hairline(ui, skin.line_soft());
            ui.add_space(10.0);

            // ---------------------------------------- ПОЛОСА 3: Порт + оба адреса сразу (строго в 1 полосу)
            let port_clean = self.friend_port.trim().to_string();
            let valid_port = port_clean.parse::<u16>().ok().filter(|p| *p > 0);
            let show_port = valid_port.map(|p| p.to_string()).unwrap_or_else(|| self.cfg.server_port.to_string());
            let lan_full = lan_ip.as_ref().map(|ip| format!("{ip}:{show_port}"));
            let pub_full = my_public.as_ref().map(|ip| format!("{ip}:{show_port}"));

            let pill_w = ((w - 400.0) / 2.0).clamp(112.0, 175.0);
            ui.allocate_ui_with_layout(
                Vec2::new(w, 28.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new("Порт:").font(theme::body(12.0)).color(skin.dim()));
                    widgets::compact_input(
                        ui,
                        skin,
                        "friend-port",
                        &mut self.friend_port,
                        "25565",
                        58.0,
                        26.0,
                        theme::mono(11.5),
                    );
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Сеть:")
                            .font(theme::body(11.5))
                            .color(if lan_full.is_some() { skin.green() } else { skin.faint() }),
                    );
                    let lan_txt = lan_full.as_deref().unwrap_or("не найден");
                    widgets::copyable_pill(
                        ui,
                        skin,
                        "my-lan-addr",
                        lan_txt,
                        pill_w,
                        26.0,
                        lan_full.as_ref().map(|_| skin.green()),
                    );
                    if widgets::button_icon(
                        ui,
                        skin,
                        crate::icons::Icon::Copy,
                        "Копировать",
                        Btn::Outline,
                        true,
                        26.0,
                        10.5,
                        8.0,
                    )
                    .clicked()
                    {
                        if let Some(a) = &lan_full {
                            copy_str = Some((a.clone(), "Адрес для одной сети скопирован"));
                        } else {
                            copy_str = Some((format!("127.0.0.1:{show_port}"), "Скопирован локальный адрес"));
                        }
                    }

                    ui.add_space(2.0);
                    ui.label(RichText::new("Интернет:").font(theme::body(11.5)).color(skin.faint()));
                    let pub_txt = pub_full.as_deref().unwrap_or("определяю…");
                    widgets::copyable_pill(ui, skin, "my-pub-addr", pub_txt, pill_w, 26.0, None);
                    if widgets::button_icon(
                        ui,
                        skin,
                        crate::icons::Icon::Copy,
                        "Копировать",
                        Btn::Outline,
                        true,
                        26.0,
                        10.5,
                        8.0,
                    )
                    .clicked()
                    {
                        if let Some(a) = &pub_full {
                            copy_str = Some((a.clone(), "Внешний адрес скопирован"));
                        } else if let Some(a) = &lan_full {
                            copy_str = Some((a.clone(), "Внешний IP определяется — скопирован сетевой адрес"));
                        }
                    }
                },
            );

            ui.add_space(8.0);

            // ---------------------------------------- ПОЛОСА 4: Подключиться к другу по адресу (строго в 1 полосу)
            ui.allocate_ui_with_layout(
                Vec2::new(w, 30.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(RichText::new("Адрес друга:").font(theme::body(12.5)).color(skin.dim()));
                    let input_w = (w - 430.0).clamp(160.0, 270.0);
                    let resp = widgets::compact_input(
                        ui,
                        skin,
                        "friend-code",
                        &mut self.friend_code,
                        "192.168.1.5:25565 или IP",
                        input_w,
                        28.0,
                        theme::mono(11.5),
                    );
                    let enter = (resp.lost_focus() || resp.has_focus())
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let can_join = !self.friend_code.trim().is_empty();
                    let clicked_friend = widgets::button_sized(
                        ui,
                        skin,
                        "Зайти к другу",
                        Btn::Primary,
                        true,
                        28.0,
                        11.5,
                        12.0,
                    )
                    .clicked();
                    if (clicked_friend || enter) && can_join {
                        join_addr = Some(self.friend_code.trim().to_string());
                    } else if clicked_friend && !can_join {
                        resp.request_focus();
                        self.friend_state = Some(FriendState::Err("Введи адрес друга, например 192.168.1.5:25565".into()));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new("откроет игру и сразу зайдёт")
                                .font(theme::mono(9.5))
                                .color(skin.faint()),
                        );
                    });
                },
            );

            // ---------------------------------------- ПОЛОСА 5: Живые процессы и ошибки (красный / зелёный)
            if let Some(st) = &self.friend_state {
                ui.add_space(8.0);
                let (dot_col, text_col, filled, text) = match st {
                    FriendState::Working(s) => (skin.ink(), skin.dim(), false, s.as_str()),
                    FriendState::Ok(s) => (skin.green(), skin.green(), true, s.as_str()),
                    FriendState::Err(s) => (skin.red(), skin.red(), true, s.as_str()),
                };
                ui.horizontal(|ui| {
                    widgets::status_dot(ui, dot_col, filled);
                    ui.add(egui::Label::new(RichText::new(text).font(theme::body(12.0)).color(text_col)).wrap());
                });
            }
        });

        if create {
            self.room_create();
        }
        if let Some(code) = join_code {
            self.room_join(&code);
            self.room_code_buf.clear();
        }
        if leave {
            self.room_leave();
            self.friend_state = Some(FriendState::Working("Вышел из комнаты".into()));
            self.toast("Ты вышел из комнаты");
        }
        if copy_code {
            if let Some(room) = &self.room {
                util::copy_text(&ctx, &room.code);
                self.friend_state = Some(FriendState::Ok(format!("Код комнаты {} скопирован", room.code)));
                self.toast(format!("Код {} скопирован", room.code));
            }
        }
        if let Some(p) = host_port_changed {
            self.cfg.host_port = p;
            self.friend_port = p.to_string();
            self.save_cfg();
            let hosting = self.room.as_ref().map(|r| r.me().host).unwrap_or(false);
            if let Some(room) = &self.room {
                room.update_me(|m| m.port = p);
            }
            if hosting {
                self.open_router_port(p);
            }
        }
        if let Some(on) = toggle_host {
            self.set_hosting(on);
            let msg = if on {
                "Раздаёшь игру — друзья видят оба адреса"
            } else {
                "Раздача игры остановлена"
            };
            self.friend_state = Some(if on {
                FriendState::Ok(msg.into())
            } else {
                FriendState::Working(msg.into())
            });
            self.toast(msg);
        }
        if let Some((addr, msg)) = copy_str {
            util::copy_text(&ctx, &addr);
            self.friend_state = Some(FriendState::Ok(format!("{msg}: {addr}")));
            self.toast(format!("{msg}: {addr}"));
        }
        if let Some(addr) = join_addr {
            self.join_address(&addr);
        }
    }
}
