//! Self-drawn vector icons (no third-party bitmaps). Every icon is drawn
//! into a square rect and adapts to the palette (light/dark).

use crate::theme::ThemePalette;
use egui::{pos2, vec2, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use fingerprint::DeviceClass;
use scan_core::ServiceType;

fn sub(r: Rect, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    // Coordinates in a 16x16 design grid.
    let s = r.width() / 16.0;
    Rect::from_min_max(pos2(r.min.x + x0 * s, r.min.y + y0 * s), pos2(r.min.x + x1 * s, r.min.y + y1 * s))
}

fn p(r: Rect, x: f32, y: f32) -> Pos2 {
    let s = r.width() / 16.0;
    pos2(r.min.x + x * s, r.min.y + y * s)
}

fn sc(r: Rect) -> f32 {
    r.width() / 16.0
}

fn fill(painter: &Painter, r: Rect, radius: f32, c: Color32) {
    painter.rect_filled(r, CornerRadius::same(radius.round() as u8), c);
}

fn outline(painter: &Painter, r: Rect, radius: f32, c: Color32, w: f32) {
    painter.rect_stroke(r, CornerRadius::same(radius.round() as u8), Stroke::new(w, c), StrokeKind::Inside);
}

fn line(painter: &Painter, a: Pos2, b: Pos2, c: Color32, w: f32) {
    painter.line_segment([a, b], Stroke::new(w, c));
}

fn poly(painter: &Painter, pts: Vec<Pos2>, c: Color32) {
    painter.add(Shape::convex_polygon(pts, c, Stroke::NONE));
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn frame_color(pal: &ThemePalette, dim: bool) -> Color32 {
    if dim {
        pal.text_disabled
    } else if pal.dark {
        Color32::from_rgb(0xC8, 0xCC, 0xD2)
    } else {
        Color32::from_rgb(0x55, 0x5B, 0x63)
    }
}

fn screen(pal: &ThemePalette, dim: bool) -> Color32 {
    if dim {
        pal.icon_screen_dim
    } else {
        pal.icon_screen
    }
}

/// Monitor with blue screen (the classic "computer" status icon).
pub fn computer(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 1.0, 2.0, 15.0, 12.0), 1.0 * s, f);
    let scr = sub(r, 2.2, 3.2, 13.8, 10.8);
    fill(painter, scr, 0.0, screen(pal, dim));
    // Reflection highlight
    if !dim {
        poly(painter, vec![p(r, 2.2, 3.2), p(r, 8.5, 3.2), p(r, 5.0, 10.8), p(r, 2.2, 10.8)], Color32::from_white_alpha(38));
    }
    fill(painter, sub(r, 6.5, 12.0, 9.5, 13.6), 0.0, f);
    fill(painter, sub(r, 4.0, 13.6, 12.0, 14.8), 0.6 * s, f);
}

pub fn laptop(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    fill(painter, sub(r, 2.5, 3.0, 13.5, 11.0), sc(r), f);
    fill(painter, sub(r, 3.5, 4.0, 12.5, 10.2), 0.0, screen(pal, dim));
    poly(painter, vec![p(r, 2.5, 11.2), p(r, 13.5, 11.2), p(r, 15.5, 13.8), p(r, 0.5, 13.8)], f);
}

pub fn server(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 4.0, 1.0, 12.0, 15.0), s, f);
    for y in [3.0, 6.0, 9.0] {
        fill(painter, sub(r, 5.2, y, 10.8, y + 1.6), 0.0, screen(pal, dim));
    }
    painter.circle_filled(p(r, 8.0, 12.6), 0.9 * s, if dim { pal.text_disabled } else { pal.accent });
}

pub fn router(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    line(painter, p(r, 4.5, 8.5), p(r, 3.0, 2.0), f, 1.3 * s);
    line(painter, p(r, 11.5, 8.5), p(r, 13.0, 2.0), f, 1.3 * s);
    fill(painter, sub(r, 1.0, 8.5, 15.0, 13.5), s, f);
    for x in [3.5, 6.0, 8.5] {
        painter.circle_filled(p(r, x, 11.0), 0.8 * s, if dim { pal.text_disabled } else { pal.accent });
    }
}

