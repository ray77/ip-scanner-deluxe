use crate::commands::{ExportFormat, UiCommand};
use crate::i18n::t;
use crate::table::HostRow;
use egui::Ui;
use platform::launch::ClientKind;
use scan_core::ServiceType;
use storage::{AppTheme, Column, Language};

pub struct MenuState {
    pub alive_only: bool,
    pub show_offline: bool,
    pub show_unknown: bool,
    pub theme: AppTheme,
    pub alternating: bool,
    pub columns: Vec<Column>,
    pub toolbar_ip_c: bool,
    pub toolbar_expand: bool,
    pub language: Language,
    pub scanning: bool,
}

fn item(ui: &mut Ui, label: &str, shortcut: &str) -> bool {
    let b = egui::Button::new(label).shortcut_text(shortcut);
    ui.add(b).clicked()
}

fn check(ui: &mut Ui, label: &str, mut value: bool) -> bool {
    ui.checkbox(&mut value, label).clicked()
}

pub fn menu_bar(ui: &mut Ui, st: &MenuState, out: &mut Vec<UiCommand>) {
    egui::MenuBar::new().ui(ui, |ui| {
        let mut tops = Vec::new();
        tops.push(
            ui.menu_button(t("menu.file"), |ui| {
                ui.set_min_width(200.0);
                if item(ui, t("file.new"), "Ctrl+N") {
                    out.push(UiCommand::NewSearch);
                }
                if item(ui, t("file.load"), "Ctrl+O") {
                    out.push(UiCommand::LoadList);
                }
                ui.menu_button(t("file.save_as"), |ui| {
                    for (f, l) in
                        [(ExportFormat::Csv, "CSV"), (ExportFormat::Html, "HTML"), (ExportFormat::Json, "JSON"), (ExportFormat::Xml, "XML")]
                    {
                        if item(ui, l, if f == ExportFormat::Csv { "Ctrl+S" } else { "" }) {
                            out.push(UiCommand::SaveAs(f));
                        }
                    }
                });
                ui.separator();
                if item(ui, t("file.favorites_import"), "") {
                    out.push(UiCommand::ImportFavorites);
                }
                if item(ui, t("file.favorites_export"), "") {
                    out.push(UiCommand::ExportFavorites);
                }
                ui.separator();
                if item(ui, t("file.quit"), "Alt+F4") {
                    out.push(UiCommand::Quit);
                }
            })
            .response,
        );
        tops.push(
            ui.menu_button(t("menu.view"), |ui| {
                ui.set_min_width(220.0);
                if item(ui, t("view.expand_all"), "") {
                    out.push(UiCommand::ExpandAll);
                }
                if item(ui, t("view.collapse_all"), "") {
                    out.push(UiCommand::CollapseAll);
                }
                ui.separator();
                if check(ui, t("view.alive_only"), st.alive_only) {
                    out.push(UiCommand::ToggleAliveOnly);
                }
                if check(ui, t("view.show_offline"), st.show_offline) {
                    out.push(UiCommand::ToggleShowOffline);
                }
                if check(ui, t("view.show_unknown"), st.show_unknown) {
                    out.push(UiCommand::ToggleShowUnknown);
                }
                ui.separator();
                ui.menu_button(t("view.toolbar"), |ui| {
                    if check(ui, t("view.toolbar_ipc"), st.toolbar_ip_c) {
                        out.push(UiCommand::ToggleToolbarIpC);
                    }
                    if check(ui, t("view.toolbar_expand"), st.toolbar_expand) {
                        out.push(UiCommand::ToggleToolbarExpand);
                    }
                });
                ui.menu_button(t("view.columns"), |ui| column_chooser(ui, &st.columns, out));
                ui.separator();
                if check(ui, t("view.dark"), st.theme == AppTheme::Dark) {
                    out.push(UiCommand::SetTheme(if st.theme == AppTheme::Dark { AppTheme::Light } else { AppTheme::Dark }));
                }
                if check(ui, t("view.theme_system"), st.theme == AppTheme::System) {
                    out.push(UiCommand::SetTheme(if st.theme == AppTheme::System { AppTheme::Light } else { AppTheme::System }));
                }
                if check(ui, t("view.alternating"), st.alternating) {
                    out.push(UiCommand::ToggleAlternating);
                }
            })
            .response,
        );
        tops.push(
            ui.menu_button(t("menu.settings"), |ui| {
                ui.set_min_width(180.0);
                if item(ui, t("settings.options"), "") {
                    out.push(UiCommand::OpenOptions);
                }
                if item(ui, t("settings.macdb"), "") {
                    out.push(UiCommand::OpenMacDb);
                }
                if item(ui, t("settings.profiles"), "") {
                    out.push(UiCommand::OpenProfiles);
                }
                if item(ui, t("settings.fingerprints"), "") {
                    out.push(UiCommand::OpenFingerprints);
                }
                ui.menu_button(t("settings.language"), |ui| {
                    for (l, k) in [
                        (Language::System, "lang.system"),
                        (Language::German, "lang.de"),
                        (Language::English, "lang.en"),
                        (Language::Spanish, "lang.es"),
                        (Language::Italian, "lang.it"),
                    ] {
                        if ui.radio(st.language == l, t(k)).clicked() {
                            out.push(UiCommand::SetLanguage(l));
                        }
                    }
                });
            })
            .response,
        );
        tops.push(
            ui.menu_button(t("menu.help"), |ui| {
                ui.set_min_width(200.0);
                if item(ui, t("help.about"), "") {
                    out.push(UiCommand::About);
                }
                if item(ui, t("help.diagnose"), "") {
                    out.push(UiCommand::Diagnose);
                }
                if item(ui, t("help.log"), "") {
                    out.push(UiCommand::OpenLog);
                }
            })
            .response,
        );
        switch_menu_on_hover(ui, &tops);
    });
}

