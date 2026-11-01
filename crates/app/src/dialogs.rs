//! Dialog windows. Larger dialogs are separate native windows (egui
//! viewports); small confirmations are modal overlays.

use crate::app::{App, EditKind, OptionsTab};
use egui::{Context, RichText, Ui, ViewportBuilder, ViewportClass, ViewportId};
use platform::launch::{default_template, ClientKind};
use platform::power::ShutdownOptions;
use scan_core::{ScanConfig, ScanProfile};
use storage::{AppTheme, Language};
use ui::i18n::{t, tf};
use ui::theme::ThemePalette;
use ui::UiCommand;

/// Show `content` in its own native window. Returns false once closed.
fn window(ctx: &Context, id: &str, title: &str, size: [f32; 2], pal: &ThemePalette, mut content: impl FnMut(&mut Ui, &mut bool)) -> bool {
    let mut open = true;
    let vid = ViewportId::from_hash_of(id);
    let builder = ViewportBuilder::default().with_title(title).with_inner_size(size).with_min_inner_size([300.0, 160.0]);
    ctx.show_viewport_immediate(vid, builder, |ctx, class| {
        if class == ViewportClass::Embedded {
            let mut win_open = true;
            egui::Window::new(title).id(egui::Id::new(id)).open(&mut win_open).default_size(size).show(ctx, |ui| content(ui, &mut open));
            if !win_open {
                open = false;
            }
        } else {
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(pal.panel_bg).inner_margin(egui::Margin::same(10)))
                .show(ctx, |ui| content(ui, &mut open));
            if ctx.input(|i| i.viewport().close_requested()) {
                open = false;
            }
        }
    });
    open
}

fn mono_box(ui: &mut Ui, pal: &ThemePalette, text: &str, height: f32) {
    egui::Frame::new().fill(pal.input_bg).stroke(egui::Stroke::new(1.0, pal.input_border)).inner_margin(egui::Margin::same(6)).show(
        ui,
        |ui| {
            egui::ScrollArea::vertical().max_height(height).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                ui.add(egui::Label::new(RichText::new(text).monospace()).selectable(true));
            });
        },
    );
}

fn kv(ui: &mut Ui, k: &str, v: &str) {
    if v.is_empty() {
        return;
    }
    ui.label(RichText::new(k).color(ui.visuals().weak_text_color()));
    ui.add(egui::Label::new(v).selectable(true).wrap());
    ui.end_row();
}

impl App {
    pub fn show_dialogs(&mut self, ctx: &Context, pal: &ThemePalette, out: &mut Vec<UiCommand>) {
        self.tool_windows(ctx, pal);
        self.options_window(ctx, pal);
        self.info_windows(ctx, pal, out);
        self.modals(ctx);
    }

