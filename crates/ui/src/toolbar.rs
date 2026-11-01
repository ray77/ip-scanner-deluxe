use crate::commands::UiCommand;
use crate::i18n::t;
use crate::icons;
use crate::theme::ThemePalette;
use crate::widgets::{scan_button, tool_button, toolbar_separator};
use egui::Ui;

pub struct ToolbarState {
    pub scanning: bool,
    pub paused: bool,
    pub show_ip_c: bool,
    pub show_expand: bool,
}

/// Toolbar row: [▶ Scannen] [||] | [IP] [C] | [expand] [collapse]
pub fn show(ui: &mut Ui, pal: &ThemePalette, st: &ToolbarState, out: &mut Vec<UiCommand>) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        ui.add_space(crate::theme::SIDE_MARGIN);
        if scan_button(ui, pal, st.scanning).clicked() {
            out.push(UiCommand::StartStop);
        }
        ui.add_space(3.0);
        let pause = tool_button(ui, pal, st.scanning, st.paused, |p, r| {
            if st.paused {
                icons::play(p, r, pal.accent);
            } else {
                icons::pause(p, r, pal.icon_fg);
            }
        })
        .on_hover_text(if st.paused { t("tb.resume") } else { t("tb.pause") });
        if pause.clicked() {
            out.push(UiCommand::TogglePause);
        }
        if st.show_ip_c {
            toolbar_separator(ui, pal);
            if tool_button(ui, pal, !st.scanning, false, |p, r| icons::ip_button(p, r, pal)).on_hover_text(t("tb.ip")).clicked() {
                out.push(UiCommand::FillLocalSubnet);
            }
            if tool_button(ui, pal, !st.scanning, false, |p, r| icons::c_button(p, r, pal)).on_hover_text(t("tb.c")).clicked() {
                out.push(UiCommand::FillClassC);
            }
        }
        if st.show_expand {
            toolbar_separator(ui, pal);
            if tool_button(ui, pal, true, false, |p, r| icons::expand_all(p, r, pal, true)).on_hover_text(t("tb.expand")).clicked() {
                out.push(UiCommand::ExpandAll);
            }
            if tool_button(ui, pal, true, false, |p, r| icons::expand_all(p, r, pal, false)).on_hover_text(t("tb.collapse")).clicked() {
                out.push(UiCommand::CollapseAll);
            }
        }
    });
}