/// Classic menu bar: while one menu is open, hovering another menu title opens that one instead.
fn switch_menu_on_hover(ui: &Ui, tops: &[egui::Response]) {
    let ids: Vec<egui::Id> = tops.iter().map(egui::Popup::default_response_id).collect();
    let open = ids.iter().position(|id| egui::Popup::is_id_open(ui.ctx(), *id));
    let hovered = tops.iter().position(egui::Response::hovered);
    if let (Some(open), Some(hovered)) = (open, hovered) {
        if open != hovered {
            egui::Popup::open_id(ui.ctx(), ids[hovered]);
            ui.ctx().request_repaint();
        }
    }
}

pub fn column_chooser(ui: &mut Ui, visible: &[Column], out: &mut Vec<UiCommand>) {
    for c in Column::ALL {
        let on = visible.contains(&c);
        let mut v = on;
        let resp = ui.add_enabled(c != Column::Status && c != Column::Ip, egui::Checkbox::new(&mut v, t(c.key())));
        if resp.clicked() {
            out.push(UiCommand::ToggleColumn(c));
        }
    }
}

pub struct ContextCaps {
    pub power_supported: bool,
    /// False in Store builds compiled with `store-no-remote-shutdown`: the entries are hidden.
    pub remote_power: bool,
}

