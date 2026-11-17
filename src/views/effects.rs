//! Эффекты — освещение, видимость в жидкостях, очистка экрана и внутриигровая автоматизация.

use crate::app::Launcher;
use crate::automation::{self, BindTarget};
use crate::effects::{self, AppliedState, EffectModSpec, EffectsConfig};
use crate::loader::Loader;
use crate::theme::{self, Skin};
use crate::widgets::{self, Btn};
use egui::{RichText, Ui};

impl Launcher {
    pub fn view_effects(&mut self, ui: &mut Ui, skin: &Skin) {
        let mut apply_now = false;
        let mut reset_now = false;
        let mut auto_apply = false;
        let mut install_targets: Vec<EffectModSpec> = Vec::new();
        let mut start_bind: Option<BindTarget> = None;

        let spec = self.active_spec();
        let instance = self.cfg.instance_dir();
        let is_vanilla = spec.loader == Loader::Vanilla;
        let applied = effects::inspect_instance_applied(&instance, &self.effects);
        let missing_mods = effects::missing_mod_targets(&self.effects, &spec, &self.mods_cache);
        let snap = self.auto_snap.clone();
        let active_n = self.effects.active_count();
        let downloading_mods = self
            .effects_mod_task
            .as_ref()
            .map(|t| !t.finished)
            .unwrap_or(false);

        widgets::header(
            ui,
            skin,
            "Эффекты",
            "Применяются сразу во время игры: options.txt, SadEffects.zip, загрузка модов и автоматизация",
            |ui| {
                if !missing_mods.is_empty() && !is_vanilla && !downloading_mods {
                    let label = format!("Скачать моды ({})", missing_mods.len());
                    if widgets::button(ui, skin, &label, Btn::Primary, true).clicked() {
                        install_targets = missing_mods.clone();
                    }
                }
                if widgets::button(ui, skin, "Применить сейчас", Btn::Outline, true).clicked() {
                    apply_now = true;
                }
                if widgets::button(
                    ui,
                    skin,
                    "Сбросить",
                    Btn::Ghost,
                    active_n > 0 || self.effects.anti_afk || self.effects.autoclicker,
                )
                .clicked()
                {
                    reset_now = true;
                }
            },
        );

        // Live status strip: Minecraft state + inline mod installer progress + status toast
        theme::card_frame(skin).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                widgets::status_dot(
                    ui,
                    if snap.mc_running { skin.green() } else { skin.faint() },
                    snap.mc_running,
                );
                if snap.mc_running {
                    let focus_word = if snap.mc_focused {
                        "окно активно"
                    } else {
                        "свёрнут / не в фокусе"
                    };
                    let title = if snap.mc_title.is_empty() {
                        "Minecraft запущен".to_string()
                    } else {
                        format!("Minecraft запущен · {}", snap.mc_title)
                    };
                    ui.label(
                        RichText::new(format!("{title} ({focus_word})"))
                            .font(theme::display(13.0))
                            .color(skin.green()),
                    );
                } else {
                    ui.label(
                        RichText::new(format!("Minecraft не запущен · сборка {}", spec.title()))
                            .font(theme::display(13.0))
                            .color(skin.dim()),
                    );
                }
                if is_vanilla {
                    widgets::badge(ui, skin, "Vanilla: для модов выбери Fabric/Forge", false);
                }
            });
            ui.add_space(3.0);
            let hint = if snap.mc_running {
                "options.txt и SadEffects.zip обновляются прямо сейчас · Нажми F3+T в игре для обновления ресурспака; скачанные .jar моды включатся после перезапуска игры."
            } else {
                "Настройки сохраняются в options.txt и ресурспак SadEffects.zip сразу при изменении; моды ставятся в один клик прямо сюда."
            };
            ui.add(
                egui::Label::new(RichText::new(hint).font(theme::body(11.5)).color(skin.faint()))
                    .wrap(),
            );

            if let Some(task) = self.effects_mod_task.as_ref().filter(|t| !t.finished) {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    widgets::status_dot(ui, skin.green(), true);
                    ui.label(
                        RichText::new(&task.stage)
                            .font(theme::body(12.0))
                            .color(skin.ink()),
                    );
                    if widgets::button_sized(ui, skin, "Отмена", Btn::Ghost, true, 24.0, 9.0, 9.5).clicked() {
                        task.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                });
            } else if let Some((ok, msg)) = &self.effects_status {
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    widgets::status_dot(ui, if *ok { skin.green() } else { skin.red() }, *ok);
                    ui.add(
                        egui::Label::new(
                            RichText::new(msg)
                                .font(theme::body(12.0))
                                .color(if *ok { skin.green() } else { skin.red() }),
                        )
                        .wrap(),
                    );
                });
            }
        });
        ui.add_space(8.0);

        let mut changed = false;
        let minor = effects::mc_minor_version(&spec.mc);

        egui::ScrollArea::vertical()
            .id_salt("effects-scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                // -------------------------------- 1. Освещение и видимость в жидкостях
                widgets::section_label(ui, skin, "ОСВЕЩЕНИЕ И ВИДИМОСТЬ В ЖИДКОСТЯХ");
                ui.add_space(6.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // Fullbright
                    let fb_mod_ok = effects::is_effect_mod_installed("fullbright", &self.mods_cache);
                    let fb_badge = readiness_badge(
                        self.effects.fullbright,
                        applied,
                        minor < 19,
                        fb_mod_ok,
                        is_vanilla,
                        "options.txt (до 1.18)",
                        "мод Gamma Utils",
                    );
                    let fb_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.fullbright,
                        "Fullbright (Гамма / Ночное зрение)",
                        "Убирает темноту в пещерах, ночью, в Незере и Энде без факелов и зелий.",
                        fb_badge,
                        if minor >= 19 && !fb_mod_ok && !is_vanilla && !downloading_mods {
                            Some("Скачать мод")
                        } else {
                            None
                        },
                    );
                    if fb_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }
                    if fb_res.clicked_mod {
                        if let Some(m) = effects::effect_mod_spec("fullbright") {
                            install_targets.push(m);
                        }
                    }
                    if self.effects.fullbright {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            ui.label(
                                RichText::new(format!("Яркость: {:.0}", self.effects.fullbright_gamma))
                                    .font(theme::mono(11.0))
                                    .color(skin.dim()),
                            );
                            let sl = widgets::slider(ui, skin, &mut self.effects.fullbright_gamma, 1.0, 15.0);
                            if sl.changed() {
                                self.effects.fullbright_gamma = self.effects.fullbright_gamma.round();
                                changed = true;
                            }
                            if sl.drag_stopped() {
                                auto_apply = true;
                            }
                        });
                    }

                    divider(ui, skin);

                    // No Fluid
                    let nf_mod_ok = effects::is_effect_mod_installed("no_fluid", &self.mods_cache);
                    let nf_builtin = !self.effects.no_fluid_lava;
                    let nf_badge = readiness_badge(
                        self.effects.no_fluid,
                        applied,
                        nf_builtin,
                        nf_mod_ok,
                        is_vanilla,
                        "встроено (вода)",
                        "мод Clear Water/Lava",
                    );
                    let nf_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_fluid,
                        "No Fluid (Чистая вода и лава)",
                        "Убирает оверлей и густой туман при погружении под воду или в лаву.",
                        nf_badge,
                        if !nf_mod_ok && !is_vanilla && !downloading_mods {
                            Some("Скачать мод")
                        } else {
                            None
                        },
                    );
                    if nf_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }
                    if nf_res.clicked_mod {
                        if let Some(m) = effects::effect_mod_spec("no_fluid") {
                            install_targets.push(m);
                        }
                    }
                    if self.effects.no_fluid {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            if widgets::chip(ui, skin, "Вода (оверлей)", self.effects.no_fluid_water).clicked() {
                                self.effects.no_fluid_water = !self.effects.no_fluid_water;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Лава (туман через мод)", self.effects.no_fluid_lava).clicked() {
                                self.effects.no_fluid_lava = !self.effects.no_fluid_lava;
                                changed = true;
                            }
                        });
                    }

                    divider(ui, skin);

                    // Custom Fog
                    let fog_mod_ok = effects::is_effect_mod_installed("custom_fog", &self.mods_cache);
                    let fog_badge = readiness_badge(
                        self.effects.custom_fog,
                        applied,
                        false,
                        fog_mod_ok,
                        is_vanilla,
                        "",
                        "мод NoFog / Fog",
                    );
                    let fog_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.custom_fog,
                        "Custom Fog (Кастомный туман)",
                        "Отключает туман на дистанции прорисовки или меняет его плотность и цвет.",
                        fog_badge,
                        if !fog_mod_ok && !is_vanilla && !downloading_mods {
                            Some("Скачать мод")
                        } else {
                            None
                        },
                    );
                    if fog_res.toggled {
                        changed = true;
                    }
                    if fog_res.clicked_mod {
                        if let Some(m) = effects::effect_mod_spec("custom_fog") {
                            install_targets.push(m);
                        }
                    }
                    if self.effects.custom_fog {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            if widgets::chip(ui, skin, "Отключить туман", self.effects.fog_disable).clicked() {
                                self.effects.fog_disable = true;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Свой цвет и плотность", !self.effects.fog_disable).clicked() {
                                self.effects.fog_disable = false;
                                changed = true;
                            }
                            if !self.effects.fog_disable {
                                for tone in ["Нейтральный", "Холодный", "Тёплый", "Тёмный"] {
                                    if widgets::chip(ui, skin, tone, self.effects.fog_tint == tone).clicked() {
                                        self.effects.fog_tint = tone.to_string();
                                        changed = true;
                                    }
                                }
                            }
                        });
                        if !self.effects.fog_disable {
                            ui.horizontal_wrapped(|ui| {
                                ui.add_space(46.0);
                                ui.label(
                                    RichText::new(format!(
                                        "Плотность: {}%",
                                        (self.effects.fog_density * 100.0).round() as u32
                                    ))
                                    .font(theme::mono(11.0))
                                    .color(skin.dim()),
                                );
                                if widgets::slider(ui, skin, &mut self.effects.fog_density, 0.0, 1.0)
                                    .changed()
                                {
                                    changed = true;
                                }
                            });
                        }
                    }
                });

                ui.add_space(14.0);

                // -------------------------------- 2. Очистка экрана от помех (No Render)
                widgets::section_label(ui, skin, "ОЧИСТКА ЭКРАНА ОТ ПОМЕХ (NO RENDER)");
                ui.add_space(6.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // No Fire
                    let fire_badge = readiness_badge(
                        self.effects.no_fire,
                        applied,
                        true,
                        false,
                        is_vanilla,
                        "ресурспак",
                        "",
                    );
                    let fire_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_fire,
                        "No Fire (Убрать огонь)",
                        "Уменьшает или полностью скрывает пламя на экране от первого лица.",
                        fire_badge,
                        None,
                    );
                    if fire_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }
                    if self.effects.no_fire {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            if widgets::chip(ui, skin, "Низкий огонь", !self.effects.no_fire_hide).clicked() {
                                self.effects.no_fire_hide = false;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Скрыть полностью", self.effects.no_fire_hide).clicked() {
                                self.effects.no_fire_hide = true;
                                changed = true;
                                auto_apply = true;
                            }
                        });
                    }

                    divider(ui, skin);

                    // No Bad Effects
                    let bad_mod_ok = effects::is_effect_mod_installed("no_bad_effects", &self.mods_cache);
                    let bad_builtin = !self.effects.anti_blindness && !self.effects.anti_nausea;
                    let bad_badge = readiness_badge(
                        self.effects.no_bad_effects,
                        applied,
                        bad_builtin,
                        bad_mod_ok,
                        is_vanilla,
                        "Darkness встроен",
                        "Darkness встроен + мод",
                    );
                    let bad_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_bad_effects,
                        "No Bad Effects (Анти-слепота)",
                        "Убирает затемнение и искажение экрана от Слепоты, Тошноты и Тьмы Вардена.",
                        bad_badge,
                        if !bad_mod_ok && !is_vanilla && !downloading_mods {
                            Some("Скачать мод")
                        } else {
                            None
                        },
                    );
                    if bad_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }
                    if bad_res.clicked_mod {
                        if let Some(m) = effects::effect_mod_spec("no_bad_effects") {
                            install_targets.push(m);
                        }
                    }
                    if self.effects.no_bad_effects {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            if widgets::chip(ui, skin, "Darkness (Варден)", self.effects.anti_darkness).clicked() {
                                self.effects.anti_darkness = !self.effects.anti_darkness;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Blindness", self.effects.anti_blindness).clicked() {
                                self.effects.anti_blindness = !self.effects.anti_blindness;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Nausea", self.effects.anti_nausea).clicked() {
                                self.effects.anti_nausea = !self.effects.anti_nausea;
                                changed = true;
                            }
                        });
                    }

                    divider(ui, skin);

                    // No Hurt Cam
                    let hc_badge = readiness_badge(
                        self.effects.no_hurt_cam,
                        applied,
                        true,
                        false,
                        is_vanilla,
                        "options.txt",
                        "",
                    );
                    let hc_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_hurt_cam,
                        "No Hurt Cam (Без тряски камеры)",
                        "Убирает наклон и подёргивание камеры при получении урона.",
                        hc_badge,
                        None,
                    );
                    if hc_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }

                    divider(ui, skin);

                    // No Weather
                    let nw_badge = readiness_badge(
                        self.effects.no_weather,
                        applied,
                        true,
                        false,
                        is_vanilla,
                        "ресурспак",
                        "",
                    );
                    let nw_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_weather,
                        "No Weather (Убрать погоду)",
                        "Скрывает капли дождя, снег и вспышки молний для обзора и FPS.",
                        nw_badge,
                        None,
                    );
                    if nw_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }

                    divider(ui, skin);

                    // No Particles
                    let np_badge = readiness_badge(
                        self.effects.no_particles,
                        applied,
                        true,
                        false,
                        is_vanilla,
                        "ресурспак",
                        "",
                    );
                    let np_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_particles,
                        "No Particles (Убрать частицы)",
                        "Выборочно скрывает громоздкие частицы и переводит частицы на минимум.",
                        np_badge,
                        None,
                    );
                    if np_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }
                    if self.effects.no_particles {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            if widgets::chip(ui, skin, "Взрывы", self.effects.part_explosions).clicked() {
                                self.effects.part_explosions = !self.effects.part_explosions;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Дым", self.effects.part_smoke).clicked() {
                                self.effects.part_smoke = !self.effects.part_smoke;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Зелья", self.effects.part_potions).clicked() {
                                self.effects.part_potions = !self.effects.part_potions;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Криты и блоки", self.effects.part_blocks).clicked() {
                                self.effects.part_blocks = !self.effects.part_blocks;
                                changed = true;
                                auto_apply = true;
                            }
                        });
                    }

                    divider(ui, skin);

                    // No Overlay
                    let ov_badge = readiness_badge(
                        self.effects.no_overlay,
                        applied,
                        true,
                        false,
                        is_vanilla,
                        "ресурспак",
                        "",
                    );
                    let ov_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_overlay,
                        "No Overlay (Убрать оверлеи)",
                        "Скрывает текстуру надетой тыквы, замерзание в рыхлом снегу и анимацию тотема.",
                        ov_badge,
                        None,
                    );
                    if ov_res.toggled {
                        changed = true;
                        auto_apply = true;
                    }
                    if self.effects.no_overlay {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            if widgets::chip(ui, skin, "Тыква", self.effects.ov_pumpkin).clicked() {
                                self.effects.ov_pumpkin = !self.effects.ov_pumpkin;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Рыхлый снег", self.effects.ov_snow).clicked() {
                                self.effects.ov_snow = !self.effects.ov_snow;
                                changed = true;
                                auto_apply = true;
                            }
                            if widgets::chip(ui, skin, "Тотем", self.effects.ov_totem).clicked() {
                                self.effects.ov_totem = !self.effects.ov_totem;
                                changed = true;
                                auto_apply = true;
                            }
                        });
                    }
                });

                ui.add_space(14.0);

                // -------------------------------- 3. Освещение мира и окружения
                widgets::section_label(ui, skin, "ОСВЕЩЕНИЕ МИРА И ОКРУЖЕНИЯ");
                ui.add_space(6.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // World Time
                    let wt_mod_ok = effects::is_effect_mod_installed("world_time", &self.mods_cache);
                    let wt_badge = readiness_badge(
                        self.effects.world_time,
                        applied,
                        false,
                        wt_mod_ok,
                        is_vanilla,
                        "",
                        "мод Time Changer",
                    );
                    let wt_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.world_time,
                        "World Time (Время суток)",
                        "Визуально фиксирует время суток (постоянный день или закат) независимо от сервера.",
                        wt_badge,
                        if !wt_mod_ok && !is_vanilla && !downloading_mods {
                            Some("Скачать мод")
                        } else {
                            None
                        },
                    );
                    if wt_res.toggled {
                        changed = true;
                    }
                    if wt_res.clicked_mod {
                        if let Some(m) = effects::effect_mod_spec("world_time") {
                            install_targets.push(m);
                        }
                    }
                    if self.effects.world_time {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            for (label, ticks) in [("Утро", 0u32), ("День", 6000), ("Закат", 12000), ("Ночь", 18000)] {
                                if widgets::chip(ui, skin, label, self.effects.world_time_ticks == ticks).clicked() {
                                    self.effects.world_time_ticks = ticks;
                                    changed = true;
                                }
                            }
                            let mut t = self.effects.world_time_ticks as f32;
                            if widgets::slider(ui, skin, &mut t, 0.0, 24000.0).changed() {
                                self.effects.world_time_ticks = (t / 500.0).round() as u32 * 500;
                                changed = true;
                            }
                            ui.label(
                                RichText::new(format!("{} тиков", self.effects.world_time_ticks))
                                    .font(theme::mono(10.5))
                                    .color(skin.faint()),
                            );
                        });
                    }

                    divider(ui, skin);

                    // Ambience
                    let am_mod_ok = effects::is_effect_mod_installed("ambience", &self.mods_cache);
                    let am_badge = readiness_badge(
                        self.effects.ambience,
                        applied,
                        false,
                        am_mod_ok,
                        is_vanilla,
                        "",
                        "мод Sky / Ambience",
                    );
                    let am_res = effect_row(
                        ui,
                        skin,
                        &mut self.effects.ambience,
                        "Ambience (Цвет неба и света)",
                        "Настраивает оттенок освещения, мягкость теней и контрастный тон неба.",
                        am_badge,
                        if !am_mod_ok && !is_vanilla && !downloading_mods {
                            Some("Скачать мод")
                        } else {
                            None
                        },
                    );
                    if am_res.toggled {
                        changed = true;
                    }
                    if am_res.clicked_mod {
                        if let Some(m) = effects::effect_mod_spec("ambience") {
                            install_targets.push(m);
                        }
                    }
                    if self.effects.ambience {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(46.0);
                            if widgets::chip(ui, skin, "Мягкие тени", self.effects.ambience_soft_shadows).clicked() {
                                self.effects.ambience_soft_shadows = !self.effects.ambience_soft_shadows;
                                changed = true;
                            }
                            for tone in ["Контрастный", "Нейтральный", "Тёплый", "Холодный"] {
                                if widgets::chip(ui, skin, tone, self.effects.ambience_tone == tone).clicked() {
                                    self.effects.ambience_tone = tone.to_string();
                                    changed = true;
                                }
                            }
                        });
                    }
                });

                ui.add_space(14.0);

                // -------------------------------- 4. Автоматизация (только внутри Minecraft)
                widgets::section_label(ui, skin, "АВТОМАТИЗАЦИЯ (ТОЛЬКО ВНУТРИ MINECRAFT)");
                ui.add_space(6.0);
                theme::card_frame(skin).show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    // Anti-AFK
                    let afk_badge = if self.effects.anti_afk {
                        if snap.mc_running {
                            let left = snap
                                .next_afk_in
                                .unwrap_or(automation::clamp_interval_secs(self.effects.anti_afk_secs));
                            Some((format!("Активна · след. шаг {}с ({})", left, snap.afk_ticks), true))
                        } else {
                            Some(("Включена · ждёт запуска игры".into(), false))
                        }
                    } else {
                        None
                    };
                    if effect_row(
                        ui,
                        skin,
                        &mut self.effects.anti_afk,
                        "Защита от АФК (Anti-AFK)",
                        "Делает короткое микродвижение вперёд (W) по таймеру, только когда открыт Minecraft.",
                        afk_badge,
                        None,
                    )
                    .toggled
                    {
                        changed = true;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(46.0);
                        ui.label(
                            RichText::new(format!(
                                "Интервал: {} с",
                                automation::clamp_interval_secs(self.effects.anti_afk_secs)
                            ))
                            .font(theme::mono(11.0))
                            .color(skin.dim()),
                        );
                        let mut secs = self.effects.anti_afk_secs as f32;
                        if widgets::slider(ui, skin, &mut secs, 5.0, 300.0).changed() {
                            let rounded = ((secs / 5.0).round() as u32 * 5).clamp(5, 300);
                            self.effects.anti_afk_secs = rounded;
                            changed = true;
                        }
                        for preset in [30u32, 60, 120] {
                            if widgets::chip(
                                ui,
                                skin,
                                &format!("{preset}с"),
                                self.effects.anti_afk_secs == preset,
                            )
                            .clicked()
                            {
                                self.effects.anti_afk_secs = preset;
                                changed = true;
                            }
                        }
                        let capturing_afk = self.bind_capture == Some(BindTarget::AntiAfk);
                        let bind_text = if capturing_afk {
                            "Нажми клавишу (Esc — отмена)…".to_string()
                        } else {
                            format!("Бинд: {}", automation::bind_display(&self.effects.anti_afk_bind))
                        };
                        if widgets::button_icon(
                            ui,
                            skin,
                            crate::icons::Icon::Keyboard,
                            &bind_text,
                            if capturing_afk { Btn::Primary } else { Btn::Outline },
                            true,
                            26.0,
                            10.0,
                            9.0,
                        )
                        .clicked()
                        {
                            start_bind = Some(BindTarget::AntiAfk);
                        }
                    });

                    divider(ui, skin);

                    // Minecraft-only Autoclicker
                    let ac_badge = if self.effects.autoclicker {
                        if !snap.mc_running {
                            Some(("Включён · ждёт запуска игры".into(), false))
                        } else if snap.mc_focused {
                            Some((format!("Работает в игре · кликов: {}", snap.click_count), true))
                        } else {
                            Some(("Пауза: окно Minecraft не активно".into(), false))
                        }
                    } else if snap.mc_running {
                        Some(("Готов к запуску по бинду".into(), false))
                    } else {
                        None
                    };
                    if effect_row(
                        ui,
                        skin,
                        &mut self.effects.autoclicker,
                        "Автокликер (только окно Minecraft)",
                        "Автоматически нажимает ЛКМ или ПКМ строго внутри активного окна Minecraft и сам ставится на паузу при переключении окон.",
                        ac_badge,
                        None,
                    )
                    .toggled
                    {
                        changed = true;
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.add_space(46.0);
                        if widgets::chip(ui, skin, "ЛКМ", !self.effects.autoclicker_rmb).clicked() {
                            self.effects.autoclicker_rmb = false;
                            changed = true;
                        }
                        if widgets::chip(ui, skin, "ПКМ", self.effects.autoclicker_rmb).clicked() {
                            self.effects.autoclicker_rmb = true;
                            changed = true;
                        }
                        ui.label(
                            RichText::new(format!(
                                "{:.0} кл/с",
                                automation::clamp_cps(self.effects.autoclicker_cps)
                            ))
                            .font(theme::mono(11.0))
                            .color(skin.dim()),
                        );
                        if widgets::slider(ui, skin, &mut self.effects.autoclicker_cps, 1.0, 25.0).changed() {
                            self.effects.autoclicker_cps = self.effects.autoclicker_cps.round().clamp(1.0, 25.0);
                            changed = true;
                        }
                        let capturing_ac = self.bind_capture == Some(BindTarget::Autoclicker);
                        let bind_text = if capturing_ac {
                            "Нажми клавишу / мышь 4–5…".to_string()
                        } else {
                            format!("Бинд: {}", automation::bind_display(&self.effects.autoclicker_bind))
                        };
                        if widgets::button_icon(
                            ui,
                            skin,
                            crate::icons::Icon::Keyboard,
                            &bind_text,
                            if capturing_ac { Btn::Primary } else { Btn::Outline },
                            true,
                            26.0,
                            10.0,
                            9.0,
                        )
                        .clicked()
                        {
                            start_bind = Some(BindTarget::Autoclicker);
                        }
                    });
                });

                ui.add_space(16.0);
            });

        if changed {
            self.effects.anti_afk_secs = automation::clamp_interval_secs(self.effects.anti_afk_secs);
            self.effects.autoclicker_cps = automation::clamp_cps(self.effects.autoclicker_cps);
            effects::save(&self.effects);
            self.automation.update_from_config(&self.effects);
        }
        if let Some(target) = start_bind {
            if self.bind_capture == Some(target) {
                self.bind_capture = None;
                self.automation.set_capturing_bind(false);
            } else {
                self.bind_capture = Some(target);
                self.automation.set_capturing_bind(true);
            }
        }
        if reset_now {
            self.effects = EffectsConfig::default();
            effects::save(&self.effects);
            self.automation.update_from_config(&self.effects);
            apply_now = true;
        }
        if apply_now {
            self.apply_effects_now(true);
        } else if auto_apply {
            self.apply_effects_now(false);
        }
        if !install_targets.is_empty() {
            self.start_effects_mod_install(install_targets);
        }
    }
}

