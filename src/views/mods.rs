//! Моды — catalog, the user's own build list, installed jars, project card.

use super::{downloads_label, search_field, short_date, truncate};
use crate::app::Launcher;
use crate::icons::Icon;
use crate::mods::{self, ModHit};
use crate::theme::{self, Skin};
use crate::widgets::{self, Btn};
use egui::{Align, Align2, CornerRadius, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::time::{Duration, Instant};

const CELL: Vec2 = Vec2::new(168.0, 252.0);

fn grid_cell_size(avail_w: f32) -> Vec2 {
    let gap = 10.0;
    let min_w = 166.0;
    let cols = ((avail_w + gap) / (min_w + gap)).floor().max(1.0);
    let cell_w = ((avail_w - gap * (cols - 1.0)) / cols - 0.5).floor().max(min_w);
    Vec2::new(cell_w, CELL.y)
}

impl Launcher {
    /// Active download card with a cancel button. Returns true on cancel.
    fn download_card(&mut self, ui: &mut Ui, skin: &Skin) -> bool {
        let mut cancel = false;
        let Some(t) = self.mod_task.as_ref().filter(|t| !t.finished) else {
            return false;
        };
        let (title, frac, percent, last, counted) = (
            t.title.clone(),
            t.frac(),
            t.total > 0,
            t.last_file.clone(),
            (t.done, t.total),
        );
        let is_pack = t.kind == mods::KIND_MODPACK;
        let speed = self.speed_label();
        theme::card_frame(skin).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                widgets::lead_text(
                    ui,
                    250.0,
                    RichText::new(format!("{} · {}", if is_pack { "Модпак" } else { "Скачивание" }, title))
                        .font(theme::body(12.5))
                        .color(skin.dim()),
                    false,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if widgets::button_sized(ui, skin, "Отменить", Btn::Ghost, true, 24.0, 10.5, 9.5).clicked() {
                        cancel = true;
                    }
                    let label = match (is_pack && counted.1 > 1, percent) {
                        (true, _) => format!("{}/{}", counted.0, counted.1),
                        (false, true) => format!("{}%", (frac * 100.0).round() as u32),
                        _ => "…".to_string(),
                    };
                    ui.label(RichText::new(label).font(theme::mono(11.0)).color(skin.dim()));
                    // Internet speed sits right before the file counter.
                    if !speed.is_empty() {
                        ui.label(RichText::new(format!("{speed} ·")).font(theme::mono(11.0)).color(skin.ink()));
                    }
                });
            });
            if !last.is_empty() {
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(RichText::new(format!("→ {last}")).font(theme::mono(10.5)).color(skin.faint()))
                        .truncate(),
                );
            }
            ui.add_space(6.0);
            widgets::progress_bar(ui, skin, frac);
        });
        ui.add_space(10.0);
        cancel
    }

    pub fn view_mods(&mut self, ui: &mut Ui, skin: &Skin) {
        if let Some(hit) = self.detail.clone() {
            let busy = self.mod_task.as_ref().map(|t| !t.finished).unwrap_or(false);
            draw_detail(self, ui, skin, hit, busy);
            return;
        }

        let profile = crate::profiles::get_active_profile(&self.cfg);
        let spec = profile.spec();
        widgets::header(
            ui,
            skin,
            "Моды",
            &format!("Каталог для «{}» · {}", truncate(&profile.name, 28), spec.title()),
            |ui| {
                if widgets::button(ui, skin, "Папка сборки", Btn::Ghost, true).clicked() {
                    let dir = self.cfg.instance_dir();
                    let _ = std::fs::create_dir_all(&dir);
                    crate::util::open_path(&dir);
                }
            },
        );

        let installed_n = self.mods_cache.len();
        ui.horizontal(|ui| {
            if widgets::chip(ui, skin, "Каталог", self.mods_page == 0).clicked() {
                self.mods_page = 0;
            }
            if widgets::chip(ui, skin, &format!("Установленные · {installed_n}"), self.mods_page == 1).clicked() {
                self.mods_page = 1;
            }
        });
        ui.add_space(10.0);

        if self.mods_page == 0 {
            self.catalog(ui, skin, &spec);
        } else {
            self.installed_page(ui, skin, &profile);
        }
    }

    fn catalog(&mut self, ui: &mut Ui, skin: &Skin, spec: &crate::loader::Spec) {
        let busy = self.search.busy;
        let download_busy = self.mod_task.as_ref().map(|t| !t.finished).unwrap_or(false);
        let kind_idx = self.search.kind.min(mods::KINDS.len() - 1);
        let kind = mods::KINDS[kind_idx].0;
        let mut search_now = false;
        let mut load_more = false;
        let mut install_hit: Option<ModHit> = None;
        let mut add_build: Option<ModHit> = None;
        let mut open_detail: Option<ModHit> = None;

        // Another build became active: its Minecraft/loader need other results.
        let spec_key = format!("{}|{}", spec.mc, spec.loader.id());
        if self.search.spec_key != spec_key {
            self.search.spec_key = spec_key;
            if !self.search.results.is_empty() || self.search.searched {
                self.search.reset();
                search_now = true;
            }
        }
        // Nothing loaded yet (first open, or a filter changed): fetch.
        if !busy && !self.search.searched && self.search.results.is_empty() {
            search_now = true;
        }
        // A stale first page refreshes itself quietly on return.
        if !busy
            && !self.search.results.is_empty()
            && self.catalog_at.elapsed() >= Duration::from_secs(600)
            && self.search.offset <= mods::page_size("modrinth")
        {
            self.search.offset = 0;
            self.catalog_at = Instant::now();
            search_now = true;
        }

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            for (i, (_, label)) in mods::KINDS.iter().enumerate() {
                if widgets::chip(ui, skin, label, kind_idx == i).clicked() && self.search.kind != i {
                    self.search.kind = i;
                    self.search.category.clear();
                    self.search.reset();
                    search_now = true;
                }
            }
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            let maps_only_cf = kind == mods::KIND_MAP;
            for (idx, icon, label) in [(1usize, Icon::Modrinth, "Modrinth"), (0, Icon::CurseForge, "CurseForge")] {
                let selected = if maps_only_cf { idx == 0 } else { self.search.source == idx };
                if widgets::chip_icon(ui, skin, icon, label, selected).clicked() && self.search.source != idx {
                    self.search.source = idx;
                    self.search.category.clear();
                    self.search.reset();
                    search_now = true;
                }
            }
            ui.add_space(10.0);
            for (idx, icon, label) in [
                (0usize, Icon::Download, "Скачивания"),
                (1, Icon::Heart, "Популярность"),
                (2, Icon::Refresh, "Обновления"),
                (3, Icon::Sparkle, "Новые"),
            ] {
                if widgets::chip_icon(ui, skin, icon, label, self.search.sort == idx).clicked() && self.search.sort != idx {
                    self.search.sort = idx;
                    self.search.reset();
                    search_now = true;
                }
            }
            if self.category_dropdown(ui, skin, kind) {
                self.search.reset();
                search_now = true;
            }
        });
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 32.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                let field_w = (ui.available_width() - 96.0).max(160.0);
                let source = mods::effective_source(kind, if self.search.source == 0 { "curseforge" } else { "modrinth" });
                let hint = match crate::categories::label(source, kind, &self.search.category) {
                    Some(cat) => format!("Поиск в «{cat}» — по-русски или по-английски"),
                    None => "Название по-русски или по-английски: «проминенс», «JEI», «мини карта»".to_string(),
                };
                let resp = search_field(ui, skin, "mod-search", &mut self.search.query, &hint, field_w);
                if self.focus_search {
                    self.focus_search = false;
                    resp.request_focus();
                }
                if resp.changed() {
                    self.search.typed_at = Some(Instant::now());
                }
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    search_now = true;
                }
                let label = if busy { "Ищу…" } else { "Найти" };
                if widgets::button(ui, skin, label, Btn::Primary, true).clicked() {
                    search_now = true;
                }
            },
        );
        // Search as you type: fires after a short pause, not per keystroke.
        if let Some(at) = self.search.typed_at {
            if at.elapsed() >= Duration::from_millis(420) {
                search_now = true;
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(420) - at.elapsed());
            }
        }
        if search_now {
            self.search.typed_at = None;
        }
        ui.add_space(4.0);
        let hint = match kind {
            mods::KIND_MAP => format!("карты с CurseForge лягут в saves · Minecraft {}", spec.mc),
            mods::KIND_MODPACK => "модпак ставится одной кнопкой: своя сборка, нужный Minecraft и загрузчик скачаются сами".to_string(),
            mods::KIND_SHADER => format!("шейдеры лягут в shaderpacks · Minecraft {}", spec.mc),
            mods::KIND_RESOURCEPACK => format!("ресурспаки лягут в resourcepacks · Minecraft {}", spec.mc),
            _ => format!("только совместимые с {}", spec.title()),
        };
        ui.add(egui::Label::new(RichText::new(hint).font(theme::mono(9.0)).color(skin.faint())).wrap());
        ui.add_space(10.0);

        if let Some(err) = self.search.error.clone() {
            theme::card_frame(skin).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(truncate(&err, 200)).font(theme::body(12.0)).color(skin.red()),
                    ).wrap());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::button_sized(ui, skin, "Повторить", Btn::Outline, true, 26.0, 11.0, 10.0).clicked() {
                            search_now = true;
                        }
                    });
                });
            });
            ui.add_space(10.0);
        }

        if self.download_card(ui, skin) {
            self.cancel_active_downloads();
        }

        let shown_at = self.search.shown_at;
        egui::ScrollArea::vertical()
            .id_salt("mods-catalog")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let results = std::mem::take(&mut self.search.results);
                if results.is_empty() {
                    if busy || !self.search.searched {
                        skeleton_grid(ui, skin, 10);
                    } else if self.search.error.is_none() {
                        widgets::empty_state(
                            ui,
                            skin,
                            Icon::Course,
                            "Ничего не нашлось",
                            "Попробуйте другое название или источник.",
                        );
                    }
                } else {
                    let label = if self.search.query.trim().is_empty() { "ПОПУЛЯРНОЕ" } else { "РЕЗУЛЬТАТЫ" };
                    widgets::section_label(ui, skin, &format!("{label} · {}", results.len()));
                    ui.add_space(6.0);
                    let ctx = ui.ctx().clone();
                    let cell_sz = grid_cell_size(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                        for (i, hit) in results.iter().enumerate() {
                            let (slot, resp) = ui.allocate_exact_size(cell_sz, Sense::click());
                            if !ui.is_rect_visible(slot) {
                                // Off-screen cards cost nothing: no icon
                                // download, no pack lookup, no painting.
                                continue;
                            }
                            // Cards land as a list: 18 ms apart, capped so the
                            // last one never waits more than ~0.3 s.
                            let delay = (i.min(16) as f32) * 0.018;
                            let appear = theme::arrive(&ctx, shown_at, delay, 0.32);
                            let icon = hit.icon_url.as_deref().and_then(|u| self.mod_icon(&ctx, u));
                            let info = if hit.kind == mods::KIND_MODPACK { self.pack_info(hit, false) } else { None };
                            let (add, install, open) =
                                hit_card(ui, skin, hit, slot, resp, icon.as_ref(), info, download_busy, appear);
                            if add {
                                add_build = Some(hit.clone());
                            }
                            if install {
                                install_hit = Some(hit.clone());
                            }
                            if open {
                                open_detail = Some(hit.clone());
                            }
                        }
                    });
                    ui.add_space(14.0);
                    // Infinite scroll: reaching the bottom loads the next page
                    // (usually already prefetched, so it appears instantly).
                    if !self.search.exhausted {
                        let (sentinel, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::hover());
                        if busy {
                            ui.painter().text(
                                sentinel.center(),
                                Align2::CENTER_CENTER,
                                "Загружаю ещё…",
                                theme::mono(10.0),
                                skin.faint(),
                            );
                        } else if ui.is_rect_visible(sentinel) {
                            load_more = true;
                        }
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("Это всё").font(theme::mono(9.5)).color(skin.faint()));
                        });
                    }
                    ui.add_space(8.0);
                }
                self.search.results = results;
            });

        if let Some(hit) = open_detail {
            self.open_detail(hit);
        }
        if search_now {
            self.search.offset = 0;
            self.start_search(ui.ctx());
        } else if load_more {
            self.start_search(ui.ctx());
        }
        if let Some(hit) = install_hit {
            self.start_mod_install(ui.ctx(), hit);
        }
        if let Some(hit) = add_build {
            let spec = self.active_spec();
            self.build_add(&hit, &spec);
        }
    }

    /// «Категория ▾» chip right of the sort chips. Opens a list with its
    /// own filter field; picking one narrows the catalog (and the search)
    /// to it. Returns true when the selection changed.
    fn category_dropdown(&mut self, ui: &mut Ui, skin: &Skin, kind: &str) -> bool {
        let wanted = if self.search.source == 0 { "curseforge" } else { "modrinth" };
        let source = mods::effective_source(kind, wanted);
        let list = crate::categories::list(source, kind);
        if list.is_empty() {
            return false;
        }
        let current = crate::categories::label(source, kind, &self.search.category);
        let chip_label = match current {
            Some(name) => format!("Категория: {name}"),
            None => "Категории: все".to_string(),
        };
        ui.add_space(10.0);
        let resp = widgets::chip_icon(ui, skin, Icon::Layers, &chip_label, current.is_some());
        let mut changed = false;
        let popup_id = resp.id.with("category-popup");
        let popup = egui::Popup::from_toggle_button_response(&resp)
            .id(popup_id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .width(240.0)
            .frame(
                egui::Frame::new()
                    .fill(skin.black())
                    .stroke(Stroke::new(1.0, skin.line()))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(10)),
            );
        let mut close = false;
        popup.show(|ui| {
            ui.set_min_width(220.0);
            widgets::compact_input(
                ui,
                skin,
                "cat-filter",
                &mut self.search.category_filter,
                "Найти категорию…",
                ui.available_width(),
                30.0,
                theme::body(12.5),
            );
            ui.add_space(6.0);
            let filter = crate::smart::normalize(&self.search.category_filter);
            egui::ScrollArea::vertical()
                .id_salt("cat-list")
                .max_height(320.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let all = self.search.category.is_empty();
                    if filter.is_empty()
                        && ui
                            .add(
                                egui::Button::selectable(
                                    all,
                                    RichText::new("Все категории").font(theme::body(13.0)),
                                )
                                .wrap_mode(egui::TextWrapMode::Truncate),
                            )
                            .clicked()
                    {
                        if !all {
                            self.search.category.clear();
                            changed = true;
                        }
                        close = true;
                    }
                    for (id, name) in list {
                        let norm = crate::smart::normalize(name);
                        let latin = crate::smart::normalize(id);
                        if !filter.is_empty() && !norm.contains(&filter) && !latin.contains(&filter) {
                            continue;
                        }
                        let selected = self.search.category == *id;
                        if ui
                            .add(
                                egui::Button::selectable(
                                    selected,
                                    RichText::new(*name).font(theme::body(13.0)),
                                )
                                .wrap_mode(egui::TextWrapMode::Truncate),
                            )
                            .clicked()
                        {
                            if !selected {
                                self.search.category = id.to_string();
                                changed = true;
                            }
                            close = true;
                        }
                    }
                });
        });
        if close {
            egui::Popup::close_id(ui.ctx(), popup_id);
            self.search.category_filter.clear();
        }
        changed
    }

    fn installed_page(&mut self, ui: &mut Ui, skin: &Skin, profile: &crate::profiles::Profile) {
        let installed = self.mods_cache.clone();
        let build_busy = self.build_task.as_ref().map(|t| !t.finished).unwrap_or(false);
        let mut toggle_req: Option<(String, bool)> = None;
        let mut delete_req: Option<String> = None;
        let mut confirm = self.confirm_delete.clone();
        let mut remove_build: Option<usize> = None;
        let mut install_build = false;
        let mut clear_build = false;
        let mut delete_all = false;
        let mut wipe_builds = false;

        if self.download_card(ui, skin) {
            self.cancel_active_downloads();
        }

        // Build list (плюсик) — installs into the active build.
        let build_count = self.build.len();
        if build_count > 0 || build_busy {
            theme::card_frame(skin).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    widgets::badge(ui, skin, &format!("СПИСОК · {build_count}"), true);
                    if let Some(t) = self.build_task.as_ref().filter(|t| !t.finished) {
                        ui.label(
                            RichText::new(format!("Ставлю · {}/{} · {}", t.done, t.total, truncate(&t.last_file, 30)))
                                .font(theme::mono(10.0))
                                .color(skin.dim()),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if widgets::button_sized(ui, skin, "Установить список", Btn::Primary, !build_busy && build_count > 0, 28.0, 11.5, 11.0).clicked() {
                            install_build = true;
                        }
                        if widgets::button_sized(ui, skin, "Очистить", Btn::Ghost, !build_busy && build_count > 0, 28.0, 11.0, 9.5).clicked() {
                            clear_build = true;
                        }
                    });
                });
                if build_count > 0 {
                    ui.add_space(6.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                        for (i, e) in self.build.iter().enumerate() {
                            if widgets::chip(ui, skin, &format!("{} ×", truncate(&e.title, 26)), false).clicked() {
                                remove_build = Some(i);
                            }
                        }
                    });
                }
                if let Some(t) = self.build_task.as_ref().filter(|t| !t.finished) {
                    ui.add_space(6.0);
                    widgets::progress_bar(ui, skin, t.frac());
                }
                if let Some(err) = self.build_task.as_ref().and_then(|t| t.error.clone()) {
                    ui.add_space(4.0);
                    ui.label(RichText::new(truncate(&err, 200)).font(theme::body(11.5)).color(skin.red()));
                }
            });
            ui.add_space(10.0);
        }

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            widgets::badge(ui, skin, &truncate(&profile.name, 30), true);
            ui.label(
                RichText::new(format!("{} · {} файлов", profile.spec().title(), installed.len()))
                    .font(theme::mono(9.5))
                    .color(skin.faint()),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if self.confirm_wipe { "Точно удалить все сборки?" } else { "Удалить все сборки" };
                let kind = if self.confirm_wipe { Btn::Primary } else { Btn::Outline };
                if widgets::button_sized(ui, skin, label, kind, true, 26.0, 11.0, 10.5).clicked() {
                    if self.confirm_wipe {
                        wipe_builds = true;
                    }
                    self.confirm_wipe = !self.confirm_wipe;
                }
                if widgets::button_sized(ui, skin, "Удалить все моды сборки", Btn::Ghost, !installed.is_empty(), 26.0, 11.0, 10.5).clicked() {
                    delete_all = true;
                }
            });
        });
        if self.confirm_wipe {
            ui.label(
                RichText::new("Уйдут все свои сборки и модпаки с их папками и неиспользуемые версии загрузчиков. Основная сборка Мадрид останется.")
                    .font(theme::body(11.5))
                    .color(skin.dim()),
            );
        }
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .id_salt("mods-installed")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if installed.is_empty() {
                    widgets::empty_state(
                        ui,
                        skin,
                        Icon::Plus,
                        "В этой сборке пока нет модов",
                        "Открой «Каталог» — моды лягут в папку этой сборки.",
                    );
                }
                for m in &installed {
                    let is_confirm = confirm.as_deref() == Some(m.file.as_str());
                    let (toggle, delete) = mod_row(ui, skin, &m.name, m.size, m.enabled, is_confirm);
                    if toggle {
                        toggle_req = Some((m.file.clone(), !m.enabled));
                    }
                    if delete {
                        if is_confirm {
                            delete_req = Some(m.file.clone());
                            confirm = None;
                        } else {
                            confirm = Some(m.file.clone());
                        }
                    }
                }
                ui.add_space(12.0);
            });
        self.confirm_delete = confirm;

        if let Some((file, enabled)) = toggle_req {
            match mods::set_enabled(&self.cfg, &file, enabled) {
                Ok(_) => self.toast(if enabled { "Включено" } else { "Отключено" }),
                Err(e) => self.toast(format!("{e:#}")),
            }
            self.touch_mods();
        }
        if let Some(file) = delete_req {
            match mods::delete(&self.cfg, &file) {
                Ok(_) => self.toast("Удалено"),
                Err(e) => self.toast(format!("{e:#}")),
            }
            self.touch_mods();
        }
        if delete_all {
            // Background removal: the list empties live, the window stays responsive.
            let dir = self.cfg.mods_dir();
            let files: Vec<std::path::PathBuf> = installed.iter().map(|m| dir.join(&m.file)).collect();
            crate::remove::queue_files(format!("Моды · {}", files.len()), files);
            self.touch_mods();
        }
        if wipe_builds {
            self.delete_all_builds();
        }
        if let Some(i) = remove_build {
            self.build_remove(i);
        }
        if clear_build {
            self.build.clear();
            mods::save_build(&self.build);
            self.toast("Список очищен");
        }
        if install_build {
            self.start_build_install(ui.ctx());
        }
    }
}

