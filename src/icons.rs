//! Векторный SVG-рендерер для мелких иконок интерфейса (сетка 24×24 из `assets/icons/*.svg`).
//! Разбирает SVG один раз при первом обращении и рисует со скруглёнными концами штриха и кривыми Безье.

use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke};
use std::sync::OnceLock;

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    Effects,
    Modrinth,
    CurseForge,
    Layers,
    Check,
    Trash,
    Folder,
    Search,
    Copy,
    ArrowLeft,
    ArrowRight,
    ArrowDown,
    ChevronDown,
    Keyboard,
}

#[derive(Clone, Debug)]
enum SvgPrim {
    Polyline {
        pts: Vec<(f32, f32)>,
        closed: bool,
        fill: bool,
        stroke: bool,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        fill: bool,
        stroke: bool,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rx: f32,
        fill: bool,
        stroke: bool,
    },
}

fn svg_source(icon: Icon) -> &'static str {
    match icon {
        Icon::Cube => include_str!("../assets/icons/cube.svg"),
        Icon::Compass => include_str!("../assets/icons/compass.svg"),
        Icon::Course => include_str!("../assets/icons/course.svg"),
        Icon::Server => include_str!("../assets/icons/server.svg"),
        Icon::Sliders => include_str!("../assets/icons/sliders.svg"),
        Icon::Plus => include_str!("../assets/icons/plus.svg"),
        Icon::Close => include_str!("../assets/icons/close.svg"),
        Icon::Minimize => include_str!("../assets/icons/minimize.svg"),
        Icon::Maximize => include_str!("../assets/icons/maximize.svg"),
        Icon::Restore => include_str!("../assets/icons/restore.svg"),
        Icon::Play => include_str!("../assets/icons/play.svg"),
        Icon::Stop => include_str!("../assets/icons/stop.svg"),
        Icon::Globe => include_str!("../assets/icons/globe.svg"),
        Icon::Download => include_str!("../assets/icons/download.svg"),
        Icon::Heart => include_str!("../assets/icons/heart.svg"),
        Icon::Refresh => include_str!("../assets/icons/refresh.svg"),
        Icon::Sparkle => include_str!("../assets/icons/sparkle.svg"),
        Icon::Effects => include_str!("../assets/icons/effects.svg"),
        Icon::Modrinth => include_str!("../assets/icons/modrinth.svg"),
        Icon::CurseForge => include_str!("../assets/icons/curseforge.svg"),
        Icon::Layers => include_str!("../assets/icons/layers.svg"),
        Icon::Check => include_str!("../assets/icons/check.svg"),
        Icon::Trash => include_str!("../assets/icons/trash.svg"),
        Icon::Folder => include_str!("../assets/icons/folder.svg"),
        Icon::Search => include_str!("../assets/icons/search.svg"),
        Icon::Copy => include_str!("../assets/icons/copy.svg"),
        Icon::ArrowLeft => include_str!("../assets/icons/arrow-left.svg"),
        Icon::ArrowRight => include_str!("../assets/icons/arrow-right.svg"),
        Icon::ArrowDown => include_str!("../assets/icons/arrow-down.svg"),
        Icon::ChevronDown => include_str!("../assets/icons/chevron-down.svg"),
        Icon::Keyboard => include_str!("../assets/icons/keyboard.svg"),
    }
}

