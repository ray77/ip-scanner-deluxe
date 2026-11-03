//! Execution of UI commands.

use crate::app::{App, EditDialog, EditKind, OptionsDialog, OptionsTab, PowerDialog};
use crate::export;
use platform::launch::{open_client, open_url, ClientKind};
use platform::power::{RemotePowerProvider, ShutdownOptions, SystemPowerProvider};
use scan_core::{class_c_range, HostRecord, ServiceType};
use std::net::IpAddr;
use storage::Column;
use ui::commands::ExportFormat;
use ui::i18n::{t, tf};
use ui::UiCommand;

/// Main window that owns native file dialogs; `None` with the software renderer.
pub type Parent<'a> = Option<&'a eframe::Frame>;

fn file_dialog(parent: Parent<'_>) -> rfd::FileDialog {
    match parent {
        Some(frame) => rfd::FileDialog::new().set_parent(frame),
        None => rfd::FileDialog::new(),
    }
}

impl App {
    /// Runs `cmd`; `parent` owns native file dialogs so they open centered on
    /// (and modal to) the main window.
    pub fn execute(&mut self, cmd: UiCommand, parent: Parent<'_>) {
        match cmd {
            UiCommand::NewSearch => {
                if self.is_scanning() {
                    self.stop_scan();
                }
                self.data.clear();
                self.results_table = Default::default();
                self.data.last_scan = None;
            }
            UiCommand::LoadList => self.load_list(parent),
            UiCommand::SaveAs(fmt) => self.save_as(fmt, parent),
            UiCommand::ImportFavorites => self.import_favorites(parent),
            UiCommand::ExportFavorites => self.export_favorites(parent),
            UiCommand::Quit => self.ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            UiCommand::ExpandAll => {
                let state = if self.tab == 0 { &mut self.results_table } else { &mut self.favorites_table };
                let rows = if self.tab == 0 { &self.rows } else { &self.fav_rows };
                state.expanded = rows.iter().filter(|r| !r.services.is_empty()).map(|r| r.ip).collect();
            }
            UiCommand::CollapseAll => {
                self.results_table.expanded.clear();
                self.favorites_table.expanded.clear();
                if let Some(s) = &mut self.results_table.selected {
                    s.child = None;
                }
            }
            UiCommand::ToggleAliveOnly => {
                self.settings.show_alive_only = !self.settings.show_alive_only;
                self.mark_settings();
            }
            UiCommand::ToggleShowOffline => {
                self.settings.show_offline = !self.settings.show_offline;
                self.mark_settings();
            }
            UiCommand::ToggleShowUnknown => {
                self.settings.show_unknown = !self.settings.show_unknown;
                self.mark_settings();
            }
            UiCommand::ToggleColumn(c) => {
                if let Some(pos) = self.settings.columns.iter().position(|x| *x == c) {
                    if c != Column::Status && c != Column::Ip {
                        self.settings.columns.remove(pos);
                    }
                } else {
                    self.settings.columns.push(c);
                }
                self.mark_settings();
            }
            UiCommand::SetTheme(th) => {
                self.settings.theme = th;
                self.applied_dark = None;
                self.mark_settings();
            }
            UiCommand::ToggleAlternating => {
                self.settings.alternating_rows = !self.settings.alternating_rows;
                self.mark_settings();
            }
            UiCommand::ToggleToolbarIpC => {
                self.settings.toolbar_ip_c = !self.settings.toolbar_ip_c;
                self.mark_settings();
            }
            UiCommand::ToggleToolbarExpand => {
                self.settings.toolbar_expand = !self.settings.toolbar_expand;
                self.mark_settings();
            }
            UiCommand::OpenOptions => self.open_options(OptionsTab::Scan),
            UiCommand::OpenProfiles => self.open_options(OptionsTab::Scan),
            UiCommand::OpenMacDb => self.dialogs.macdb = true,
            UiCommand::ImportMacDb => self.import_oui(parent),
            UiCommand::OpenFingerprints => self.dialogs.fingerprints = true,
            UiCommand::SetLanguage(l) => {
                self.settings.language = l;
                ui::set_language(l);
                self.mark_settings();
            }
            UiCommand::About => self.dialogs.about = true,
            UiCommand::Diagnose => self.dialogs.diagnose = Some(self.diagnostics()),
            UiCommand::OpenLog => {
                if let Err(e) = open_path(&self.paths.log_file()) {
                    self.toast(e, true);
                }
            }
            UiCommand::StartStop => {
                if self.is_scanning() {
                    self.stop_scan();
                } else if self.range_text.trim().is_empty() {
                    self.execute(UiCommand::FillLocalSubnet, parent);
                    if !self.range_text.is_empty() {
                        self.start_scan(self.range_text.clone(), false);
                    }
                } else {
                    self.start_scan(self.range_text.clone(), false);
                }
            }
            UiCommand::TogglePause => {
                if let Some(s) = self.scan.as_ref().filter(|s| !s.finished) {
                    if s.control.is_paused() {
                        s.control.resume();
                    } else {
                        s.control.pause();
                    }
                }
            }
            UiCommand::FillLocalSubnet => {
                self.interfaces = platform::interfaces::scannable_interfaces();
                if let Some(r) = self.selected_interface().and_then(|i| i.subnet_range()) {
                    self.range_text = r;
                    self.validate_range();
                }
            }
            UiCommand::FillClassC => {
                let src = if self.range_text.trim().is_empty() {
                    self.selected_interface().and_then(|i| i.primary_ipv4()).map(|(a, _)| a.to_string()).unwrap_or_default()
                } else {
                    self.range_text.clone()
                };
                if let Some(r) = class_c_range(&src) {
                    self.range_text = r;
                    self.validate_range();
                }
            }
            UiCommand::FocusSearch => self.focus_search = true,
            UiCommand::OpenDefault(ip, child) => self.open_default(ip, child),
            UiCommand::OpenService(ip, idx) => self.open_service(ip, idx),
            UiCommand::OpenClient(ip, kind, port) => self.launch(ip, kind, port),
            UiCommand::OpenShares(ip) => self.launch(ip, ClientKind::Smb, 445),
            UiCommand::Ping(ip) => {
                let id = self.next_tool_id();
                let s = crate::tools::start_ping(self.rt.handle(), ip, false, id, self.repaint_fn());
                self.dialogs.tools.push(s);
            }
            UiCommand::Traceroute(ip) => {
                let id = self.next_tool_id();
                let s = crate::tools::start_traceroute(self.rt.handle(), ip, id, self.repaint_fn());
                self.dialogs.tools.push(s);
            }
            UiCommand::PortInfo(ip) => self.dialogs.port_info = Some(ip),
            UiCommand::Properties(ip) => self.dialogs.properties = Some(ip),
            UiCommand::Rescan(ip) => {
                if !self.is_scanning() {
                    self.start_scan(ip.to_string(), true);
                }
            }
            UiCommand::AddFavorite(ip) => self.data.add_favorite(ip, self.store.as_ref()),
            UiCommand::RemoveFavorite(ip) => self.data.remove_favorite(ip, self.store.as_ref()),
            UiCommand::EditAlias(ip) => {
                let cur = self.host(ip).and_then(|h| self.data.annotation_for(&h).and_then(|a| a.alias.clone())).unwrap_or_default();
                self.dialogs.edit = Some(EditDialog { ip, kind: EditKind::Alias, text: cur, focus: true });
            }
            UiCommand::EditComment(ip) => {
                let cur = self.host(ip).map(|h| h.comment).unwrap_or_default();
                self.dialogs.edit = Some(EditDialog { ip, kind: EditKind::Comment, text: cur, focus: true });
            }
            UiCommand::CopyText(s) => {
                self.ctx.copy_text(s);
                self.toast(t("msg.copied"), false);
            }
            UiCommand::WakeOnLan(ip) => self.wake_on_lan(ip),
            UiCommand::Shutdown(_) | UiCommand::Reboot(_) if !crate::REMOTE_POWER => {}
            UiCommand::Shutdown(ip) => {
                self.dialogs.power = Some(PowerDialog { ip, reboot: false, message: String::new(), timeout: 30, force: false })
            }
            UiCommand::Reboot(ip) => {
                self.dialogs.power = Some(PowerDialog { ip, reboot: true, message: String::new(), timeout: 30, force: false })
            }
            UiCommand::ScanFavorites => {
                let ips: Vec<String> = self.data.favorites.iter().filter_map(|f| f.ip).map(|i| i.to_string()).collect();
                if !ips.is_empty() && !self.is_scanning() {
                    self.start_scan(ips.join(", "), true);
                }
            }
        }
    }

