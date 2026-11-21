//! Section view for managing personal custom builds and installed modpacks.

use crate::app::Launcher;
use crate::icons::Icon;
use crate::profiles::{self, ProfileKind};
use crate::theme::{self, Skin};
use crate::widgets::{self, Btn};
use egui::{Align, Color32, CornerRadius, CursorIcon, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use rfd::FileDialog;

impl Launcher {
    pub fn view_build(&mut self, ui: &mut Ui, skin: &Skin) {
        let profiles = profiles::load_profiles();
        let active_id = self.cfg.active_profile.clone();

        // Ensure a selected profile exists
        if self.build_selected_id.is_empty() || !profiles.iter().any(|p| p.id == self.build_selected_id) {
            self.build_selected_id = if !active_id.is_empty() {
                active_id.clone()
            } else {
                "default".to_string()
            };
        }

        let mut switch_to: Option<String> = None;
        let mut delete_profile_id: Option<String> = None;
        let mut toggle_mod_req: Option<(String, bool)> = None;
        let mut delete_mod_req: Option<String> = None;
        let mut add_local_jar = false;
        let mut open_mods_dir = false;
        let mut go_to_catalog = false;
        let mut update_pack: Option<crate::profiles::Profile> = None;

        let mut wipe_all = false;
        // Header
        widgets::header(
            ui,
            skin,
            "Сборки",
            "У каждой сборки своя папка: моды, настройки и миры не смешиваются",
            |ui| {
                if profiles.len() > 1 {
                    let label = if self.confirm_wipe { "Точно удалить все?" } else { "Удалить все сборки" };
                    let kind = if self.confirm_wipe { Btn::Primary } else { Btn::Ghost };
                    if widgets::button_sized(ui, skin, label, kind, true, 30.0, 12.0, 12.0).clicked() {
                        if self.confirm_wipe {
                            wipe_all = true;
                        }
                        self.confirm_wipe = !self.confirm_wipe;
                    }
                }
                let label = if self.build_creating { "Закрыть форму" } else { "+ Создать сборку" };
                if widgets::button_sized(ui, skin, label, Btn::Primary, true, 30.0, 12.0, 14.0).clicked() {
                    self.build_creating = !self.build_creating;
                    if self.build_creating {
                        self.build_new_name.clear();
                        self.build_new_mc = crate::config::MADRID_MC.to_string();
                        self.build_new_loader = "forge".to_string();
                    }
                }
            },
        );

        // Create new build form (collapsible card)
        if self.build_creating {
            let frame = egui::Frame::new()
                .fill(skin.fill())
                .stroke(Stroke::new(1.0, skin.line()))
                .corner_radius(CornerRadius::same(14))
                .inner_margin(egui::Margin::symmetric(20, 16));
            frame.show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Новая личная сборка")
                            .font(theme::display(16.0))
                            .color(skin.ink()),
                    );
                });
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Название:")
                            .font(theme::body(13.0))
                            .color(skin.dim()),
                    );
                    ui.add_space(8.0);
                    super::search_field(
                        ui,
                        skin,
                        "new-build-name",
                        &mut self.build_new_name,
                        "например: Выживание с друзьями, PvP сборка",
                        380.0,
                    );
                });
                ui.add_space(8.0);

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    ui.label(
                        RichText::new("Версия MC:")
                            .font(theme::body(13.0))
                            .color(skin.dim()),
                    );
                    for v in ["1.21.1", "1.20.1", "1.19.2", "1.16.5", "1.12.2"] {
                        if widgets::chip(ui, skin, v, self.build_new_mc == v).clicked() {
                            self.build_new_mc = v.to_string();
                        }
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    ui.label(
                        RichText::new("Загрузчик:")
                            .font(theme::body(13.0))
                            .color(skin.dim()),
                    );
                    for l in [
                        crate::loader::Loader::Forge,
                        crate::loader::Loader::NeoForge,
                        crate::loader::Loader::Fabric,
                        crate::loader::Loader::Quilt,
                        crate::loader::Loader::Vanilla,
                    ] {
                        if widgets::chip(ui, skin, l.label(), self.build_new_loader == l.id()).clicked() {
                            self.build_new_loader = l.id().to_string();
                        }
                    }
                });
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    let can_create = !self.build_new_name.trim().is_empty();
                    if widgets::button_sized(ui, skin, "Создать сборку", Btn::Primary, can_create, 32.0, 13.0, 16.0).clicked() {
                        match profiles::create_custom_profile(
                            &self.cfg,
                            &self.build_new_name,
                            &self.build_new_mc,
                            &self.build_new_loader,
                            "Личная сборка",
                        ) {
                            Ok(created) => {
                                self.build_selected_id = created.id.clone();
                                self.build_creating = false;
                                self.toast(format!("Создана сборка: {}", created.name));
                            }
                            Err(e) => {
                                self.toast(format!("Ошибка: {e}"));
                            }
                        }
                    }
                    ui.add_space(8.0);
                    if widgets::button_sized(ui, skin, "Отмена", Btn::Ghost, true, 32.0, 13.0, 14.0).clicked() {
                        self.build_creating = false;
                    }
                });
            });
            ui.add_space(12.0);
        }

        // Profile switcher list (horizontal wrapped chips / cards)
        widgets::section_label(ui, skin, "ВСЕ СБОРКИ И МОДПАКИ");
        ui.add_space(6.0);

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            for p in &profiles {
                let is_active = p.id == active_id;
                let is_selected = p.id == self.build_selected_id;
                let mod_count = profiles::list_profile_mods(&self.cfg, &p.id).len();

                let icon = match p.kind {
                    ProfileKind::Default => Icon::Cube,
                    ProfileKind::Custom => Icon::Layers,
                    ProfileKind::Modpack => Icon::Globe,
                };

                let label = format!(
                    "{}{} · {}",
                    super::truncate(&p.name, 26),
                    if is_active { " · активна" } else { "" },
                    mod_count
                );

                let resp = widgets::chip_icon(ui, skin, icon, &label, is_selected);
                if resp.clicked() {
                    self.build_selected_id = p.id.clone();
                }
            }
        });
        ui.add_space(14.0);

        // Currently selected profile details
        let current_profile = profiles
            .iter()
            .find(|p| p.id == self.build_selected_id)
            .cloned()
            .unwrap_or_else(|| profiles[0].clone());

        let is_current_active = current_profile.id == active_id;
        let mods = profiles::list_profile_mods(&self.cfg, &current_profile.id);

        let frame = egui::Frame::new()
            .fill(skin.fill())
            .stroke(Stroke::new(1.0, skin.line_soft()))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(egui::Margin::symmetric(20, 16));

        frame.show(ui, |ui| {
            ui.set_width(ui.available_width());

            // Title row
            ui.horizontal(|ui| {
                let kind_badge = match current_profile.kind {
                    ProfileKind::Default => "ОСНОВНАЯ",
                    ProfileKind::Custom => "СВОЯ СБОРКА",
                    ProfileKind::Modpack => "МОДПАК",
                };
                widgets::badge(ui, skin, kind_badge, false);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(&current_profile.name)
                        .font(theme::display(20.0))
                        .color(skin.ink()),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if is_current_active {
                        widgets::badge(ui, skin, "АКТИВНА ДЛЯ ЗАПУСКА", true);
                    } else if widgets::button_sized(ui, skin, "Сделать активной для запуска", Btn::Primary, true, 28.0, 11.5, 14.0).clicked() {
                        switch_to = Some(current_profile.id.clone());
                    }
                });
            });
            ui.add_space(8.0);

            // Sub info & actions bar
            ui.label(
                RichText::new(format!(
                    "{} · модов: {}",
                    current_profile.spec().title(),
                    mods.len()
                ))
                .font(theme::mono(11.0))
                .color(skin.faint()),
            );
            // Pack builds: which version is installed and whether the pack
            // has a newer one (asked live from Modrinth / CurseForge).
            if current_profile.kind == ProfileKind::Modpack {
                if let Some(state) = self.pack_check(&current_profile) {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                        let installed = if current_profile.pack_version.is_empty() {
                            "Версия сборки неизвестна".to_string()
                        } else {
                            let date = crate::util::ru_date(&current_profile.pack_date);
                            format!(
                                "Версия сборки: {}{}",
                                super::truncate(&current_profile.pack_version, 40),
                                if date.is_empty() { String::new() } else { format!(" · от {date}") }
                            )
                        };
                        ui.label(RichText::new(installed).font(theme::body(12.0)).color(skin.dim()));
                        match state {
                            crate::app::PackCheck::Checking => {
                                ui.label(RichText::new("проверяю новую версию…").font(theme::mono(10.0)).color(skin.faint()));
                            }
                            crate::app::PackCheck::Current => {
                                ui.label(RichText::new("последняя версия").font(theme::mono(10.0)).color(skin.green()));
                            }
                            crate::app::PackCheck::Failed => {
                                ui.label(RichText::new("не удалось проверить версию").font(theme::mono(10.0)).color(skin.faint()));
                            }
                            crate::app::PackCheck::Update(newer) => {
                                let date = crate::util::ru_date(&newer.date);
                                let ago = crate::mods::ago(&newer.date, crate::util::now_unix());
                                let text = if date.is_empty() {
                                    format!("Вышла версия {}", super::truncate(&newer.name, 40))
                                } else {
                                    format!("Вышла версия {} · {date} ({ago})", super::truncate(&newer.name, 40))
                                };
                                ui.label(RichText::new(text).font(theme::body(12.0)).color(skin.ink()));
                                let busy = self.mod_task.is_some();
                                if widgets::button_sized(ui, skin, "Обновить сборку", Btn::Primary, !busy, 26.0, 11.0, 12.0)
                                    .on_hover_text("Скачать новую версию пака в эту же сборку — миры, скриншоты и настройки останутся")
                                    .clicked()
                                {
                                    update_pack = Some(current_profile.clone());
                                }
                            }
                        }
                    });
                }
            }
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                if widgets::button_sized(ui, skin, "Добавить моды из каталога →", Btn::Outline, true, 26.0, 11.0, 12.0).clicked() {
                    go_to_catalog = true;
                }
                if widgets::button_sized(ui, skin, "+ Добавить .jar", Btn::Outline, true, 26.0, 11.0, 10.0).clicked() {
                    add_local_jar = true;
                }
                if widgets::button_sized(ui, skin, "Папка сборки", Btn::Ghost, true, 26.0, 11.0, 10.0).clicked() {
                    open_mods_dir = true;
                }
                if current_profile.kind != ProfileKind::Default {
                    let is_confirm = self.build_confirm_del.as_deref() == Some(&current_profile.id);
                    let del_label = if is_confirm { "Точно удалить?" } else { "Удалить сборку" };
                    if widgets::button_sized(ui, skin, del_label, Btn::Ghost, true, 26.0, 11.0, 10.0).clicked() {
                        if is_confirm {
                            delete_profile_id = Some(current_profile.id.clone());
                            self.build_confirm_del = None;
                        } else {
                            self.build_confirm_del = Some(current_profile.id.clone());
                        }
                    }
                }
            });
            ui.add_space(10.0);
            widgets::hairline(ui, skin.line_soft());
            ui.add_space(10.0);

            // Filter search field for mods inside this build
            let w = ui.available_width().max(200.0);
            let resp = super::search_field(
                ui,
                skin,
                "build-mod-filter",
                &mut self.build_search,
                "Фильтр модов в этой сборке…",
                w,
            );
            if self.focus_search {
                self.focus_search = false;
                resp.request_focus();
            }
            ui.add_space(8.0);

            // Mod items list
            let query = self.build_search.trim().to_lowercase();
            let filtered_mods: Vec<_> = mods
                .iter()
                .filter(|m| query.is_empty() || m.name.to_lowercase().contains(&query) || m.file.to_lowercase().contains(&query))
                .collect();

            if mods.is_empty() {
                widgets::empty_state(
                    ui,
                    skin,
                    Icon::Plus,
                    "В этой сборке пока нет модов",
                    "Нажмите «Добавить моды из каталога» или «+ Добавить .jar», чтобы собрать свой модпак.",
                );
            } else if filtered_mods.is_empty() {
                ui.label(
                    RichText::new("По запросу ничего не найдено.")
                        .font(theme::body(13.0))
                        .color(skin.dim()),
                );
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("build-mods-scroll")
                    .max_height(340.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for m in &filtered_mods {
                            let (toggle, delete) = mod_build_row(
                                ui,
                                skin,
                                &m.name,
                                &m.file,
                                m.size,
                                m.enabled,
                                self.build_confirm_del.as_deref() == Some(&m.file),
                            );
                            if toggle {
                                toggle_mod_req = Some((m.file.clone(), !m.enabled));
                            }
                            if delete {
                                if self.build_confirm_del.as_deref() == Some(&m.file) {
                                    delete_mod_req = Some(m.file.clone());
                                    self.build_confirm_del = None;
                                } else {
                                    self.build_confirm_del = Some(m.file.clone());
                                }
                            }
                        }
                    });
            }
        });

        // Handle async/state mutations
        if let Some(target) = switch_to {
            match profiles::switch_profile(&mut self.cfg, &target) {
                Ok(_) => {
                    self.toast("Сборка активирована для запуска!");
                    self.touch_mods();
                }
                Err(e) => {
                    self.toast(format!("Ошибка переключения: {e}"));
                }
            }
        }

        if let Some(pid) = delete_profile_id {
            match profiles::delete_profile(&mut self.cfg, &pid) {
                Ok(_) => {
                    self.toast("Сборка удалена");
                    self.build_selected_id = "default".to_string();
                    self.touch_mods();
                }
                Err(e) => {
                    self.toast(format!("Не удалось удалить: {e}"));
                }
            }
        }

        if let Some((file, enable)) = toggle_mod_req {
            match profiles::toggle_profile_mod(&self.cfg, &current_profile.id, &file, enable) {
                Ok(_) => {
                    self.touch_mods();
                }
                Err(e) => {
                    self.toast(format!("Ошибка: {e}"));
                }
            }
        }

        if let Some(file) = delete_mod_req {
            match profiles::delete_profile_mod(&self.cfg, &current_profile.id, &file) {
                Ok(_) => {
                    self.toast("Мод удалён");
                    self.touch_mods();
                }
                Err(e) => {
                    self.toast(format!("Ошибка удаления: {e}"));
                }
            }
        }

        if add_local_jar {
            if let Some(picked) = FileDialog::new()
                .add_filter("Minecraft Mod (.jar)", &["jar"])
                .pick_file()
            {
                if let Ok(bytes) = std::fs::read(&picked) {
                    let file_name = picked
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "custom_mod.jar".to_string());
                    match profiles::add_jar_to_profile(&self.cfg, &current_profile.id, &file_name, &bytes) {
                        Ok(_) => {
                            self.toast(format!("Добавлен мод: {file_name}"));
                            self.touch_mods();
                        }
                        Err(e) => {
                            self.toast(format!("Не удалось добавить: {e}"));
                        }
                    }
                }
            }
        }

        if wipe_all {
            self.delete_all_builds();
        }

        if open_mods_dir {
            let p_dir = profiles::profile_mods_dir(&self.cfg, &current_profile.id);
            let _ = std::fs::create_dir_all(&p_dir);
            crate::util::open_path(&p_dir);
        }

        if let Some(profile) = update_pack {
            let ctx = ui.ctx().clone();
            self.update_pack_build(&ctx, &profile);
        }
        if go_to_catalog {
            self.go(crate::app::Tab::Mods);
            self.mods_page = 0; // Catalog
        }
    }
}