pub fn switch(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 0.5, 5.0, 15.5, 11.0), s, f);
    let port = screen(pal, dim);
    for i in 0..6 {
        let x = 1.6 + f32::from(i as u8) * 2.3;
        fill(painter, sub(r, x, 6.4, x + 1.5, 7.9), 0.0, port);
        fill(painter, sub(r, x, 8.4, x + 1.5, 9.9), 0.0, port);
    }
}

pub fn access_point(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 2.0, 10.0, 14.0, 14.0), 2.0 * s, f);
    painter.circle_filled(p(r, 8.0, 12.0), 0.9 * s, if dim { pal.text_disabled } else { pal.accent });
    for (rad, a) in [(3.0, 1.3), (5.5, 1.1)] {
        let c = screen(pal, dim);
        let pts: Vec<Pos2> = (0..=12)
            .map(|i| {
                let t = std::f32::consts::PI * (1.25 + 0.5 * i as f32 / 12.0);
                p(r, 8.0 + rad * t.cos(), 8.5 + rad * t.sin())
            })
            .collect();
        painter.add(Shape::line(pts, Stroke::new(a * s, c)));
    }
}

pub fn firewall(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let brick = if dim { pal.icon_screen_dim } else { Color32::from_rgb(0xC8, 0x5A, 0x3C) };
    let mortar = if pal.dark { pal.table_bg } else { Color32::WHITE };
    fill(painter, sub(r, 1.0, 2.0, 15.0, 14.0), 0.0, brick);
    let s = sc(r);
    for y in [5.0, 8.0, 11.0] {
        line(painter, p(r, 1.0, y), p(r, 15.0, y), mortar, 0.8 * s);
    }
    for (row, y0) in [2.0f32, 5.0, 8.0, 11.0].iter().enumerate() {
        let off = if row % 2 == 0 { 4.5 } else { 8.0 };
        let mut x = off;
        while x < 15.0 {
            line(painter, p(r, x, *y0), p(r, x, y0 + 3.0), mortar, 0.8 * s);
            x += 7.0;
        }
    }
}

pub fn printer(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let paper = if pal.dark { Color32::from_rgb(0xE6, 0xE6, 0xE6) } else { Color32::WHITE };
    let s = sc(r);
    fill(painter, sub(r, 4.0, 1.5, 12.0, 6.0), 0.0, paper);
    outline(painter, sub(r, 4.0, 1.5, 12.0, 6.0), 0.0, f, 0.8 * s);
    fill(painter, sub(r, 1.0, 5.5, 15.0, 11.5), 1.5 * s, f);
    painter.circle_filled(p(r, 12.6, 7.4), 0.7 * s, if dim { pal.text_disabled } else { pal.accent });
    fill(painter, sub(r, 4.0, 9.5, 12.0, 14.5), 0.0, paper);
    outline(painter, sub(r, 4.0, 9.5, 12.0, 14.5), 0.0, f, 0.8 * s);
    line(painter, p(r, 5.5, 11.5), p(r, 10.5, 11.5), f, 0.6 * s);
    line(painter, p(r, 5.5, 13.0), p(r, 9.0, 13.0), f, 0.6 * s);
}

pub fn nas(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 2.0, 1.5, 14.0, 14.5), 1.5 * s, f);
    for x in [3.2, 6.0, 8.8, 11.6] {
        fill(painter, sub(r, x, 3.0, x + 1.4, 10.5), 0.0, screen(pal, dim));
    }
    painter.circle_filled(p(r, 4.0, 12.6), 0.7 * s, if dim { pal.text_disabled } else { pal.accent });
}