    fn next_tool_id(&mut self) -> u64 {
        self.dialogs.next_tool_id += 1;
        self.dialogs.next_tool_id
    }

    pub fn host(&self, ip: IpAddr) -> Option<HostRecord> {
        if let Some(h) = self.data.get(ip) {
            return Some(h.clone());
        }
        // Favorites without current scan data.
        self.data.favorites.iter().find(|f| f.ip == Some(ip)).map(|f| {
            let mut h = HostRecord::new(ip, scan_core::HostStatus::Unknown);
            h.mac = f.mac;
            h.hostname = f.name.clone();
            h.vendor = f.vendor.clone();
            h.comment = f.comment.clone();
            h
        })
    }

    pub(crate) fn open_options(&mut self, tab: OptionsTab) {
        self.dialogs.options = Some(OptionsDialog { tab, draft: self.settings.clone(), ports_error: None });
    }

    fn template(&self, kind: ClientKind) -> Option<String> {
        self.settings.client_templates.get(&kind).cloned().filter(|s| !s.trim().is_empty())
    }

    fn launch(&mut self, ip: IpAddr, kind: ClientKind, port: u16) {
        let host = self.host(ip).and_then(|h| h.names.netbios.clone().or(h.hostname.clone()));
        let tpl = self.template(kind);
        tracing::info!("open {} {ip}:{port}", kind.label());
        if let Err(e) = open_client(kind, &ip.to_string(), host.as_deref(), port, tpl.as_deref()) {
            self.toast(format!("{}: {e}", kind.label()), true);
        }
    }