fn mod_build_row(
    ui: &mut Ui,
    skin: &Skin,
    name: &str,
    _file: &str,
    size: u64,
    enabled: bool,
    confirm: bool,
) -> (bool, bool) {
    let mut toggle = false;
    let mut delete = false;

    let width = ui.available_width() - 14.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 36.0), Sense::hover());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.1);

    let bg = if hover > 0.01 {
        skin.hover().gamma_multiply(hover * 0.7)
    } else {
        Color32::TRANSPARENT
    };
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, CornerRadius::same(8), bg);
    }

    // Toggle dot / button
    let dot_rect = Rect::from_center_size(Pos2::new(rect.left() + 18.0, rect.center().y), Vec2::splat(16.0));
    let dot_resp = ui
        .allocate_rect(dot_rect, Sense::click())
        .on_hover_text(if enabled { "Мод включён — нажми, чтобы выключить" } else { "Мод выключен — нажми, чтобы включить" });
    // Painted inside its own rect: allocating here would push it a row down.
    let dot_hover = ui.ctx().animate_bool_with_time(dot_resp.id, dot_resp.hovered(), 0.12);
    if dot_hover > 0.01 {
        ui.painter().circle_filled(dot_rect.center(), 9.0, skin.hover().gamma_multiply(dot_hover));
    }
    widgets::paint_status_dot(ui.painter(), dot_rect.center(), if enabled { skin.green() } else { skin.faint() }, enabled);
    if dot_resp.clicked() {
        toggle = true;
    }
    if dot_resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    // Label
    let name_pos = Pos2::new(rect.left() + 38.0, rect.center().y - 1.0);
    crate::widgets::text_fit(
        ui.painter(),
        name_pos,
        egui::Align2::LEFT_CENTER,
        name,
        theme::body(13.0),
        if enabled { skin.ink() } else { skin.faint() },
        (rect.width() - 38.0 - 150.0).max(60.0),
    );

    // Size
    let mb = size as f64 / (1024.0 * 1024.0);
    let size_label = if mb >= 0.1 {
        format!("{:.1} МБ", mb)
    } else {
        format!("{} КБ", size / 1024)
    };
    let size_pos = Pos2::new(rect.right() - 96.0, rect.center().y);
    ui.painter().text(
        size_pos,
        egui::Align2::RIGHT_CENTER,
        size_label,
        theme::mono(10.5),
        skin.faint(),
    );

    // Delete: a worded button; the second click confirms («Точно?»).
    let del_rect = Rect::from_min_max(
        Pos2::new(rect.right() - 82.0, rect.center().y - 12.0),
        Pos2::new(rect.right() - 6.0, rect.center().y + 12.0),
    );
    let del_resp = ui
        .allocate_rect(del_rect, Sense::click())
        .on_hover_text(if confirm { "Нажми ещё раз — мод удалится из сборки" } else { "Удалить мод из сборки" });
    let del_hover = ui.ctx().animate_bool_with_time(del_resp.id, del_resp.hovered(), 0.12);
    let press = ui.ctx().animate_bool_with_time(del_resp.id.with("press"), del_resp.is_pointer_button_down_on(), 0.08);
    let del_draw = del_rect.shrink(press * 1.2);
    let del_color = if confirm {
        Color32::from_rgb(240, 80, 80)
    } else {
        theme::lerp_color(skin.faint(), skin.ink(), del_hover)
    };
    if del_hover > 0.01 || confirm {
        ui.painter().rect_stroke(
            del_draw,
            CornerRadius::same(8),
            egui::Stroke::new(1.0, if confirm { del_color } else { skin.line().gamma_multiply(del_hover) }),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        del_draw.center(),
        egui::Align2::CENTER_CENTER,
        if confirm { "Точно?" } else { "Удалить" },
        theme::body(11.5),
        del_color,
    );
    if del_resp.clicked() {
        delete = true;
    }
    if del_resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    (toggle, delete)
}