pub fn camera(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 1.0, 4.0, 11.5, 10.0), 1.5 * s, f);
    poly(painter, vec![p(r, 11.5, 5.5), p(r, 15.0, 3.8), p(r, 15.0, 10.2), p(r, 11.5, 8.5)], f);
    painter.circle_filled(p(r, 4.5, 7.0), 1.6 * s, screen(pal, dim));
    fill(painter, sub(r, 5.0, 10.0, 7.0, 13.5), 0.0, f);
    fill(painter, sub(r, 3.0, 13.5, 9.0, 14.8), 0.5 * s, f);
}

pub fn television(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 0.5, 2.5, 15.5, 12.0), s, f);
    fill(painter, sub(r, 1.5, 3.5, 14.5, 11.0), 0.0, screen(pal, dim));
    line(painter, p(r, 5.0, 14.5), p(r, 11.0, 14.5), f, 1.2 * s);
    line(painter, p(r, 8.0, 12.0), p(r, 8.0, 14.5), f, 1.2 * s);
}

pub fn media_player(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 1.5, 3.0, 14.5, 13.0), 2.0 * s, f);
    poly(painter, vec![p(r, 6.0, 5.5), p(r, 11.0, 8.0), p(r, 6.0, 10.5)], screen(pal, dim));
}

pub fn smartphone(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 4.5, 0.8, 11.5, 15.2), 1.5 * s, f);
    fill(painter, sub(r, 5.4, 2.4, 10.6, 12.8), 0.0, screen(pal, dim));
    painter.circle_filled(p(r, 8.0, 14.0), 0.6 * s, screen(pal, dim));
}

pub fn tablet(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 2.5, 1.0, 13.5, 15.0), 1.5 * s, f);
    fill(painter, sub(r, 3.5, 2.5, 12.5, 12.8), 0.0, screen(pal, dim));
    painter.circle_filled(p(r, 8.0, 13.9), 0.6 * s, screen(pal, dim));
}

pub fn iot(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    for i in 0..4 {
        let o = 4.2 + f32::from(i as u8) * 2.5;
        line(painter, p(r, o, 1.0), p(r, o, 15.0), f, 0.9 * s);
        line(painter, p(r, 1.0, o), p(r, 15.0, o), f, 0.9 * s);
    }
    fill(painter, sub(r, 3.0, 3.0, 13.0, 13.0), s, f);
    fill(painter, sub(r, 5.0, 5.0, 11.0, 11.0), 0.0, screen(pal, dim));
}

pub fn home(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    poly(painter, vec![p(r, 8.0, 1.0), p(r, 15.5, 8.0), p(r, 0.5, 8.0)], f);
    fill(painter, sub(r, 2.5, 7.5, 13.5, 15.0), 0.0, f);
    fill(painter, sub(r, 6.5, 9.5, 9.5, 15.0), 0.0, screen(pal, dim));
}

pub fn game_console(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 0.5, 4.5, 15.5, 12.5), 4.0 * s, f);
    let c = screen(pal, dim);
    line(painter, p(r, 3.0, 8.5), p(r, 6.0, 8.5), c, 1.2 * s);
    line(painter, p(r, 4.5, 7.0), p(r, 4.5, 10.0), c, 1.2 * s);
    painter.circle_filled(p(r, 11.0, 7.5), 0.9 * s, c);
    painter.circle_filled(p(r, 12.8, 9.4), 0.9 * s, c);
}

pub fn ups(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    let f = frame_color(pal, dim);
    let s = sc(r);
    fill(painter, sub(r, 3.0, 2.5, 13.0, 15.0), s, f);
    fill(painter, sub(r, 6.0, 1.0, 10.0, 2.5), 0.0, f);
    let bolt = if dim { pal.text_disabled } else { Color32::from_rgb(0xF2, 0xC2, 0x1B) };
    poly(painter, vec![p(r, 9.0, 4.0), p(r, 5.5, 9.5), p(r, 8.0, 9.5)], bolt);
    poly(painter, vec![p(r, 8.0, 8.5), p(r, 10.5, 8.5), p(r, 7.0, 14.0)], bolt);
}

