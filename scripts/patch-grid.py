import io, re

p = "src/views/mods.rs"
src = io.open(p, encoding="utf-8").read()

# 1) auto-refresh: re-fetch catalog every 5 minutes
src = src.replace('''        // The catalog is never empty: popular content loads right away.
        if page == 0
            && self.search.results.is_empty()
            && !busy
            && !self.search.searched
        {
            search_now = true;
        }''', '''        // The catalog is never empty: popular content loads right away
        // and re-fetches itself every 5 minutes.
        if page == 0 && !busy {
            if self.search.results.is_empty() && !self.search.searched {
                search_now = true;
            } else if !self.search.results.is_empty()
                && self.catalog_at.elapsed() >= Duration::from_secs(300)
            {
                self.search.results.clear();
                self.search.searched = false;
                self.catalog_at = Instant::now();
                search_now = true;
            }
        }''')

src = src.replace("use crate::app::{Launcher, Tab};",
                  "use crate::app::{Launcher, Tab};\nuse std::time::{Duration, Instant};")

# 2) grid layout: replace the results list loop with big cards
old_loop = '''                    } else {
                        let results = self.search.results.clone();
                        for (i, hit) in results.iter().enumerate() {
                            let icon = hit
                                .icon_url
                                .as_deref()
                                .and_then(|u| self.mod_icon(ui.ctx(), u));
                            let (added, do_install, open) = hit_row(
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
                    }
                    ui.add_space(12.0);
                });'''
new_loop = '''                    } else {
                        let label = if self.search.query.trim().is_empty() {
                            "ПОПУЛЯРНОЕ СЕЙЧАС"
                        } else {
                            "РЕЗУЛЬТАТЫ"
                        };
                        widgets::section_label(ui, skin, label);
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
                        });
                    }
                    ui.add_space(12.0);
                });'''
assert old_loop in src, "results loop not found"
src = src.replace(old_loop, new_loop, 1)

# 3) replace hit_row with big-card hit_card
m = re.search(r"/// Returns \(added_to_build, install_now, open_detail\)\.\nfn hit_row\(.*?\n\}\n", src, re.S)
assert m, "hit_row not found"
card_fn = '''/// One catalog tile: big icon, title, downloads, two actions.
/// Returns (added_to_build, install_now, open_detail).
fn hit_card(
    ui: &mut Ui,
    skin: &Skin,
    hit: &ModHit,
    icon: Option<&egui::TextureHandle>,
    busy: bool,
) -> (bool, bool, bool) {
    let cell_w = 158.0;
    let cell_h = 214.0;
    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(cell_w, cell_h), Sense::click());
    let hovered = resp.hovered();
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(12),
        if hovered {
            skin.hover().gamma_multiply(0.9)
        } else {
            skin.hover().gamma_multiply(0.4)
        },
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(12),
        Stroke::new(1.0, if hovered { skin.line() } else { skin.line_soft() }),
        egui::StrokeKind::Inside,
    );
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // big icon
    let icon_size = 126.0;
    let irect = Rect::from_min_size(
        Pos2::new(rect.left() + (cell_w - icon_size) / 2.0, rect.top() + 10.0),
        Vec2::splat(icon_size),
    );
    if let Some(tex) = icon {
        ui.painter().image(
            tex.id(),
            irect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().rect_filled(irect, CornerRadius::same(10), skin.fill());
        ui.painter().text(
            irect.center(),
            Align2::CENTER_CENTER,
            "★",
            theme::display(30.0),
            skin.faint(),
        );
    }
    ui.painter().rect_stroke(
        irect,
        CornerRadius::same(10),
        Stroke::new(1.0, skin.line_soft()),
        egui::StrokeKind::Inside,
    );

    // title + meta
    let ty = irect.bottom() + 8.0;
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, ty + 8.0),
        Align2::LEFT_CENTER,
        truncate(&hit.title, 26),
        theme::body(12.5),
        skin.ink(),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 10.0, ty + 26.0),
        Align2::LEFT_CENTER,
        format!("{} · {}", downloads_label(hit.downloads),
            if hit.source == "curseforge" { "CF" } else { "Modrinth" }),
        theme::mono(8.5),
        skin.faint(),
    );

    // actions
    let mut add = false;
    let mut install = false;
    let mut open = resp.clicked();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(
                Pos2::new(rect.left() + 8.0, rect.bottom() - 34.0),
                Vec2::new(cell_w - 16.0, 26.0),
            ))
            .layout(Layout::right_to_left(Align::Center)),
    );
    if widgets::button_sized(&mut child, skin, "Скачать", Btn::Primary, !busy, 24.0, 10.5, 10.0)
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
        24.0,
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
src = src[:m.start()] + card_fn + src[m.end():]
io.open(p, "w", encoding="utf-8", newline="\n").write(src)
print("grid ok")