fn cache_slot(icon: Icon) -> &'static OnceLock<Vec<SvgPrim>> {
    static CUBE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static COMPASS: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static COURSE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static SERVER: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static SLIDERS: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static PLUS: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static CLOSE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static MINIMIZE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static MAXIMIZE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static RESTORE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static PLAY: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static STOP: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static GLOBE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static DOWNLOAD: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static HEART: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static REFRESH: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static SPARKLE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static EFFECTS: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static MODRINTH: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static CURSEFORGE: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static LAYERS: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static CHECK: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static TRASH: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static FOLDER: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static SEARCH: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static COPY: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static ARROW_LEFT: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static ARROW_RIGHT: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static ARROW_DOWN: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static CHEVRON_DOWN: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    static KEYBOARD: OnceLock<Vec<SvgPrim>> = OnceLock::new();
    match icon {
        Icon::Cube => &CUBE,
        Icon::Compass => &COMPASS,
        Icon::Course => &COURSE,
        Icon::Server => &SERVER,
        Icon::Sliders => &SLIDERS,
        Icon::Plus => &PLUS,
        Icon::Close => &CLOSE,
        Icon::Minimize => &MINIMIZE,
        Icon::Maximize => &MAXIMIZE,
        Icon::Restore => &RESTORE,
        Icon::Play => &PLAY,
        Icon::Stop => &STOP,
        Icon::Globe => &GLOBE,
        Icon::Download => &DOWNLOAD,
        Icon::Heart => &HEART,
        Icon::Refresh => &REFRESH,
        Icon::Sparkle => &SPARKLE,
        Icon::Effects => &EFFECTS,
        Icon::Modrinth => &MODRINTH,
        Icon::CurseForge => &CURSEFORGE,
        Icon::Layers => &LAYERS,
        Icon::Check => &CHECK,
        Icon::Trash => &TRASH,
        Icon::Folder => &FOLDER,
        Icon::Search => &SEARCH,
        Icon::Copy => &COPY,
        Icon::ArrowLeft => &ARROW_LEFT,
        Icon::ArrowRight => &ARROW_RIGHT,
        Icon::ArrowDown => &ARROW_DOWN,
        Icon::ChevronDown => &CHEVRON_DOWN,
        Icon::Keyboard => &KEYBOARD,
    }
}

fn attr<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    for quote in ['"', '\''] {
        let needle = format!("{key}={quote}");
        if let Some(i) = tag.find(&needle) {
            let rest = &tag[i + needle.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(&rest[..end]);
            }
        }
    }
    None
}

fn attr_f32(tag: &str, key: &str, default: f32) -> f32 {
    attr(tag, key).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(default)
}

fn parse_points(raw: &str) -> Vec<(f32, f32)> {
    let nums: Vec<f32> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f32>().ok())
        .collect();
    nums.chunks_exact(2).map(|p| (p[0], p[1])).collect()
}