pub fn virtual_machine(painter: &Painter, r: Rect, pal: &ThemePalette, dim: bool) {
    computer(painter, r, pal, dim);
    let s = sc(r);
    let c = if pal.dark { pal.table_bg } else { Color32::WHITE };
    line(painter, p(r, 5.0, 4.8), p(r, 8.0, 9.5), c, 1.4 * s);
    line(painter, p(r, 8.0, 9.5), p(r, 11.0, 4.8), c, 1.4 * s);
}

/// Device icon for a device class (Alive) or greyed (NoResponse).
pub fn device(painter: &Painter, r: Rect, class: DeviceClass, pal: &ThemePalette, dim: bool) {
    use DeviceClass::*;
    match class {
        Desktop | Unknown => computer(painter, r, pal, dim),
        Laptop => laptop(painter, r, pal, dim),
        Server => server(painter, r, pal, dim),
        VirtualMachine => virtual_machine(painter, r, pal, dim),
        Router => router(painter, r, pal, dim),
        Firewall => firewall(painter, r, pal, dim),
        Switch => switch(painter, r, pal, dim),
        AccessPoint => access_point(painter, r, pal, dim),
        Printer => printer(painter, r, pal, dim),
        Nas => nas(painter, r, pal, dim),
        Camera => camera(painter, r, pal, dim),
        Television => television(painter, r, pal, dim),
        MediaPlayer => media_player(painter, r, pal, dim),
        Smartphone => smartphone(painter, r, pal, dim),
        Tablet => tablet(painter, r, pal, dim),
        IoT => iot(painter, r, pal, dim),
        HomeAutomation => home(painter, r, pal, dim),
        GameConsole => game_console(painter, r, pal, dim),
        UPS => ups(painter, r, pal, dim),
    }
}

/// Question mark (status "unknown").
pub fn question(painter: &Painter, r: Rect, pal: &ThemePalette) {
    let s = sc(r);
    painter.circle_stroke(r.center(), 6.5 * s, Stroke::new(1.2 * s, pal.text_secondary));
    painter.text(r.center() + vec2(0.0, 0.3 * s), egui::Align2::CENTER_CENTER, "?", crate::theme::font(10.5 * s), pal.text_secondary);
}

/// Rotating spinner (status "scanning"). Needs continuous repaint.
pub fn spinner(painter: &Painter, r: Rect, pal: &ThemePalette, time: f64) {
    let s = sc(r);
    let c = r.center();
    let rad = 5.5 * s;
    let start = (time * 6.0) as f32;
    let n = 18;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = start + (i as f32 / n as f32) * std::f32::consts::PI * 1.5;
            pos2(c.x + rad * a.cos(), c.y + rad * a.sin())
        })
        .collect();
    painter.add(Shape::line(pts, Stroke::new(1.8 * s, pal.icon_screen)));
}

pub fn globe(painter: &Painter, r: Rect, pal: &ThemePalette) {
    let s = sc(r);
    let c = p(r, 8.0, 8.0);
    let blue = pal.icon_screen;
    painter.circle_filled(c, 6.5 * s, mix(blue, Color32::WHITE, if pal.dark { 0.0 } else { 0.15 }));
    let w = if pal.dark { pal.table_bg } else { Color32::WHITE };
    let st = Stroke::new(0.8 * s, w);
    painter.circle_stroke(c, 6.5 * s, Stroke::new(0.8 * s, mix(blue, Color32::BLACK, 0.25)));
    painter.line_segment([p(r, 1.5, 8.0), p(r, 14.5, 8.0)], st);
    painter.line_segment([p(r, 8.0, 1.5), p(r, 8.0, 14.5)], st);
    let ell: Vec<Pos2> = (0..=24)
        .map(|i| {
            let a = i as f32 / 24.0 * std::f32::consts::TAU;
            pos2(c.x + 3.0 * s * a.cos(), c.y + 6.5 * s * a.sin())
        })
        .collect();
    painter.add(Shape::line(ell, st));
    painter.line_segment([p(r, 2.5, 4.8), p(r, 13.5, 4.8)], st);
    painter.line_segment([p(r, 2.5, 11.2), p(r, 13.5, 11.2)], st);
}

