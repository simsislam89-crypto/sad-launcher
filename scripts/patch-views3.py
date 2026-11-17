import io

# ============ views/mods.rs: draw_detail + honest source + build chip in catalog
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

# 1) start_search honest source
p2 = "src/app.rs"
src2 = io.open(p2, encoding="utf-8").read()
src2 = src2.replace('''        let kind = mods::KINDS[self.search.kind.min(mods::KINDS.len() - 1)].0.to_string();
        let has_key = !self.cfg.cf_api_key.trim().is_empty();
        let source = mods::pick_source(
            if self.search.source == 0 { "curseforge" } else { "modrinth" },
            &kind,
            has_key,
        )
        .to_string();''', '''        let kind = mods::KINDS[self.search.kind.min(mods::KINDS.len() - 1)].0.to_string();
        let has_key = !self.cfg.cf_api_key.trim().is_empty();
        let user_source = if self.search.source == 0 {
            "curseforge"
        } else {
            "modrinth"
        };
        // Shaders/resourcepacks only exist on Modrinth; CurseForge packs need a key.
        let source = match kind.as_str() {
            mods::KIND_SHADER | mods::KIND_RESOURCEPACK => "modrinth".to_string(),
            mods::KIND_MODPACK => {
                if has_key {
                    user_source.to_string()
                } else {
                    "modrinth".to_string()
                }
            }
            _ => user_source.to_string(),
        };''')
io.open(p2, "w", encoding="utf-8", newline="\n").write(src2)

# 2) catalog: build chip
src = src.replace('''            if self.download_card(ui, skin) {
                self.cancel_active_downloads();
            }

            egui::ScrollArea::vertical()
                .id_salt("mods-catalog")''', '''            if self.download_card(ui, skin) {
                self.cancel_active_downloads();
            }

            let build_count = self.build.len();
            if build_count > 0 {
                ui.horizontal(|ui| {
                    if widgets::chip(
                        ui,
                        skin,
                        &format!("СБОРКА · {} — открыть", build_count),
                        false,
                    )
                    .clicked()
                    {
                        self.mods_page = 1;
                    }
                });
                ui.add_space(6.0);
            }

            egui::ScrollArea::vertical()
                .id_salt("mods-catalog")''')

# 3) draw_detail helper at end of file
helper = '''

/// Полноэкранная карточка проекта: описание, кнопки, сайт.
fn draw_detail(
    app: &mut Launcher,
    ui: &mut Ui,
    skin: &Skin,
    hit: ModHit,
    busy: bool,
) {
    let full = ui.max_rect();
    ui.painter().rect_filled(
        full,
        CornerRadius::same(18),
        egui::Color32::from_rgba_unmultiplied(8, 8, 10, 250),
    );
    let inner = full.shrink2(egui::vec2(26.0, 16.0));
    let mut panel = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::top_down(Align::Min)),
    );

    let icon = hit
        .icon_url
        .as_deref()
        .and_then(|u| app.mod_icon(ui.ctx(), u));

    panel.horizontal(|ui| {
        if widgets::button(ui, skin, "← Назад", Btn::Ghost, true).clicked() {
            app.detail = None;
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(if hit.source == "modrinth" {
                    "Modrinth"
                } else {
                    "CurseForge"
                })
                .font(theme::mono(9.5))
                .color(skin.faint()),
            );
        });
    });
    panel.add_space(8.0);

    panel.horizontal(|ui| {
        if let Some(tex) = &icon {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
            ui.painter().image(
                tex.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            ui.painter().rect_stroke(
                r,
                CornerRadius::same(10),
                Stroke::new(1.0, skin.line()),
                egui::StrokeKind::Inside,
            );
            ui.add_space(12.0);
        }
        ui.vertical(|ui| {
            ui.label(
                RichText::new(truncate(&hit.title, 44))
                    .font(theme::display(24.0))
                    .color(skin.ink()),
            );
            ui.label(
                RichText::new(format!(
                    "{} · {} загрузок · {}",
                    if hit.author.is_empty() { "—" } else { &hit.author },
                    downloads_label(hit.downloads),
                    if hit.source == "modrinth" { "Modrinth" } else { "CurseForge" }
                ))
                .font(theme::mono(10.0))
                .color(skin.faint()),
            );
            if !hit.summary.is_empty() {
                ui.label(
                    RichText::new(truncate(&hit.summary, 120))
                        .font(theme::body(12.0))
                        .color(skin.dim()),
                );
            }
        });
    });
    panel.add_space(12.0);

    panel.horizontal(|ui| {
        let mut install_now = false;
        let mut add_now = false;
        let mut open_site = false;
        if widgets::button(ui, skin, "Скачать", Btn::Primary, !busy).clicked() {
            install_now = true;
        }
        if hit.kind != crate::mods::KIND_MODPACK {
            if widgets::button(ui, skin, "В сборку", Btn::Outline, !busy).clicked() {
                add_now = true;
            }
        }
        if hit.source == "modrinth" {
            if widgets::button(ui, skin, "Открыть сайт", Btn::Ghost, true).clicked() {
                open_site = true;
            }
        }
        if install_now {
            let h = hit.clone();
            let ctx = ui.ctx().clone();
            app.start_mod_install(ctx, h);
        }
        if add_now {
            let (mc, loader) = app.target_version();
            let h = hit.clone();
            app.build_add(&h, mc, loader);
        }
        if open_site {
            let url = format!("https://modrinth.com/mod/{}", hit.slug);
            let _ = open::that_detached(&url);
        }
    });
    panel.add_space(10.0);

    widgets::hairline(ui, skin.line_soft());
    panel.add_space(8.0);

    let body_state = app.detail_body.clone();
    match body_state {
        None => {
            panel.label(
                RichText::new("Загружаю описание…")
                    .font(theme::mono(10.0))
                    .color(skin.faint()),
            );
        }
        Some(Err(e)) => {
            panel.label(
                RichText::new(truncate(&e, 200))
                    .font(theme::body(12.0))
                    .color(skin.dim()),
            );
            if hit.source == "curseforge" {
                panel.label(
                    RichText::new("Карточки CurseForge открываются на их сайте.")
                        .font(theme::body(11.0))
                        .color(skin.faint()),
                );
            }
        }
        Some(Ok(body)) => {
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
        }
    }
}
'''

src += helper
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("views3 ok")