fn parse_path_d(d: &str) -> Vec<(Vec<(f32, f32)>, bool)> {
    let mut subpaths: Vec<(Vec<(f32, f32)>, bool)> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    let mut pos = (0.0f32, 0.0f32);
    let mut start = (0.0f32, 0.0f32);

    let mut toks: Vec<String> = Vec::new();
    let mut buf = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !buf.is_empty() {
                toks.push(std::mem::take(&mut buf));
            }
            toks.push(ch.to_string());
        } else if ch == ',' || ch.is_whitespace() {
            if !buf.is_empty() {
                toks.push(std::mem::take(&mut buf));
            }
        } else {
            if ch == '-' && !buf.is_empty() && !buf.ends_with(['e', 'E']) {
                toks.push(std::mem::take(&mut buf));
            }
            buf.push(ch);
        }
    }
    if !buf.is_empty() {
        toks.push(buf);
    }

    let mut i = 0usize;
    let mut cmd = 'M';
    let next_f = |i: &mut usize| -> Option<f32> {
        let v = toks.get(*i)?.parse::<f32>().ok()?;
        *i += 1;
        Some(v)
    };

    while i < toks.len() {
        if let Some(c) = toks[i].chars().next().filter(|c| c.is_ascii_alphabetic() && toks[i].len() == 1) {
            cmd = c;
            i += 1;
            if cmd == 'Z' || cmd == 'z' {
                if !cur.is_empty() {
                    subpaths.push((std::mem::take(&mut cur), true));
                }
                pos = start;
                continue;
            }
        }
        match cmd {
            'M' | 'm' => {
                if !cur.is_empty() {
                    subpaths.push((std::mem::take(&mut cur), false));
                }
                if let (Some(x), Some(y)) = (next_f(&mut i), next_f(&mut i)) {
                    pos = if cmd == 'm' { (pos.0 + x, pos.1 + y) } else { (x, y) };
                    start = pos;
                    cur.push(pos);
                    cmd = if cmd == 'm' { 'l' } else { 'L' };
                }
            }
            'L' | 'l' => {
                if let (Some(x), Some(y)) = (next_f(&mut i), next_f(&mut i)) {
                    pos = if cmd == 'l' { (pos.0 + x, pos.1 + y) } else { (x, y) };
                    cur.push(pos);
                }
            }
            'H' | 'h' => {
                if let Some(x) = next_f(&mut i) {
                    pos.0 = if cmd == 'h' { pos.0 + x } else { x };
                    cur.push(pos);
                }
            }
            'V' | 'v' => {
                if let Some(y) = next_f(&mut i) {
                    pos.1 = if cmd == 'v' { pos.1 + y } else { y };
                    cur.push(pos);
                }
            }
            'C' | 'c' => {
                if let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    next_f(&mut i),
                    next_f(&mut i),
                    next_f(&mut i),
                    next_f(&mut i),
                    next_f(&mut i),
                    next_f(&mut i),
                ) {
                    let p0 = pos;
                    let (p1, p2, p3) = if cmd == 'c' {
                        ((p0.0 + x1, p0.1 + y1), (p0.0 + x2, p0.1 + y2), (p0.0 + x, p0.1 + y))
                    } else {
                        ((x1, y1), (x2, y2), (x, y))
                    };
                    let steps = 10;
                    for s in 1..=steps {
                        let t = s as f32 / steps as f32;
                        let u = 1.0 - t;
                        let bx = u * u * u * p0.0 + 3.0 * u * u * t * p1.0 + 3.0 * u * t * t * p2.0 + t * t * t * p3.0;
                        let by = u * u * u * p0.1 + 3.0 * u * u * t * p1.1 + 3.0 * u * t * t * p2.1 + t * t * t * p3.1;
                        cur.push((bx, by));
                    }
                    pos = p3;
                }
            }
            'Q' | 'q' => {
                if let (Some(x1), Some(y1), Some(x), Some(y)) =
                    (next_f(&mut i), next_f(&mut i), next_f(&mut i), next_f(&mut i))
                {
                    let p0 = pos;
                    let (p1, p2) = if cmd == 'q' {
                        ((p0.0 + x1, p0.1 + y1), (p0.0 + x, p0.1 + y))
                    } else {
                        ((x1, y1), (x, y))
                    };
                    let steps = 8;
                    for s in 1..=steps {
                        let t = s as f32 / steps as f32;
                        let u = 1.0 - t;
                        let bx = u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0;
                        let by = u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1;
                        cur.push((bx, by));
                    }
                    pos = p2;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    if !cur.is_empty() {
        subpaths.push((cur, false));
    }
    subpaths
}

fn parse_svg(xml: &str) -> Vec<SvgPrim> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(open) = rest.find('<') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('>') else { break };
        let tag = after[..close].trim();
        rest = &after[close + 1..];

        if tag.starts_with('/') || tag.starts_with('?') || tag.starts_with("svg") {
            continue;
        }
        let fill = attr(tag, "fill").map(|v| v != "none").unwrap_or(false);
        let stroke = attr(tag, "stroke").map(|v| v != "none").unwrap_or(!fill);

        if tag.starts_with("path") {
            if let Some(d) = attr(tag, "d") {
                for (pts, closed) in parse_path_d(d) {
                    if !pts.is_empty() {
                        out.push(SvgPrim::Polyline { pts, closed, fill, stroke });
                    }
                }
            }
        } else if tag.starts_with("line") {
            let x1 = attr_f32(tag, "x1", 0.0);
            let y1 = attr_f32(tag, "y1", 0.0);
            let x2 = attr_f32(tag, "x2", 0.0);
            let y2 = attr_f32(tag, "y2", 0.0);
            out.push(SvgPrim::Polyline {
                pts: vec![(x1, y1), (x2, y2)],
                closed: false,
                fill: false,
                stroke: true,
            });
        } else if tag.starts_with("circle") {
            let cx = attr_f32(tag, "cx", 12.0);
            let cy = attr_f32(tag, "cy", 12.0);
            let r = attr_f32(tag, "r", 1.0);
            out.push(SvgPrim::Circle { cx, cy, r, fill, stroke });
        } else if tag.starts_with("rect") {
            let x = attr_f32(tag, "x", 0.0);
            let y = attr_f32(tag, "y", 0.0);
            let w = attr_f32(tag, "width", 0.0);
            let h = attr_f32(tag, "height", 0.0);
            let rx = attr_f32(tag, "rx", 0.0);
            out.push(SvgPrim::Rect { x, y, w, h, rx, fill, stroke });
        } else if tag.starts_with("polyline") || tag.starts_with("polygon") {
            let closed = tag.starts_with("polygon");
            if let Some(raw) = attr(tag, "points") {
                let pts = parse_points(raw);
                if !pts.is_empty() {
                    out.push(SvgPrim::Polyline { pts, closed, fill, stroke });
                }
            }
        }
    }
    out
}

