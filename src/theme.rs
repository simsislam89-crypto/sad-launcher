//! Design tokens: one ink, one paper, and steps of translucency in between.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

pub const FAM_DISPLAY: &str = "display";

#[derive(Clone, Copy)]
pub struct Skin {
    pub alpha: f32,
}

impl Skin {
    pub fn new(alpha: f32) -> Self {
        Self {
            alpha: alpha.clamp(0.15, 0.99),
        }
    }

    fn a(&self) -> u8 {
        (self.alpha * 255.0).round() as u8
    }

    pub fn base(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(8, 8, 10, self.a())
    }

    pub fn ink(&self) -> Color32 {
        Color32::from_rgb(238, 238, 241)
    }

    /// Secondary text. 8.1:1 on the base slab.
    pub fn dim(&self) -> Color32 {
        Color32::from_rgb(169, 169, 176)
    }

    /// Tertiary text and mono meta. 5.2:1 on the base slab.
    pub fn faint(&self) -> Color32 {
        Color32::from_rgb(134, 134, 142)
    }

    pub fn line(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 22)
    }

    pub fn line_soft(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 12)
    }

    pub fn hover(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 15)
    }

    pub fn fill(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 9)
    }

    pub fn fill_strong(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(255, 255, 255, 32)
    }

    pub fn black(&self) -> Color32 {
        Color32::from_rgb(11, 11, 13)
    }

    pub fn dim_ink(&self, t: f32) -> Color32 {
        lerp_color(self.dim(), self.ink(), t)
    }

    /// Semantic green: same network, server reachable, active/ready.
    pub fn green(&self) -> Color32 {
        Color32::from_rgb(104, 224, 146)
    }

    /// Semantic red: errors, unreachable server, failed actions.
    pub fn red(&self) -> Color32 {
        Color32::from_rgb(242, 108, 108)
    }
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}

/// Smallest sizes that still read as crisp glyphs on a 96-dpi screen:
/// anything below turns into grey mush, so tiny captions are lifted.
fn readable(size: f32, min: f32) -> f32 {
    if size < min {
        // Keep relative steps between small styles, just shifted up.
        min + (size - min) * 0.35
    } else {
        size
    }
    .round_to_half()
}

trait HalfStep {
    fn round_to_half(self) -> f32;
}

impl HalfStep for f32 {
    /// Half-pixel steps: glyph sizes the hinter grid-fits cleanly.
    fn round_to_half(self) -> f32 {
        (self * 2.0).round() / 2.0
    }
}

pub fn body(size: f32) -> FontId {
    FontId::new(readable(size, 11.5), FontFamily::Proportional)
}

pub fn display(size: f32) -> FontId {
    FontId::new(readable(size, 12.0), FontFamily::Name(FAM_DISPLAY.into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::new(readable(size, 10.5), FontFamily::Monospace)
}

fn read_face(file: &str) -> Option<Arc<FontData>> {
    let path = format!("C:\\Windows\\Fonts\\{file}");
    // Hinted onto the pixel grid, no sub-pixel offsets: stems land on
    // whole pixels, so letters look printed rather than smeared.
    let tweak = egui::FontTweak {
        hinting: Some(true),
        subpixel_binning: Some(false),
        ..Default::default()
    };
    std::fs::read(path)
        .ok()
        .map(|bytes| Arc::new(FontData::from_owned(bytes).tweak(tweak)))
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    if let Some(f) = read_face("segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), f);
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "segoe".into());
    }
    if let Some(f) = read_face("segoeuib.ttf") {
        fonts.font_data.insert("segoe-bold".into(), f);
        fonts
            .families
            .entry(FontFamily::Name(FAM_DISPLAY.into()))
            .or_default()
            .push("segoe-bold".into());
    }
    if let Some(f) = read_face("consola.ttf") {
        fonts.font_data.insert("consola".into(), f);
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "consola".into());
    }

    // Cyrillic fallback and UI glyphs come from egui's bundled faces.
    let display = fonts
        .families
        .entry(FontFamily::Name(FAM_DISPLAY.into()))
        .or_default();
    if display.is_empty() {
        display.push("Ubuntu-Light".into());
    }
    ctx.set_fonts(fonts);
}

