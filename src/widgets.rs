//! Shared controls. Everything is painted by hand so one stroke weight holds.

use crate::icons::{self, Icon};
use crate::theme::{self, Skin};
use egui::{Align, Color32, CursorIcon, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

pub fn hairline(ui: &mut Ui, color: Color32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0, color);
}

pub fn vline(painter: &egui::Painter, x: f32, y0: f32, y1: f32, color: Color32) {
    painter.line_segment(
        [Pos2::new(x, y0), Pos2::new(x, y1)],
        Stroke::new(1.0, color),
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    Primary,
    Outline,
    Ghost,
}

pub fn button(ui: &mut Ui, skin: &Skin, label: &str, kind: Btn, enabled: bool) -> Response {
    button_sized(ui, skin, label, kind, enabled, 32.0, 13.0, 15.0)
}

#[allow(clippy::too_many_arguments)]
pub fn button_sized(
    ui: &mut Ui,
    skin: &Skin,
    label: &str,
    kind: Btn,
    enabled: bool,
    height: f32,
    font_size: f32,
    pad: f32,
) -> Response {
    let font = theme::body(font_size);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, skin.ink());
    let width = galley.size().x + pad * 2.0;
    let (slot, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered() && enabled, 0.12);
    // Pressed buttons sink a pixel: the click is felt before anything loads.
    let press = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let rect = slot.shrink(press * 1.2);

    let (bg, fg, border) = match kind {
        Btn::Primary => (
            Color32::from_rgba_unmultiplied(
                255,
                255,
                255,
                if enabled { (18.0 + 18.0 * hover) as u8 } else { 7 },
            ),
            if enabled { skin.ink() } else { skin.faint() },
            if enabled {
                Color32::from_rgba_unmultiplied(255, 255, 255, (86.0 + 70.0 * hover) as u8)
            } else {
                skin.line_soft()
            },
        ),
        Btn::Outline => (
            Color32::from_rgba_unmultiplied(
                255,
                255,
                255,
                if enabled { (6.0 + 16.0 * hover) as u8 } else { 0 },
            ),
            if enabled {
                skin.dim_ink(0.35 + 0.65 * hover)
            } else {
                skin.faint()
            },
            theme::lerp_color(skin.line(), skin.fill_strong(), hover),
        ),
        Btn::Ghost => (
            skin.hover().gamma_multiply(hover * 0.9),
            if enabled {
                skin.dim_ink(0.25 + 0.75 * hover)
            } else {
                skin.faint()
            },
            if enabled && hover > 0.02 {
                skin.line_soft().gamma_multiply(hover)
            } else {
                Color32::TRANSPARENT
            },
        ),
    };

    if bg.a() > 0 {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(9), bg);
    }
    if border.a() > 0 {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(9),
            Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );
    }
    let painter = ui.painter();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        theme::body(font_size),
        fg,
    );
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Transparent button with a small SVG icon on the left of its label.
#[allow(clippy::too_many_arguments)]
pub fn button_icon(
    ui: &mut Ui,
    skin: &Skin,
    icon: icons::Icon,
    label: &str,
    kind: Btn,
    enabled: bool,
    height: f32,
    font_size: f32,
    pad: f32,
) -> Response {
    let font = theme::body(font_size);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), skin.ink());
    let icon_sz = (font_size + 1.0).clamp(10.0, 15.0);
    let gap = 6.0;
    let width = pad * 2.0 + icon_sz + gap + galley.size().x;
    let (slot, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let hover = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered() && enabled, 0.12);
    let press = ui
        .ctx()
        .animate_bool_with_time(resp.id.with("press"), enabled && resp.is_pointer_button_down_on(), 0.08);
    let rect = slot.shrink(press * 1.2);

    let (bg, fg, border) = match kind {
        Btn::Primary => (
            Color32::from_rgba_unmultiplied(
                255,
                255,
                255,
                if enabled { (18.0 + 18.0 * hover) as u8 } else { 7 },
            ),
            if enabled { skin.ink() } else { skin.faint() },
            if enabled {
                Color32::from_rgba_unmultiplied(255, 255, 255, (86.0 + 70.0 * hover) as u8)
            } else {
                skin.line_soft()
            },
        ),
        Btn::Outline => (
            Color32::from_rgba_unmultiplied(
                255,
                255,
                255,
                if enabled { (6.0 + 16.0 * hover) as u8 } else { 0 },
            ),
            if enabled {
                skin.dim_ink(0.35 + 0.65 * hover)
            } else {
                skin.faint()
            },
            theme::lerp_color(skin.line(), skin.fill_strong(), hover),
        ),
        Btn::Ghost => (
            skin.hover().gamma_multiply(hover * 0.9),
            if enabled {
                skin.dim_ink(0.25 + 0.75 * hover)
            } else {
                skin.faint()
            },
            if enabled && hover > 0.02 {
                skin.line_soft().gamma_multiply(hover)
            } else {
                Color32::TRANSPARENT
            },
        ),
    };
    if bg.a() > 0 {
        ui.painter().rect_filled(rect, egui::CornerRadius::same(9), bg);
    }
    if border.a() > 0 {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(9),
            Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );
    }
    let start_x = rect.left() + pad;
    let icon_rect = Rect::from_center_size(
        Pos2::new(start_x + icon_sz * 0.5, rect.center().y),
        Vec2::splat(icon_sz),
    );
    icons::paint(ui.painter(), icon_rect, icon, fg, 1.5);
    ui.painter().text(
        Pos2::new(start_x + icon_sz + gap, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        fg,
    );
    if enabled {
        resp.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn chip(ui: &mut Ui, skin: &Skin, label: &str, selected: bool) -> Response {
    let font = theme::body(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, skin.ink());
    let width = galley.size().x + 24.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 27.0), Sense::click());
    let (sel, fg) = chip_body(ui, skin, rect, &resp, selected);
    let _ = sel;
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        theme::body(12.5),
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

/// Shared chip background: selection fills in over 180 ms, hover lightens.
/// Keeps a translucent glass look without solid opaque fills.
fn chip_body(ui: &Ui, skin: &Skin, rect: Rect, resp: &Response, selected: bool) -> (f32, Color32) {
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
    let sel = ui.ctx().animate_bool_with_time(resp.id.with("sel"), selected, 0.18);
    let radius = egui::CornerRadius::same(14);
    if hover > 0.01 && sel < 1.0 {
        ui.painter().rect_filled(rect, radius, skin.hover().gamma_multiply(hover * (1.0 - sel)));
    }
    if sel > 0.0 {
        // The translucent highlight grows from the centre instead of snapping on.
        let grow = rect.shrink2(Vec2::new(rect.width() * 0.5 * (1.0 - sel), 0.0));
        ui.painter().rect_filled(
            grow,
            radius,
            Color32::from_rgba_unmultiplied(255, 255, 255, (28.0 * sel) as u8),
        );
    }
    let border = theme::lerp_color(
        theme::lerp_color(skin.line(), skin.fill_strong(), hover),
        Color32::from_rgba_unmultiplied(255, 255, 255, 145),
        sel,
    );
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    (sel, theme::lerp_color(skin.dim(), skin.ink(), sel.max(hover * 0.6)))
}

pub fn chip_icon(ui: &mut Ui, skin: &Skin, icon: Icon, label: &str, selected: bool) -> Response {
    let font = theme::body(12.5);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, skin.ink());
    let icon_sz = 14.0;
    let gap = 6.0;
    let width = galley.size().x + icon_sz + gap + 22.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 27.0), Sense::click());
    let (_, fg) = chip_body(ui, skin, rect, &resp, selected);
    let start_x = rect.left() + 11.0;
    let icon_rect = Rect::from_min_size(
        Pos2::new(start_x, rect.center().y - icon_sz * 0.5),
        Vec2::splat(icon_sz),
    );
    icons::paint(ui.painter(), icon_rect, icon, fg, 1.5);

    let text_pos = Pos2::new(start_x + icon_sz + gap, rect.center().y);
    ui.painter().text(
        text_pos,
        egui::Align2::LEFT_CENTER,
        label,
        theme::body(12.5),
        fg,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn status_dot(ui: &mut Ui, color: Color32, filled: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
    paint_status_dot(ui.painter(), rect.center(), color, filled);
}

/// The same dot painted at a fixed point (rows that lay out by hand).
pub fn paint_status_dot(painter: &egui::Painter, c: Pos2, color: Color32, filled: bool) {
    let ui = painter;
    if filled {
        ui.circle_filled(c, 3.2, color);
        ui.circle_stroke(c, 4.6, Stroke::new(1.0, color.gamma_multiply(0.35)));
    } else {
        ui.circle_stroke(c, 3.6, Stroke::new(1.0, color));
    }
}

/// Progress line. The fill glides to new values; at 0 (size unknown yet)
/// a soft segment sweeps across so the user sees work is happening.
pub fn progress_bar(ui: &mut Ui, skin: &Skin, frac: f32) {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, 4.0), Sense::hover());
    let radius = egui::CornerRadius::same(2);
    ui.painter().rect_filled(rect, radius, skin.fill());
    if frac <= 0.001 {
        if theme::reduced_motion() {
            let seg = Rect::from_min_size(rect.min, Vec2::new(width * 0.25, 4.0));
            ui.painter().rect_filled(seg, radius, skin.dim());
            return;
        }
        let t = ui.input(|i| i.time) as f32;
        let seg = width * 0.3;
        let x = (t * 0.75).fract() * (width + seg) - seg;
        let sweep = Rect::from_min_max(
            Pos2::new(rect.left() + x.max(0.0), rect.top()),
            Pos2::new((rect.left() + x + seg).min(rect.right()), rect.bottom()),
        );
        if sweep.width() > 0.0 {
            ui.painter().rect_filled(sweep, radius, skin.dim());
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        return;
    }
    let shown = theme::tween(ui.ctx(), resp.id, frac.clamp(0.0, 1.0), 0.4);
    let fill = Rect::from_min_size(rect.min, Vec2::new((width * shown).max(2.0), 4.0));
    ui.painter().rect_filled(fill, radius, skin.ink());
}