    fn open_service(&mut self, ip: IpAddr, idx: usize) {
        let Some(h) = self.host(ip) else { return };
        let Some(s) = h.services.get(idx).cloned() else { return };
        let port = s.port.unwrap_or(0);
        use ServiceType as S;
        let kind = match s.service_type {
            S::Http | S::HomeAssistant | S::SynologyDsm | S::Docker | S::Elasticsearch | S::Ipp => Some(ClientKind::Http),
            S::Https => Some(ClientKind::Https),
            S::Ftp => Some(ClientKind::Ftp),
            S::Ssh => Some(ClientKind::Ssh),
            S::Telnet => Some(ClientKind::Telnet),
            S::Rdp => Some(ClientKind::Rdp),
            S::Vnc => Some(ClientKind::Vnc),
            S::Smb | S::Shares => Some(ClientKind::Smb),
            _ => None,
        };
        match kind {
            Some(k) => self.launch(ip, k, if port == 0 { k.default_port() } else { port }),
            None => {
                if let Some(uri) = s.uri.as_deref() {
                    if let Err(e) = open_url(uri) {
                        self.toast(e.to_string(), true);
                    }
                } else {
                    self.dialogs.port_info = Some(ip);
                }
            }
        }
    }

    fn open_default(&mut self, ip: IpAddr, child: Option<usize>) {
        if let Some(c) = child {
            self.open_service(ip, c);
            return;
        }
        let Some(h) = self.host(ip) else { return };
        let pref = [
            ServiceType::Http,
            ServiceType::Https,
            ServiceType::SynologyDsm,
            ServiceType::HomeAssistant,
            ServiceType::Rdp,
            ServiceType::Shares,
            ServiceType::Ssh,
            ServiceType::Ftp,
        ];
        for p in pref {
            if let Some(i) = h.services.iter().position(|s| s.service_type == p) {
                self.open_service(ip, i);
                return;
            }
        }
        self.dialogs.properties = Some(ip);
    }

    fn wake_on_lan(&mut self, ip: IpAddr) {
        let Some(h) = self.host(ip) else { return };
        let Some(mac) = h.mac else {
            self.toast(t("ctx.no_mac"), true);
            return;
        };
        let ifaces = platform::interfaces::scannable_interfaces();
        let mut bcasts: Vec<std::net::Ipv4Addr> = match platform::interfaces::interface_for(ip, &ifaces) {
            Some(i) if i.contains(ip) => i.broadcasts(),
            _ => ifaces.iter().flat_map(|i| i.broadcasts()).collect(),
        };
        bcasts.push(std::net::Ipv4Addr::BROADCAST);
        bcasts.dedup();
        let port = self.settings.wol_port;
        let ports: Vec<u16> = if port == 7 || port == 9 { vec![7, 9] } else { vec![port] };
        match protocols::wol::send(&mac, &bcasts, &ports) {
            Ok(n) => {
                tracing::info!("WOL sent to {mac} ({n} packets)");
                self.toast(tf("wol.sent", &[("mac", &mac.to_string()), ("n", &n.to_string())]), false);
            }
            Err(e) => self.toast(format!("Wake-on-LAN: {e}"), true),
        }
    }