struct RowOut {
    toggled: bool,
    clicked_mod: bool,
}

fn divider(ui: &mut Ui, skin: &Skin) {
    ui.add_space(8.0);
    widgets::hairline(ui, skin.line_soft());
    ui.add_space(8.0);
}

fn readiness_badge(
    enabled: bool,
    applied: AppliedState,
    builtin_ok: bool,
    mod_ok: bool,
    is_vanilla: bool,
    builtin_label: &str,
    mod_label: &str,
) -> Option<(String, bool)> {
    if builtin_ok && mod_ok {
        let ready = !enabled || applied.ready();
        return Some((
            if ready {
                format!("Готово ({builtin_label} + мод)")
            } else {
                "Нужно применить".into()
            },
            ready,
        ));
    }
    if builtin_ok {
        if enabled {
            return Some((
                if applied.ready() {
                    format!("Готово ({builtin_label})")
                } else {
                    "Нажми Применить".into()
                },
                applied.ready(),
            ));
        }
        return Some((builtin_label.to_string(), true));
    }
    if mod_ok {
        return Some(("Готово (мод установлен)".into(), true));
    }
    if is_vanilla {
        return Some(("Нужен Fabric / Forge".into(), false));
    }
    if enabled {
        Some((format!("Нужен {mod_label}"), false))
    } else if !mod_label.is_empty() {
        Some((mod_label.to_string(), false))
    } else {
        None
    }
}

fn effect_row(
    ui: &mut Ui,
    skin: &Skin,
    on: &mut bool,
    title: &str,
    desc: &str,
    badge_info: Option<(String, bool)>,
    mod_btn: Option<&str>,
) -> RowOut {
    let mut toggled = false;
    let mut clicked_mod = false;
    ui.horizontal(|ui| {
        if widgets::toggle(ui, skin, on).changed() {
            toggled = true;
        }
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(title)
                        .font(theme::display(14.0))
                        .color(if *on { skin.ink() } else { skin.dim() }),
                );
                if let Some((text, ready)) = badge_info {
                    if ready {
                        widgets::semantic_badge(ui, &text, skin.green());
                    } else {
                        widgets::badge(ui, skin, &text, false);
                    }
                }
                if let Some(btn_label) = mod_btn {
                    if widgets::button_sized(ui, skin, btn_label, Btn::Outline, true, 24.0, 9.5, 9.0).clicked() {
                        clicked_mod = true;
                    }
                }
            });
            ui.add(
                egui::Label::new(
                    RichText::new(desc)
                        .font(theme::body(11.5))
                        .color(skin.faint()),
                )
                .wrap(),
            );
        });
    });
    RowOut { toggled, clicked_mod }
}
