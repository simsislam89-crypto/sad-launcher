//! Hand-drawn icon set on a 24×24 grid, one stroke weight, no glyph fallbacks.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Cube,
    Compass,
    Course,
    Server,
    Sliders,
    Plus,
    Close,
    Minimize,
    Maximize,
    Restore,
    Play,
    Stop,
    Globe,
    Download,
    Heart,
    Refresh,
    Sparkle,
    Modrinth,
    CurseForge,
    Layers,
    Check,
    Trash,
    Folder,
}

struct Pen<'a> {
    p: &'a Painter,
    o: Pos2,
    k: f32,
    w: f32,
    color: Color32,
}

impl<'a> Pen<'a> {
    fn pt(&self, x: f32, y: f32) -> Pos2 {
        self.o + egui::vec2(x * self.k, y * self.k)
    }
    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.p
            .line_segment([self.pt(a.0, a.1), self.pt(b.0, b.1)], Stroke::new(self.w, self.color));
    }
    fn poly(&self, pts: &[(f32, f32)]) {
        for pair in pts.windows(2) {
            self.line(pair[0], pair[1]);
        }
    }
    fn closed(&self, pts: &[(f32, f32)]) {
        self.poly(pts);
        if let (Some(first), Some(last)) = (pts.first(), pts.last()) {
            self.line(*last, *first);
        }
    }
    fn circle(&self, c: (f32, f32), r: f32) {
        self.p
            .circle_stroke(self.pt(c.0, c.1), r * self.k, Stroke::new(self.w, self.color));
    }
    fn dot(&self, c: (f32, f32), r: f32) {
        self.p
            .circle_filled(self.pt(c.0, c.1), r * self.k, self.color);
    }
    fn ellipse(&self, c: (f32, f32), rx: f32, ry: f32) {
        let steps = 30;
        let mut prev = None;
        for i in 0..=steps {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            let p = self.pt(c.0 + rx * a.cos(), c.1 + ry * a.sin());
            if let Some(pp) = prev {
                self.p.line_segment([pp, p], Stroke::new(self.w, self.color));
            }
            prev = Some(p);
        }
    }
    fn rect(&self, r: (f32, f32, f32, f32), rad: f32) {
        let rect = Rect::from_min_max(self.pt(r.0, r.1), self.pt(r.2, r.3));
        self.p.rect_stroke(
            rect,
            egui::CornerRadius::same((rad * self.k).max(1.0) as u8),
            Stroke::new(self.w, self.color),
            egui::StrokeKind::Inside,
        );
    }
    fn fill_poly(&self, pts: &[(f32, f32)]) {
        let pts: Vec<Pos2> = pts.iter().map(|(x, y)| self.pt(*x, *y)).collect();
        self.p.add(Shape::convex_polygon(pts, self.color, Stroke::NONE));
    }
}

pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32, weight: f32) {
    let s = rect.width().min(rect.height());
    let k = s / 24.0;
    let o = rect.center() - egui::vec2(12.0 * k, 12.0 * k);
    let pen = Pen {
        p,
        o,
        k,
        w: (weight * k).max(1.0),
        color,
    };

    match icon {
        Icon::Cube => {
            pen.closed(&[
                (12.0, 3.2),
                (20.4, 7.8),
                (20.4, 16.2),
                (12.0, 20.8),
                (3.6, 16.2),
                (3.6, 7.8),
            ]);
            pen.poly(&[(3.6, 7.8), (12.0, 12.4), (20.4, 7.8)]);
            pen.line((12.0, 12.4), (12.0, 20.8));
        }
        Icon::Compass => {
            pen.circle((12.0, 12.0), 8.7);
            pen.fill_poly(&[(16.6, 7.4), (13.4, 13.4), (10.6, 10.6)]);
            pen.poly(&[(7.4, 16.6), (10.6, 10.6), (13.4, 13.4)]);
            pen.line((10.6, 13.4), (13.4, 10.6));
        }
        Icon::Course => {
            pen.rect((4.8, 2.8, 19.2, 21.2), 3.0);
            pen.poly(&[(13.4, 2.8), (19.2, 8.6), (13.4, 8.6)]);
            pen.line((13.4, 2.8), (13.4, 8.6));
            pen.line((19.2, 8.6), (13.4, 8.6));
            pen.line((8.2, 13.2), (15.8, 13.2));
            pen.line((8.2, 16.8), (13.0, 16.8));
        }
        Icon::Server => {
            pen.rect((3.2, 4.0, 20.8, 20.0), 3.0);
            pen.line((3.2, 9.3), (20.8, 9.3));
            pen.line((3.2, 14.7), (20.8, 14.7));
            pen.dot((7.0, 6.6), 1.05);
            pen.dot((7.0, 12.0), 1.05);
            pen.dot((7.0, 17.4), 1.05);
        }
        Icon::Sliders => {
            pen.line((3.2, 6.4), (12.2, 6.4));
            pen.line((16.6, 6.4), (20.8, 6.4));
            pen.circle((14.4, 6.4), 2.2);
            pen.line((3.2, 12.0), (7.0, 12.0));
            pen.line((11.4, 12.0), (20.8, 12.0));
            pen.circle((9.2, 12.0), 2.2);
            pen.line((3.2, 17.6), (14.2, 17.6));
            pen.line((18.6, 17.6), (20.8, 17.6));
            pen.circle((16.4, 17.6), 2.2);
        }
        Icon::Plus => {
            pen.line((12.0, 4.8), (12.0, 19.2));
            pen.line((4.8, 12.0), (19.2, 12.0));
        }
        Icon::Play => pen.fill_poly(&[(8.2, 5.2), (19.0, 12.0), (8.2, 18.8)]),
        Icon::Stop => {
            let rect = Rect::from_min_max(pen.pt(7.0, 7.0), pen.pt(17.0, 17.0));
            pen.p.rect_filled(
                rect,
                egui::CornerRadius::same((1.6 * k).max(1.0) as u8),
                color,
            );
        }
        Icon::Close => {
            pen.line((6.4, 6.4), (17.6, 17.6));
            pen.line((17.6, 6.4), (6.4, 17.6));
        }
        Icon::Minimize => pen.line((6.0, 12.0), (18.0, 12.0)),
        Icon::Maximize => pen.rect((6.0, 6.0, 18.0, 18.0), 1.5),
        Icon::Restore => {
            pen.rect((4.5, 8.0, 15.5, 19.0), 1.5);
            pen.poly(&[(8.5, 8.0), (8.5, 4.5), (19.5, 4.5), (19.5, 15.5), (15.5, 15.5)]);
        }
        Icon::Globe => {
            pen.circle((12.0, 12.0), 8.8);
            pen.ellipse((12.0, 12.0), 4.2, 8.8);
            pen.line((3.4, 12.0), (20.6, 12.0));
        }
        Icon::Download => {
            pen.line((12.0, 4.0), (12.0, 14.5));
            pen.poly(&[(7.5, 10.5), (12.0, 15.0), (16.5, 10.5)]);
            pen.poly(&[(4.5, 14.0), (4.5, 19.5), (19.5, 19.5), (19.5, 14.0)]);
        }
        Icon::Heart => {
            pen.closed(&[
                (12.0, 7.5),
                (14.5, 4.5),
                (18.5, 5.0),
                (20.5, 8.5),
                (20.0, 12.0),
                (12.0, 19.5),
                (4.0, 12.0),
                (3.5, 8.5),
                (5.5, 5.0),
                (9.5, 4.5),
            ]);
        }
        Icon::Refresh => {
            pen.poly(&[
                (4.5, 12.0),
                (5.5, 7.5),
                (9.0, 4.8),
                (14.5, 4.8),
                (18.8, 8.2),
                (19.5, 11.5),
            ]);
            pen.poly(&[(15.0, 11.5), (19.5, 11.5), (19.5, 7.0)]);
            pen.poly(&[
                (19.5, 12.0),
                (18.5, 16.5),
                (15.0, 19.2),
                (9.5, 19.2),
                (5.2, 15.8),
                (4.5, 12.5),
            ]);
            pen.poly(&[(9.0, 12.5), (4.5, 12.5), (4.5, 17.0)]);
        }
        Icon::Sparkle => {
            pen.closed(&[
                (12.0, 3.0),
                (14.5, 9.5),
                (21.0, 12.0),
                (14.5, 14.5),
                (12.0, 21.0),
                (9.5, 14.5),
                (3.0, 12.0),
                (9.5, 9.5),
            ]);
        }
        Icon::Modrinth => {
            pen.closed(&[
                (12.0, 3.5),
                (19.5, 7.8),
                (19.5, 16.2),
                (12.0, 20.5),
                (4.5, 16.2),
                (4.5, 7.8),
            ]);
            pen.poly(&[(7.5, 15.0), (7.5, 9.5), (12.0, 13.5), (16.5, 9.5), (16.5, 15.0)]);
        }
        Icon::CurseForge => {
            pen.closed(&[
                (5.0, 19.0),
                (19.0, 19.0),
                (16.0, 13.0),
                (18.0, 13.0),
                (12.0, 4.0),
                (11.0, 9.0),
                (8.0, 13.0),
                (10.0, 13.0),
            ]);
        }
        Icon::Layers => {
            pen.closed(&[(12.0, 3.5), (20.5, 7.8), (12.0, 12.0), (3.5, 7.8)]);
            pen.poly(&[(3.5, 12.2), (12.0, 16.5), (20.5, 12.2)]);
            pen.poly(&[(3.5, 16.5), (12.0, 20.8), (20.5, 16.5)]);
        }
        Icon::Check => {
            pen.poly(&[(4.5, 12.5), (9.5, 17.5), (19.5, 6.5)]);
        }
        Icon::Trash => {
            pen.line((5.0, 7.0), (19.0, 7.0));
            pen.poly(&[(9.0, 7.0), (9.0, 4.5), (15.0, 4.5), (15.0, 7.0)]);
            pen.poly(&[(6.5, 7.0), (7.5, 20.0), (16.5, 20.0), (17.5, 7.0)]);
            pen.line((10.0, 10.5), (10.0, 16.5));
            pen.line((14.0, 10.5), (14.0, 16.5));
        }
        Icon::Folder => {
            pen.closed(&[
                (3.5, 5.5),
                (9.5, 5.5),
                (11.5, 8.0),
                (20.5, 8.0),
                (20.5, 19.5),
                (3.5, 19.5),
            ]);
            pen.line((3.5, 9.5), (20.5, 9.5));
        }
    }
}