pub fn slider(ui: &mut Ui, skin: &Skin, value: &mut f32, min: f32, max: f32) -> Response {
    let width = ui.available_width().min(260.0);
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::click_and_drag());
    let track_y = rect.center().y;
    let x0 = rect.left() + 8.0;
    let x1 = rect.right() - 8.0;
    if resp.dragged() || resp.clicked() {
        if let Some(pos) = ui.ctx().pointer_interact_pos() {
            let t = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
            let next = min + (max - min) * t;
            if (next - *value).abs() > f32::EPSILON {
                *value = next;
                resp.mark_changed();
            }
        }
    }
    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let knob_x = x0 + (x1 - x0) * t;
    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered() || resp.dragged(), 0.12);
    ui.painter().line_segment(
        [Pos2::new(x0, track_y), Pos2::new(x1, track_y)],
        Stroke::new(4.0, Color32::from_rgba_unmultiplied(255, 255, 255, 38)),
    );
    ui.painter().line_segment(
        [Pos2::new(x0, track_y), Pos2::new(knob_x, track_y)],
        Stroke::new(4.0, skin.ink()),
    );
    if hover > 0.01 {
        ui.painter().circle_filled(
            Pos2::new(knob_x, track_y),
            10.5,
            Color32::from_rgba_unmultiplied(255, 255, 255, (28.0 * hover) as u8),
        );
    }
    ui.painter().circle_filled(
        Pos2::new(knob_x, track_y),
        7.0,
        skin.ink(),
    );
    ui.painter().circle_stroke(
        Pos2::new(knob_x, track_y),
        7.0,
        Stroke::new(1.5, skin.black()),
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub fn toggle(ui: &mut Ui, skin: &Skin, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(42.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.16);
    let radius = 12.0;
    let bg = theme::lerp_color(skin.fill_strong(), skin.ink(), t);
    ui.painter().rect_filled(rect, egui::CornerRadius::same(12), bg);
    let knob_x = rect.left() + radius + (rect.width() - 2.0 * radius) * t;
    ui.painter().circle_filled(
        Pos2::new(knob_x, rect.center().y),
        8.0,
        theme::lerp_color(skin.dim(), skin.black(), t),
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

pub struct FieldStyle {
    pub rows: usize,
    pub font: f32,
    pub lead: f32,
    pub secret: bool,
}

impl Default for FieldStyle {
    fn default() -> Self {
        Self {
            rows: 1,
            font: 14.0,
            lead: 12.0,
            secret: false,
        }
    }
}

pub fn text_field(
    ui: &mut Ui,
    skin: &Skin,
    id: &str,
    buf: &mut String,
    hint: &str,
    style: FieldStyle,
) -> Response {
    let inner_width = ui.available_width();
    let frame = egui::Frame::new()
        .fill(skin.fill())
        .stroke(Stroke::new(1.0, skin.line()))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(14, style.lead as i8));

    let resp = frame
        .show(ui, |ui| {
            ui.set_width(inner_width - 28.0);
            let edit = if style.rows <= 1 {
                egui::TextEdit::singleline(buf)
            } else {
                egui::TextEdit::multiline(buf).desired_rows(style.rows)
            };
            ui.add(
                edit.id_salt(id)
                    .hint_text(egui::RichText::new(hint).color(skin.faint()))
                    .desired_width(f32::INFINITY)
                    .font(theme::body(style.font))
                    .text_color(skin.ink())
                    .margin(egui::Margin::same(0))
                    .password(style.secret)
                    .frame(egui::Frame::NONE),
            )
        })
        .inner;

    if resp.has_focus() {
        ui.painter().rect_stroke(
            egui::Rect::from_min_max(
                resp.rect.min - Vec2::new(15.0, style.lead + 1.0),
                resp.rect.max + Vec2::new(15.0, style.lead + 1.0),
            ),
            egui::CornerRadius::same(12),
            Stroke::new(1.0, skin.fill_strong()),
            egui::StrokeKind::Inside,
        );
    }
    resp
}

pub fn section_label(ui: &mut Ui, skin: &Skin, text: &str) {
    ui.add_space(2.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        text,
        theme::mono(9.5),
        skin.faint(),
    );
}

pub fn stat(ui: &mut Ui, skin: &Skin, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.label(egui::RichText::new(label).font(theme::mono(9.5)).color(skin.faint()));
        ui.label(egui::RichText::new(value).font(theme::display(21.0)).color(skin.ink()));
    });
}