    pub fn run_power(&mut self, ip: IpAddr, reboot: bool, opts: ShutdownOptions) {
        let target = self.host(ip).and_then(|h| h.names.netbios.clone()).unwrap_or_else(|| ip.to_string());
        let msgs = self.messages.clone();
        let repaint = self.repaint_fn();
        tracing::info!("remote {} requested for {target}", if reboot { "reboot" } else { "shutdown" });
        self.rt.spawn(async move {
            let p = SystemPowerProvider;
            let r = if reboot { p.reboot(&target, opts).await } else { p.shutdown(&target, opts).await };
            let m = match r {
                Ok(()) => (tf("power.done", &[("host", &target)]), false),
                Err(e) if e.kind() == std::io::ErrorKind::Unsupported => (t("power.unsupported").to_string(), true),
                Err(e) => (tf("power.failed", &[("e", &e.to_string())]), true),
            };
            msgs.lock().push(m);
            repaint();
        });
    }

    fn export_rows(&self) -> Vec<(HostRecord, String)> {
        let rows = if self.tab == 0 { &self.rows } else { &self.fav_rows };
        rows.iter().filter_map(|r| self.host(r.ip).map(|h| (h, r.name.clone()))).collect()
    }

    fn save_as(&mut self, fmt: ExportFormat, parent: Parent<'_>) {
        let name = format!("scan-{}.{}", chrono::Local::now().format("%Y%m%d-%H%M"), fmt.ext());
        let Some(path) = file_dialog(parent).add_filter(fmt.ext().to_uppercase(), &[fmt.ext()]).set_file_name(name).save_file() else {
            return;
        };
        match export::export(&path, fmt, &self.export_rows(), self.settings.csv_bom) {
            Ok(()) => self.toast(tf("msg.exported", &[("p", &path.display().to_string())]), false),
            Err(e) => self.toast(tf("err.export", &[("e", &e.to_string())]), true),
        }
    }

    fn load_list(&mut self, parent: Parent<'_>) {
        let Some(path) = file_dialog(parent).add_filter("Scan", &["json", "xml", "csv"]).pick_file() else { return };
        match export::load_list(&path) {
            Ok(hosts) => {
                if self.is_scanning() {
                    self.stop_scan();
                }
                self.data.clear();
                for h in hosts {
                    self.data.upsert(h);
                }
                self.settings.show_unknown = true;
                self.mark_settings();
            }
            Err(e) => self.toast(tf("err.load", &[("e", &e.to_string())]), true),
        }
    }

    fn import_favorites(&mut self, parent: Parent<'_>) {
        let Some(path) = file_dialog(parent).add_filter("XML", &["xml"]).pick_file() else { return };
        let r = std::fs::read_to_string(&path).map_err(anyhow::Error::from).and_then(|s| export::favorites_from_xml(&s));
        match r {
            Ok(favs) => {
                for f in favs {
                    if let Some(s) = &self.store {
                        let _ = s.upsert_favorite(&f);
                    }
                    self.data.favorites.retain(|x| x.id() != f.id());
                    self.data.favorites.push(f);
                }
                self.data.dirty = true;
                self.tab = 1;
            }
            Err(e) => self.toast(tf("err.load", &[("e", &e.to_string())]), true),
        }
    }

    fn export_favorites(&mut self, parent: Parent<'_>) {
        let Some(path) = file_dialog(parent).add_filter("XML", &["xml"]).set_file_name("favoriten.xml").save_file() else {
            return;
        };
        match std::fs::write(&path, export::favorites_to_xml(&self.data.favorites)) {
            Ok(()) => self.toast(tf("msg.exported", &[("p", &path.display().to_string())]), false),
            Err(e) => self.toast(tf("err.export", &[("e", &e.to_string())]), true),
        }
    }

    pub fn apply_options(&mut self, draft: storage::AppSettings) {
        let lang_changed = draft.language != self.settings.language;
        let theme_changed = draft.theme != self.settings.theme;
        self.settings = draft;
        if lang_changed {
            ui::set_language(self.settings.language);
        }
        if theme_changed {
            self.applied_dark = None;
        }
        self.mark_settings();
        self.save_settings();
    }

