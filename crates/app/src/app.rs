//! Main window and event loop glue.

use crate::state::Data;
use crate::tools::ToolSession;
use parking_lot::Mutex;
use platform::interfaces::NetworkInterface;
use scan_core::{parse_targets, ScanControl, ScanEvent, Scanner, SystemProbe};
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use storage::{AppPaths, AppSettings, AppTheme, Store};
use tokio::sync::mpsc;
use ui::i18n::{t, tf};
use ui::menus::{ContextCaps, MenuState};
use ui::table::{HostRow, TableConfig, TableState};
use ui::theme::ThemePalette;
use ui::toolbar::ToolbarState;
use ui::UiCommand;
use vendor_db::VendorDb;

pub const BUNDLED_VENDOR_DB: &[u8] = include_bytes!("../../../assets/vendor.redb");

pub struct RunningScan {
    pub control: ScanControl,
    pub rx: mpsc::UnboundedReceiver<ScanEvent>,
    pub total: u64,
    pub completed: u64,
    pub started: Instant,
    pub seen: HashSet<IpAddr>,
    pub range: String,
    pub finished: bool,
    pub merge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionsTab {
    Scan,
    Performance,
    Names,
    Programs,
    Interface,
    Network,
}

pub struct OptionsDialog {
    pub tab: OptionsTab,
    pub draft: AppSettings,
    pub ports_error: Option<String>,
}

pub enum EditKind {
    Alias,
    Comment,
}

pub struct EditDialog {
    pub ip: IpAddr,
    pub kind: EditKind,
    pub text: String,
    pub focus: bool,
}

pub struct PowerDialog {
    pub ip: IpAddr,
    pub reboot: bool,
    pub message: String,
    pub timeout: u32,
    pub force: bool,
}

#[derive(Default)]
pub struct Dialogs {
    pub options: Option<OptionsDialog>,
    pub about: bool,
    pub diagnose: Option<String>,
    pub macdb: bool,
    pub macdb_lookup: String,
    pub fingerprints: bool,
    pub port_info: Option<IpAddr>,
    pub properties: Option<IpAddr>,
    pub edit: Option<EditDialog>,
    pub power: Option<PowerDialog>,
    pub tools: Vec<ToolSession>,
    pub next_tool_id: u64,
}

pub struct Toast {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

pub struct App {
    pub rt: tokio::runtime::Runtime,
    pub paths: AppPaths,
    pub store: Option<Store>,
    pub settings: AppSettings,
    pub vendors: Arc<VendorDb>,
    pub vendor_source: String,
    pub engine: Arc<fingerprint::FingerprintEngine>,
    pub scanner: Scanner<SystemProbe>,
    pub interfaces: Vec<NetworkInterface>,
    pub data: Data,
    pub scan: Option<RunningScan>,
    pub warnings: Vec<String>,
    pub range_text: String,
    pub range_error: Option<String>,
    pub search: String,
    pub last_query: String,
    pub tab: usize,
    pub results_table: TableState,
    pub favorites_table: TableState,
    pub rows: Vec<HostRow>,
    pub fav_rows: Vec<HostRow>,
    pub view_dirty: bool,
    pub dialogs: Dialogs,
    pub toast: Option<Toast>,
    pub messages: Arc<Mutex<Vec<(String, bool)>>>,
    pub focus_search: bool,
    pub applied_dark: Option<bool>,
    pub settings_dirty: bool,
    pub last_settings_save: Instant,
    pub ctx: egui::Context,
    pub cli: crate::screenshot::CliOptions,
    pub frames: u64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, rt: tokio::runtime::Runtime, paths: AppPaths, cli: crate::screenshot::CliOptions) -> Self {
        // 1. Settings (demo mode never touches the user's data)
        // Demo and screenshot runs never touch the user's real settings.
        let store = if cli.demo || cli.screenshot.is_some() { Store::in_memory() } else { Store::open(&paths.store_file()) };
        let store = match store {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::error!("cannot open settings database: {e}");
                None
            }
        };
        let mut settings: AppSettings = store.as_ref().map(|s| s.load_settings()).unwrap_or_default();
        if let Some(dark) = cli.dark {
            settings.theme = if dark { AppTheme::Dark } else { AppTheme::Light };
        }
        if cli.demo {
            settings.last_range = "192.168.0.90-110".into();
            settings.language = storage::Language::German;
        }
        if cli.lang_en {
            settings.language = storage::Language::English;
        }
        ui::set_language(settings.language);
        // 2. Theme + fonts
        ui::install_fonts(&cc.egui_ctx);
        // 3. OUI database (user import overrides the bundled one)
        let (vendors, vendor_source) = Self::open_vendor_db(&paths);
        // Fingerprints
        let engine = Arc::new(Self::load_engine(&paths));
        // 4. Interfaces
        let interfaces = platform::interfaces::scannable_interfaces();
        let probe = Arc::new(SystemProbe::with_interfaces(platform::interfaces::list_interfaces()));
        let scanner = Scanner::new(probe, vendors.clone(), engine.clone());
        let data = Data::load(store.as_ref());
        // 5. Last used range (or own subnet)
        let mut app = App {
            rt,
            paths,
            store,
            range_text: settings.last_range.clone(),
            settings,
            vendors,
            vendor_source,
            engine,
            scanner,
            interfaces,
            data,
            scan: None,
            warnings: Vec::new(),
            range_error: None,
            search: String::new(),
            last_query: String::new(),
            tab: 0,
            results_table: TableState::default(),
            favorites_table: TableState::default(),
            rows: Vec::new(),
            fav_rows: Vec::new(),
            view_dirty: true,
            dialogs: Dialogs::default(),
            toast: None,
            messages: Arc::new(Mutex::new(Vec::new())),
            focus_search: false,
            applied_dark: None,
            settings_dirty: false,
            last_settings_save: Instant::now(),
            ctx: cc.egui_ctx.clone(),
            cli,
            frames: 0,
        };
        if app.cli.demo {
            for h in crate::screenshot::demo_hosts() {
                app.data.upsert(h);
            }
            app.data.last_scan = Some(SystemTime::now());
            if app.cli.expand {
                app.results_table.expanded.insert("192.168.0.110".parse().unwrap_or(IpAddr::from([0, 0, 0, 0])));
            }
            app.results_table.selected =
                Some(ui::table::Selection { ip: "192.168.0.110".parse().unwrap_or(IpAddr::from([0, 0, 0, 0])), child: None });
        }
        // `--dialog <name>` opens a dialog at start (manual checks, docs).
        match app.cli.dialog.as_deref() {
            Some("about") => app.dialogs.about = true,
            Some("options") => app.open_options(OptionsTab::Scan),
            Some("macdb") => app.dialogs.macdb = true,
            Some("fingerprints") => app.dialogs.fingerprints = true,
            Some("diagnose") => app.dialogs.diagnose = Some(app.diagnostics()),
            _ => {}
        }
        if app.range_text.trim().is_empty() {
            if let Some(r) = app.selected_interface().and_then(|i| i.subnet_range()) {
                app.range_text = r;
            }
        }
        app.validate_range();
        if !platform::icmp::capability().is_available() {
            app.warnings.push(t("diag.icmp_fallback").into());
        }
        app
    }

