//! Result table with tree view, sortable/movable/resizable columns and
//! row virtualisation (only visible rows are laid out).

use crate::commands::UiCommand;
use crate::i18n::t;
use crate::icons;
use crate::menus::{column_chooser, host_context_menu, ContextCaps};
use crate::theme::{font, ThemePalette};
use egui::text::{LayoutJob, TextWrapping};
use egui::{pos2, vec2, Align2, Color32, CornerRadius, CursorIcon, FontId, Id, Key, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui};
use fingerprint::DeviceClass;
use scan_core::{HostStatus, ServiceType};
use std::collections::{BTreeMap, HashSet};
use std::net::IpAddr;
use storage::{Column, SortState};

pub const ROW_H: f32 = 22.0;
pub const HEADER_H: f32 = 24.0;
const MIN_COL_W: f32 = 28.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ServiceRow {
    pub service_type: ServiceType,
    pub label: String,
    pub port: Option<u16>,
    pub uri: Option<String>,
}

/// Pre-formatted row. Built by the application whenever data changes.
#[derive(Debug, Clone, PartialEq)]
pub struct HostRow {
    pub ip: IpAddr,
    pub status: HostStatus,
    pub class: DeviceClass,
    pub alive: bool,
    pub favorite: bool,
    pub has_mac: bool,
    pub name: String,
    pub mac: String,
    pub cells: Vec<String>,
    pub services: Vec<ServiceRow>,
    pub tooltip: String,
}