pub fn lock(painter: &Painter, r: Rect, color: Color32) {
    let s = sc(r);
    painter.circle_stroke(p(r, 12.0, 9.5), 1.9 * s, Stroke::new(1.1 * s, color));
    fill(painter, sub(r, 9.2, 9.8, 14.8, 14.8), 0.8 * s, color);
}

pub fn folder(painter: &Painter, r: Rect, _pal: &ThemePalette) {
    let s = sc(r);
    let back = Color32::from_rgb(0xE0, 0xA8, 0x2E);
    let front = Color32::from_rgb(0xF6, 0xC8, 0x5A);
    poly(painter, vec![p(r, 1.0, 3.0), p(r, 6.0, 3.0), p(r, 7.5, 4.6), p(r, 1.0, 4.6)], back);
    fill(painter, sub(r, 1.0, 4.2, 15.0, 13.5), 0.8 * s, back);
    fill(painter, sub(r, 1.0, 6.0, 15.0, 13.5), 0.8 * s, front);
}

pub fn terminal(painter: &Painter, r: Rect, pal: &ThemePalette) {
    let s = sc(r);
    let bg = Color32::from_rgb(0x2B, 0x2F, 0x36);
    fill(painter, sub(r, 1.0, 2.5, 15.0, 13.5), 1.2 * s, bg);
    outline(painter, sub(r, 1.0, 2.5, 15.0, 13.5), 1.2 * s, mix(bg, pal.icon_fg, 0.5), 0.6 * s);
    let g = Color32::from_rgb(0x8B, 0xE2, 0x78);
    line(painter, p(r, 3.5, 5.5), p(r, 6.0, 8.0), g, 1.2 * s);
    line(painter, p(r, 6.0, 8.0), p(r, 3.5, 10.5), g, 1.2 * s);
    line(painter, p(r, 7.5, 10.8), p(r, 11.5, 10.8), g, 1.2 * s);
}

pub fn remote_desktop(painter: &Painter, r: Rect, pal: &ThemePalette) {
    computer(painter, r, pal, false);
    let s = sc(r);
    let w = Color32::WHITE;
    line(painter, p(r, 5.0, 9.0), p(r, 10.5, 4.8), w, 1.3 * s);
    poly(painter, vec![p(r, 11.5, 4.0), p(r, 11.0, 7.3), p(r, 8.3, 4.6)], w);
}

pub fn database(painter: &Painter, r: Rect, pal: &ThemePalette) {
    let s = sc(r);
    let c = mix(pal.icon_screen, Color32::BLACK, 0.1);
    fill(painter, sub(r, 3.0, 3.0, 13.0, 13.0), 0.0, c);
    for y in [3.0, 13.0] {
        let pts: Vec<Pos2> = (0..=16)
            .map(|i| {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                p(r, 8.0 + 5.0 * a.cos(), y + 1.6 * a.sin())
            })
            .collect();
        painter.add(Shape::convex_polygon(pts, if y < 5.0 { mix(c, Color32::WHITE, 0.3) } else { c }, Stroke::NONE));
    }
    let w = Color32::from_white_alpha(120);
    line(painter, p(r, 3.0, 7.0), p(r, 13.0, 7.0), w, 0.6 * s);
    line(painter, p(r, 3.0, 10.0), p(r, 13.0, 10.0), w, 0.6 * s);
}