pub fn style(ctx: &egui::Context, skin: &Skin) {
    let mut style = egui::Style {
        visuals: egui::Visuals::dark(),
        ..Default::default()
    };
    style.visuals.override_text_color = Some(skin.ink());
    style.visuals.text_options.font_hinting = true;
    style.visuals.text_options.subpixel_binning = false;
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = skin.base();
    style.visuals.extreme_bg_color = Color32::from_rgba_unmultiplied(0, 0, 0, 90);
    style.visuals.faint_bg_color = skin.fill();
    style.visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(255, 255, 255, 46);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, skin.ink());
    style.visuals.hyperlink_color = skin.ink();
    style.visuals.window_stroke = egui::Stroke::new(1.0, skin.line());
    style.visuals.window_corner_radius = egui::CornerRadius::same(14);

    style.visuals.widgets.noninteractive.bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, skin.line_soft());
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, skin.dim());
    style.visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(10);

    style.visuals.widgets.inactive.bg_fill = skin.fill();
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, skin.line());
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, skin.ink());
    style.visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(10);

    style.visuals.widgets.hovered.bg_fill = skin.hover();
    style.visuals.widgets.hovered.weak_bg_fill = skin.hover();
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, skin.fill_strong());
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, skin.ink());
    style.visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(10);

    style.visuals.widgets.active.bg_fill = skin.hover();
    style.visuals.widgets.active.weak_bg_fill = skin.hover();
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, skin.ink());
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, skin.ink());
    style.visuals.widgets.active.corner_radius = egui::CornerRadius::same(10);

    style.visuals.widgets.open = style.visuals.widgets.active;

    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    // Reserve space for scrollbars so handles never cover text, and keep them clearly visible.
    style.spacing.scroll.floating = true;
    style.spacing.scroll.floating_allocated_width = 9.0;
    style.spacing.scroll.floating_width = 5.0;
    style.spacing.scroll.bar_width = 7.0;
    style.spacing.scroll.bar_inner_margin = 4.0;
    style.spacing.scroll.bar_outer_margin = 2.0;
    style.spacing.scroll.handle_min_length = 28.0;
    style.spacing.scroll.foreground_color = true;
    style.spacing.scroll.dormant_handle_opacity = 0.36;
    style.spacing.scroll.active_handle_opacity = 0.72;
    style.spacing.scroll.dormant_background_opacity = 0.08;
    style.spacing.scroll.active_background_opacity = 0.18;
    style.spacing.scroll.interact_background_opacity = 0.24;
    style.spacing.interact_size.y = 30.0;
    style.spacing.text_edit_width = 280.0;

    style.text_styles.insert(egui::TextStyle::Body, body(14.0));
    style.text_styles.insert(egui::TextStyle::Button, body(13.0));
    style.text_styles.insert(egui::TextStyle::Small, body(11.5));
    style.text_styles.insert(egui::TextStyle::Heading, display(20.0));
    style.text_styles.insert(egui::TextStyle::Monospace, mono(12.0));

    ctx.all_styles_mut(|target| *target = style.clone());
}

pub fn card_frame(skin: &Skin) -> egui::Frame {
    egui::Frame::new()
        .fill(skin.fill())
        .stroke(egui::Stroke::new(1.0, skin.line_soft()))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(16, 14))
}

// ---------------------------------------------------------------- motion

/// Windows «Показывать анимацию в Windows». When it is off, movement
/// collapses to zero; fades and colour changes that carry state remain.
static REDUCED_MOTION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn SystemParametersInfoW(action: u32, param: u32, pv: *mut std::ffi::c_void, winini: u32) -> i32;
}

/// Linux/macOS: `SAD_REDUCED_MOTION=1` turns movement off (no portable
/// system switch to read without extra libraries).
#[cfg(unix)]
pub fn init_motion() {
    let off = std::env::var("SAD_REDUCED_MOTION").map(|v| v == "1").unwrap_or(false);
    REDUCED_MOTION.store(off, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(windows)]
pub fn init_motion() {
    const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;
    let mut enabled: i32 = 1;
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut i32 as *mut std::ffi::c_void,
            0,
        )
    };
    REDUCED_MOTION.store(ok != 0 && enabled == 0, std::sync::atomic::Ordering::Relaxed);
}

pub fn reduced_motion() -> bool {
    REDUCED_MOTION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Exponential ease-out: fast start, soft landing, no bounce.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        1.0
    } else {
        1.0 - 2f32.powf(-10.0 * t)
    }
}

/// Eased 0..1 progress of an animation that started at `since`, after
/// `delay` seconds, lasting `dur` seconds. Requests repaints while running.
pub fn arrive(ctx: &egui::Context, since: std::time::Instant, delay: f32, dur: f32) -> f32 {
    let t = (since.elapsed().as_secs_f32() - delay) / dur.max(0.001);
    if t < 1.0 {
        ctx.request_repaint();
    }
    ease_out(t)
}

/// Distance in pixels a moving element travels; zero under reduced motion.
pub fn travel(px: f32) -> f32 {
    if reduced_motion() {
        0.0
    } else {
        px
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ease_out_is_monotonic_and_lands() {
        assert_eq!(super::ease_out(0.0), 0.0);
        assert_eq!(super::ease_out(1.0), 1.0);
        assert_eq!(super::ease_out(2.0), 1.0);
        let mut prev = 0.0;
        for i in 1..=10 {
            let v = super::ease_out(i as f32 / 10.0);
            assert!(v > prev);
            prev = v;
        }
        assert!(super::ease_out(0.3) > 0.8, "front-loaded");
    }
}

/// Eased tween toward `target` (exponential ease-out, restarts from the
/// current value when the target moves). Instant under reduced motion.
pub fn tween(ctx: &egui::Context, id: egui::Id, target: f32, dur: f32) -> f32 {
    if reduced_motion() {
        return target;
    }
    let now = ctx.input(|i| i.time);
    let state = ctx.data_mut(|d| d.get_temp::<(f32, f32, f64)>(id));
    let Some((from, to, start)) = state else {
        ctx.data_mut(|d| d.insert_temp(id, (target, target, now)));
        return target;
    };
    let t = ((now - start) as f32 / dur.max(0.001)).clamp(0.0, 1.0);
    let current = from + (to - from) * ease_out(t);
    if (target - to).abs() > 0.001 {
        ctx.data_mut(|d| d.insert_temp(id, (current, target, now)));
        ctx.request_repaint();
        return current;
    }
    if t < 1.0 {
        ctx.request_repaint();
    }
    current
}