    pub fn open_vendor_db(paths: &AppPaths) -> (Arc<VendorDb>, String) {
        let user = paths.vendor_db_file();
        if user.exists() {
            match VendorDb::open(&user) {
                Ok(db) => return (Arc::new(db), user.display().to_string()),
                Err(e) => tracing::warn!("user vendor db unusable ({e}), using bundled"),
            }
        }
        match VendorDb::from_bytes(BUNDLED_VENDOR_DB) {
            Ok(db) => (Arc::new(db), "bundled".into()),
            Err(e) => {
                tracing::error!("bundled vendor db failed: {e}");
                (Arc::new(VendorDb::empty().unwrap_or_else(|_| unreachable_db())), "none".into())
            }
        }
    }

    pub fn load_engine(paths: &AppPaths) -> fingerprint::FingerprintEngine {
        let extra = std::fs::read_to_string(paths.fingerprint_file())
            .ok()
            .and_then(|s| match fingerprint::rules::parse_rules(&s) {
                Ok(r) => Some(r),
                Err(e) => {
                    tracing::warn!("custom fingerprints invalid: {e}");
                    None
                }
            })
            .unwrap_or_default();
        fingerprint::FingerprintEngine::with_builtin_and(extra)
    }

    pub fn selected_interface(&self) -> Option<&NetworkInterface> {
        let pool: Vec<&NetworkInterface> =
            self.interfaces.iter().filter(|i| !(self.settings.hide_virtual_interfaces && i.virtual_tag.is_some())).collect();
        if let Some(name) = &self.settings.last_interface {
            if let Some(i) = pool.iter().find(|i| &i.name == name) {
                return Some(i);
            }
        }
        pool.iter().find(|i| i.is_default).or(pool.first()).copied()
    }