pub fn generic_service(painter: &Painter, r: Rect, pal: &ThemePalette) {
    let s = sc(r);
    let c = pal.icon_fg;
    for i in 0..8 {
        let a = i as f32 / 8.0 * std::f32::consts::TAU;
        let (x, y) = (8.0 + 5.8 * a.cos(), 8.0 + 5.8 * a.sin());
        painter.circle_filled(p(r, x, y), 1.4 * s, c);
    }
    painter.circle_filled(p(r, 8.0, 8.0), 4.8 * s, c);
    painter.circle_filled(p(r, 8.0, 8.0), 2.0 * s, if pal.dark { pal.table_bg } else { Color32::WHITE });
}

pub fn bonjour(painter: &Painter, r: Rect, pal: &ThemePalette) {
    let s = sc(r);
    painter.circle_stroke(p(r, 8.0, 8.0), 6.0 * s, Stroke::new(1.2 * s, pal.icon_screen));
    painter.circle_stroke(p(r, 8.0, 8.0), 3.4 * s, Stroke::new(1.2 * s, pal.icon_screen));
    painter.circle_filled(p(r, 8.0, 8.0), 1.2 * s, pal.icon_screen);
}

/// Icon for a service child row.
pub fn service(painter: &Painter, r: Rect, t: &ServiceType, pal: &ThemePalette) {
    use ServiceType as S;
    match t {
        S::Http | S::HomeAssistant | S::SynologyDsm | S::Docker | S::Elasticsearch => globe(painter, r, pal),
        S::Https => {
            globe(painter, r, pal);
            lock(painter, r, Color32::from_rgb(0xE8, 0xB3, 0x1E));
        }
        S::Ftp => {
            folder(painter, r, pal);
            let s = sc(r);
            line(painter, p(r, 8.0, 12.0), p(r, 8.0, 7.5), Color32::from_rgb(0x2F, 0x6D, 0xB5), 1.4 * s);
            poly(painter, vec![p(r, 5.6, 8.6), p(r, 8.0, 6.0), p(r, 10.4, 8.6)], Color32::from_rgb(0x2F, 0x6D, 0xB5));
        }
        S::Ssh | S::Telnet => terminal(painter, r, pal),
        S::Rdp | S::Vnc | S::Radmin => remote_desktop(painter, r, pal),
        S::Smb | S::Shares | S::Afp => folder(painter, r, pal),
        S::Ipp | S::JetDirect | S::Lpd => printer(painter, r, pal, false),
        S::PostgreSql | S::MySql | S::MsSql | S::Oracle | S::MongoDb | S::Redis => database(painter, r, pal),
        S::MdnsService => bonjour(painter, r, pal),
        S::Rtsp => camera(painter, r, pal, false),
        _ => generic_service(painter, r, pal),
    }
}

/// Tree expander (Windows style triangles).
pub fn chevron(painter: &Painter, r: Rect, expanded: bool, color: Color32) {
    let c = r.center();
    let h = r.height() * 0.22;
    if expanded {
        poly(painter, vec![pos2(c.x + h, c.y - h), pos2(c.x + h, c.y + h), pos2(c.x - h, c.y + h)], color);
    } else {
        let pts = vec![pos2(c.x - h * 0.6, c.y - h * 1.2), pos2(c.x + h * 0.8, c.y), pos2(c.x - h * 0.6, c.y + h * 1.2)];
        painter.add(Shape::closed_line(pts, Stroke::new(1.0_f32, color)));
    }
}

pub fn play(painter: &Painter, r: Rect, color: Color32) {
    poly(painter, vec![p(r, 4.0, 2.0), p(r, 14.0, 8.0), p(r, 4.0, 14.0)], color);
}

pub fn stop(painter: &Painter, r: Rect, color: Color32) {
    fill(painter, sub(r, 3.5, 3.5, 12.5, 12.5), sc(r), color);
}

pub fn pause(painter: &Painter, r: Rect, color: Color32) {
    fill(painter, sub(r, 4.0, 3.0, 7.0, 13.0), 0.0, color);
    fill(painter, sub(r, 9.0, 3.0, 12.0, 13.0), 0.0, color);
}