/// Placeholder tiles with a soft breathing pulse while the first page loads.
fn skeleton_grid(ui: &mut Ui, skin: &Skin, count: usize) {
    let t = ui.input(|i| i.time) as f32;
    let cell_sz = grid_cell_size(ui.available_width());
    ui.ctx().request_repaint_after(Duration::from_millis(33));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
        for i in 0..count {
            let (rect, _) = ui.allocate_exact_size(cell_sz, Sense::hover());
            let wave = if theme::reduced_motion() {
                0.5
            } else {
                0.5 + 0.5 * (t * 2.4 - i as f32 * 0.35).sin()
            };
            let fill = skin.fill().gamma_multiply(0.7 + 0.6 * wave);
            let p = ui.painter();
            p.rect_filled(rect, CornerRadius::same(14), skin.fill().gamma_multiply(0.6));
            p.rect_filled(
                Rect::from_min_size(rect.min + Vec2::new(12.0, 12.0), Vec2::new(rect.width() - 24.0, 120.0)),
                CornerRadius::same(10),
                fill,
            );
            for (y, w) in [(144.0, 0.7), (162.0, 0.9), (178.0, 0.45)] {
                p.rect_filled(
                    Rect::from_min_size(rect.min + Vec2::new(12.0, y), Vec2::new((rect.width() - 24.0) * w, 8.0)),
                    CornerRadius::same(4),
                    fill,
                );
            }
        }
    });
}

