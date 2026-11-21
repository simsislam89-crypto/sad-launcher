//! Full-window media viewer, the way Telegram shows photos and videos:
//! the launcher dims, the picture or the YouTube player sits in the middle
//! as large as fits, round arrows on the sides switch items (also ← / →),
//! «Закрыть» / Esc / a click on the dark area closes it.

use crate::app::Launcher;
use crate::icons::Icon;
use crate::theme::{self, Skin};
use crate::widgets::{self, Btn};
use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, Vec2};
use std::time::Instant;

/// One item of the viewer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Media {
    /// YouTube video id.
    Video(String),
    /// Image URL.
    Image(String),
}

pub struct Viewer {
    pub items: Vec<Media>,
    pub index: usize,
    pub title: String,
    pub opened: Instant,
    /// When the current item was shown (its fade-in).
    pub shown: Instant,
}

impl Viewer {
    pub fn new(items: Vec<Media>, index: usize, title: &str) -> Viewer {
        let index = index.min(items.len().saturating_sub(1));
        Viewer { items, index, title: title.to_string(), opened: Instant::now(), shown: Instant::now() }
    }

    pub fn current(&self) -> Option<&Media> {
        self.items.get(self.index)
    }

    /// Moves by `step` (±1); stops at the first and the last item.
    pub fn step(&mut self, step: i32) -> bool {
        let next = self.index as i32 + step;
        if next < 0 || next >= self.items.len() as i32 {
            return false;
        }
        self.index = next as usize;
        self.shown = Instant::now();
        true
    }
}

/// Largest 16:9 (or image-shaped) rect that fits `area` with margins for
/// the arrows and the bottom bar.
pub fn media_rect(area: Rect, aspect: f32) -> Rect {
    let avail = Vec2::new((area.width() - 2.0 * 84.0).max(160.0), (area.height() - 2.0 * 64.0).max(90.0));
    let aspect = if aspect.is_finite() && aspect > 0.05 { aspect } else { 16.0 / 9.0 };
    let size = if avail.x / avail.y > aspect {
        Vec2::new(avail.y * aspect, avail.y)
    } else {
        Vec2::new(avail.x, avail.x / aspect)
    };
    Rect::from_center_size(area.center() - Vec2::new(0.0, 8.0), size)
}

impl Launcher {
    /// Opens the viewer at `index` of `items` (videos first, then images).
    pub fn open_viewer(&mut self, items: Vec<Media>, index: usize, title: &str) {
        if items.is_empty() {
            return;
        }
        self.video_state = None;
        self.viewer = Some(Viewer::new(items, index, title));
    }