/// "IP" button: monitor with the letters IP.
pub fn ip_button(painter: &Painter, r: Rect, pal: &ThemePalette) {
    computer(painter, r, pal, false);
    painter.text(p(r, 8.0, 7.1), egui::Align2::CENTER_CENTER, "IP", crate::theme::font(6.3 * sc(r)), Color32::WHITE);
}

/// "C" button: class C subnet.
pub fn c_button(painter: &Painter, r: Rect, pal: &ThemePalette) {
    let s = sc(r);
    fill(painter, sub(r, 1.5, 1.5, 14.5, 14.5), 2.0 * s, pal.icon_screen);
    painter.text(p(r, 8.0, 8.2), egui::Align2::CENTER_CENTER, "C", crate::theme::font(10.5 * s), Color32::WHITE);
}

pub fn expand_all(painter: &Painter, r: Rect, pal: &ThemePalette, expand: bool) {
    let s = sc(r);
    let f = pal.icon_fg;
    outline(painter, sub(r, 1.0, 1.0, 8.0, 8.0), 0.0, f, 1.0 * s);
    line(painter, p(r, 2.8, 4.5), p(r, 6.2, 4.5), f, 1.0 * s);
    if expand {
        line(painter, p(r, 4.5, 2.8), p(r, 4.5, 6.2), f, 1.0 * s);
    }
    line(painter, p(r, 4.5, 8.0), p(r, 4.5, 13.5), f, 0.8 * s);
    for y in [10.5, 13.5] {
        line(painter, p(r, 4.5, y), p(r, 7.5, y), f, 0.8 * s);
        fill(painter, sub(r, 8.5, y - 1.2, 15.0, y + 1.2), 0.0, if expand { pal.icon_screen } else { pal.text_disabled });
    }
}

pub fn star(painter: &Painter, r: Rect, color: Color32, filled: bool) {
    let c = r.center();
    let ro = r.width() * 0.45;
    let ri = ro * 0.45;
    let pts: Vec<Pos2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let rad = if i % 2 == 0 { ro } else { ri };
            pos2(c.x + rad * a.cos(), c.y + rad * a.sin())
        })
        .collect();
    if filled {
        // Not convex: fan from centre.
        for i in 0..10 {
            poly(painter, vec![c, pts[i], pts[(i + 1) % 10]], color);
        }
    } else {
        painter.add(Shape::closed_line(pts, Stroke::new(1.0_f32, color)));
    }
}

pub fn magnifier(painter: &Painter, r: Rect, color: Color32) {
    let s = sc(r);
    painter.circle_stroke(p(r, 6.5, 6.5), 4.2 * s, Stroke::new(1.4 * s, color));
    line(painter, p(r, 9.6, 9.6), p(r, 14.0, 14.0), color, 1.8 * s);
}

pub fn cross(painter: &Painter, r: Rect, color: Color32) {
    let s = sc(r);
    line(painter, p(r, 4.0, 4.0), p(r, 12.0, 12.0), color, 1.4 * s);
    line(painter, p(r, 12.0, 4.0), p(r, 4.0, 12.0), color, 1.4 * s);
}

pub fn dropdown_arrow(painter: &Painter, r: Rect, color: Color32) {
    let c = r.center();
    let w = r.width() * 0.25;
    poly(painter, vec![pos2(c.x - w, c.y - w * 0.5), pos2(c.x + w, c.y - w * 0.5), pos2(c.x, c.y + w * 0.6)], color);
}

pub fn warning(painter: &Painter, r: Rect, pal: &ThemePalette) {
    poly(painter, vec![p(r, 8.0, 1.5), p(r, 15.0, 14.5), p(r, 1.0, 14.5)], pal.warning);
    painter.text(p(r, 8.0, 9.8), egui::Align2::CENTER_CENTER, "!", crate::theme::font(9.0 * sc(r)), Color32::BLACK);
}