fn mod_row(ui: &mut Ui, skin: &Skin, name: &str, size: u64, enabled: bool, confirm: bool) -> (bool, bool) {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::hover());
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.14);
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(10),
        theme::lerp_color(skin.hover().gamma_multiply(0.45), skin.hover(), hover),
    );
    let on = ui.ctx().animate_bool_with_time(resp.id.with("on"), enabled, 0.2);
    ui.painter().circle_filled(
        Pos2::new(rect.left() + 14.0, rect.center().y),
        2.5 + on,
        theme::lerp_color(skin.faint(), skin.ink(), on),
    );
    let painter = ui.painter().clone();
    let text_w = (rect.width() - 28.0 - 262.0).max(60.0);
    widgets::text_fit(
        &painter,
        Pos2::new(rect.left() + 28.0, rect.center().y - 8.0),
        Align2::LEFT_CENTER,
        name,
        theme::body(13.0),
        theme::lerp_color(skin.dim(), skin.ink(), on),
        text_w,
    );
    widgets::text_fit(
        &painter,
        Pos2::new(rect.left() + 28.0, rect.center().y + 10.0),
        Align2::LEFT_CENTER,
        &format!("{} · {}", if enabled { "включён" } else { "отключён" }, crate::util::human_bytes(size)),
        theme::mono(9.0),
        skin.faint(),
        text_w,
    );
    let mut toggle = false;
    let mut delete = false;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(
                Pos2::new(rect.right() - 250.0, rect.center().y - 15.0),
                Vec2::new(238.0, 30.0),
            ))
            .layout(Layout::right_to_left(Align::Center)),
    );
    if widgets::button_sized(
        &mut child,
        skin,
        if confirm { "Точно?" } else { "Удалить" },
        if confirm { Btn::Outline } else { Btn::Ghost },
        true,
        28.0,
        11.5,
        10.0,
    )
    .clicked()
    {
        delete = true;
    }
    if widgets::button_sized(&mut child, skin, if enabled { "Выключить" } else { "Включить" }, Btn::Ghost, true, 28.0, 11.5, 10.0).clicked() {
        toggle = true;
    }
    (toggle, delete)
}