/// Right-click menu for a device row. Only actions that make sense for the
/// device are offered.
pub fn host_context_menu(ui: &mut Ui, row: &HostRow, caps: &ContextCaps, out: &mut Vec<UiCommand>) {
    ui.set_min_width(210.0);
    let ip = row.ip;
    let find = |t: &[ServiceType]| row.services.iter().find(|s| t.contains(&s.service_type)).and_then(|s| s.port);
    let http = find(&[ServiceType::Http, ServiceType::HomeAssistant, ServiceType::SynologyDsm, ServiceType::Docker]);
    let https = find(&[ServiceType::Https]);
    let ftp = find(&[ServiceType::Ftp]);
    let ssh = find(&[ServiceType::Ssh]);
    let rdp = find(&[ServiceType::Rdp]);
    let smb = row.services.iter().any(|s| matches!(s.service_type, ServiceType::Smb | ServiceType::Shares));
    let any_open = http.is_some() || https.is_some() || ftp.is_some() || ssh.is_some() || rdp.is_some() || smb;

    ui.add_enabled_ui(any_open, |ui| {
        ui.menu_button(t("ctx.open"), |ui| {
            for (port, kind) in [
                (http, ClientKind::Http),
                (https, ClientKind::Https),
                (ftp, ClientKind::Ftp),
                (ssh, ClientKind::Ssh),
                (rdp, ClientKind::Rdp),
            ] {
                if let Some(p) = port {
                    let label = if p == kind.default_port() { kind.label().to_string() } else { format!("{} ({p})", kind.label()) };
                    if ui.button(label).clicked() {
                        out.push(UiCommand::OpenClient(ip, kind, p));
                        ui.close();
                    }
                }
            }
            if smb && ui.button(t("ctx.shares")).clicked() {
                out.push(UiCommand::OpenShares(ip));
                ui.close();
            }
        });
    });
    ui.menu_button(t("ctx.tools"), |ui| {
        if ui.button(t("ctx.ping")).clicked() {
            out.push(UiCommand::Ping(ip));
            ui.close();
        }
        if ui.button(t("ctx.traceroute")).clicked() {
            out.push(UiCommand::Traceroute(ip));
            ui.close();
        }
        if ui.button(t("ctx.portinfo")).clicked() {
            out.push(UiCommand::PortInfo(ip));
            ui.close();
        }
        if ui.button(t("ctx.rescan")).clicked() {
            out.push(UiCommand::Rescan(ip));
            ui.close();
        }
    });
    ui.menu_button(t("ctx.device"), |ui| {
        if row.favorite {
            if ui.button(t("ctx.remove_fav")).clicked() {
                out.push(UiCommand::RemoveFavorite(ip));
                ui.close();
            }
        } else if ui.button(t("ctx.add_fav")).clicked() {
            out.push(UiCommand::AddFavorite(ip));
            ui.close();
        }
        if ui.button(t("ctx.rename")).clicked() {
            out.push(UiCommand::EditAlias(ip));
            ui.close();
        }
        if ui.button(t("ctx.comment")).clicked() {
            out.push(UiCommand::EditComment(ip));
            ui.close();
        }
        ui.separator();
        if ui.add_enabled(!row.mac.is_empty(), egui::Button::new(t("ctx.copy_mac"))).clicked() {
            out.push(UiCommand::CopyText(row.mac.clone()));
            ui.close();
        }
        if ui.button(t("ctx.copy_ip")).clicked() {
            out.push(UiCommand::CopyText(ip.to_string()));
            ui.close();
        }
        if ui.button(t("ctx.copy_name")).clicked() {
            out.push(UiCommand::CopyText(row.name.clone()));
            ui.close();
        }
    });
    ui.menu_button(t("ctx.power"), |ui| {
        let wol = ui.add_enabled(row.has_mac, egui::Button::new(t("ctx.wol")));
        let wol = if row.has_mac { wol } else { wol.on_disabled_hover_text(t("ctx.no_mac")) };
        if wol.clicked() {
            out.push(UiCommand::WakeOnLan(ip));
            ui.close();
        }
        if caps.remote_power {
            let enabled = caps.power_supported && row.alive;
            let sd = ui.add_enabled(enabled, egui::Button::new(t("ctx.shutdown")));
            let sd = if caps.power_supported { sd } else { sd.on_disabled_hover_text(t("ctx.windows_only")) };
            if sd.clicked() {
                out.push(UiCommand::Shutdown(ip));
                ui.close();
            }
            let rb = ui.add_enabled(enabled, egui::Button::new(t("ctx.reboot")));
            let rb = if caps.power_supported { rb } else { rb.on_disabled_hover_text(t("ctx.windows_only")) };
            if rb.clicked() {
                out.push(UiCommand::Reboot(ip));
                ui.close();
            }
        }
    });
    ui.separator();
    if ui.button(t("ctx.properties")).clicked() {
        out.push(UiCommand::Properties(ip));
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::switch_menu_on_hover;
    use egui::{Event, PointerButton, Pos2, RawInput, Rect};

    /// Draws a bar with menus "A" and "B" and returns their title rects and popup ids.
    fn frame(ctx: &egui::Context, events: Vec<Event>) -> Vec<(Rect, egui::Id)> {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))), events, ..Default::default() };
        let mut tops = Vec::new();
        let _ = ctx.run(input, |ctx| {
            egui::TopBottomPanel::top("bar").show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    let mut responses = Vec::new();
                    for name in ["A", "B"] {
                        responses.push(
                            ui.menu_button(name, |ui| {
                                let _ = ui.button("item");
                            })
                            .response,
                        );
                    }
                    switch_menu_on_hover(ui, &responses);
                    tops = responses.iter().map(|r| (r.rect, egui::Popup::default_response_id(r))).collect();
                });
            });
        });
        tops
    }

    fn click(pos: Pos2) -> Vec<Event> {
        let button = |pressed| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() };
        vec![Event::PointerMoved(pos), button(true), button(false)]
    }

    #[test]
    fn hovering_other_title_switches_open_menu() {
        let ctx = egui::Context::default();
        let tops = frame(&ctx, Vec::new());
        let (a, b) = (tops[0], tops[1]);
        frame(&ctx, click(a.0.center()));
        frame(&ctx, Vec::new());
        assert!(egui::Popup::is_id_open(&ctx, a.1), "click opens A");

        frame(&ctx, vec![Event::PointerMoved(b.0.center())]);
        frame(&ctx, Vec::new());
        assert!(egui::Popup::is_id_open(&ctx, b.1), "hover switches to B");
        assert!(!egui::Popup::is_id_open(&ctx, a.1));
    }

    #[test]
    fn hovering_titles_without_open_menu_opens_nothing() {
        let ctx = egui::Context::default();
        let tops = frame(&ctx, Vec::new());
        frame(&ctx, vec![Event::PointerMoved(tops[1].0.center())]);
        frame(&ctx, Vec::new());
        assert!(!egui::Popup::is_any_open(&ctx));
    }
}