impl HostRow {
    pub fn cell(&self, c: Column) -> &str {
        self.cells.get(c as usize).map(String::as_str).unwrap_or("")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub ip: IpAddr,
    pub child: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct HeaderDrag {
    column: Column,
    grab_dx: f32,
}

#[derive(Default)]
pub struct TableState {
    pub selected: Option<Selection>,
    pub expanded: HashSet<IpAddr>,
    pub scroll_to_selected: bool,
    drag: Option<HeaderDrag>,
    resize: Option<(Column, f32)>,
    h_offset: f32,
    v_offset: f32,
    view_h: f32,
}

pub struct TableConfig<'a> {
    pub columns: &'a mut Vec<Column>,
    pub widths: &'a mut BTreeMap<Column, f32>,
    pub sort: &'a mut SortState,
    pub alternating: bool,
    pub id: &'a str,
}

fn width_of(widths: &BTreeMap<Column, f32>, c: Column) -> f32 {
    widths.get(&c).copied().unwrap_or_else(|| c.default_width())
}

fn truncated(text: &str, f: FontId, color: Color32, max_w: f32) -> LayoutJob {
    let mut job = LayoutJob::single_section(text.to_string(), TextFormat { font_id: f, color, ..Default::default() });
    job.wrap = TextWrapping { max_width: max_w.max(4.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    job
}

/// Draw the table. Returns true when column layout or sort changed.
pub fn show(
    ui: &mut Ui,
    pal: &ThemePalette,
    rows: &[HostRow],
    cfg: TableConfig<'_>,
    st: &mut TableState,
    caps: &ContextCaps,
    out: &mut Vec<UiCommand>,
) -> bool {
    let mut changed = false;
    let outer = ui.available_rect_before_wrap();
    ui.allocate_rect(outer, Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(outer, CornerRadius::ZERO, pal.table_bg);

    // Flattened tree.
    let mut flat: Vec<(usize, Option<usize>)> = Vec::with_capacity(rows.len());
    for (i, r) in rows.iter().enumerate() {
        flat.push((i, None));
        if st.expanded.contains(&r.ip) {
            for j in 0..r.services.len() {
                flat.push((i, Some(j)));
            }
        }
    }

    let inner = outer.shrink(1.0);
    let cols: Vec<Column> = cfg.columns.clone();
    let widths: Vec<f32> = cols.iter().map(|c| width_of(cfg.widths, *c)).collect();
    let total_w: f32 = widths.iter().sum();
    // Last column stretches to the window edge (visual only).
    let fill_extra = (inner.width() - total_w - 12.0).max(0.0);

    // ---- Header -----------------------------------------------------------
    let header_rect = Rect::from_min_size(inner.min, vec2(inner.width(), HEADER_H));
    let hp = painter.with_clip_rect(header_rect);
    crate::widgets::gradient_rect(&hp, header_rect, 0.0, pal.header_bg, pal.header_bg_bottom);
    hp.line_segment(
        [header_rect.left_bottom() - vec2(0.0, 0.5), header_rect.right_bottom() - vec2(0.0, 0.5)],
        Stroke::new(1.0_f32, pal.header_border),
    );
    let mut x = header_rect.min.x - st.h_offset;
    let mut col_spans: Vec<(Column, f32, f32)> = Vec::new();
    for (i, c) in cols.iter().enumerate() {
        let w = widths[i] + if i + 1 == cols.len() { fill_extra } else { 0.0 };
        col_spans.push((*c, x, x + w));
        x += w;
    }
    let pointer = ui.ctx().pointer_latest_pos();
    for (i, (c, x0, x1)) in col_spans.iter().copied().enumerate() {
        let cell = Rect::from_min_max(pos2(x0, header_rect.min.y), pos2(x1, header_rect.max.y));
        let id = Id::new((cfg.id, "hdr", c));
        let resp = ui.interact(cell.intersect(header_rect), id, Sense::click_and_drag());
        if resp.hovered() && st.drag.is_none() && st.resize.is_none() {
            hp.rect_filled(cell, CornerRadius::ZERO, if pal.dark { pal.hover_bg } else { Color32::from_rgb(0xD9, 0xEB, 0xF9) });
        }
        hp.line_segment([pos2(x1 - 0.5, cell.min.y + 3.0), pos2(x1 - 0.5, cell.max.y - 3.0)], Stroke::new(1.0_f32, pal.header_border));
        let text_w = (x1 - x0 - 12.0).max(4.0);
        let galley = ui.fonts_mut(|f| f.layout_job(truncated(t(c.key()), font(12.0), pal.text_primary, text_w)));
        hp.galley(pos2(x0 + 6.0, cell.center().y - galley.size().y / 2.0 + 1.0), galley, pal.text_primary);
        if cfg.sort.column == c && cfg.sort.user {
            // Sort chevron at the top centre (Windows list view style).
            let cx = (x0 + x1) / 2.0;
            let y = cell.min.y + 3.0;
            let pts = if cfg.sort.ascending {
                vec![pos2(cx - 4.0, y + 3.5), pos2(cx, y), pos2(cx + 4.0, y + 3.5)]
            } else {
                vec![pos2(cx - 4.0, y), pos2(cx, y + 3.5), pos2(cx + 4.0, y)]
            };
            hp.add(egui::Shape::line(pts, Stroke::new(1.0_f32, pal.text_secondary)));
        }
        // Resize handle on the right edge.
        let handle = Rect::from_min_max(pos2(x1 - 4.0, cell.min.y), pos2(x1 + 3.0, cell.max.y));
        let hresp = ui.interact(handle.intersect(header_rect), Id::new((cfg.id, "rsz", c)), Sense::drag());
        if hresp.hovered() || hresp.dragged() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeColumn);
        }
        if hresp.drag_started() {
            st.resize = Some((c, widths[i]));
        }
        if hresp.dragged() {
            if let Some((rc, _)) = st.resize {
                if rc == c {
                    let w = (width_of(cfg.widths, c) + hresp.drag_delta().x).max(MIN_COL_W);
                    cfg.widths.insert(c, w);
                    changed = true;
                }
            }
        }
        if hresp.drag_stopped() {
            st.resize = None;
        }
        if st.resize.is_none() {
            if resp.drag_started() && c != Column::Status {
                let grab = pointer.map(|p| p.x - x0).unwrap_or(0.0);
                st.drag = Some(HeaderDrag { column: c, grab_dx: grab });
            }
            if resp.clicked() {
                if cfg.sort.column == c && cfg.sort.user {
                    cfg.sort.ascending = !cfg.sort.ascending;
                } else {
                    *cfg.sort = SortState { column: c, ascending: true, user: true };
                }
                changed = true;
            }
        }
        resp.context_menu(|ui| column_chooser(ui, &cols, out));
    }
    // Column drag & drop.
    if let Some(drag) = st.drag {
        if let Some(p) = pointer {
            let w = width_of(cfg.widths, drag.column);
            let ghost = Rect::from_min_size(pos2(p.x - drag.grab_dx, header_rect.min.y), vec2(w, HEADER_H));
            let fg = painter.clone().with_layer_id(egui::LayerId::new(egui::Order::Tooltip, Id::new("hdr_ghost")));
            fg.rect_filled(ghost, CornerRadius::ZERO, pal.selected_bg.gamma_multiply(0.9));
            fg.rect_stroke(ghost, CornerRadius::ZERO, Stroke::new(1.0_f32, pal.selected_border), StrokeKind::Inside);
            fg.text(ghost.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, t(drag.column.key()), font(12.0), pal.text_primary);
            // insertion marker
            let target = col_spans.iter().position(|(_, a, b)| p.x < (a + b) / 2.0).unwrap_or(col_spans.len()).max(1);
            let mx = if target < col_spans.len() { col_spans[target].1 } else { col_spans.last().map(|s| s.2).unwrap_or(0.0) };
            fg.line_segment([pos2(mx, header_rect.min.y), pos2(mx, header_rect.max.y)], Stroke::new(2.0_f32, pal.icon_screen));
            if ui.input(|i| i.pointer.any_released()) {
                if let Some(from) = cfg.columns.iter().position(|c| *c == drag.column) {
                    let mut to = target;
                    if to > from {
                        to -= 1;
                    }
                    if to != from {
                        let c = cfg.columns.remove(from);
                        cfg.columns.insert(to.min(cfg.columns.len()), c);
                        changed = true;
                    }
                }
                st.drag = None;
            }
        } else {
            st.drag = None;
        }
    }

    // ---- Body -------------------------------------------------------------
    let body_rect = Rect::from_min_max(pos2(inner.min.x, header_rect.max.y), inner.max);
    let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(body_rect).id_salt((cfg.id, "body")));
    let content_h = flat.len() as f32 * ROW_H;
    let mut area = egui::ScrollArea::both().id_salt((cfg.id, "scroll")).auto_shrink([false, false]);

    // Keyboard navigation.
    let keyboard = !ui.ctx().wants_keyboard_input();
    if keyboard && !flat.is_empty() {
        let cur = st.selected.and_then(|s| flat.iter().position(|(i, ch)| rows[*i].ip == s.ip && *ch == s.child));
        let (up, down, left, right, enter, del, home, end, pgup, pgdn) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowUp),
                i.key_pressed(Key::ArrowDown),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Enter),
                i.key_pressed(Key::Delete),
                i.key_pressed(Key::Home),
                i.key_pressed(Key::End),
                i.key_pressed(Key::PageUp),
                i.key_pressed(Key::PageDown),
            )
        });
        let page = ((st.view_h / ROW_H) as usize).max(1);
        let mut target = None;
        if down {
            target = Some(cur.map_or(0, |c| (c + 1).min(flat.len() - 1)));
        }
        if up {
            target = Some(cur.map_or(0, |c| c.saturating_sub(1)));
        }
        if home {
            target = Some(0);
        }
        if end {
            target = Some(flat.len() - 1);
        }
        if pgdn {
            target = Some(cur.map_or(0, |c| (c + page).min(flat.len() - 1)));
        }
        if pgup {
            target = Some(cur.map_or(0, |c| c.saturating_sub(page)));
        }
        if let Some(c) = cur {
            let (hi, ch) = flat[c];
            let ip = rows[hi].ip;
            if right && ch.is_none() && !rows[hi].services.is_empty() {
                st.expanded.insert(ip);
            }
            if left {
                if ch.is_some() {
                    target = flat.iter().position(|(i, c2)| *i == hi && c2.is_none());
                } else {
                    st.expanded.remove(&ip);
                }
            }
            if enter {
                out.push(UiCommand::OpenDefault(ip, ch));
            }
            if del && rows[hi].favorite {
                out.push(UiCommand::RemoveFavorite(ip));
            }
        }
        if let Some(tgt) = target {
            let (hi, ch) = flat[tgt];
            st.selected = Some(Selection { ip: rows[hi].ip, child: ch });
            st.scroll_to_selected = true;
        }
    }
    if st.scroll_to_selected {
        st.scroll_to_selected = false;
        if let Some(pos) = st.selected.and_then(|s| flat.iter().position(|(i, ch)| rows[*i].ip == s.ip && *ch == s.child)) {
            let y = pos as f32 * ROW_H;
            if y < st.v_offset {
                area = area.vertical_scroll_offset(y);
            } else if y + ROW_H > st.v_offset + st.view_h {
                area = area.vertical_scroll_offset(y + ROW_H - st.view_h);
            }
        }
    }

    let time = ui.input(|i| i.time);
    let mut any_spinner = false;
    let output = area.show_viewport(&mut body_ui, |ui, viewport| {
        // Only scroll horizontally when the columns are wider than the view.
        let content_w = if total_w > body_rect.width() - 14.0 { total_w } else { body_rect.width() - 14.0 };
        ui.set_min_size(vec2(content_w, content_h));
        let origin = ui.min_rect().min;
        let first = (viewport.min.y / ROW_H).floor().max(0.0) as usize;
        let last = ((viewport.max.y / ROW_H).ceil() as usize + 1).min(flat.len());
        let painter = ui.painter().clone();
        let row_w = (total_w + fill_extra).max(body_rect.width());
        for (fi, &(hi, child)) in flat.iter().enumerate().take(last).skip(first) {
            let row = &rows[hi];
            let rr = Rect::from_min_size(origin + vec2(0.0, fi as f32 * ROW_H), vec2(row_w, ROW_H));
            let sel = st.selected.is_some_and(|s| s.ip == row.ip && s.child == child);
            let resp = ui.interact(rr, Id::new((cfg.id, "row", row.ip, child)), Sense::click());
            let bg = if sel {
                Some(pal.selected_bg)
            } else if resp.hovered() {
                Some(pal.hover_bg)
            } else if cfg.alternating && fi % 2 == 1 {
                Some(pal.table_alt_bg)
            } else {
                None
            };
            if let Some(bg) = bg {
                painter.rect_filled(rr, CornerRadius::ZERO, bg);
            }
            if sel {
                painter.rect_stroke(rr, CornerRadius::ZERO, Stroke::new(1.0_f32, pal.selected_border), StrokeKind::Inside);
            }
            let text_color = match row.status {
                HostStatus::NoResponse | HostStatus::Unknown => pal.text_secondary,
                _ => pal.text_primary,
            };
            let mut x = rr.min.x;
            match child {
                None => {
                    for (ci, c) in cols.iter().enumerate() {
                        let w = widths[ci] + if ci + 1 == cols.len() { fill_extra } else { 0.0 };
                        let cell = Rect::from_min_size(pos2(x, rr.min.y), vec2(w, ROW_H));
                        if *c == Column::Status {
                            if !row.services.is_empty() {
                                let ch = Rect::from_center_size(pos2(cell.min.x + 10.0, cell.center().y), vec2(12.0, 12.0));
                                icons::chevron(&painter, ch, st.expanded.contains(&row.ip), pal.text_secondary);
                            }
                            let ic = Rect::from_center_size(pos2(cell.min.x + 29.0, cell.center().y), vec2(16.0, 16.0));
                            let ip = painter.with_clip_rect(cell);
                            match row.status {
                                HostStatus::Alive => icons::device(&ip, ic, row.class, pal, false),
                                HostStatus::NoResponse => icons::device(&ip, ic, row.class, pal, true),
                                HostStatus::Unknown => icons::question(&ip, ic, pal),
                                HostStatus::Scanning => {
                                    icons::spinner(&ip, ic, pal, time);
                                    any_spinner = true;
                                }
                            }
                            if row.favorite {
                                let sr = Rect::from_center_size(pos2(cell.min.x + 38.0, cell.center().y + 5.0), vec2(8.0, 8.0));
                                icons::star(&ip, sr, Color32::from_rgb(0xF2, 0xB3, 0x1B), true);
                            }
                        } else {
                            let text = row.cell(*c);
                            if !text.is_empty() {
                                let job = truncated(text, font(12.0), text_color, w - 10.0);
                                let g = ui.fonts_mut(|f| f.layout_job(job));
                                painter.with_clip_rect(cell).galley(
                                    pos2(cell.min.x + 5.0, cell.center().y - g.size().y / 2.0),
                                    g,
                                    text_color,
                                );
                            }
                        }
                        x += w;
                    }
                }
                Some(j) => {
                    // Child: icon + text spanning from the second column to the end.
                    let status_w = cols.first().map(|c| if *c == Column::Status { widths[0] } else { 0.0 }).unwrap_or(0.0);
                    let svc = &row.services[j];
                    let start = x + status_w + 6.0;
                    let ic = Rect::from_center_size(pos2(start + 8.0, rr.center().y), vec2(16.0, 16.0));
                    icons::service(&painter, ic, &svc.service_type, pal);
                    let avail = rr.max.x - start - 30.0;
                    let job = truncated(&svc.label, font(12.0), pal.text_primary, avail);
                    let g = ui.fonts_mut(|f| f.layout_job(job));
                    painter.galley(pos2(start + 22.0, rr.center().y - g.size().y / 2.0), g, pal.text_primary);
                }
            }
            // Interaction
            if resp.clicked() || resp.secondary_clicked() || resp.double_clicked() {
                let on_chevron =
                    child.is_none() && resp.interact_pointer_pos().is_some_and(|p| p.x < rr.min.x + 18.0) && !row.services.is_empty();
                if on_chevron && resp.clicked() {
                    if !st.expanded.remove(&row.ip) {
                        st.expanded.insert(row.ip);
                    }
                } else {
                    st.selected = Some(Selection { ip: row.ip, child });
                }
            }
            if resp.double_clicked() {
                let on_chevron = child.is_none() && resp.interact_pointer_pos().is_some_and(|p| p.x < rr.min.x + 18.0);
                if !on_chevron {
                    out.push(UiCommand::OpenDefault(row.ip, child));
                }
            }
            let resp = if child.is_none() && !row.tooltip.is_empty() { resp.on_hover_text_at_pointer(&row.tooltip) } else { resp };
            resp.context_menu(|ui| host_context_menu(ui, row, caps, out));
        }
    });
    st.h_offset = output.state.offset.x;
    st.v_offset = output.state.offset.y;
    st.view_h = output.inner_rect.height();
    if any_spinner {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(50));
    }
    painter.rect_stroke(outer, CornerRadius::ZERO, Stroke::new(1.0_f32, pal.border), StrokeKind::Inside);
    changed
}