pub fn kv_row(ui: &mut Ui, skin: &Skin, key: &str, value: &str, mono_value: bool) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(key)
                .font(theme::body(13.0))
                .color(skin.dim()),
        );
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                egui::RichText::new(value)
                    .font(if mono_value { theme::mono(11.5) } else { theme::body(13.0) })
                    .color(skin.ink()),
            );
        });
    });
}

pub fn empty_state(ui: &mut Ui, skin: &Skin, icon: Icon, title: &str, hint: &str) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(34.0), Sense::hover());
        icons::paint(ui.painter(), rect.shrink(6.0), icon, skin.faint(), 1.4);
        ui.add_space(6.0);
        ui.label(egui::RichText::new(title).font(theme::display(15.5)).color(skin.dim()));
        ui.add_space(2.0);
        ui.label(egui::RichText::new(hint).font(theme::body(12.5)).color(skin.faint()));
    });
    ui.add_space(24.0);
}

/// Right-aligned header block used at the top of every section.
pub fn header(ui: &mut Ui, skin: &Skin, title: &str, subtitle: &str, actions: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.label(
                egui::RichText::new(title)
                    .font(theme::display(23.0))
                    .color(skin.ink()),
            );
            if !subtitle.is_empty() {
                ui.label(
                    egui::RichText::new(subtitle)
                        .font(theme::body(12.5))
                        .color(skin.faint()),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            actions(ui);
        });
    });
    ui.add_space(14.0);
}