    pub fn diagnostics(&self) -> String {
        let yes_no = |b: bool| t(if b { "diag.yes" } else { "diag.no" });
        let line = |key: &str, v: &str| format!("{}\n", tf(key, &[("v", v)]));
        let mut s = String::new();
        let ifaces = platform::interfaces::list_interfaces();
        s.push_str(&line("diag.app_version", env!("CARGO_PKG_VERSION")));
        s.push_str(&line("diag.rust", env!("IPSD_RUSTC_VERSION")));
        s.push_str(&line("diag.os", &platform::os_description()));
        s.push_str(&line("diag.arch", platform::arch()));
        s.push_str(&line("diag.portable", yes_no(self.paths.portable)));
        s.push_str(&line("diag.datadir", &self.paths.database_dir.display().to_string()));
        s.push_str(&format!("\n{}\n", t("diag.ifaces")));
        for i in ifaces.iter().filter(|i| i.is_up && (!i.ipv4.is_empty() || !i.ipv6.is_empty())) {
            s.push_str(&format!(
                "  {} ({}) {:?}{}\n",
                i.display_name,
                i.name,
                i.kind,
                i.virtual_tag.as_ref().map(|t| format!(" [{t}]")).unwrap_or_default()
            ));
            for (a, p) in &i.ipv4 {
                s.push_str(&format!("    IPv4: {a}/{p}\n"));
            }
            for a in &i.ipv6 {
                s.push_str(&format!("    IPv6: {a}\n"));
            }
            if let Some(m) = i.mac {
                s.push_str(&format!("    MAC: {m}\n"));
            }
            if let Some(g) = i.gateway {
                s.push_str(&format!("    Default Gateway: {g}\n"));
            }
        }
        let cap = platform::icmp::capability();
        let icmp = match &cap {
            platform::IcmpCapability::Unprivileged => t("diag.icmp_unpriv").to_string(),
            platform::IcmpCapability::Raw => t("diag.icmp_raw").to_string(),
            platform::IcmpCapability::Unavailable(e) => tf("diag.icmp_no", &[("e", &e.to_string())]),
        };
        s.push('\n');
        s.push_str(&line("diag.icmp", &icmp));
        s.push_str(&line("diag.raw", yes_no(platform::icmp::raw_socket_available())));
        let neighbors = match platform::neighbor::neighbor_table() {
            Ok(t) => tf("diag.entries", &[("n", &t.len().to_string())]),
            Err(e) => tf("diag.unreadable", &[("e", &e.to_string())]),
        };
        s.push_str(&line("diag.neighbors", &neighbors));
        let info = self.vendors.info();
        s.push('\n');
        s.push_str(&format!("{}\n", tf("diag.macdb", &[("v", &info.version), ("s", &self.vendor_source)])));
        let (n, l, m, ms) = (info.total().to_string(), info.ma_l.to_string(), info.ma_m.to_string(), info.ma_s.to_string());
        s.push_str(&format!("{}\n", tf("diag.oui", &[("n", &n), ("l", &l), ("m", &m), ("s", &ms)])));
        s.push_str(&format!("{}\n", tf("diag.rules", &[("n", &self.engine.rule_count().to_string())])));
        s
    }

    pub fn import_oui(&mut self, parent: Parent<'_>) {
        let Some(files) = file_dialog(parent).add_filter("IEEE CSV", &["csv"]).pick_files() else { return };
        let out = self.paths.vendor_db_file();
        let version = format!("IEEE Import {}", chrono::Local::now().format("%Y-%m-%d"));
        match vendor_db::import_csv_files(&files, &out, &version) {
            Ok(stats) => {
                let (db, src) = App::open_vendor_db(&self.paths);
                self.vendors = db;
                self.vendor_source = src;
                self.scanner = scan_core::Scanner::new(self.scanner.probe().clone(), self.vendors.clone(), self.engine.clone());
                self.toast(tf("macdb.imported", &[("n", &stats.total().to_string())]), false);
            }
            Err(e) => self.toast(tf("err.load", &[("e", &e.to_string())]), true),
        }
    }

    pub fn reset_oui(&mut self) {
        let _ = std::fs::remove_file(self.paths.vendor_db_file());
        let (db, src) = App::open_vendor_db(&self.paths);
        self.vendors = db;
        self.vendor_source = src;
        self.scanner = scan_core::Scanner::new(self.scanner.probe().clone(), self.vendors.clone(), self.engine.clone());
    }

    pub fn reload_fingerprints(&mut self) {
        self.engine = std::sync::Arc::new(App::load_engine(&self.paths));
        self.scanner = scan_core::Scanner::new(self.scanner.probe().clone(), self.vendors.clone(), self.engine.clone());
    }
}

pub fn open_path(p: &std::path::Path) -> Result<(), String> {
    if !p.exists() {
        return Err(tf("err.not_found", &[("p", &p.display().to_string())]));
    }
    open::that_detached(p).map_err(|e| e.to_string())
}
