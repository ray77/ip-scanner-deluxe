//! Small custom widgets: gradient buttons, range/search fields, tabs, status bar.

use crate::i18n::t;
use crate::icons;
use crate::theme::{font, ThemePalette};
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui};

/// Rounded rectangle with a vertical gradient.
pub fn gradient_rect(painter: &egui::Painter, rect: Rect, radius: f32, top: Color32, bottom: Color32) {
    let mut pts: Vec<Pos2> = Vec::new();
    let r = radius.min(rect.height() / 2.0).min(rect.width() / 2.0);
    let corners = [
        (pos2(rect.max.x - r, rect.min.y + r), -90.0f32),
        (pos2(rect.max.x - r, rect.max.y - r), 0.0),
        (pos2(rect.min.x + r, rect.max.y - r), 90.0),
        (pos2(rect.min.x + r, rect.min.y + r), 180.0),
    ];
    for (c, start) in corners {
        for i in 0..=6 {
            let a = (start + 90.0 * i as f32 / 6.0).to_radians();
            pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    }
    let lerp = |y: f32| {
        let t = ((y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
        let l = |a: u8, b: u8| (f32::from(a) * (1.0 - t) + f32::from(b) * t).round() as u8;
        Color32::from_rgba_premultiplied(l(top.r(), bottom.r()), l(top.g(), bottom.g()), l(top.b(), bottom.b()), l(top.a(), bottom.a()))
    };
    let mut mesh = Mesh::default();
    let center = rect.center();
    mesh.vertices.push(Vertex { pos: center, uv: WHITE_UV, color: lerp(center.y) });
    for p in &pts {
        mesh.vertices.push(Vertex { pos: *p, uv: WHITE_UV, color: lerp(p.y) });
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % n]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Large green "Scannen" button (red "Stoppen" while scanning).
pub fn scan_button(ui: &mut Ui, pal: &ThemePalette, scanning: bool) -> Response {
    let size = vec2(100.0, 34.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    let (mut top, mut bottom, border) =
        if scanning { (pal.stop_top, pal.stop_bottom, pal.stop_border) } else { (pal.accent_top, pal.accent_bottom, pal.accent_border) };
    if resp.is_pointer_button_down_on() {
        std::mem::swap(&mut top, &mut bottom);
    } else if resp.hovered() {
        top = top.gamma_multiply(1.08);
        bottom = bottom.gamma_multiply(1.08);
    }
    gradient_rect(painter, rect, 3.0, top, bottom);
    painter.rect_stroke(rect, CornerRadius::same(3), Stroke::new(1.0, border), StrokeKind::Inside);
    // Inner highlight line
    painter.line_segment(
        [pos2(rect.min.x + 3.0, rect.min.y + 1.5), pos2(rect.max.x - 3.0, rect.min.y + 1.5)],
        Stroke::new(1.0, Color32::from_white_alpha(50)),
    );
    let icon = Rect::from_min_size(pos2(rect.min.x + 10.0, rect.center().y - 9.0), vec2(18.0, 18.0));
    if scanning {
        icons::stop(painter, icon, pal.accent_text);
    } else {
        icons::play(painter, icon, pal.accent_text);
    }
    painter.text(
        pos2(rect.min.x + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        if scanning { t("tb.stop") } else { t("tb.scan") },
        font(13.5),
        pal.accent_text,
    );
    resp.on_hover_text(if scanning { "F5 / Esc" } else { "F5" })
}

/// Flat toolbar icon button (32x32), highlighted on hover like Windows toolbars.
pub fn tool_button(ui: &mut Ui, pal: &ThemePalette, enabled: bool, checked: bool, draw: impl FnOnce(&egui::Painter, Rect)) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(32.0, 32.0), if enabled { Sense::click() } else { Sense::hover() });
    let painter = ui.painter();
    if enabled && (resp.hovered() || checked) {
        let (bg, br) = if pal.dark {
            (pal.hover_bg, pal.selected_border)
        } else {
            (Color32::from_rgb(0xE5, 0xF3, 0xFF), Color32::from_rgb(0xCC, 0xE8, 0xFF))
        };
        let bg = if resp.is_pointer_button_down_on() || checked { pal.selected_bg } else { bg };
        painter.rect_filled(rect, CornerRadius::same(2), bg);
        painter.rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, br), StrokeKind::Inside);
    }
    let icon = Rect::from_center_size(rect.center(), vec2(20.0, 20.0));
    if enabled {
        draw(painter, icon);
    } else {
        let p = painter.clone().with_layer_id(painter.layer_id());
        draw(&p, icon);
        painter.rect_filled(rect, CornerRadius::ZERO, pal.toolbar_bg.gamma_multiply(0.6));
    }
    resp
}

pub fn toolbar_separator(ui: &mut Ui, pal: &ThemePalette) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 30.0), Sense::hover());
    let x = rect.center().x;
    ui.painter().line_segment([pos2(x, rect.min.y + 2.0), pos2(x, rect.max.y - 2.0)], Stroke::new(1.0, pal.toolbar_border));
}

