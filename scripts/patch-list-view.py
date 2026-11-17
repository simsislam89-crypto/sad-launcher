import io

# app.rs: layout state
p = "src/app.rs"
src = io.open(p, encoding="utf-8").read()
src = src.replace("    pub catalog_at: Instant,", "    pub catalog_at: Instant,\n    /// 0 = сетка карточек, 1 = список\n    pub mods_layout: usize,")
src = src.replace("            catalog_at: Instant::now(),", "            catalog_at: Instant::now(),\n            mods_layout: 0,")
io.open(p, "w", encoding="utf-8", newline="\n").write(src)

# views/mods.rs: toggle chips + two renderers
p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

src = src.replace('''                        widgets::section_label(ui, skin, label);
                        ui.add_space(6.0);
                        let results = self.search.results.clone();
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                            for (i, hit) in results.iter().enumerate() {
                                let icon = hit
                                    .icon_url
                                    .as_deref()
                                    .and_then(|u| self.mod_icon(ui.ctx(), u));
                                let (added, do_install, open) = hit_card(
                                    ui,
                                    skin,
                                    hit,
                                    icon.as_ref(),
                                    busy || download_busy,
                                );
                                if added {
                                    add_build = Some(i);
                                }
                                if do_install {
                                    install_hit = Some(hit.clone());
                                }
                                if open {
                                    open_detail = Some(hit.clone());
                                }
                            }
                        });''', '''                        widgets::section_label(ui, skin, label);
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if widgets::chip(ui, skin, "▦ Сетка", self.mods_layout == 0).clicked() {
                                self.mods_layout = 0;
                            }
                            if widgets::chip(ui, skin, "☰ Список", self.mods_layout == 1).clicked() {
                                self.mods_layout = 1;
                            }
                        });
                        ui.add_space(6.0);
                        let results = self.search.results.clone();
                        if self.mods_layout == 0 {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                                for (i, hit) in results.iter().enumerate() {
                                    let icon = hit
                                        .icon_url
                                        .as_deref()
                                        .and_then(|u| self.mod_icon(ui.ctx(), u));
                                    let (added, do_install, open) = hit_card(
                                        ui,
                                        skin,
                                        hit,
                                        icon.as_ref(),
                                        busy || download_busy,
                                    );
                                    if added {
                                        add_build = Some(i);
                                    }
                                    if do_install {
                                        install_hit = Some(hit.clone());
                                    }
                                    if open {
                                        open_detail = Some(hit.clone());
                                    }
                                }
                            });
                        } else {
                            for (i, hit) in results.iter().enumerate() {
                                let icon = hit
                                    .icon_url
                                    .as_deref()
                                    .and_then(|u| self.mod_icon(ui.ctx(), u));
                                let (added, do_install, open) = list_row(
                                    ui,
                                    skin,
                                    hit,
                                    icon.as_ref(),
                                    busy || download_busy,
                                );
                                if added {
                                    add_build = Some(i);
                                }
                                if do_install {
                                    install_hit = Some(hit.clone());
                                }
                                if open {
                                    open_detail = Some(hit.clone());
                                }
                            }
                        }''')

# add list_row fn after hit_card
list_fn = '''
/// Табличный вид: строка с маленькой иконкой, описанием и действиями.
/// Returns (added_to_build, install_now, open_detail).
fn list_row(
    ui: &mut Ui,
    skin: &Skin,
    hit: &ModHit,
    icon: Option<&egui::TextureHandle>,
    busy: bool,
) -> (bool, bool, bool) {
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 56.0), Sense::click());
    let hovered = resp.hovered();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        skin.hover().gamma_multiply(if hovered { 0.8 } else { 0.4 }),
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, if hovered { skin.line() } else { skin.line_soft() }),
        egui::StrokeKind::Inside,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let side = rect.height() - 12.0;
    let irect = Rect::from_center_size(
        Pos2::new(rect.left() + 8.0 + side / 2.0, rect.center().y),
        Vec2::splat(side),
    );
    if let Some(tex) = icon {
        ui.painter().image(
            tex.id(),
            irect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().rect_filled(irect, CornerRadius::same(6), skin.fill());
        ui.painter().text(
            irect.center(),
            Align2::CENTER_CENTER,
            "★",
            theme::display(14.0),
            skin.faint(),
        );
    }
    ui.painter().rect_stroke(
        irect,
        CornerRadius::same(6),
        Stroke::new(1.0, skin.line_soft()),
        egui::StrokeKind::Inside,
    );

    let text_x = irect.right() + 10.0;
    ui.painter().text(
        Pos2::new(text_x, rect.center().y - 13.0),
        Align2::LEFT_CENTER,
        truncate(&hit.title, 44),
        theme::body(13.0),
        skin.ink(),
    );
    ui.painter().text(
        Pos2::new(text_x, rect.center().y + 2.0),
        Align2::LEFT_CENTER,
        format!(
            "{} · {} · {} загрузок",
            hit.author,
            if hit.source == "curseforge" { "CF" } else { "Modrinth" },
            downloads_label(hit.downloads)
        ),
        theme::mono(8.5),
        skin.faint(),
    );
    ui.painter().text(
        Pos2::new(text_x, rect.center().y + 16.0),
        Align2::LEFT_CENTER,
        truncate(&hit.summary, 90),
        theme::body(10.5),
        skin.dim(),
    );

    let mut add = false;
    let mut install = false;
    let open = resp.clicked();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(
                Pos2::new(rect.right() - 150.0, rect.center().y - 15.0),
                Vec2::new(142.0, 30.0),
            ))
            .layout(Layout::right_to_left(Align::Center)),
    );
    if widgets::button_sized(&mut child, skin, "Скачать", Btn::Primary, !busy, 26.0, 11.0, 10.5)
        .clicked()
    {
        install = true;
    }
    if widgets::button_sized(
        &mut child,
        skin,
        "+ сборка",
        Btn::Ghost,
        !busy && hit.kind != crate::mods::KIND_MODPACK,
        26.0,
        10.0,
        9.5,
    )
    .clicked()
    {
        add = true;
    }
    (add, install, open)
}
'''
src += list_fn
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("list view ok")
