//! Эффекты — освещение, видимость в жидкостях и очистка экрана от помех.

use crate::app::{Launcher, Tab};
use crate::effects::{self, EffectsConfig};
use crate::theme::{self, Skin};
use crate::widgets::{self, Btn};
use egui::{RichText, Ui};

impl Launcher {
    pub fn view_effects(&mut self, ui: &mut Ui, skin: &Skin) {
        let mut apply_now = false;
        let mut reset_now = false;
        let mut open_mod_search: Option<&'static str> = None;
        let active_n = self.effects.active_count();

        widgets::header(
            ui,
            skin,
            "Эффекты",
            "Применяются к активной сборке: options.txt + ресурспак SadEffects.zip и подбор модов",
            |ui| {
                if widgets::button(ui, skin, "Применить сейчас", Btn::Primary, true).clicked() {
                    apply_now = true;
                }
                if widgets::button(ui, skin, "Сбросить", Btn::Ghost, active_n > 0).clicked() {
                    reset_now = true;
                }
            },
        );

        if let Some((ok, msg)) = &self.effects_status {
            ui.horizontal(|ui| {
                widgets::status_dot(ui, if *ok { skin.green() } else { skin.red() }, true);
                ui.label(
                    RichText::new(msg)
                        .font(theme::body(12.0))
                        .color(if *ok { skin.green() } else { skin.red() }),
                );
            });
            ui.add_space(6.0);
        }

        let mut changed = false;

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
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.fullbright,
                        "Fullbright (Гамма / Ночное зрение)",
                        "Убирает темноту в пещерах, ночью, в Незере и Энде без факелов и зелий.",
                        Some(("работает через options.txt (на 1.19+ нужен мод)", true)),
                    );
                    if self.effects.fullbright {
                        ui.horizontal(|ui| {
                            ui.add_space(50.0);
                            ui.label(
                                RichText::new(format!("Яркость: {:.0}", self.effects.fullbright_gamma))
                                    .font(theme::mono(11.0))
                                    .color(skin.dim()),
                            );
                            if widgets::slider(ui, skin, &mut self.effects.fullbright_gamma, 1.0, 15.0)
                                .changed()
                            {
                                self.effects.fullbright_gamma = self.effects.fullbright_gamma.round();
                                changed = true;
                            }
                            if widgets::button_sized(
                                ui,
                                skin,
                                "Мод Gamma на 1.19+",
                                Btn::Outline,
                                true,
                                26.0,
                                10.5,
                                9.0,
                            )
                            .clicked()
                            {
                                open_mod_search = Some("gamma utils");
                            }
                        });
                    }

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // No Fluid
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_fluid,
                        "No Fluid (Чистая вода и лава)",
                        "Убирает оверлей и густой туман при погружении под воду или в лаву.",
                        Some(("вода встроена · туман лавы требует мод", true)),
                    );
                    if self.effects.no_fluid {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(50.0);
                            if widgets::chip(ui, skin, "Вода (оверлей)", self.effects.no_fluid_water).clicked() {
                                self.effects.no_fluid_water = !self.effects.no_fluid_water;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Лава (нужен мод)", self.effects.no_fluid_lava).clicked() {
                                self.effects.no_fluid_lava = !self.effects.no_fluid_lava;
                                changed = true;
                            }
                            if widgets::button_sized(ui, skin, "Найти мод", Btn::Ghost, true, 26.0, 10.5, 9.0).clicked() {
                                open_mod_search = Some("clear water");
                            }
                        });
                    }

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // Custom Fog
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.custom_fog,
                        "Custom Fog (Кастомный туман)",
                        "Отключает туман на дистанции прорисовки или меняет его плотность и цвет.",
                        Some(("для рендера тумана нужен мод NoFog / Fog", false)),
                    );
                    if self.effects.custom_fog {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(50.0);
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
                            if widgets::button_sized(ui, skin, "Мод NoFog", Btn::Outline, true, 26.0, 10.5, 9.0).clicked() {
                                open_mod_search = Some("nofog");
                            }
                        });
                        if !self.effects.fog_disable {
                            ui.horizontal(|ui| {
                                ui.add_space(50.0);
                                ui.label(
                                    RichText::new(format!("Плотность: {}%", (self.effects.fog_density * 100.0).round() as u32))
                                        .font(theme::mono(11.0))
                                        .color(skin.dim()),
                                );
                                if widgets::slider(ui, skin, &mut self.effects.fog_density, 0.0, 1.0).changed() {
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
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_fire,
                        "No Fire (Убрать огонь)",
                        "Уменьшает или полностью скрывает пламя на экране от первого лица.",
                        Some(("встроено через ресурспак", true)),
                    );
                    if self.effects.no_fire {
                        ui.horizontal(|ui| {
                            ui.add_space(50.0);
                            if widgets::chip(ui, skin, "Низкий огонь", !self.effects.no_fire_hide).clicked() {
                                self.effects.no_fire_hide = false;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Скрыть полностью", self.effects.no_fire_hide).clicked() {
                                self.effects.no_fire_hide = true;
                                changed = true;
                            }
                        });
                    }

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // No Bad Effects
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_bad_effects,
                        "No Bad Effects (Анти-слепота)",
                        "Убирает затемнение и искажение экрана от Слепоты, Тошноты и Тьмы Вардена.",
                        Some(("Darkness встроен · Blindness/Nausea через мод", true)),
                    );
                    if self.effects.no_bad_effects {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(50.0);
                            if widgets::chip(ui, skin, "Darkness (Варден)", self.effects.anti_darkness).clicked() {
                                self.effects.anti_darkness = !self.effects.anti_darkness;
                                changed = true;
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

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // No Hurt Cam
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_hurt_cam,
                        "No Hurt Cam (Без тряски камеры)",
                        "Убирает наклон и подёргивание камеры при получении урона.",
                        Some(("встроено через options.txt", true)),
                    );

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // No Weather
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_weather,
                        "No Weather (Убрать погоду)",
                        "Скрывает капли дождя, снег и вспышки молний для обзора и FPS.",
                        Some(("встроено через ресурспак + options.txt", true)),
                    );

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // No Particles
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_particles,
                        "No Particles (Убрать частицы)",
                        "Выборочно скрывает громоздкие частицы и переводит частицы на минимум.",
                        Some(("встроено через ресурспак + options.txt", true)),
                    );
                    if self.effects.no_particles {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(50.0);
                            if widgets::chip(ui, skin, "Взрывы", self.effects.part_explosions).clicked() {
                                self.effects.part_explosions = !self.effects.part_explosions;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Дым", self.effects.part_smoke).clicked() {
                                self.effects.part_smoke = !self.effects.part_smoke;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Зелья", self.effects.part_potions).clicked() {
                                self.effects.part_potions = !self.effects.part_potions;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Криты и блоки", self.effects.part_blocks).clicked() {
                                self.effects.part_blocks = !self.effects.part_blocks;
                                changed = true;
                            }
                        });
                    }

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // No Overlay
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.no_overlay,
                        "No Overlay (Убрать оверлеи)",
                        "Скрывает текстуру надетой тыквы, замерзание в рыхлом снегу и анимацию тотема.",
                        Some(("встроено через ресурспак", true)),
                    );
                    if self.effects.no_overlay {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(50.0);
                            if widgets::chip(ui, skin, "Тыква", self.effects.ov_pumpkin).clicked() {
                                self.effects.ov_pumpkin = !self.effects.ov_pumpkin;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Рыхлый снег", self.effects.ov_snow).clicked() {
                                self.effects.ov_snow = !self.effects.ov_snow;
                                changed = true;
                            }
                            if widgets::chip(ui, skin, "Тотем", self.effects.ov_totem).clicked() {
                                self.effects.ov_totem = !self.effects.ov_totem;
                                changed = true;
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
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.world_time,
                        "World Time (Время суток)",
                        "Визуально фиксирует время суток (постоянный день или закат) независимо от сервера.",
                        Some(("требует клиентский мод Time Changer", false)),
                    );
                    if self.effects.world_time {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(50.0);
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

                    ui.add_space(8.0);
                    widgets::hairline(ui, skin.line_soft());
                    ui.add_space(8.0);

                    // Ambience
                    changed |= effect_row(
                        ui,
                        skin,
                        &mut self.effects.ambience,
                        "Ambience (Цвет неба и света)",
                        "Настраивает оттенок освещения, мягкость теней и контрастный тон неба.",
                        Some(("требует клиентский мод / шейдеры", false)),
                    );
                    if self.effects.ambience {
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(50.0);
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
                            if widgets::button_sized(ui, skin, "Моды в каталоге", Btn::Outline, true, 26.0, 10.5, 9.0).clicked() {
                                open_mod_search = Some("ambience");
                            }
                        });
                    }
                });

                ui.add_space(16.0);
            });

        if changed {
            effects::save(&self.effects);
        }
        if reset_now {
            self.effects = EffectsConfig::default();
            effects::save(&self.effects);
            apply_now = true;
        }
        if apply_now {
            let instance = self.cfg.instance_dir();
            let spec = self.active_spec();
            match effects::apply_to_instance(&instance, &spec.mc, &self.effects) {
                Ok(rep) => {
                    let msg = match (rep.pack_written, rep.options_updated) {
                        (true, _) => "Применено: options.txt + ресурспак SadEffects.zip",
                        (false, true) => "Применено к options.txt (ресурспак не требуется)",
                        (false, false) => "Настройки актуальны (options.txt без изменений)",
                    };
                    self.effects_status = Some((true, msg.into()));
                    self.toast(msg);
                }
                Err(e) => {
                    let msg = format!("Не удалось применить эффекты: {e:#}");
                    self.effects_status = Some((false, msg.clone()));
                    self.toast(msg);
                }
            }
        }
        if let Some(q) = open_mod_search {
            self.go(Tab::Mods);
            self.mods_page = 0;
            self.detail = None;
            self.search.kind = 0;
            self.search.query = q.to_string();
            self.search.reset();
            self.start_search(ui.ctx());
        }
    }
}

fn effect_row(
    ui: &mut Ui,
    skin: &Skin,
    on: &mut bool,
    title: &str,
    desc: &str,
    badge_info: Option<(&str, bool)>,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        if widgets::toggle(ui, skin, on).changed() {
            changed = true;
        }
        ui.add_space(6.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(title)
                        .font(theme::display(14.0))
                        .color(if *on { skin.ink() } else { skin.dim() }),
                );
                if let Some((text, builtin)) = badge_info {
                    if builtin {
                        widgets::semantic_badge(ui, text, skin.green());
                    } else {
                        widgets::badge(ui, skin, text, false);
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
    changed
}