fn field_frame(ui: &Ui, rect: Rect, pal: &ThemePalette, focused: bool, error: bool) {
    let border = if error {
        pal.error
    } else if focused {
        if pal.dark {
            pal.selected_border
        } else {
            Color32::from_rgb(0x00, 0x78, 0xD7)
        }
    } else {
        pal.input_border
    };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, pal.input_bg);
    ui.painter().rect_stroke(rect, CornerRadius::ZERO, Stroke::new(1.0, border), StrokeKind::Inside);
}

/// IP range input with history drop-down. Returns (text response, chosen history entry).
pub fn range_field(
    ui: &mut Ui,
    pal: &ThemePalette,
    text: &mut String,
    history: &[String],
    error: Option<&str>,
    width: f32,
) -> (Response, Option<String>) {
    let height = 22.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let id = ui.id().with("range_field");
    let edit_rect = Rect::from_min_max(rect.min + vec2(1.0, 1.0), pos2(rect.max.x - 18.0, rect.max.y - 1.0));
    let edit_id = id.with("edit");
    let focused = ui.memory(|m| m.has_focus(edit_id));
    field_frame(ui, rect, pal, focused, error.is_some());
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
    let resp = child.add(
        egui::TextEdit::singleline(text)
            .id(edit_id)
            .frame(false)
            .margin(egui::Margin::symmetric(4, 3))
            .hint_text(t("range.hint"))
            .desired_width(edit_rect.width())
            .font(font(12.5))
            .text_color(if error.is_some() { pal.error } else { pal.text_primary }),
    );
    let resp = match error {
        Some(e) => resp.on_hover_text(format!("{}: {e}", t("err.range"))),
        None => resp,
    };
    let arrow_rect = Rect::from_min_max(pos2(rect.max.x - 18.0, rect.min.y + 1.0), rect.max - vec2(1.0, 1.0));
    let arrow = ui.interact(arrow_rect, id.with("arrow"), Sense::click());
    if arrow.hovered() {
        ui.painter().rect_filled(arrow_rect, CornerRadius::ZERO, pal.hover_bg);
    }
    icons::dropdown_arrow(ui.painter(), Rect::from_center_size(arrow_rect.center(), vec2(12.0, 12.0)), pal.text_secondary);
    let mut chosen = None;
    egui::Popup::from_toggle_button_response(&arrow).close_behavior(egui::PopupCloseBehavior::CloseOnClick).width(width - 2.0).show(|ui| {
        if history.is_empty() {
            ui.weak("—");
        }
        for h in history {
            if ui.selectable_label(false, h).clicked() {
                chosen = Some(h.clone());
            }
        }
    });
    (resp, chosen)
}

/// Search box with magnifier and clear button.
pub fn search_field(ui: &mut Ui, pal: &ThemePalette, text: &mut String, width: f32, focus: bool) -> Response {
    let height = 22.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let id = ui.id().with("search_field");
    let edit_rect = Rect::from_min_max(rect.min + vec2(1.0, 1.0), pos2(rect.max.x - 20.0, rect.max.y - 1.0));
    let has_focus = ui.memory(|m| m.has_focus(id));
    field_frame(ui, rect, pal, has_focus, false);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect));
    let resp = child.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .frame(false)
            .margin(egui::Margin::symmetric(4, 3))
            .hint_text(t("search.hint"))
            .desired_width(edit_rect.width())
            .font(font(12.5)),
    );
    if focus {
        resp.request_focus();
    }
    let btn = Rect::from_min_max(pos2(rect.max.x - 20.0, rect.min.y + 1.0), rect.max - vec2(1.0, 1.0));
    let icon = Rect::from_center_size(btn.center(), vec2(13.0, 13.0));
    if text.is_empty() {
        icons::magnifier(ui.painter(), icon, pal.text_secondary);
    } else {
        let b = ui.interact(btn, id.with("clear"), Sense::click());
        if b.hovered() {
            ui.painter().rect_filled(btn, CornerRadius::ZERO, pal.hover_bg);
        }
        icons::cross(ui.painter(), icon, pal.text_secondary);
        if b.clicked() {
            text.clear();
        }
    }
    resp
}

