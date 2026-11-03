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
        });
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
        });
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
        });
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
        });
    });
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