    pub fn palette(&self) -> ThemePalette {
        let dark = match self.settings.theme {
            AppTheme::Light => false,
            AppTheme::Dark => true,
            AppTheme::System => self.ctx.system_theme().is_some_and(|t| t == egui::Theme::Dark),
        };
        ThemePalette::get(dark)
    }

    pub fn validate_range(&mut self) {
        self.range_error = if self.range_text.trim().is_empty() { None } else { parse_targets(&self.range_text).err().map(|e| e.message) };
    }

    pub fn is_scanning(&self) -> bool {
        self.scan.as_ref().is_some_and(|s| !s.finished)
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        self.toast = Some(Toast { text: text.into(), error, at: Instant::now() });
    }

    pub fn repaint_fn(&self) -> Arc<dyn Fn() + Send + Sync> {
        let ctx = self.ctx.clone();
        Arc::new(move || ctx.request_repaint())
    }

    pub fn mark_settings(&mut self) {
        self.settings_dirty = true;
        self.view_dirty = true;
    }

    pub fn save_settings(&mut self) {
        if let Some(s) = &self.store {
            if let Err(e) = s.save_settings(&self.settings) {
                tracing::warn!("saving settings failed: {e}");
            }
        }
        self.settings_dirty = false;
        self.last_settings_save = Instant::now();
    }

    // ---- scanning -------------------------------------------------------

    pub fn start_scan(&mut self, range: String, merge: bool) {
        let spec = match parse_targets(&range) {
            Ok(s) => s,
            Err(e) => {
                self.range_error = Some(e.message.clone());
                self.toast(format!("{}: {}", t("err.range"), e.message), true);
                return;
            }
        };
        if let Some(s) = self.scan.take() {
            s.control.cancel();
        }
        if !merge {
            self.settings.push_history(&range);
            self.settings.last_range = range.clone();
            self.mark_settings();
            if self.settings.clear_on_scan {
                self.data.clear();
                self.results_table.expanded.clear();
            }
        }
        self.warnings.retain(|w| w.starts_with("ICMP"));
        let mut cfg = self.settings.scan.clone();
        cfg.ports = match cfg.profile {
            scan_core::ScanProfile::Fast => scan_core::ports::fast_ports(),
            scan_core::ScanProfile::Standard => scan_core::ports::standard_ports(),
            scan_core::ScanProfile::Accurate => scan_core::ports::accurate_ports(),
            scan_core::ScanProfile::Custom => {
                scan_core::ports::parse_port_list(&self.settings.custom_ports).unwrap_or_else(|_| scan_core::ports::standard_ports())
            }
        };
        let (tx, rx) = mpsc::unbounded_channel();
        let control = self.scanner.start(self.rt.handle(), spec.clone(), cfg, tx, self.repaint_fn());
        tracing::info!("scan started: {range} ({} targets)", spec.len());
        self.scan = Some(RunningScan {
            control,
            rx,
            total: spec.len(),
            completed: 0,
            started: Instant::now(),
            seen: HashSet::new(),
            range,
            finished: false,
            merge,
        });
    }

    pub fn stop_scan(&mut self) {
        if let Some(s) = &mut self.scan {
            s.control.cancel();
            s.finished = true;
        }
        self.finish_scanning_rows();
        tracing::info!("scan stopped by user");
    }

    fn finish_scanning_rows(&mut self) {
        let ips: Vec<IpAddr> = self.data.hosts.iter().filter(|h| h.status == scan_core::HostStatus::Scanning).map(|h| h.ip).collect();
        for ip in ips {
            if let Some(h) = self.data.get_mut(ip) {
                h.status = scan_core::HostStatus::Alive;
            }
        }
    }