/// One catalog tile. `appear` (0..1) drives its arrival: it rises 10 px
/// and fades in; hovering lifts it 2 px with a soft shadow.
/// Returns (added_to_build, install_now, open_detail).
#[allow(clippy::too_many_arguments)]
fn hit_card(
    ui: &mut Ui,
    skin: &Skin,
    hit: &ModHit,
    slot: Rect,
    resp: egui::Response,
    icon: Option<&egui::TextureHandle>,
    info: Option<crate::packinfo::PackInfo>,
    busy: bool,
    appear: f32,
) -> (bool, bool, bool) {
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.16);
    let lift = theme::travel(10.0) * (1.0 - appear) - theme::travel(2.0) * hover;
    // Scale: arrives from 96 %, grows 1.5 % under the pointer, sinks 1 %
    // while pressed — the tile answers before anything loads.
    let press = ui.ctx().animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on(), 0.08);
    let scale = if theme::reduced_motion() {
        1.0
    } else {
        0.96 + 0.04 * appear + 0.015 * hover - 0.01 * press
    };
    let rect = Rect::from_center_size(slot.center(), slot.size() * scale).translate(Vec2::new(0.0, lift));
    let mut p = ui.painter().clone();
    p.multiply_opacity(0.15 + 0.85 * appear);

    if hover > 0.01 {
        let shadow = egui::Shadow {
            offset: [0, 8],
            blur: 22,
            spread: 0,
            color: egui::Color32::from_black_alpha((110.0 * hover) as u8),
        };
        p.add(shadow.as_shape(rect, CornerRadius::same(14)));
    }
    p.rect_filled(
        rect,
        CornerRadius::same(14),
        theme::lerp_color(skin.hover().gamma_multiply(0.45), skin.hover().gamma_multiply(1.1), hover),
    );
    p.rect_stroke(
        rect,
        CornerRadius::same(14),
        Stroke::new(1.0, theme::lerp_color(skin.line_soft(), skin.fill_strong(), hover)),
        egui::StrokeKind::Inside,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let cell_w = slot.width();
    // Icon grows 3% under the pointer — the tile answers before the click.
    let side = (cell_w - 24.0) * (1.0 + 0.03 * hover);
    let irect = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 12.0 + 60.0), Vec2::new(side, 120.0 * (1.0 + 0.03 * hover)));
    if let Some(tex) = icon {
        // Icons fade in as they arrive instead of popping.
        let fade = ui.ctx().animate_bool_with_time(resp.id.with("img"), true, 0.25);
        p.image(
            tex.id(),
            irect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE.gamma_multiply(fade),
        );
    } else {
        p.rect_filled(irect, CornerRadius::same(10), skin.fill());
        crate::icons::paint(&p, Rect::from_center_size(irect.center(), Vec2::splat(30.0)), Icon::Cube, skin.faint(), 1.4);
    }
    p.rect_stroke(irect, CornerRadius::same(10), Stroke::new(1.0, skin.line_soft()), egui::StrokeKind::Inside);
    if !hit.tags.is_empty() {
        let font = theme::mono(8.5);
        let galley = p.layout_no_wrap(truncate(&hit.tags, 20), font, skin.ink());
        let pill = Rect::from_min_size(
            Pos2::new(irect.left() + 6.0, irect.bottom() - 6.0 - 16.0),
            Vec2::new(galley.size().x + 12.0, 16.0),
        );
        p.rect_filled(pill, CornerRadius::same(8), egui::Color32::from_black_alpha(190));
        p.galley(Pos2::new(pill.left() + 6.0, pill.center().y - galley.size().y / 2.0), galley, skin.ink());
    }
    // Freshness: when the project last shipped a file («3 дн. назад»).
    let ago = mods::ago(&hit.updated, crate::util::now_unix());
    if !ago.is_empty() {
        let font = theme::mono(8.5);
        let galley = p.layout_no_wrap(ago, font, skin.ink());
        let w = galley.size().x + 12.0 + 12.0;
        // Top-right corner: the bottom-left one holds «1.20.1 · Forge».
        let pill = Rect::from_min_size(Pos2::new(irect.right() - 6.0 - w, irect.top() + 6.0), Vec2::new(w, 16.0));
        p.rect_filled(pill, CornerRadius::same(8), egui::Color32::from_black_alpha(190));
        let icon = Rect::from_center_size(Pos2::new(pill.left() + 10.0, pill.center().y), Vec2::splat(9.0));
        crate::icons::paint(&p, icon, Icon::Refresh, skin.ink(), 1.3);
        p.galley(Pos2::new(pill.left() + 18.0, pill.center().y - galley.size().y / 2.0), galley, skin.ink());
    }

    // Text lines are measured with the real font and end with «…» —
    // nothing runs into the next line or under the buttons.
    let ty = rect.top() + 140.0;
    let w = cell_w - 24.0;
    let x = rect.left() + 12.0;
    widgets::text_fit(&p, Pos2::new(x, ty), Align2::LEFT_TOP, &hit.title, theme::display(13.5), skin.ink(), w);
    widgets::text_fit(&p, Pos2::new(x, ty + 20.0), Align2::LEFT_TOP, &hit.summary, theme::body(11.0), skin.dim(), w);
    let cats = if hit.categories.is_empty() { "—".to_string() } else { hit.categories.join(" · ") };
    widgets::text_fit(&p, Pos2::new(x, ty + 38.0), Align2::LEFT_TOP, &cats, theme::body(11.0), skin.ink().gamma_multiply(0.85), w);
    // Downloads first (live from the source), then what is inside a pack.
    let meta = match info {
        Some(info) => Some(format!("{} · {}", downloads_label(hit.downloads), info.label())),
        None if hit.kind == mods::KIND_MODPACK => Some(format!("{} · считаю моды…", downloads_label(hit.downloads))),
        None => Some(format!("{} · {}", downloads_label(hit.downloads), if hit.source == "curseforge" { "CurseForge" } else { "Modrinth" })),
    };
    if let Some(meta_txt) = meta {
        let icon_rect = Rect::from_center_size(Pos2::new(x + 5.0, ty + 61.5), Vec2::splat(10.0));
        crate::icons::paint(&p, icon_rect, Icon::ArrowDown, skin.faint(), 1.6);
        widgets::text_fit(&p, Pos2::new(x + 13.0, ty + 55.0), Align2::LEFT_TOP, &meta_txt, theme::mono(10.5), skin.faint(), (w - 13.0).max(20.0));
    }

    let _ = busy;
    let mut add = false;
    let mut install = false;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(Pos2::new(rect.left() + 10.0, rect.bottom() - 36.0), Vec2::new(cell_w - 20.0, 26.0)))
            .layout(Layout::right_to_left(Align::Center)),
    );
    child.multiply_opacity(0.15 + 0.85 * appear);
    let is_pack = hit.kind == mods::KIND_MODPACK;
    // Both buttons say what they do: no bare «+».
    let (main_label, main_hint) = if is_pack {
        ("Установить", "Скачать модпак и создать для него отдельную сборку")
    } else {
        ("Скачать", "Скачать в активную сборку прямо сейчас")
    };
    if widgets::button_sized(&mut child, skin, main_label, Btn::Primary, true, 26.0, 11.0, 10.0)
        .on_hover_text(main_hint)
        .clicked()
    {
        install = true;
    }
    if !is_pack
        && widgets::button_sized(&mut child, skin, "Добавить", Btn::Outline, true, 26.0, 11.0, 9.0)
            .on_hover_text("Добавить в список своей сборки — поставишь всё разом на вкладке «Сборка»")
            .clicked()
    {
        add = true;
    }
    let open = resp.clicked() && !install && !add;
    (add, install, open)
}

