//! Section views. Shared painters live here so every tab keeps one language.

pub mod build;
pub mod effects;
pub mod mods;
pub mod play;
pub mod server;
pub mod settings;
pub mod versions;

use crate::icons::{self, Icon};
use crate::theme::{self, Skin};
use crate::widgets;
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, Pos2, Rect, Response, RichText, Sense, Stroke, Ui,
    Vec2,
};

pub fn truncate(text: &str, max: usize) -> String {
    let cleaned = text.replace(['\n', '\r'], " ");
    if cleaned.chars().count() <= max {
        cleaned
    } else {
        let cut: String = cleaned.chars().take(max).collect();
        format!("{cut}…")
    }
}

pub fn downloads_label(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} млн", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0} тыс", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub fn short_date(iso: &str) -> String {
    if iso.len() >= 10 {
        format!("{}.{}.{}", &iso[8..10], &iso[5..7], &iso[0..4])
    } else {
        iso.to_string()
    }
}

pub fn search_field(
    ui: &mut Ui,
    skin: &Skin,
    id: &str,
    buf: &mut String,
    hint: &str,
    width: f32,
) -> Response {
    widgets::compact_input(ui, skin, id, buf, hint, width, 32.0, theme::body(13.0))
}

pub fn play_button(ui: &mut Ui, skin: &Skin, running: bool) -> Response {
    let width = if running { 162.0 } else { 142.0 };
    let (slot, resp) = ui.allocate_exact_size(Vec2::new(width, 52.0), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let press = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("press"), resp.is_pointer_button_down_on(), 0.08);
    let rect = slot.shrink(press * 1.2);
    let bg = if running {
        Color32::from_rgba_unmultiplied(242, 108, 108, (22.0 + 24.0 * hover) as u8)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, (20.0 + 20.0 * hover) as u8)
    };
    let border = if running {
        skin.red().gamma_multiply(0.55 + 0.35 * hover)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, (105.0 + 75.0 * hover) as u8)
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(13), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(13),
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    let fg = if running { skin.red() } else { skin.ink() };
    icons::paint(
        ui.painter(),
        Rect::from_center_size(
            Pos2::new(rect.left() + 28.0, rect.center().y),
            Vec2::splat(15.0),
        ),
        if running { Icon::Stop } else { Icon::Play },
        fg,
        1.6,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 46.0, rect.center().y),
        Align2::LEFT_CENTER,
        if running { "Остановить" } else { "Играть" },
        theme::display(15.0),
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Returns true when "Очистить" was pressed.
#[allow(clippy::too_many_arguments)]
pub fn console(
    ui: &mut Ui,
    skin: &Skin,
    title: &str,
    state: &str,
    lines: &[String],
    running: bool,
    max_height: f32,
    salt: &str,
) -> bool {
    let mut clear = false;
    let frame = egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 70))
        .stroke(Stroke::new(1.0, skin.line_soft()))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(14, 11));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            widgets::status_dot(
                ui,
                if running { skin.green() } else { skin.faint() },
                running,
            );
            ui.label(
                RichText::new(title)
                    .font(theme::mono(9.0))
                    .color(skin.faint()),
            );
            ui.label(
                RichText::new(state)
                    .font(theme::mono(8.5))
                    .color(skin.faint().gamma_multiply(0.8)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if widgets::button_sized(ui, skin, "Очистить", widgets::Btn::Ghost, true, 24.0, 11.0, 9.0)
                    .clicked()
                {
                    clear = true;
                }
            });
        });
        ui.add_space(5.0);
        egui::ScrollArea::vertical()
            .id_salt(salt)
            .max_height(max_height.max(48.0))
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if lines.is_empty() {
                    ui.label(
                        RichText::new("Логи появятся здесь после запуска.")
                            .font(theme::mono(10.0))
                            .color(skin.faint()),
                    );
                }
                for line in lines.iter().rev().take(220).rev() {
                    let color = if line.contains("ERROR") || line.contains("Exception") || line.contains("FATAL") {
                        skin.red()
                    } else if line.contains("WARN") {
                        skin.dim()
                    } else {
                        skin.faint()
                    };
                    ui.label(
                        RichText::new(line)
                            .font(theme::mono(9.5))
                            .color(color),
                    );
                }
            });
    });
    clear
}

pub fn loader_badge(ui: &mut Ui, skin: &Skin, loader: &str) {
    let l = crate::loader::Loader::parse(loader);
    widgets::badge(ui, skin, &l.label().to_uppercase(), l != crate::loader::Loader::Vanilla);
}