pub fn paint(p: &Painter, rect: Rect, icon: Icon, color: Color32, weight: f32) {
    let prims = cache_slot(icon).get_or_init(|| parse_svg(svg_source(icon)));
    let s = rect.width().min(rect.height());
    let k = s / 24.0;
    let o = rect.center() - egui::vec2(12.0 * k, 12.0 * k);
    let pt = |x: f32, y: f32| -> Pos2 { o + egui::vec2(x * k, y * k) };
    let w = (weight * k).max(1.0);
    let stroke_style = Stroke::new(w, color);
    let cap_r = w * 0.48;

    for prim in prims {
        match prim {
            SvgPrim::Polyline { pts, closed, fill, stroke } => {
                let mapped: Vec<Pos2> = pts.iter().map(|(x, y)| pt(*x, *y)).collect();
                if *fill && mapped.len() >= 3 {
                    p.add(Shape::convex_polygon(mapped.clone(), color, Stroke::NONE));
                }
                if *stroke && mapped.len() >= 2 {
                    for pair in mapped.windows(2) {
                        p.line_segment([pair[0], pair[1]], stroke_style);
                    }
                    if *closed {
                        if let (Some(&first), Some(&last)) = (mapped.first(), mapped.last()) {
                            p.line_segment([last, first], stroke_style);
                        }
                    }
                    if cap_r >= 0.7 {
                        for &node in &mapped {
                            p.circle_filled(node, cap_r, color);
                        }
                    }
                }
            }
            SvgPrim::Circle { cx, cy, r, fill, stroke } => {
                let c = pt(*cx, *cy);
                let rr = r * k;
                if *fill {
                    p.circle_filled(c, rr, color);
                }
                if *stroke {
                    p.circle_stroke(c, rr, stroke_style);
                }
            }
            SvgPrim::Rect { x, y, w: rw, h: rh, rx, fill, stroke } => {
                let rr = Rect::from_min_max(pt(*x, *y), pt(*x + *rw, *y + *rh));
                let cr = CornerRadius::same((rx * k).round().max(1.0) as u8);
                if *fill {
                    p.rect_filled(rr, cr, color);
                }
                if *stroke {
                    p.rect_stroke(rr, cr, stroke_style, egui::StrokeKind::Inside);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_svg_icon_parses_with_primitives() {
        let all = [
            Icon::Cube,
            Icon::Compass,
            Icon::Course,
            Icon::Server,
            Icon::Sliders,
            Icon::Plus,
            Icon::Close,
            Icon::Minimize,
            Icon::Maximize,
            Icon::Restore,
            Icon::Play,
            Icon::Stop,
            Icon::Globe,
            Icon::Download,
            Icon::Heart,
            Icon::Refresh,
            Icon::Sparkle,
            Icon::Effects,
            Icon::Modrinth,
            Icon::CurseForge,
            Icon::Layers,
            Icon::Check,
            Icon::Trash,
            Icon::Folder,
            Icon::Search,
            Icon::Copy,
            Icon::ArrowLeft,
            Icon::ArrowRight,
            Icon::ArrowDown,
            Icon::ChevronDown,
            Icon::Keyboard,
        ];
        for icon in all {
            let parsed = parse_svg(svg_source(icon));
            assert!(!parsed.is_empty(), "empty SVG parsed for {icon:?}");
        }
    }
}