    fn pump_events(&mut self) {
        let Some(scan) = &mut self.scan else { return };
        let mut events = Vec::new();
        for _ in 0..20_000 {
            match scan.rx.try_recv() {
                Ok(e) => events.push(e),
                Err(_) => break,
            }
        }
        let stopped = scan.finished;
        for ev in events {
            match ev {
                ScanEvent::ScanStarted { total_targets } => {
                    if let Some(s) = &mut self.scan {
                        s.total = total_targets;
                    }
                }
                ScanEvent::HostDiscovered { host } | ScanEvent::HostUpdated { host } => {
                    if stopped {
                        continue;
                    }
                    let ip = host.ip;
                    let alive = host.status == scan_core::HostStatus::Alive;
                    if let Some(s) = &mut self.scan {
                        s.seen.insert(ip);
                    }
                    self.data.upsert(*host);
                    if alive {
                        self.data.record_history(ip, self.store.as_ref());
                    }
                }
                ScanEvent::Progress { completed, total, .. } => {
                    if let Some(s) = &mut self.scan {
                        s.completed = completed;
                        s.total = total;
                    }
                }
                ScanEvent::ScanPaused | ScanEvent::ScanResumed => {}
                ScanEvent::Warning(w) => {
                    if !self.warnings.contains(&w) {
                        tracing::warn!("{w}");
                        self.warnings.push(w);
                    }
                }
                ScanEvent::ScanFinished => {
                    self.on_scan_finished();
                }
                ScanEvent::ScanCancelled => {
                    if let Some(s) = &mut self.scan {
                        s.finished = true;
                    }
                }
            }
        }
    }

    fn on_scan_finished(&mut self) {
        let Some(s) = &mut self.scan else { return };
        s.finished = true;
        let elapsed = s.started.elapsed();
        let seen = std::mem::take(&mut s.seen);
        let range = s.range.clone();
        let merge = s.merge;
        if !merge && !self.settings.clear_on_scan {
            if let Ok(spec) = parse_targets(&range) {
                let stale: Vec<IpAddr> =
                    self.data.hosts.iter().filter(|h| spec.contains(h.ip) && !seen.contains(&h.ip)).map(|h| h.ip).collect();
                for ip in stale {
                    if let Some(h) = self.data.get_mut(ip) {
                        h.status = scan_core::HostStatus::NoResponse;
                    }
                }
            }
        }
        self.finish_scanning_rows();
        self.data.last_scan = Some(SystemTime::now());
        self.data.dirty = true;
        let (a, d, u) = self.data.counts();
        tracing::info!("scan finished in {:.1}s: {a} alive, {d} no response, {u} unknown", elapsed.as_secs_f32());
    }

    // ---- frame ----------------------------------------------------------