/// The project card: slides in from the right, gallery, description,
/// versions and compatibility with the active build.
fn draw_detail(app: &mut Launcher, ui: &mut Ui, skin: &Skin, hit: ModHit, busy: bool) {
    let ctx = ui.ctx().clone();
    let t = theme::arrive(&ctx, app.detail_at, 0.0, 0.30);
    ui.multiply_opacity(0.3 + 0.7 * t);
    let shift = theme::travel(28.0) * (1.0 - t);
    let full = ui.max_rect();
    let mut panel = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(full.translate(Vec2::new(shift, 0.0)))
            .layout(Layout::top_down(Align::Min)),
    );
    let ui = &mut panel;
    let mut back = ui.input(|i| i.key_pressed(egui::Key::Escape));
    let body_state = app.detail_body.clone();
    let detail = body_state.as_ref().and_then(|r| r.as_ref().ok()).cloned();

    ui.horizontal(|ui| {
        if widgets::button_icon(ui, skin, Icon::ArrowLeft, "Каталог", Btn::Ghost, true, 30.0, 12.5, 11.0).clicked() {
            back = true;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(if hit.source == "modrinth" { "Modrinth" } else { "CurseForge" })
                    .font(theme::mono(9.5))
                    .color(skin.faint()),
            );
        });
    });
    ui.add_space(10.0);

    let icon = hit.icon_url.as_deref().and_then(|u| app.mod_icon(&ctx, u));
    let pack_info = if hit.kind == mods::KIND_MODPACK { app.pack_info(&hit, true) } else { None };
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::splat(76.0), Sense::hover());
        if let Some(tex) = &icon {
            ui.painter().image(tex.id(), r, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), egui::Color32::WHITE);
        } else {
            ui.painter().rect_filled(r, CornerRadius::same(14), skin.fill());
        }
        ui.painter().rect_stroke(r, CornerRadius::same(14), Stroke::new(1.0, skin.line()), egui::StrokeKind::Inside);
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(RichText::new(truncate(&hit.title, 48)).font(theme::display(24.0)).color(skin.ink()));
            let author = detail.as_ref().map(|d| d.author.clone()).filter(|a| !a.is_empty()).unwrap_or_else(|| hit.author.clone());
            let downloads = detail.as_ref().map(|d| d.downloads).unwrap_or(hit.downloads);
            let updated = detail.as_ref().map(|d| d.updated.clone()).filter(|u| !u.is_empty()).unwrap_or_else(|| hit.updated.clone());
            let updated = match (crate::util::ru_date(&updated), mods::ago(&updated, crate::util::now_unix())) {
                (date, _) if date.is_empty() => String::new(),
                (date, ago) => format!(" · обновлено {date} ({ago})"),
            };
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "{} · {} скачиваний{}{}",
                        if author.is_empty() { "автор неизвестен".to_string() } else { author },
                        downloads_label(downloads),
                        updated,
                        if hit.tags.is_empty() { String::new() } else { format!(" · {}", hit.tags) }
                    ))
                    .font(theme::mono(10.0))
                    .color(skin.faint()),
                )
                .wrap(),
            );
            if !hit.summary.is_empty() {
                ui.add(egui::Label::new(RichText::new(truncate(&hit.summary, 160)).font(theme::body(12.5)).color(skin.dim())).wrap());
            }
            let mut facts: Vec<String> = Vec::new();
            if !hit.categories.is_empty() {
                facts.push(hit.categories.join(" · "));
            }
            if hit.kind == mods::KIND_MODPACK {
                facts.push(match pack_info {
                    Some(i) => i.label(),
                    None => "считаю моды и размер…".to_string(),
                });
            }
            if !facts.is_empty() {
                ui.add(egui::Label::new(RichText::new(facts.join("   ·   ")).font(theme::body(12.5)).color(skin.ink())).wrap());
            }
        });
    });
    ui.add_space(12.0);

    let is_pack = hit.kind == mods::KIND_MODPACK;
    let mut install_now = false;
    let mut add_now = false;
    let mut open_site = false;
    ui.horizontal(|ui| {
        let label = if is_pack { "Скачать модпак" } else { "Скачать" };
        if widgets::button(ui, skin, label, Btn::Primary, !busy).clicked() {
            install_now = true;
        }
        if !is_pack && widgets::button(ui, skin, "В список сборки", Btn::Outline, !busy).clicked() {
            add_now = true;
        }
        if widgets::button(ui, skin, "Открыть сайт", Btn::Ghost, true).clicked() {
            open_site = true;
        }
        if let Some(d) = &detail {
            if let Some(ok) = d.compatible {
                ui.add_space(6.0);
                if ok {
                    widgets::badge(ui, skin, "подходит к сборке", true);
                } else {
                    widgets::badge(ui, skin, "нет версии под сборку", false);
                }
            }
        }
    });
    if is_pack {
        ui.add_space(4.0);
        ui.label(
            RichText::new("Пак станет отдельной сборкой; его Minecraft, загрузчик и Java скачаются сами.")
                .font(theme::mono(9.0))
                .color(skin.faint()),
        );
    }
    ui.add_space(10.0);
    widgets::hairline(ui, skin.line_soft());
    ui.add_space(8.0);

    if app.download_card(ui, skin) {
        app.cancel_active_downloads();
    }

    egui::ScrollArea::vertical()
        .id_salt("mod-detail-body")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            match &body_state {
                None => {
                    ui.add_space(4.0);
                    for w in [0.92, 0.85, 0.97, 0.6] {
                        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() * w, 10.0), Sense::hover());
                        let pulse = 0.6 + 0.4 * ((ui.input(|i| i.time) as f32) * 3.0).sin().abs();
                        ui.painter().rect_filled(r, CornerRadius::same(5), skin.fill().gamma_multiply(1.0 + pulse));
                        ui.add_space(4.0);
                    }
                    ui.ctx().request_repaint_after(Duration::from_millis(33));
                }
                Some(Err(e)) => {
                    ui.label(RichText::new(truncate(e, 220)).font(theme::body(12.0)).color(skin.dim()));
                    ui.label(RichText::new("Описание можно открыть на сайте проекта.").font(theme::body(11.0)).color(skin.faint()));
                }
                Some(Ok(d)) => {
                    draw_videos(app, ui, skin, &ctx, &hit, &d.videos);
                    if !d.gallery.is_empty() {
                        egui::ScrollArea::horizontal()
                            .id_salt("mod-gallery")
                            .max_height(150.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 10.0;
                                    for url in &d.gallery {
                                        let (r, resp) = ui.allocate_exact_size(Vec2::new(240.0, 135.0), Sense::click());
                                        let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.15);
                                        ui.painter().rect_filled(r, CornerRadius::same(10), skin.fill());
                                        if ui.is_rect_visible(r) {
                                            if let Some(tex) = app.remote_image(&ctx, url, 480) {
                                                let fade = ui.ctx().animate_bool_with_time(resp.id.with("f"), true, 0.3);
                                                ui.painter().image(
                                                    tex.id(),
                                                    r,
                                                    cover_uv(tex.size_vec2(), r.size()),
                                                    egui::Color32::WHITE.gamma_multiply(fade * (0.85 + 0.15 * hover)),
                                                );
                                            }
                                        }
                                        ui.painter().rect_stroke(
                                            r,
                                            CornerRadius::same(10),
                                            Stroke::new(1.0, theme::lerp_color(skin.line_soft(), skin.fill_strong(), hover)),
                                            egui::StrokeKind::Inside,
                                        );
                                        if resp.clicked() {
                                            let _ = open::that_detached(url);
                                        }
                                    }
                                });
                            });
                        ui.add_space(12.0);
                    }
                    let body = if d.body.is_empty() { d.summary.clone() } else { d.body.clone() };
                    if body.is_empty() {
                        ui.label(RichText::new("У проекта нет описания.").font(theme::body(12.0)).color(skin.faint()));
                    } else {
                        ui.add(
                            egui::Label::new(RichText::new(body).font(theme::body(13.0)).color(skin.dim()).line_height(Some(20.0)))
                                .wrap(),
                        );
                    }
                    if !d.versions.is_empty() {
                        ui.add_space(14.0);
                        widgets::section_label(ui, skin, "ВЕРСИИ");
                        ui.add_space(4.0);
                        let spec = app.active_spec();
                        for v in &d.versions {
                            let fits = v.game_versions.iter().any(|g| g == &spec.mc)
                                && (hit.kind != mods::KIND_MOD
                                    || v.loaders.iter().any(|l| crate::loader::Loader::parse(l) == spec.loader));
                            ui.horizontal(|ui| {
                                widgets::status_dot(ui, if fits { skin.ink() } else { skin.faint() }, fits);
                                ui.label(RichText::new(truncate(&v.name, 46)).font(theme::mono(10.0)).color(if fits { skin.ink() } else { skin.dim() }));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let date = short_date(&v.date);
                                    let targets = format!(
                                        "{} · {}{}",
                                        truncate(&v.game_versions.join(", "), 40),
                                        v.loaders.join("/"),
                                        if date.is_empty() { String::new() } else { format!(" · {date}") }
                                    );
                                    ui.label(RichText::new(targets).font(theme::mono(8.5)).color(skin.faint()));
                                });
                            });
                        }
                    }
                    ui.add_space(16.0);
                }
            }
        });

    if back {
        app.detail = None;
        app.detail_rx = None;
        app.detail_body = None;
        app.tab_at = Instant::now();
    }
    if install_now {
        app.start_mod_install(&ctx, hit.clone());
    }
    if add_now {
        let spec = app.active_spec();
        app.build_add(&hit, &spec);
    }
    if open_site {
        let url = detail.map(|d| d.page_url).unwrap_or_else(|| {
            if hit.source == "modrinth" {
                format!("https://modrinth.com/project/{}", hit.slug)
            } else {
                format!("https://www.curseforge.com/projects/{}", hit.id)
            }
        });
        let _ = open::that_detached(&url);
    }
}