pub fn badge(ui: &mut Ui, skin: &Skin, text: &str, filled: bool) {
    let font = theme::mono(9.5);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, skin.ink());
    let width = galley.size().x + 14.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
    if filled {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(9),
            Color32::from_rgba_unmultiplied(255, 255, 255, 22),
        );
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(9),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 110)),
            egui::StrokeKind::Inside,
        );
    } else {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(9),
            Stroke::new(1.0, skin.line()),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        theme::mono(9.5),
        if filled { skin.ink() } else { skin.dim() },
    );
}

/// Translucent tinted badge (green/red semantic states).
pub fn semantic_badge(ui: &mut Ui, text: &str, color: Color32) {
    let font = theme::mono(9.5);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, color);
    let width = galley.size().x + 14.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 18.0), Sense::hover());
    ui.painter().rect_filled(rect, egui::CornerRadius::same(9), color.gamma_multiply(0.14));
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(9),
        Stroke::new(1.0, color.gamma_multiply(0.48)),
        egui::StrokeKind::Inside,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        theme::mono(9.5),
        color,
    );
}

/// Styled compact singleline input (ports, room codes, addresses, filters).
#[allow(clippy::too_many_arguments)]
pub fn compact_input_ex(
    ui: &mut Ui,
    skin: &Skin,
    id: &str,
    buf: &mut String,
    hint: &str,
    width: f32,
    height: f32,
    font: egui::FontId,
    left_icon: Option<icons::Icon>,
) -> Response {
    let h = height.max(24.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    let cr = egui::CornerRadius::same(8);
    ui.painter().rect_filled(rect, cr, skin.fill());
    let pad_l = if left_icon.is_some() { 28.0 } else { 9.0 };
    if let Some(ic) = left_icon {
        let icon_rect = Rect::from_center_size(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            Vec2::splat(13.0),
        );
        icons::paint(ui.painter(), icon_rect, ic, skin.faint(), 1.5);
    }
    let inner = Rect::from_min_max(
        Pos2::new(rect.left() + pad_l, rect.top()),
        Pos2::new(rect.right() - 9.0, rect.bottom()),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing = Vec2::ZERO;
    let resp = child.add(
        egui::TextEdit::singleline(buf)
            .id_salt(id)
            .hint_text(egui::RichText::new(hint).color(skin.faint()))
            .desired_width(inner.width())
            .min_size(Vec2::new(inner.width(), inner.height()))
            .align(egui::Align2::LEFT_CENTER)
            .font(font)
            .text_color(skin.ink())
            .margin(egui::Margin::same(0))
            .frame(egui::Frame::NONE),
    );
    ui.painter().rect_stroke(
        rect,
        cr,
        Stroke::new(1.0, if resp.has_focus() { skin.fill_strong() } else { skin.line() }),
        egui::StrokeKind::Inside,
    );
    resp
}

#[allow(clippy::too_many_arguments)]
pub fn compact_input(
    ui: &mut Ui,
    skin: &Skin,
    id: &str,
    buf: &mut String,
    hint: &str,
    width: f32,
    height: f32,
    font: egui::FontId,
) -> Response {
    compact_input_ex(ui, skin, id, buf, hint, width, height, font, None)
}

/// Selectable read-only pill: shrinks tightly around its content, text vertically centered.
pub fn copyable_pill(
    ui: &mut Ui,
    skin: &Skin,
    id: &str,
    value: &str,
    max_width: f32,
    height: f32,
    accent: Option<Color32>,
) -> Response {
    let font = theme::mono(11.5);
    let text_w = ui
        .painter()
        .layout_no_wrap(value.to_string(), font.clone(), skin.ink())
        .size()
        .x;
    let w = (text_w + 18.0).clamp(48.0, max_width.max(56.0));
    let h = height.clamp(24.0, 28.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let cr = egui::CornerRadius::same(8);
    let border = accent.map(|c| c.gamma_multiply(0.45)).unwrap_or_else(|| skin.line());
    let fill = accent.map(|c| c.gamma_multiply(0.08)).unwrap_or_else(|| skin.fill());
    ui.painter().rect_filled(rect, cr, fill);
    ui.painter().rect_stroke(
        rect,
        cr,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    );
    let mut view = value.to_string();
    let inner = rect.shrink2(Vec2::new(9.0, 0.0));
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(Align::Center)),
    );
    child.spacing_mut().item_spacing = Vec2::ZERO;
    child.add(
        egui::TextEdit::singleline(&mut view)
            .id_salt(id)
            .desired_width(inner.width())
            .min_size(Vec2::new(inner.width(), inner.height()))
            .align(egui::Align2::LEFT_CENTER)
            .font(font)
            .text_color(accent.unwrap_or_else(|| skin.ink()))
            .margin(egui::Margin::same(0))
            .frame(egui::Frame::NONE),
    )
}

/// One line of text that never runs past `max_width`: it is measured with
/// the real font and ends with «…» when it does not fit. Used wherever text
/// is painted at a fixed spot next to other text or buttons.
#[allow(clippy::too_many_arguments)]
pub fn text_fit(
    painter: &egui::Painter,
    pos: Pos2,
    align: egui::Align2,
    text: &str,
    font: egui::FontId,
    color: Color32,
    max_width: f32,
) -> Rect {
    let clean = text.replace(['\n', '\r'], " ");
    let mut job = egui::text::LayoutJob::simple_singleline(clean, font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: max_width.max(8.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = painter.layout_job(job);
    let rect = align.anchor_size(pos, galley.size());
    painter.galley(rect.min, galley, color);
    rect
}

/// Leading text of a row that also has right-aligned controls: it gets the
/// row width minus `reserve`, so it can never slide under those controls.
/// `wrap` breaks long text into lines; otherwise it ends with «…».
pub fn lead_text(ui: &mut Ui, reserve: f32, text: egui::RichText, wrap: bool) {
    let w = (ui.available_width() - reserve).max(60.0);
    let min_h = if wrap { 0.0 } else { 24.0 };
    ui.allocate_ui_with_layout(Vec2::new(w, min_h), egui::Layout::left_to_right(Align::Center), |ui| {
        ui.set_max_width(w);
        let label = egui::Label::new(text);
        ui.add(if wrap { label.wrap() } else { label.truncate() });
    });
}