    fn rebuild_rows(&mut self) {
        if self.data.dirty || self.view_dirty || self.search != self.last_query {
            self.rows = self.data.result_rows(&self.settings, &self.search);
            self.fav_rows = self.data.favorite_rows(&self.settings, &self.search);
            self.data.dirty = false;
            self.view_dirty = false;
            self.last_query = self.search.clone();
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context, out: &mut Vec<UiCommand>) {
        let typing = ctx.wants_keyboard_input();
        ctx.input_mut(|i| {
            use egui::{Key, KeyboardShortcut, Modifiers};
            if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F)) {
                out.push(UiCommand::FocusSearch);
            }
            if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S)) {
                out.push(UiCommand::SaveAs(ui::commands::ExportFormat::Csv));
            }
            if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::N)) {
                out.push(UiCommand::NewSearch);
            }
            if i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O)) {
                out.push(UiCommand::LoadList);
            }
            if i.consume_key(Modifiers::NONE, Key::F5) {
                out.push(UiCommand::StartStop);
            }
            if !typing && i.consume_key(Modifiers::NONE, Key::Space) {
                out.push(UiCommand::TogglePause);
            }
        });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.is_scanning() && !typing {
            out.push(UiCommand::StartStop);
        }
    }

    fn status_text(&self) -> (String, Option<f32>) {
        let (a, d, u) = self.data.counts();
        if let Some(s) = self.scan.as_ref().filter(|s| !s.finished) {
            let pct = (s.completed * 100).checked_div(s.total).unwrap_or(0);
            let mut text = tf(
                "sb.progress",
                &[("c", &s.completed.to_string()), ("t", &s.total.to_string()), ("a", &a.to_string()), ("p", &pct.to_string())],
            );
            if s.control.is_paused() {
                text = format!("{} | {text}", t("sb.paused"));
            }
            return (text, Some(if s.total == 0 { 0.0 } else { s.completed as f32 / s.total as f32 }));
        }
        if self.tab == 1 {
            return (tf("sb.favorites", &[("n", &self.data.favorites.len().to_string())]), None);
        }
        (tf("sb.summary", &[("a", &a.to_string()), ("d", &d.to_string()), ("u", &u.to_string())]), None)
    }

    fn status_right(&self) -> String {
        let profile = match self.settings.scan.profile {
            scan_core::ScanProfile::Fast => t("opt.profile_fast"),
            scan_core::ScanProfile::Standard => t("opt.profile_standard"),
            scan_core::ScanProfile::Accurate => t("opt.profile_accurate"),
            scan_core::ScanProfile::Custom => t("opt.profile_custom"),
        };
        let mut parts = vec![profile.to_string()];
        if let Some(i) = self.selected_interface() {
            parts.push(i.display_name.clone());
            if let Some((a, p)) = i.primary_ipv4() {
                let m = u32::from(platform::interfaces::prefix_to_mask(p));
                parts.push(format!("{}/{p}", std::net::Ipv4Addr::from(u32::from(a) & m)));
            }
        }
        parts.join(" | ")
    }
}

impl App {
    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some(path) = self.cli.screenshot.clone() else { return };
        self.frames += 1;
        if self.frames == 10 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| {
            i.raw.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = shot {
            let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Err(e) = crate::screenshot::write_png(&path, img.size[0], img.size[1], &rgba) {
                eprintln!("screenshot failed: {e}");
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        ctx.request_repaint();
    }
}

fn unreachable_db() -> VendorDb {
    // VendorDb::empty only fails if an in-memory redb cannot be created.
    VendorDb::from_records(&[], "none").unwrap_or_else(|e| panic!("in-memory database unavailable: {e}"))
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let pal = self.palette();
        if self.applied_dark != Some(pal.dark) {
            ui::apply_theme(ctx, &pal);
            self.applied_dark = Some(pal.dark);
        }
        self.pump_events();
        let pending = std::mem::take(&mut *self.messages.lock());
        for (m, err) in pending {
            self.toast(m, err);
        }
        self.rebuild_rows();
        let mut out: Vec<UiCommand> = Vec::new();
        self.handle_shortcuts(ctx, &mut out);
        let scanning = self.is_scanning();

        const SIDE: i8 = ui::theme::SIDE_MARGIN as i8;

        // Menu bar (~22 px)
        egui::TopBottomPanel::top("menu")
            .exact_height(22.0)
            .frame(
                egui::Frame::new()
                    .fill(if pal.dark { pal.panel_bg } else { egui::Color32::WHITE })
                    .inner_margin(egui::Margin::symmetric(SIDE - 3, 0)),
            )
            .show(ctx, |ui| {
                let st = MenuState {
                    alive_only: self.settings.show_alive_only,
                    show_offline: self.settings.show_offline,
                    show_unknown: self.settings.show_unknown,
                    theme: self.settings.theme,
                    alternating: self.settings.alternating_rows,
                    columns: self.settings.columns.clone(),
                    toolbar_ip_c: self.settings.toolbar_ip_c,
                    toolbar_expand: self.settings.toolbar_expand,
                    language: self.settings.language,
                    scanning,
                };
                ui.style_mut().spacing.button_padding = egui::vec2(7.0, 2.0);
                ui::menus::menu_bar(ui, &st, &mut out);
            });

        // Toolbar (~50 px)
        egui::TopBottomPanel::top("toolbar")
            .exact_height(50.0)
            .frame(egui::Frame::new().fill(pal.toolbar_bg).stroke(egui::Stroke::new(1.0_f32, pal.toolbar_border)))
            .show(ctx, |ui| {
                let st = ToolbarState {
                    scanning,
                    paused: self.scan.as_ref().is_some_and(|s| s.control.is_paused()),
                    show_ip_c: self.settings.toolbar_ip_c,
                    show_expand: self.settings.toolbar_expand,
                };
                ui::toolbar::show(ui, &pal, &st, &mut out);
            });

        // Range + search (~28 px)
        egui::TopBottomPanel::top("inputs")
            .exact_height(32.0)
            .frame(egui::Frame::new().fill(pal.panel_bg).inner_margin(egui::Margin { left: SIDE, right: SIDE, top: 5, bottom: 5 }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let avail = ui.available_width();
                    let search_w = (avail * 0.36).clamp(150.0, 280.0);
                    let range_w = avail - search_w - 6.0;
                    let before = self.range_text.clone();
                    let history = self.settings.range_history.clone();
                    let (resp, chosen) = ui
                        .push_id("range_input", |ui| {
                            ui::widgets::range_field(ui, &pal, &mut self.range_text, &history, self.range_error.as_deref(), range_w)
                        })
                        .inner;
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !scanning {
                        out.push(UiCommand::StartStop);
                    }
                    if let Some(c) = chosen {
                        self.range_text = c;
                    }
                    if before != self.range_text {
                        self.validate_range();
                    }
                    let focus = std::mem::take(&mut self.focus_search);
                    ui::widgets::search_field(ui, &pal, &mut self.search, search_w, focus);
                });
            });