/// Windows style tab strip. Returns the clicked tab index.
pub fn tabs(ui: &mut Ui, pal: &ThemePalette, labels: &[&str], active: usize) -> Option<usize> {
    let height = 24.0;
    let full = ui.available_width();
    let (strip, _) = ui.allocate_exact_size(vec2(full, height), Sense::hover());
    let painter = ui.painter();
    let base_y = strip.max.y - 0.5;
    let mut x = strip.min.x + 2.0;
    let mut clicked = None;
    let mut active_rect = None;
    for (i, label) in labels.iter().enumerate() {
        let w = painter.layout_no_wrap(label.to_string(), font(12.0), pal.text_primary).size().x + 20.0;
        let is_active = i == active;
        let r = if is_active {
            Rect::from_min_max(pos2(x - 2.0, strip.min.y + 1.0), pos2(x + w + 2.0, strip.max.y))
        } else {
            Rect::from_min_max(pos2(x, strip.min.y + 3.0), pos2(x + w, strip.max.y))
        };
        let resp = ui.interact(r, ui.id().with(("tab", i)), Sense::click());
        let bg = if is_active {
            pal.tab_active_bg
        } else if resp.hovered() {
            pal.hover_bg
        } else {
            pal.tab_inactive_bg
        };
        painter.rect_filled(r, CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, bg);
        painter.rect_stroke(r, CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, Stroke::new(1.0, pal.border), StrokeKind::Inside);
        painter.text(r.center() + vec2(0.0, 1.0), Align2::CENTER_CENTER, *label, font(12.0), pal.text_primary);
        if is_active {
            active_rect = Some(r);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        x += w + 1.0;
    }
    painter.line_segment([pos2(strip.min.x, base_y), pos2(strip.max.x, base_y)], Stroke::new(1.0, pal.border));
    if let Some(r) = active_rect {
        painter.line_segment([pos2(r.min.x + 1.0, base_y), pos2(r.max.x - 1.0, base_y)], Stroke::new(1.0, pal.tab_active_bg));
    }
    clicked
}

/// Status bar with optional thin progress bar and right-aligned info.
/// Horizontal text inset of the status bar: lines up with the table's cell text
/// and keeps clear of rounded window corners.
const STATUS_PAD_X: f32 = crate::theme::SIDE_MARGIN + 4.0;

pub fn status_bar(ui: &mut Ui, pal: &ThemePalette, left: &str, warning: Option<&str>, progress: Option<f32>, right: &str) {
    let rect = ui.available_rect_before_wrap();
    let painter = ui.painter();
    painter.line_segment([rect.left_top(), rect.right_top()], Stroke::new(1.0, pal.toolbar_border));
    let y = rect.center().y + 0.5;
    let mut x = rect.min.x + STATUS_PAD_X;
    if let Some(w) = warning {
        let ir = Rect::from_center_size(pos2(x + 7.0, y), vec2(13.0, 13.0));
        icons::warning(painter, ir, pal);
        let resp = ui.interact(ir, ui.id().with("sb_warn"), Sense::hover());
        resp.on_hover_text(w);
        x += 18.0;
    }
    let lr = painter.text(pos2(x, y), Align2::LEFT_CENTER, left, font(12.0), pal.text_primary);
    let mut right_edge = rect.max.x - STATUS_PAD_X;
    if !right.is_empty() {
        let rr = painter.text(pos2(right_edge, y), Align2::RIGHT_CENTER, right, font(11.5), pal.text_secondary);
        right_edge = rr.min.x - 10.0;
    }
    if let Some(p) = progress {
        let w = 140.0f32.min(right_edge - lr.max.x - 16.0);
        if w > 30.0 {
            let bar = Rect::from_min_size(pos2(right_edge - w, y - 6.0), vec2(w, 12.0));
            painter.rect_filled(bar, CornerRadius::ZERO, if pal.dark { pal.input_bg } else { Color32::from_rgb(0xE6, 0xE6, 0xE6) });
            painter.rect_stroke(bar, CornerRadius::ZERO, Stroke::new(1.0, pal.input_border), StrokeKind::Inside);
            let fillw = (bar.width() - 2.0) * p.clamp(0.0, 1.0);
            painter.rect_filled(
                Rect::from_min_size(bar.min + vec2(1.0, 1.0), vec2(fillw, bar.height() - 2.0)),
                CornerRadius::ZERO,
                pal.progress,
            );
        }
    }
}