/// «Видео»: YouTube previews from the description — how the game looks
/// with this pack. A click opens the video in the browser (egui has no
/// video player, and a built-in one would cost tens of МБ). Without
/// videos in the description — one button to search YouTube.
fn draw_videos(app: &mut Launcher, ui: &mut Ui, skin: &Skin, ctx: &egui::Context, hit: &ModHit, videos: &[String]) {
    widgets::section_label(ui, skin, "ВИДЕО");
    ui.add_space(6.0);
    if videos.is_empty() {
        ui.horizontal(|ui| {
            let what = if hit.kind == mods::KIND_MODPACK { "modpack" } else { "mod" };
            if widgets::button_icon(ui, skin, Icon::Search, "Найти видео на YouTube", Btn::Outline, true, 28.0, 11.5, 12.0)
                .on_hover_text("В описании проекта нет видео — откроется поиск YouTube")
                .clicked()
            {
                let query = format!("{} minecraft {what}", hit.title);
                let _ = open::that_detached(format!("https://www.youtube.com/results?search_query={}", enc_query(&query)));
            }
            ui.label(RichText::new("автор не добавил видео в описание").font(theme::mono(9.0)).color(skin.faint()));
        });
        ui.add_space(12.0);
        return;
    }
    egui::ScrollArea::horizontal().id_salt("mod-videos").max_height(170.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            for id in videos {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(240.0, 135.0), Sense::click());
                let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.15);
                let p = ui.painter();
                p.rect_filled(r, CornerRadius::same(10), skin.fill());
                if ui.is_rect_visible(r) {
                    let thumb = format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg");
                    if let Some(tex) = app.remote_image(ctx, &thumb, 480) {
                        p.image(
                            tex.id(),
                            r,
                            cover_uv(tex.size_vec2(), r.size()),
                            egui::Color32::WHITE.gamma_multiply(0.75 + 0.25 * hover),
                        );
                    }
                }
                // Glass play button: ring + triangle, no solid fill.
                let c = r.center();
                let radius = 20.0 + 2.0 * hover;
                p.circle_filled(c, radius, egui::Color32::from_black_alpha(150));
                p.circle_stroke(c, radius, Stroke::new(1.5, skin.ink().gamma_multiply(0.9)));
                let s = 7.0 + hover;
                p.add(egui::Shape::convex_polygon(
                    vec![Pos2::new(c.x - s * 0.7, c.y - s), Pos2::new(c.x - s * 0.7, c.y + s), Pos2::new(c.x + s, c.y)],
                    skin.ink(),
                    Stroke::NONE,
                ));
                p.rect_stroke(
                    r,
                    CornerRadius::same(10),
                    Stroke::new(1.0, theme::lerp_color(skin.line_soft(), skin.fill_strong(), hover)),
                    egui::StrokeKind::Inside,
                );
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.on_hover_text("Смотреть на YouTube").clicked() {
                    let _ = open::that_detached(format!("https://www.youtube.com/watch?v={id}"));
                }
            }
        });
    });
    ui.add_space(12.0);
}

/// Percent-encodes a search query for a URL.
fn enc_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// UV rect that crops a texture to fill `target` (CSS object-fit: cover).
fn cover_uv(tex: Vec2, target: Vec2) -> Rect {
    if tex.x <= 0.0 || tex.y <= 0.0 {
        return Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    }
    let tex_ratio = tex.x / tex.y;
    let target_ratio = target.x / target.y;
    if tex_ratio > target_ratio {
        let w = target_ratio / tex_ratio;
        Rect::from_min_max(Pos2::new((1.0 - w) / 2.0, 0.0), Pos2::new((1.0 + w) / 2.0, 1.0))
    } else {
        let h = tex_ratio / target_ratio;
        Rect::from_min_max(Pos2::new(0.0, (1.0 - h) / 2.0), Pos2::new(1.0, (1.0 + h) / 2.0))
    }
}