        // Status bar (~22 px)
        egui::TopBottomPanel::bottom("status").exact_height(22.0).frame(egui::Frame::new().fill(pal.panel_bg)).show(ctx, |ui| {
            let (text, progress) = self.status_text();
            let toast = self.toast.as_ref().filter(|t| t.at.elapsed().as_secs() < 6);
            let right = match toast {
                Some(t) if t.error => format!("⚠ {}", t.text),
                Some(t) => t.text.clone(),
                None => self.status_right(),
            };
            let warn = (!self.warnings.is_empty()).then(|| self.warnings.join("\n"));
            ui::widgets::status_bar(ui, &pal, &text, warn.as_deref(), progress, &right);
            if toast.is_some() {
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
        });

        // Tabs + table
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(pal.panel_bg).inner_margin(egui::Margin { left: SIDE, right: SIDE, top: 2, bottom: 4 }))
            .show(ctx, |ui| {
                let labels = [t("tab.results"), t("tab.favorites")];
                if let Some(i) = ui::widgets::tabs(ui, &pal, &labels, self.tab) {
                    self.tab = i;
                }
                let caps = ContextCaps { power_supported: cfg!(windows) };
                let (rows, state, id) = if self.tab == 0 {
                    (&self.rows, &mut self.results_table, "results")
                } else {
                    (&self.fav_rows, &mut self.favorites_table, "favorites")
                };
                let changed = ui::table::show(
                    ui,
                    &pal,
                    rows,
                    TableConfig {
                        columns: &mut self.settings.columns,
                        widths: &mut self.settings.column_widths,
                        sort: &mut self.settings.sort,
                        alternating: self.settings.alternating_rows,
                        id,
                    },
                    state,
                    &caps,
                    &mut out,
                );
                if changed {
                    self.settings_dirty = true;
                    self.view_dirty = true;
                }
            });

        self.show_dialogs(ctx, &pal, &mut out);

        for cmd in out {
            self.execute(cmd, frame);
        }

        // Persist window geometry and settings lazily.
        ctx.input(|i| {
            if let Some(r) = i.viewport().inner_rect {
                let size = [r.width(), r.height()];
                if self.settings.window_size != Some(size) {
                    self.settings.window_size = Some(size);
                    self.settings_dirty = true;
                }
            }
            if let Some(r) = i.viewport().outer_rect {
                let pos = [r.min.x, r.min.y];
                if self.settings.window_pos != Some(pos) {
                    self.settings.window_pos = Some(pos);
                    self.settings_dirty = true;
                }
            }
        });
        if self.settings_dirty && self.last_settings_save.elapsed().as_secs() >= 2 {
            self.save_settings();
        }
        if self.is_scanning() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        self.screenshot_step(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(s) = &self.scan {
            s.control.cancel();
        }
        for tool in &self.dialogs.tools {
            tool.stop();
        }
        self.save_settings();
        tracing::info!("exit");
    }
}