    fn tool_windows(&mut self, ctx: &Context, pal: &ThemePalette) {
        let mut keep = Vec::new();
        let tools = std::mem::take(&mut self.dialogs.tools);
        for s in tools {
            let text = s.output.lock().lines.join("\n");
            let running = s.output.lock().running;
            let mut copy = None;
            let open = window(ctx, &format!("tool-{}", s.id), &s.title, [480.0, 320.0], pal, |ui, open| {
                ui.horizontal(|ui| {
                    if running {
                        if ui.button(t("dlg.stop")).clicked() {
                            s.stop();
                        }
                        ui.spinner();
                    }
                    if ui.button(t("dlg.copy_clip")).clicked() {
                        copy = Some(text.clone());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t("dlg.close")).clicked() {
                            *open = false;
                        }
                    });
                });
                ui.add_space(4.0);
                mono_box(ui, pal, &text, ui.available_height() - 14.0);
            });
            if let Some(c) = copy {
                ctx.copy_text(c);
            }
            if open {
                keep.push(s);
            } else {
                s.stop();
            }
        }
        self.dialogs.tools = keep;
    }

    fn options_window(&mut self, ctx: &Context, pal: &ThemePalette) {
        let Some(mut dlg) = self.dialogs.options.take() else { return };
        let interfaces = platform::interfaces::list_interfaces();
        let mut apply = false;
        let open = window(ctx, "options", t("opt.title"), [560.0, 440.0], pal, |ui, open| {
            ui.horizontal(|ui| {
                for (tab, key) in [
                    (OptionsTab::Scan, "opt.tab_scan"),
                    (OptionsTab::Performance, "opt.tab_perf"),
                    (OptionsTab::Names, "opt.tab_names"),
                    (OptionsTab::Programs, "opt.tab_programs"),
                    (OptionsTab::Interface, "opt.tab_ui"),
                    (OptionsTab::Network, "opt.tab_net"),
                ] {
                    ui.selectable_value(&mut dlg.tab, tab, t(key));
                }
            });
            ui.separator();
            let d = &mut dlg.draft;
            egui::ScrollArea::vertical().max_height(ui.available_height() - 40.0).show(ui, |ui| match dlg.tab {
                OptionsTab::Scan => {
                    egui::Grid::new("opt_scan").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label(t("opt.profile"));
                        egui::ComboBox::from_id_salt("profile")
                            .selected_text(match d.scan.profile {
                                ScanProfile::Fast => t("opt.profile_fast"),
                                ScanProfile::Standard => t("opt.profile_standard"),
                                ScanProfile::Accurate => t("opt.profile_accurate"),
                                ScanProfile::Custom => t("opt.profile_custom"),
                            })
                            .show_ui(ui, |ui| {
                                for (p, k) in [
                                    (ScanProfile::Fast, "opt.profile_fast"),
                                    (ScanProfile::Standard, "opt.profile_standard"),
                                    (ScanProfile::Accurate, "opt.profile_accurate"),
                                    (ScanProfile::Custom, "opt.profile_custom"),
                                ] {
                                    if ui.selectable_label(d.scan.profile == p, t(k)).clicked() {
                                        let keep = d.scan.clone();
                                        if p == ScanProfile::Custom {
                                            d.scan = ScanConfig { profile: ScanProfile::Custom, ..keep };
                                        } else {
                                            d.scan = ScanConfig {
                                                max_host_concurrency: keep.max_host_concurrency,
                                                max_port_concurrency: keep.max_port_concurrency,
                                                snmp_community: keep.snmp_community,
                                                ..ScanConfig::for_profile(p)
                                            };
                                        }
                                    }
                                }
                            });
                        ui.end_row();
                        ui.label(t("opt.ports"));
                        if d.scan.profile == ScanProfile::Custom {
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut d.custom_ports).desired_width(320.0).hint_text("22, 80, 443, 8000-8100"),
                            );
                            if r.changed() {
                                dlg.ports_error = scan_core::ports::parse_port_list(&d.custom_ports).err();
                            }
                        } else {
                            let ports = match d.scan.profile {
                                ScanProfile::Fast => scan_core::ports::fast_ports(),
                                ScanProfile::Accurate => scan_core::ports::accurate_ports(),
                                _ => scan_core::ports::standard_ports(),
                            };
                            let s: Vec<String> = ports.iter().map(u16::to_string).collect();
                            ui.add(egui::Label::new(RichText::new(s.join(", ")).small()).wrap());
                        }
                        ui.end_row();
                        if let Some(e) = &dlg.ports_error {
                            ui.label("");
                            ui.colored_label(ui.visuals().error_fg_color, e);
                            ui.end_row();
                        }
                        ui.label(t("opt.icmp_timeout"));
                        egui::ComboBox::from_id_salt("icmp_to").selected_text(format!("{} ms", d.scan.icmp_timeout_ms)).show_ui(ui, |ui| {
                            for v in [300u64, 500, 700, 1000, 2000] {
                                ui.selectable_value(&mut d.scan.icmp_timeout_ms, v, format!("{v} ms"));
                            }
                        });
                        ui.end_row();
                        ui.label(t("opt.tcp_timeout"));
                        ui.add(egui::DragValue::new(&mut d.scan.tcp_timeout_ms).range(100..=5000).speed(10));
                        ui.end_row();
                        ui.label(t("opt.http_timeout"));
                        ui.add(egui::DragValue::new(&mut d.scan.http_timeout_ms).range(300..=10000).speed(10));
                        ui.end_row();
                    });
                    ui.add_space(6.0);
                    ui.checkbox(&mut d.scan.http_metadata, t("opt.http_meta"));
                    ui.checkbox(&mut d.scan.banners, t("opt.banners"));
                    ui.checkbox(&mut d.scan.ipp, t("opt.ipp"));
                    ui.checkbox(&mut d.clear_on_scan, t("opt.clear"));
                }
                OptionsTab::Performance => {
                    egui::Grid::new("opt_perf").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label(t("opt.hosts"));
                        ui.add(egui::DragValue::new(&mut d.scan.max_host_concurrency).range(1..=1024));
                        ui.end_row();
                        ui.label(t("opt.portconc"));
                        ui.add(egui::DragValue::new(&mut d.scan.max_port_concurrency).range(1..=4096));
                        ui.end_row();
                    });
                }
                OptionsTab::Names => {
                    ui.checkbox(&mut d.scan.resolve_dns, t("opt.dns"));
                    ui.checkbox(&mut d.scan.resolve_mdns, t("opt.mdns"));
                    ui.checkbox(&mut d.scan.resolve_netbios, t("opt.netbios"));
                    ui.checkbox(&mut d.scan.resolve_llmnr, t("opt.llmnr"));
                    ui.checkbox(&mut d.scan.resolve_dead_hosts, t("opt.dead"));
                    ui.add_space(6.0);
                    egui::Grid::new("opt_names").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label(t("opt.name_timeout"));
                        ui.add(egui::DragValue::new(&mut d.scan.name_timeout_ms).range(200..=5000).speed(10));
                        ui.end_row();
                        ui.label(t("opt.snmp"));
                        let mut c = d.scan.snmp_community.clone().unwrap_or_default();
                        if ui.add(egui::TextEdit::singleline(&mut c).desired_width(160.0)).changed() {
                            d.scan.snmp_community = (!c.is_empty()).then_some(c);
                        }
                        ui.end_row();
                    });
                    ui.weak(t("opt.snmp_hint"));
                }
                OptionsTab::Programs => {
                    ui.weak(t("opt.programs_hint"));
                    ui.add_space(6.0);
                    egui::Grid::new("opt_prog").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                        for k in ClientKind::ALL {
                            ui.label(k.label());
                            let mut v = d.client_templates.get(&k).cloned().unwrap_or_default();
                            let hint = default_template(k);
                            if ui
                                .add(egui::TextEdit::singleline(&mut v).desired_width(360.0).hint_text(if hint.is_empty() {
                                    "(System)"
                                } else {
                                    hint
                                }))
                                .changed()
                            {
                                if v.trim().is_empty() {
                                    d.client_templates.remove(&k);
                                } else {
                                    d.client_templates.insert(k, v);
                                }
                            }
                            ui.end_row();
                        }
                    });
                }
                OptionsTab::Interface => {
                    egui::Grid::new("opt_ui").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                        ui.label(t("opt.theme"));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut d.theme, AppTheme::Light, t("opt.theme_light"));
                            ui.radio_value(&mut d.theme, AppTheme::Dark, t("opt.theme_dark"));
                            ui.radio_value(&mut d.theme, AppTheme::System, t("opt.theme_system"));
                        });
                        ui.end_row();
                        ui.label(t("settings.language"));
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut d.language, Language::System, t("lang.system"));
                            ui.radio_value(&mut d.language, Language::German, t("lang.de"));
                            ui.radio_value(&mut d.language, Language::English, t("lang.en"));
                        });
                        ui.end_row();
                        ui.label(t("opt.wol_port"));
                        ui.add(egui::DragValue::new(&mut d.wol_port).range(1..=65535));
                        ui.end_row();
                    });
                    ui.checkbox(&mut d.alternating_rows, t("view.alternating"));
                    ui.checkbox(&mut d.csv_bom, t("opt.csv_bom"));
                    ui.checkbox(&mut d.toolbar_ip_c, t("view.toolbar_ipc"));
                    ui.checkbox(&mut d.toolbar_expand, t("view.toolbar_expand"));
                }
                OptionsTab::Network => {
                    ui.label(t("opt.interface"));
                    let current = d.last_interface.clone();
                    let label = |i: &platform::NetworkInterface| {
                        let ip = i.primary_ipv4().map(|(a, p)| format!("{a}/{p}")).unwrap_or_default();
                        let tag = i.virtual_tag.as_ref().map(|t| format!(" [{t}]")).unwrap_or_default();
                        format!("{} ({:?}) {ip}{tag}", i.display_name, i.kind)
                    };
                    let sel = current
                        .as_ref()
                        .and_then(|n| interfaces.iter().find(|i| &i.name == n))
                        .map(label)
                        .unwrap_or_else(|| t("opt.auto").to_string());
                    egui::ComboBox::from_id_salt("iface").width(420.0).selected_text(sel).show_ui(ui, |ui| {
                        if ui.selectable_label(current.is_none(), t("opt.auto")).clicked() {
                            d.last_interface = None;
                        }
                        for i in interfaces.iter().filter(|i| i.kind != platform::InterfaceKind::Loopback && !i.ipv4.is_empty()) {
                            if d.hide_virtual_interfaces && i.virtual_tag.is_some() {
                                continue;
                            }
                            if ui.selectable_label(current.as_deref() == Some(i.name.as_str()), label(i)).clicked() {
                                d.last_interface = Some(i.name.clone());
                            }
                        }
                    });
                    ui.checkbox(&mut d.hide_virtual_interfaces, t("opt.hide_virtual"));
                    ui.add_space(8.0);
                    egui::Grid::new("iface_details").num_columns(5).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
                        for h in ["Name", "IPv4", "Netmask", "Gateway", "MAC"] {
                            ui.strong(h);
                        }
                        ui.end_row();
                        for i in interfaces.iter().filter(|i| i.is_up && i.kind != platform::InterfaceKind::Loopback && !i.ipv4.is_empty())
                        {
                            ui.label(&i.display_name);
                            ui.label(i.primary_ipv4().map(|(a, _)| a.to_string()).unwrap_or_default());
                            ui.label(i.netmask().map(|m| m.to_string()).unwrap_or_default());
                            ui.label(i.gateway.map(|g| g.to_string()).unwrap_or_default());
                            ui.label(i.mac.map(|m| m.to_string()).unwrap_or_default());
                            ui.end_row();
                        }
                    });
                }
            });
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Max), |ui| {
                ui.horizontal(|ui| {
                    if ui.button(t("dlg.cancel")).clicked() {
                        *open = false;
                    }
                    if ui.add_enabled(dlg.ports_error.is_none(), egui::Button::new(t("dlg.ok"))).clicked() {
                        apply = true;
                        *open = false;
                    }
                });
            });
        });
        if apply {
            self.apply_options(dlg.draft.clone());
        }
        if open {
            self.dialogs.options = Some(dlg);
        }
    }

    fn info_windows(&mut self, ctx: &Context, pal: &ThemePalette, out: &mut Vec<UiCommand>) {
        if self.dialogs.about {
            let info = self.vendors.info().clone();
            self.dialogs.about = window(ctx, "about", t("about.title"), [420.0, 260.0], pal, |ui, open| {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
                    ui::icons::computer(ui.painter(), r, pal, false);
                    ui.vertical(|ui| {
                        ui.heading(t("app.title"));
                        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    });
                });
                ui.add_space(8.0);
                ui.label(t("about.text"));
                ui.add_space(6.0);
                ui.weak(t("about.oui"));
                ui.weak(format!("MAC-DB: {} ({} Einträge)", info.version, info.total()));
                ui.add_space(8.0);
                if ui.button(t("dlg.close")).clicked() {
                    *open = false;
                }
            });
        }
        if let Some(text) = self.dialogs.diagnose.clone() {
            let mut copy = false;
            let open = window(ctx, "diagnose", t("diag.title"), [560.0, 460.0], pal, |ui, open| {
                ui.horizontal(|ui| {
                    if ui.button("Diagnose in Zwischenablage kopieren").clicked() {
                        copy = true;
                    }
                    if ui.button(t("dlg.close")).clicked() {
                        *open = false;
                    }
                });
                ui.add_space(4.0);
                mono_box(ui, pal, &text, ui.available_height() - 14.0);
            });
            if copy {
                ctx.copy_text(text);
            }
            if !open {
                self.dialogs.diagnose = None;
            }
        }
        if self.dialogs.macdb {
            let info = self.vendors.info().clone();
            let src = self.vendor_source.clone();
            let mut import = false;
            let mut reset = false;
            let lookup = self.dialogs.macdb_lookup.clone();
            let mut new_lookup = lookup.clone();
            let result = vendor_db::MacAddr::parse(&lookup).ok().map(|m| match self.vendors.lookup(&m) {
                vendor_db::VendorLookup::Vendor { name, prefix_bits, address } => {
                    format!("{name} ({prefix_bits} Bit){}", address.map(|a| format!("\n{a}")).unwrap_or_default())
                }
                vendor_db::VendorLookup::Randomized => t("vendor.randomized_long").to_string(),
                vendor_db::VendorLookup::Unknown => t("unknown").to_string(),
            });
            self.dialogs.macdb = window(ctx, "macdb", t("macdb.title"), [480.0, 300.0], pal, |ui, open| {
                egui::Grid::new("macdb").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
                    kv(ui, t("macdb.version"), &info.version);
                    kv(ui, t("macdb.source"), &format!("{} [{src}]", info.source));
                    kv(ui, t("macdb.entries"), &format!("{} / {} / {}", info.ma_l, info.ma_m, info.ma_s));
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(t("macdb.lookup"));
                    ui.add(egui::TextEdit::singleline(&mut new_lookup).desired_width(160.0).hint_text("AA:BB:CC:DD:EE:FF"));
                });
                if let Some(r) = &result {
                    ui.label(RichText::new(r).strong());
                }
                ui.add_space(8.0);
                ui.weak(t("macdb.hint"));
                ui.horizontal(|ui| {
                    if ui.button(t("macdb.import")).clicked() {
                        import = true;
                    }
                    if ui.add_enabled(src != "bundled", egui::Button::new(t("macdb.reset"))).clicked() {
                        reset = true;
                    }
                    if ui.button(t("dlg.close")).clicked() {
                        *open = false;
                    }
                });
            });
            self.dialogs.macdb_lookup = new_lookup;
            if import {
                self.import_oui();
            }
            if reset {
                self.reset_oui();
            }
        }
        if self.dialogs.fingerprints {
            let rules: Vec<(String, String, Option<String>)> = self
                .engine
                .rules()
                .map(|r| (r.id.clone(), t(&format!("class.{}", r.device_class.key())).to_string(), r.model_hint.clone()))
                .collect();
            let file = self.paths.fingerprint_file();
            let mut reload = false;
            let mut open_dir = false;
            self.dialogs.fingerprints = window(ctx, "fingerprints", t("fp.title"), [480.0, 400.0], pal, |ui, open| {
                ui.label(tf("fp.count", &[("n", &rules.len().to_string())]));
                ui.horizontal(|ui| {
                    ui.label(t("fp.user_file"));
                    ui.add(egui::Label::new(RichText::new(file.display().to_string()).small()).selectable(true));
                });
                ui.horizontal(|ui| {
                    if ui.button(t("fp.reload")).clicked() {
                        reload = true;
                    }
                    if ui.button(t("fp.open_folder")).clicked() {
                        open_dir = true;
                    }
                    if ui.button(t("dlg.close")).clicked() {
                        *open = false;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("fp_rules").num_columns(3).striped(true).show(ui, |ui| {
                        for (id, class, model) in &rules {
                            ui.monospace(id);
                            ui.label(class);
                            ui.weak(model.clone().unwrap_or_default());
                            ui.end_row();
                        }
                    });
                });
            });
            if reload {
                self.reload_fingerprints();
            }
            if open_dir {
                let _ = std::fs::create_dir_all(&self.paths.settings_dir);
                let _ = open::that_detached(&self.paths.settings_dir);
            }
        }
        if let Some(ip) = self.dialogs.port_info {
            let host = self.host(ip);
            let open = window(ctx, "portinfo", &format!("{} – {ip}", t("ports.title")), [520.0, 360.0], pal, |ui, open| {
                match &host {
                    Some(h) if !h.services.is_empty() => {
                        egui::ScrollArea::vertical().max_height(ui.available_height() - 36.0).auto_shrink([false, false]).show(ui, |ui| {
                            for s in &h.services {
                                let port = s
                                    .port
                                    .map(|p| format!("{p}/{}", if s.protocol == scan_core::TransportProtocol::Udp { "udp" } else { "tcp" }))
                                    .unwrap_or_default();
                                egui::CollapsingHeader::new(format!("{port:<10} {}", s.display_name))
                                    .id_salt((&s.display_name, s.port))
                                    .show(ui, |ui| {
                                        egui::Grid::new((&s.display_name, s.port, "m")).num_columns(2).show(ui, |ui| {
                                            if let Some(u) = &s.uri {
                                                kv(ui, "URI", u);
                                            }
                                            for (k, v) in &s.metadata {
                                                kv(ui, k, v);
                                            }
                                        });
                                    });
                            }
                        });
                    }
                    _ => {
                        ui.label(t("ports.none"));
                    }
                }
                if ui.button(t("dlg.close")).clicked() {
                    *open = false;
                }
            });
            if !open {
                self.dialogs.port_info = None;
            }
        }
        if let Some(ip) = self.dialogs.properties {
            let host = self.host(ip);
            let name = host.as_ref().map(|h| self.data.display_name(h)).unwrap_or_default();
            let first_seen = self.data.first_seen.get(&ip).copied();
            let mut cmd = None;
            let open = window(ctx, "properties", &format!("{} – {name}", t("det.title")), [520.0, 440.0], pal, |ui, open| {
                let Some(h) = &host else {
                    *open = false;
                    return;
                };
                egui::ScrollArea::vertical().max_height(ui.available_height() - 36.0).auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("props").num_columns(2).spacing([14.0, 4.0]).show(ui, |ui| {
                        kv(ui, t("col.name"), &name);
                        kv(ui, t("col.ip"), &h.ip.to_string());
                        kv(ui, t("col.mac"), &h.mac.map(|m| m.to_string()).unwrap_or_default());
                        kv(ui, t("col.vendor"), &crate::state::vendor_label(h));
                        if h.randomized_mac {
                            kv(ui, "", t("vendor.randomized_long"));
                        }
                        kv(ui, t("col.device_type"), &crate::state::class_label(h));
                        kv(ui, t("col.model"), &h.model.clone().unwrap_or_else(|| t("unknown").to_string()));
                        kv(ui, t("col.confidence"), &format!("{} %", h.fingerprint_confidence));
                        kv(ui, t("col.os"), &h.os.clone().unwrap_or_default());
                        kv(ui, t("col.latency"), &h.latency_ms.map(|l| format!("{l:.1} ms")).unwrap_or_default());
                        kv(ui, t("col.interface"), &h.interface.clone().unwrap_or_default());
                        kv(ui, t("col.comment"), &h.comment);
                        kv(ui, t("det.first_seen"), &crate::state::fmt_time(first_seen));
                        kv(ui, t("col.last_seen"), &crate::state::fmt_time(h.last_seen));
                        kv(ui, "DNS", &h.names.reverse_dns.clone().unwrap_or_default());
                        kv(ui, "mDNS", &h.names.mdns.join(", "));
                        kv(ui, "NetBIOS", &h.names.netbios.clone().unwrap_or_default());
                        kv(ui, "Workgroup", &h.names.workgroup.clone().unwrap_or_default());
                        kv(ui, "LLMNR", &h.names.llmnr.clone().unwrap_or_default());
                        kv(ui, "Fingerprint", &h.fingerprint_rule.clone().unwrap_or_default());
                    });
                    ui.add_space(6.0);
                    ui.strong(t("det.services"));
                    for s in &h.services {
                        ui.label(format!("• {}{}", s.display_name, s.port.map(|p| format!(" ({p})")).unwrap_or_default()));
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(t("ctx.portinfo")).clicked() {
                        cmd = Some(UiCommand::PortInfo(ip));
                    }
                    if ui.button(t("dlg.close")).clicked() {
                        *open = false;
                    }
                });
            });
            if let Some(c) = cmd {
                out.push(c);
            }
            if !open {
                self.dialogs.properties = None;
            }
        }
    }

    fn modals(&mut self, ctx: &Context) {
        if let Some(mut e) = self.dialogs.edit.take() {
            let mut done = None;
            egui::Modal::new(egui::Id::new("edit_modal")).show(ctx, |ui| {
                ui.set_width(320.0);
                ui.heading(match e.kind {
                    EditKind::Alias => t("edit.name_title"),
                    EditKind::Comment => t("edit.comment_title"),
                });
                ui.label(match e.kind {
                    EditKind::Alias => t("edit.name_label"),
                    EditKind::Comment => t("edit.comment_label"),
                });
                let r = ui.add(egui::TextEdit::singleline(&mut e.text).desired_width(300.0));
                if e.focus {
                    r.request_focus();
                    e.focus = false;
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.horizontal(|ui| {
                    if ui.button(t("dlg.ok")).clicked() || enter {
                        done = Some(true);
                    }
                    if ui.button(t("dlg.cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        done = Some(false);
                    }
                });
            });
            match done {
                Some(true) => {
                    let text = e.text.trim().to_string();
                    let store = self.store.as_ref();
                    match e.kind {
                        EditKind::Alias => self.data.set_annotation(e.ip, store, |a| a.alias = (!text.is_empty()).then_some(text)),
                        EditKind::Comment => self.data.set_annotation(e.ip, store, |a| a.comment = text),
                    }
                }
                Some(false) => {}
                None => self.dialogs.edit = Some(e),
            }
        }
        if let Some(mut p) = self.dialogs.power.take() {
            let name = self.host(p.ip).map(|h| self.data.display_name(&h)).unwrap_or_else(|| p.ip.to_string());
            let mut done = None;
            egui::Modal::new(egui::Id::new("power_modal")).show(ctx, |ui| {
                ui.set_width(360.0);
                ui.heading(if p.reboot { t("ctx.reboot") } else { t("ctx.shutdown") });
                ui.label(tf(if p.reboot { "power.confirm_reboot" } else { "power.confirm_shutdown" }, &[("host", &name)]));
                ui.add_space(4.0);
                egui::Grid::new("pw").num_columns(2).show(ui, |ui| {
                    ui.label(t("power.message"));
                    ui.add(egui::TextEdit::singleline(&mut p.message).desired_width(200.0));
                    ui.end_row();
                    ui.label(t("power.timeout"));
                    ui.add(egui::DragValue::new(&mut p.timeout).range(0..=600));
                    ui.end_row();
                });
                ui.checkbox(&mut p.force, t("power.force"));
                ui.horizontal(|ui| {
                    if ui.button(t("dlg.ok")).clicked() {
                        done = Some(true);
                    }
                    if ui.button(t("dlg.cancel")).clicked() {
                        done = Some(false);
                    }
                });
            });
            match done {
                Some(true) => {
                    let opts = ShutdownOptions { message: p.message.clone(), timeout_secs: p.timeout, force: p.force };
                    self.run_power(p.ip, p.reboot, opts);
                }
                Some(false) => {}
                None => self.dialogs.power = Some(p),
            }
        }
    }
}