    /// Draws the viewer over everything. A video item asks for the player
    /// window at its rect (`video_rect`), placed after the frame.
    pub fn draw_viewer(&mut self, ctx: &egui::Context, skin: &Skin) {
        let Some(viewer) = self.viewer.as_ref() else {
            return;
        };
        let (current, index, count, title, opened, shown) = match viewer.current() {
            Some(m) => (m.clone(), viewer.index, viewer.items.len(), viewer.title.clone(), viewer.opened, viewer.shown),
            None => {
                self.viewer = None;
                return;
            }
        };
        let t = theme::arrive(ctx, opened, 0.0, 0.18);
        let fade = theme::arrive(ctx, shown, 0.0, 0.22);
        let screen = ctx.content_rect();
        let mut close = false;
        let mut step = 0i32;

        let backdrop = egui::Area::new(Id::unique("viewer-backdrop"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(r, CornerRadius::same(18), Color32::from_black_alpha((228.0 * t) as u8));
                resp
            })
            .inner;
        close |= backdrop.clicked();
        ctx.input(|i| {
            close |= i.key_pressed(egui::Key::Escape);
            if i.key_pressed(egui::Key::ArrowLeft) {
                step = -1;
            }
            if i.key_pressed(egui::Key::ArrowRight) {
                step = 1;
            }
        });

        let image_tex = match &current {
            Media::Image(url) => self.remote_image(ctx, url, 1920),
            Media::Video(_) => None,
        };
        let thumb_tex = match &current {
            Media::Video(id) => self.remote_image(ctx, &format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"), 480),
            Media::Image(_) => None,
        };
        let aspect = image_tex.as_ref().map(|t| t.size_vec2().x / t.size_vec2().y.max(1.0)).unwrap_or(16.0 / 9.0);
        let media = media_rect(screen, aspect);
        let video_ready = matches!(self.video_state, Some(crate::app::VideoState::Playing));
        let video_error = match &self.video_state {
            Some(crate::app::VideoState::Error(e)) => Some(e.clone()),
            _ => None,
        };

        egui::Area::new(Id::unique("viewer-content"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_clip_rect(screen);
                let p = ui.painter().clone();
                p.rect_filled(media, CornerRadius::same(10), Color32::from_black_alpha(255));
                match &current {
                    Media::Image(_) => match &image_tex {
                        Some(tex) => {
                            p.image(
                                tex.id(),
                                media,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE.gamma_multiply(fade),
                            );
                        }
                        None => loading(&p, media, skin, "Загружаю картинку…"),
                    },
                    Media::Video(_) => {
                        // Under the player window: the preview while it
                        // starts, or the reason it cannot play.
                        if let Some(tex) = &thumb_tex {
                            p.image(
                                tex.id(),
                                media,
                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                Color32::WHITE.gamma_multiply(0.35),
                            );
                        }
                        match &video_error {
                            Some(e) => loading(&p, media, skin, e),
                            None if !video_ready => loading(&p, media, skin, "Запускаю видео…"),
                            None => {}
                        }
                    }
                }
                // Swallow clicks on the media so they do not close the viewer.
                ui.interact(media, Id::unique("viewer-media"), Sense::click());

                // Side arrows, Telegram-style: round, glassy, only where there
                // is somewhere to go.
                let arrow = |ui: &mut egui::Ui, center: Pos2, icon: Icon, id: &str| -> bool {
                    let r = Rect::from_center_size(center, Vec2::splat(48.0));
                    let resp = ui.interact(r, Id::unique(id), Sense::click());
                    let hover = ui.ctx().animate_bool_with_time(resp.id, resp.hovered(), 0.12);
                    let p = ui.painter();
                    p.circle_filled(r.center(), 24.0, Color32::from_white_alpha((10.0 + 26.0 * hover) as u8));
                    p.circle_stroke(r.center(), 24.0, Stroke::new(1.0, skin.line()));
                    crate::icons::paint(p, Rect::from_center_size(r.center(), Vec2::splat(20.0)), icon, skin.ink(), 1.8);
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    resp.clicked()
                };
                let y = screen.center().y;
                if index > 0 && arrow(ui, Pos2::new(screen.left() + 42.0, y), Icon::ArrowLeft, "viewer-prev") {
                    step = -1;
                }
                if index + 1 < count && arrow(ui, Pos2::new(screen.right() - 42.0, y), Icon::ArrowRight, "viewer-next") {
                    step = 1;
                }

                // Top-right: close. Bottom: what is shown and where it is.
                let close_rect = Rect::from_min_size(Pos2::new(screen.right() - 128.0, screen.top() + 14.0), Vec2::new(112.0, 32.0));
                let mut top = ui.new_child(egui::UiBuilder::new().max_rect(close_rect).layout(egui::Layout::right_to_left(egui::Align::Center)));
                if widgets::button_icon(&mut top, skin, Icon::Close, "Закрыть", Btn::Ghost, true, 30.0, 12.0, 12.0).clicked() {
                    close = true;
                }
                let kind = match &current {
                    Media::Video(_) => "Видео",
                    Media::Image(_) => "Скриншот",
                };
                p.text(
                    Pos2::new(screen.left() + 22.0, screen.bottom() - 40.0),
                    Align2::LEFT_TOP,
                    crate::views::truncate(&title, 60),
                    theme::body(13.0),
                    skin.ink(),
                );
                p.text(
                    Pos2::new(screen.left() + 22.0, screen.bottom() - 21.0),
                    Align2::LEFT_TOP,
                    format!("{kind} · {} из {count}", index + 1),
                    theme::mono(10.0),
                    skin.faint(),
                );
                if let Media::Video(id) = &current {
                    let r = Rect::from_min_size(Pos2::new(screen.right() - 220.0, screen.bottom() - 46.0), Vec2::new(200.0, 30.0));
                    let mut bar = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::right_to_left(egui::Align::Center)));
                    if widgets::button_icon(&mut bar, skin, Icon::Play, "Открыть на YouTube", Btn::Outline, true, 28.0, 11.5, 12.0)
                        .clicked()
                    {
                        let _ = open::that_detached(format!("https://www.youtube.com/watch?v={id}"));
                    }
                }
            });

        if let Media::Video(id) = &current {
            if video_error.is_none() {
                self.video_request = Some((id.clone(), media));
            }
        }
        if step != 0 {
            if let Some(v) = self.viewer.as_mut() {
                if v.step(step) {
                    self.video_state = None;
                }
            }
        }
        if close {
            self.viewer = None;
            self.video_state = None;
        }
        if !video_ready && matches!(current, Media::Video(_)) && video_error.is_none() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }
}

fn loading(p: &egui::Painter, rect: Rect, skin: &Skin, text: &str) {
    p.text(rect.center(), Align2::CENTER_CENTER, text, theme::body(13.0), skin.dim());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_fits_inside_with_margins() {
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1240.0, 760.0));
        let r = media_rect(area, 16.0 / 9.0);
        assert!(r.width() <= 1240.0 - 168.0 + 0.5 && r.height() <= 760.0 - 128.0 + 0.5);
        assert!(((r.width() / r.height()) - 16.0 / 9.0).abs() < 0.01);
        let tall = media_rect(area, 0.5);
        assert!(tall.height() > tall.width());
    }

    #[test]
    fn viewer_stops_at_the_ends() {
        let mut v = Viewer::new(vec![Media::Video("a".repeat(11)), Media::Image("u".into())], 0, "t");
        assert!(!v.step(-1));
        assert!(v.step(1));
        assert!(!v.step(1));
        assert_eq!(v.index, 1);
    }
}
