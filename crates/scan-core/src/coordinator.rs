//! Scan coordinator: lazily walks the targets, bounds concurrency with
//! semaphores, supports pause/resume/cancel and streams results.

use crate::discovery::{IcmpOutcome, NetProbe, TcpOutcome};
use crate::events::{ScanEvent, ScanWarning};
use crate::model::{HostNames, HostRecord, HostStatus, ServiceRecord, ServiceType};
use crate::ports::{self, service_for_port, ProbeKind};
use crate::target::TargetSpec;
use fingerprint::{FingerprintEngine, FingerprintInput};
use futures::stream::{FuturesUnordered, StreamExt};
use parking_lot::Mutex;
use protocols::mdns::MdnsService;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{mpsc, watch, Semaphore};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use vendor_db::{VendorDb, VendorLookup};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ScanProfile {
    /// Discovery + MAC + DNS + few ports.
    Fast,
    /// + mDNS, typical ports, fingerprint.
    #[default]
    Standard,
    /// + more ports, all name sources, HTTP/TLS metadata, IPP, SNMP.
    Accurate,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    pub profile: ScanProfile,
    /// Ports probed on alive hosts (resolved from the profile).
    pub ports: Vec<u16>,
    pub discovery_ports: Vec<u16>,
    pub icmp_timeout_ms: u64,
    pub tcp_timeout_ms: u64,
    pub name_timeout_ms: u64,
    pub http_timeout_ms: u64,
    pub max_host_concurrency: usize,
    pub max_port_concurrency: usize,
    pub resolve_dns: bool,
    pub resolve_netbios: bool,
    pub resolve_mdns: bool,
    pub resolve_llmnr: bool,
    pub http_metadata: bool,
    pub banners: bool,
    pub ipp: bool,
    /// SNMP is only used when a community is configured explicitly.
    pub snmp_community: Option<String>,
    /// Reverse DNS for non-responding hosts ("keine Reaktion" vs "unbekannt").
    pub resolve_dead_hosts: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        ScanConfig::for_profile(ScanProfile::Standard)
    }
}

impl ScanConfig {
    pub fn for_profile(profile: ScanProfile) -> Self {
        let base = ScanConfig {
            profile,
            ports: ports::standard_ports(),
            discovery_ports: ports::DISCOVERY_PORTS.to_vec(),
            icmp_timeout_ms: 700,
            tcp_timeout_ms: 400,
            name_timeout_ms: 1000,
            http_timeout_ms: 2000,
            max_host_concurrency: 128,
            max_port_concurrency: 256,
            resolve_dns: true,
            resolve_netbios: true,
            resolve_mdns: true,
            resolve_llmnr: true,
            http_metadata: true,
            banners: true,
            ipp: true,
            snmp_community: None,
            resolve_dead_hosts: true,
        };
        match profile {
            ScanProfile::Fast => ScanConfig {
                ports: ports::fast_ports(),
                resolve_mdns: false,
                resolve_llmnr: false,
                http_metadata: false,
                banners: false,
                ipp: false,
                ..base
            },
            ScanProfile::Standard | ScanProfile::Custom => base,
            ScanProfile::Accurate => ScanConfig { ports: ports::accurate_ports(), name_timeout_ms: 1500, http_timeout_ms: 3000, ..base },
        }
    }

    fn d(ms: u64) -> Duration {
        Duration::from_millis(ms.max(1))
    }
}

/// Handle to control a running scan.
#[derive(Clone)]
pub struct ScanControl {
    cancel: CancellationToken,
    pause: Arc<watch::Sender<bool>>,
}

impl Default for ScanControl {
    fn default() -> Self {
        Self::new()
    }
}

impl ScanControl {
    pub fn new() -> Self {
        let (tx, _) = watch::channel(false);
        ScanControl { cancel: CancellationToken::new(), pause: Arc::new(tx) }
    }
    pub fn pause(&self) {
        let _ = self.pause.send_replace(true);
    }
    pub fn resume(&self) {
        let _ = self.pause.send_replace(false);
    }
    pub fn is_paused(&self) -> bool {
        *self.pause.borrow()
    }
    pub fn cancel(&self) {
        self.cancel.cancel();
        self.resume();
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

type Notify = Arc<dyn Fn() + Send + Sync>;

struct Ctx<P: NetProbe> {
    probe: Arc<P>,
    cfg: ScanConfig,
    vendors: Arc<VendorDb>,
    engine: Arc<FingerprintEngine>,
    tx: mpsc::UnboundedSender<ScanEvent>,
    notify: Notify,
    port_sem: Arc<Semaphore>,
    results: Mutex<HashMap<IpAddr, (HostRecord, usize)>>,
    icmp_warned: AtomicBool,
    cancel: CancellationToken,
}

impl<P: NetProbe> Ctx<P> {
    fn send(&self, ev: ScanEvent) {
        let _ = self.tx.send(ev);
        (self.notify)();
    }
}

pub struct Scanner<P: NetProbe> {
    probe: Arc<P>,
    vendors: Arc<VendorDb>,
    engine: Arc<FingerprintEngine>,
}

impl<P: NetProbe> Clone for Scanner<P> {
    fn clone(&self) -> Self {
        Scanner { probe: self.probe.clone(), vendors: self.vendors.clone(), engine: self.engine.clone() }
    }
}

impl<P: NetProbe> Scanner<P> {
    pub fn new(probe: Arc<P>, vendors: Arc<VendorDb>, engine: Arc<FingerprintEngine>) -> Self {
        Scanner { probe, vendors, engine }
    }

    pub fn probe(&self) -> &Arc<P> {
        &self.probe
    }

    /// Spawn a scan on `rt`; returns immediately.
    pub fn start(
        &self,
        rt: &tokio::runtime::Handle,
        spec: TargetSpec,
        cfg: ScanConfig,
        tx: mpsc::UnboundedSender<ScanEvent>,
        notify: Notify,
    ) -> ScanControl {
        let control = ScanControl::new();
        let this = self.clone();
        let c = control.clone();
        rt.spawn(async move { this.run(spec, cfg, tx, notify, c).await });
        control
    }

    /// Run a scan to completion (or cancellation).
    pub async fn run(&self, spec: TargetSpec, cfg: ScanConfig, tx: mpsc::UnboundedSender<ScanEvent>, notify: Notify, control: ScanControl) {
        let total = spec.len();
        let ctx = Arc::new(Ctx {
            probe: self.probe.clone(),
            port_sem: Arc::new(Semaphore::new(cfg.max_port_concurrency.max(1))),
            cfg,
            vendors: self.vendors.clone(),
            engine: self.engine.clone(),
            tx,
            notify,
            results: Mutex::new(HashMap::new()),
            icmp_warned: AtomicBool::new(false),
            cancel: control.cancel.clone(),
        });
        ctx.send(ScanEvent::ScanStarted { total_targets: total });
        let started = Instant::now();
        if ctx.cfg.resolve_mdns {
            if let Some(w) = ctx.probe.mdns_begin() {
                ctx.send(ScanEvent::Warning(ScanWarning::MdnsUnavailable(w)));
            }
        }

        let host_sem = Arc::new(Semaphore::new(ctx.cfg.max_host_concurrency.max(1)));
        let completed = Arc::new(AtomicU64::new(0));
        let mut pause_rx = control.pause.subscribe();
        let cancel = control.cancel.clone();
        let mut set: JoinSet<()> = JoinSet::new();

        'targets: for ip in spec.iter() {
            if *pause_rx.borrow() {
                ctx.send(ScanEvent::ScanPaused);
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => break 'targets,
                        r = pause_rx.changed() => {
                            if r.is_err() || !*pause_rx.borrow() { break; }
                        }
                    }
                }
                if cancel.is_cancelled() {
                    break;
                }
                ctx.send(ScanEvent::ScanResumed);
            }
            let permit = tokio::select! {
                _ = cancel.cancelled() => break,
                p = host_sem.clone().acquire_owned() => match p { Ok(p) => p, Err(_) => break },
            };
            let ctx2 = ctx.clone();
            let done = completed.clone();
            set.spawn(async move {
                let _permit = permit;
                tokio::select! {
                    _ = ctx2.cancel.cancelled() => {}
                    _ = scan_host(&ctx2, ip, false) => {}
                }
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                ctx2.send(ScanEvent::Progress { completed: n, total, current: Some(ip) });
            });
            while set.try_join_next().is_some() {}
        }

        loop {
            tokio::select! {
                _ = cancel.cancelled() => { set.abort_all(); break; }
                r = set.join_next() => if r.is_none() { break },
            }
        }

        if cancel.is_cancelled() {
            ctx.probe.mdns_end();
            ctx.send(ScanEvent::ScanCancelled);
            return;
        }

        if ctx.cfg.resolve_mdns {
            mdns_late_phase(&ctx, &spec, started).await;
            ctx.probe.mdns_end();
            if cancel.is_cancelled() {
                ctx.send(ScanEvent::ScanCancelled);
                return;
            }
        }
        ctx.send(ScanEvent::ScanFinished);
    }
}

/// Hosts that only announced themselves via mDNS, and late service updates.
async fn mdns_late_phase<P: NetProbe>(ctx: &Arc<Ctx<P>>, spec: &TargetSpec, started: Instant) {
    let min = Duration::from_millis(2500);
    let el = started.elapsed();
    if el < min {
        tokio::select! {
            _ = ctx.cancel.cancelled() => return,
            _ = tokio::time::sleep(min - el) => {}
        }
    }
    for ip in ctx.probe.mdns_addresses() {
        if !spec.contains(ip) || ctx.cancel.is_cancelled() {
            continue;
        }
        let known = ctx.results.lock().get(&ip).cloned();
        let services = ctx.probe.mdns_services(ip);
        match known {
            Some((mut rec, seen)) if services.len() > seen => {
                merge_mdns(&mut rec, &services);
                apply_fingerprint(ctx, &mut rec, &services, None, None, None);
                ctx.results.lock().insert(ip, (rec.clone(), services.len()));
                ctx.send(ScanEvent::HostUpdated { host: Box::new(rec) });
            }
            Some(_) => {}
            None => {
                scan_host(ctx, ip, true).await;
            }
        }
    }
}

struct TcpDiscovery {
    alive: bool,
    rtt: Option<Duration>,
    open: Vec<u16>,
}

async fn tcp_probe_ports<P: NetProbe>(ctx: &Ctx<P>, ip: IpAddr, ports: &[u16], stop_on_alive: bool) -> TcpDiscovery {
    let timeout = ScanConfig::d(ctx.cfg.tcp_timeout_ms);
    let mut futs = FuturesUnordered::new();
    for &port in ports {
        let sem = ctx.port_sem.clone();
        let probe = ctx.probe.clone();
        futs.push(async move {
            let Ok(_p) = sem.acquire_owned().await else { return (port, TcpOutcome::NoResponse) };
            (port, probe.tcp(ip, port, timeout).await)
        });
    }
    let mut res = TcpDiscovery { alive: false, rtt: None, open: Vec::new() };
    while let Some((port, outcome)) = futs.next().await {
        match outcome {
            TcpOutcome::Open(rtt) => {
                res.open.push(port);
                res.alive = true;
                res.rtt = Some(res.rtt.map_or(rtt, |r| r.min(rtt)));
            }
            TcpOutcome::Refused(rtt) => {
                res.alive = true;
                res.rtt = Some(res.rtt.map_or(rtt, |r| r.min(rtt)));
            }
            TcpOutcome::NoResponse => {}
        }
        if stop_on_alive && res.alive {
            break;
        }
    }
    res.open.sort_unstable();
    res
}

fn vendor_of(vendors: &VendorDb, rec: &mut HostRecord) {
    if let Some(mac) = &rec.mac {
        match vendors.lookup(mac) {
            VendorLookup::Vendor { name, .. } => {
                rec.vendor = Some(name);
                rec.randomized_mac = false;
            }
            VendorLookup::Randomized => {
                rec.vendor = None;
                rec.randomized_mac = true;
            }
            VendorLookup::Unknown => rec.vendor = None,
        }
    }
}

/// Scan one address. `known_alive` skips discovery (mDNS-only hosts).
async fn scan_host<P: NetProbe>(ctx: &Arc<Ctx<P>>, ip: IpAddr, known_alive: bool) {
    let probe = &ctx.probe;
    let cfg = &ctx.cfg;
    let iface = probe.interface_for(ip);
    let on_link = iface.as_ref().is_some_and(|i| i.contains(ip));
    let is_gateway = iface.as_ref().and_then(|i| i.gateway) == Some(ip);

    // 1. Host discovery: ARP/ND (via traffic) + ICMP, TCP fallback.
    if on_link {
        probe.touch(ip);
    }
    let mut latency: Option<Duration> = None;
    let mut alive = known_alive;
    match probe.icmp(ip, ScanConfig::d(cfg.icmp_timeout_ms)).await {
        IcmpOutcome::Reply(rtt) => {
            alive = true;
            latency = Some(rtt);
        }
        IcmpOutcome::NoReply => {}
        IcmpOutcome::Unavailable(_) => {
            if !ctx.icmp_warned.swap(true, Ordering::Relaxed) {
                ctx.send(ScanEvent::Warning(ScanWarning::IcmpFallback));
            }
            if on_link {
                // Give ARP time to complete before reading the cache.
                tokio::time::sleep(ScanConfig::d(cfg.icmp_timeout_ms.min(500))).await;
            }
        }
    }

    // 2. MAC address (local segment only).
    let mut mac = if on_link { probe.mac(iface.as_ref(), ip).await } else { None };
    alive |= mac.is_some();

    let mut tcp_open: Vec<u16> = Vec::new();
    // On-link hosts that did not even answer ARP are absent; skip the TCP fallback.
    let arp_says_absent = on_link && probe.arp_reliable();
    if !(alive || arp_says_absent) {
        let d = tcp_probe_ports(ctx, ip, &cfg.discovery_ports, false).await;
        alive = d.alive;
        latency = latency.or(d.rtt);
        tcp_open = d.open;
        if on_link && mac.is_none() {
            mac = probe.mac(iface.as_ref(), ip).await;
            alive |= mac.is_some();
        }
    }
    if !alive && cfg.resolve_mdns && !probe.mdns_services(ip).is_empty() {
        alive = true;
    }

    if !alive {
        if cfg.resolve_dead_hosts && cfg.resolve_dns {
            if let Some(name) = probe.reverse_dns(ip, ScanConfig::d(cfg.name_timeout_ms)).await {
                let mut rec = HostRecord::new(ip, HostStatus::NoResponse);
                rec.names.reverse_dns = Some(name.clone());
                rec.hostname = Some(name);
                rec.all_hostnames = rec.names.all();
                rec.interface = iface.as_ref().map(|i| i.display_name.clone());
                ctx.send(ScanEvent::HostUpdated { host: Box::new(rec) });
            }
        }
        return;
    }

    let now = SystemTime::now();
    let mut rec = HostRecord::new(ip, HostStatus::Scanning);
    rec.mac = mac;
    rec.latency_ms = latency.map(|d| d.as_secs_f32() * 1000.0);
    rec.is_gateway = is_gateway;
    rec.interface = iface.as_ref().map(|i| i.display_name.clone());
    rec.last_seen = Some(now);
    vendor_of(&ctx.vendors, &mut rec);
    ctx.send(ScanEvent::HostDiscovered { host: Box::new(rec.clone()) });

    // 3. Names (all sources concurrently).
    let nt = ScanConfig::d(cfg.name_timeout_ms);
    let is_v4 = ip.is_ipv4();
    let (rdns, mdns_name, llmnr, nb) = tokio::join!(
        async {
            if cfg.resolve_dns {
                probe.reverse_dns(ip, nt).await
            } else {
                None
            }
        },
        async {
            if cfg.resolve_mdns && on_link {
                probe.mdns_name(ip, nt).await
            } else {
                None
            }
        },
        async {
            if cfg.resolve_llmnr && on_link {
                probe.llmnr_name(ip, nt).await
            } else {
                None
            }
        },
        async {
            if cfg.resolve_netbios && is_v4 {
                probe.netbios(ip, nt).await
            } else {
                None
            }
        },
    );
    rec.names = HostNames {
        reverse_dns: rdns,
        mdns: mdns_name.into_iter().collect(),
        netbios: nb.as_ref().and_then(|n| n.name.clone()),
        llmnr,
        workgroup: nb.as_ref().and_then(|n| n.workgroup.clone()),
    };
    if rec.mac.is_none() {
        // NetBIOS reports the host's own adapter address, valid even when routed.
        if let Some(m) = nb.as_ref().and_then(|n| n.mac) {
            rec.mac = Some(m);
            vendor_of(&ctx.vendors, &mut rec);
        }
    }

    // 4. Ports and services.
    let remaining: Vec<u16> = cfg.ports.iter().copied().filter(|p| !tcp_open.contains(p)).collect();
    let mut open = tcp_probe_ports(ctx, ip, &remaining, false).await.open;
    open.extend(tcp_open);
    open.sort_unstable();
    open.dedup();
    rec.open_ports = open.clone();

    let (services, ipp_model, banners, http_info) = probe_services(ctx, ip, &open).await;
    rec.services = services;
    if nb.as_ref().is_some_and(|n| n.file_server) || open.contains(&445) || open.contains(&139) {
        let host = rec.names.netbios.clone().unwrap_or_else(|| ip.to_string());
        rec.services.push(
            ServiceRecord::new(ServiceType::Shares, open.iter().find(|p| **p == 445 || **p == 139).copied(), "Shares")
                .with_uri(format!("\\\\{host}\\")),
        );
    }

    // 5. mDNS services seen so far.
    let mdns = if cfg.resolve_mdns { probe.mdns_services(ip) } else { Vec::new() };
    merge_mdns(&mut rec, &mdns);

    // 6. SNMP (explicit community only).
    let mut snmp_descr = None;
    if let Some(c) = cfg.snmp_community.as_deref().filter(|c| !c.is_empty()) {
        if let Some(sys) = probe.snmp(ip, c, nt).await {
            let mut s = ServiceRecord::new(ServiceType::Snmp, Some(161), "SNMP");
            s.protocol = crate::model::TransportProtocol::Udp;
            if let Some(d) = &sys.sys_descr {
                s = s.meta("sysDescr", d.clone());
                s.display_name = format!("SNMP, {}", truncate(d, 60));
            }
            if let Some(n) = &sys.sys_name {
                s = s.meta("sysName", n.clone());
            }
            if let Some(l) = &sys.sys_location {
                s = s.meta("sysLocation", l.clone());
            }
            if let Some(o) = &sys.sys_object_id {
                s = s.meta("sysObjectID", o.clone());
            }
            snmp_descr = sys.sys_descr.clone();
            rec.services.push(s);
        }
    }

    // 7. Names: fall back to names announced via mDNS.
    rec.hostname =
        rec.names.best().or_else(|| mdns.iter().map(|s| s.hostname.trim_end_matches(".local").to_string()).find(|h| !h.is_empty()));
    rec.all_hostnames = rec.names.all();
    for s in &mdns {
        if !s.hostname.is_empty() && !rec.all_hostnames.contains(&s.hostname) {
            rec.all_hostnames.push(s.hostname.clone());
        }
    }

    // 8. Fingerprint and finish.
    let _ = http_info;
    apply_fingerprint(ctx, &mut rec, &mdns, ipp_model, Some(banners), snmp_descr);
    rec.status = HostStatus::Alive;
    ctx.results.lock().insert(ip, (rec.clone(), mdns.len()));
    ctx.send(ScanEvent::HostUpdated { host: Box::new(rec) });
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn host_for_uri(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v) => v.to_string(),
        IpAddr::V6(v) => format!("[{v}]"),
    }
}

type ServiceProbe = (Vec<ServiceRecord>, Option<String>, Vec<String>, Vec<(String, String)>);

async fn probe_services<P: NetProbe>(ctx: &Arc<Ctx<P>>, ip: IpAddr, open: &[u16]) -> ServiceProbe {
    let cfg = &ctx.cfg;
    let ht = ScanConfig::d(cfg.http_timeout_ms);
    let bt = ScanConfig::d(cfg.name_timeout_ms.max(800));
    let h = host_for_uri(ip);
    let futs = open.iter().map(|&port| {
        let probe = ctx.probe.clone();
        let h = h.clone();
        async move {
            let (stype, name, kind) = service_for_port(port);
            let label = if name.is_empty() { format!("TCP {port}") } else { name.to_string() };
            let mut s = ServiceRecord::new(stype.clone(), Some(port), label.clone());
            let mut model = None;
            let mut banner = None;
            let mut http = None;
            match stype {
                ServiceType::Http | ServiceType::HomeAssistant | ServiceType::Docker | ServiceType::Elasticsearch => {
                    s.uri = Some(if port == 80 { format!("http://{h}/") } else { format!("http://{h}:{port}/") });
                }
                ServiceType::Https => {
                    s.uri = Some(if port == 443 { format!("https://{h}/") } else { format!("https://{h}:{port}/") });
                }
                ServiceType::Ftp => s.uri = Some(format!("ftp://{h}:{port}/")),
                ServiceType::Ssh => s.uri = Some(format!("ssh://{h}:{port}")),
                ServiceType::Telnet => s.uri = Some(format!("telnet://{h}:{port}")),
                ServiceType::Rdp => s.uri = Some(format!("rdp://{h}:{port}")),
                ServiceType::Vnc => s.uri = Some(format!("vnc://{h}:{port}")),
                ServiceType::Smb => s.uri = Some(format!("smb://{h}/")),
                ServiceType::Ipp => s.uri = Some(format!("http://{h}:{port}/")),
                _ => {}
            }
            match kind {
                ProbeKind::Http | ProbeKind::Https if cfg.http_metadata || kind == ProbeKind::Https => {
                    let tls = kind == ProbeKind::Https;
                    if let Some(info) = probe.http(ip, port, tls, ht).await {
                        if let Some(st) = info.status {
                            s = s.meta("status", st.to_string());
                        }
                        if let Some(v) = &info.server {
                            s = s.meta("server", v.clone());
                        }
                        if let Some(v) = &info.location {
                            s = s.meta("location", v.clone());
                        }
                        if let Some(t) = &info.title {
                            s = s.meta("title", t.clone());
                            s.display_name = format!("{label}, {}", truncate(t, 60));
                            let tl = t.to_lowercase();
                            if tl.contains("synology") || tl.contains("diskstation") {
                                s.service_type = ServiceType::SynologyDsm;
                                s.display_name = format!("Synology DSM, {}", truncate(t, 50));
                            } else if tl.contains("home assistant") {
                                s.service_type = ServiceType::HomeAssistant;
                            }
                        }
                        if let Some(c) = &info.tls {
                            if let Some(cn) = &c.subject_cn {
                                s = s.meta("tls_cn", cn.clone());
                            }
                            if !c.san.is_empty() {
                                s = s.meta("tls_san", c.san.join(", "));
                            }
                            if let Some(i) = &c.issuer {
                                s = s.meta("tls_issuer", i.clone());
                            }
                            if let Some(v) = &c.not_before {
                                s = s.meta("tls_not_before", v.clone());
                            }
                            if let Some(v) = &c.not_after {
                                s = s.meta("tls_not_after", v.clone());
                            }
                            s = s.meta("tls_self_signed", if c.self_signed { "ja" } else { "nein" });
                        }
                        http = Some(info);
                    }
                }
                ProbeKind::Banner if cfg.banners => {
                    if let Some(b) = probe.banner(ip, port, bt).await {
                        s = s.meta("banner", b.clone());
                        if matches!(stype, ServiceType::Ftp | ServiceType::Smtp | ServiceType::MySql) {
                            let shown = b.trim_start_matches("220").trim_start_matches('-').trim();
                            s.display_name = format!("{label}, {}", truncate(shown, 60));
                        }
                        banner = Some(b);
                    }
                }
                ProbeKind::Ipp if cfg.ipp => {
                    if let Some(info) = probe.ipp(ip, port, ht).await {
                        if let Some(m) = &info.make_and_model {
                            s = s.meta("printer-make-and-model", m.clone());
                            s.display_name = format!("IPP, {m}");
                            model = Some(m.clone());
                        }
                        if let Some(v) = &info.printer_name {
                            s = s.meta("printer-name", v.clone());
                        }
                        if let Some(v) = &info.printer_location {
                            s = s.meta("printer-location", v.clone());
                        }
                        if let Some(v) = &info.printer_info {
                            s = s.meta("printer-info", v.clone());
                        }
                        if let Some(v) = &info.state {
                            s = s.meta("printer-state", v.clone());
                        }
                    }
                }
                _ => {}
            }
            (s, model, banner, http)
        }
    });
    let results = futures::future::join_all(futs).await;
    let mut services = Vec::new();
    let mut model = None;
    let mut banners = Vec::new();
    let mut http = Vec::new();
    for (s, m, b, h) in results {
        if let Some(m) = m {
            model.get_or_insert(m);
        }
        if let Some(b) = b {
            banners.push(b);
        }
        if let Some(h) = h {
            http.push((h.title.unwrap_or_default(), h.server.unwrap_or_default()));
        }
        services.push(s);
    }
    (services, model, banners, http)
}

fn merge_mdns(rec: &mut HostRecord, mdns: &[MdnsService]) {
    for svc in mdns {
        if let Some(existing) = rec.services.iter_mut().find(|s| s.port == Some(svc.port) && s.service_type != ServiceType::Shares) {
            existing.metadata.insert("mdns_type".into(), svc.service_type.clone());
            existing.metadata.insert("mdns_instance".into(), svc.instance.clone());
            continue;
        }
        if rec.services.iter().any(|s| {
            s.service_type == ServiceType::MdnsService
                && s.metadata.get("mdns_instance") == Some(&svc.instance)
                && s.metadata.get("mdns_type") == Some(&svc.service_type)
        }) {
            continue;
        }
        let h = host_for_uri(rec.ip);
        let path = svc.txt.get("path").cloned().unwrap_or_else(|| "/".into());
        let uri = match svc.service_type.as_str() {
            "_http._tcp" => Some(format!("http://{h}:{}{path}", svc.port)),
            "_https._tcp" => Some(format!("https://{h}:{}{path}", svc.port)),
            "_ssh._tcp" | "_sftp-ssh._tcp" => Some(format!("ssh://{h}:{}", svc.port)),
            "_smb._tcp" => Some(format!("smb://{h}/")),
            "_rfb._tcp" => Some(format!("vnc://{h}:{}", svc.port)),
            _ => None,
        };
        let mut s = ServiceRecord::new(
            ServiceType::MdnsService,
            (svc.port != 0).then_some(svc.port),
            format!("{} ({})", svc.instance, svc.service_type),
        )
        .meta("mdns_type", svc.service_type.clone())
        .meta("mdns_instance", svc.instance.clone());
        s.uri = uri;
        for (k, v) in &svc.txt {
            if !v.is_empty() && k.len() < 32 {
                s.metadata.insert(format!("txt:{k}"), v.clone());
            }
        }
        rec.services.push(s);
    }
}

fn apply_fingerprint<P: NetProbe>(
    ctx: &Ctx<P>,
    rec: &mut HostRecord,
    mdns: &[MdnsService],
    ipp_model: Option<String>,
    banners: Option<Vec<String>>,
    snmp_descr: Option<String>,
) {
    let mut input = FingerprintInput {
        vendor: rec.vendor.clone(),
        hostnames: rec.all_hostnames.clone(),
        open_ports: rec.open_ports.clone(),
        mdns_services: mdns.iter().map(|s| s.service_type.clone()).collect(),
        is_gateway: rec.is_gateway,
        netbios_name: rec.names.netbios.clone(),
        ..Default::default()
    };
    for s in mdns {
        for (k, v) in &s.txt {
            input.mdns_txt.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
    for s in &rec.services {
        if let Some(t) = s.metadata.get("title") {
            input.http_titles.push(t.clone());
        }
        if let Some(t) = s.metadata.get("server") {
            input.http_servers.push(t.clone());
        }
        if let Some(t) = s.metadata.get("tls_cn") {
            input.tls_subjects.push(t.clone());
        }
        if let Some(t) = s.metadata.get("banner") {
            input.banners.push(t.clone());
        }
        if let Some(t) = s.metadata.get("printer-make-and-model") {
            input.ipp_make_model.get_or_insert(t.clone());
        }
        if let Some(t) = s.metadata.get("sysDescr") {
            input.snmp_sys_descr.get_or_insert(t.clone());
        }
    }
    if let Some(m) = ipp_model {
        input.ipp_make_model = Some(m);
    }
    if let Some(b) = banners {
        for x in b {
            if !input.banners.contains(&x) {
                input.banners.push(x);
            }
        }
    }
    if snmp_descr.is_some() {
        input.snmp_sys_descr = snmp_descr;
    }
    let m = ctx.engine.identify(&input);
    rec.device_class = m.device_class;
    rec.model = m.model;
    rec.os = m.os;
    rec.fingerprint_confidence = m.confidence;
    rec.fingerprint_rule = m.rule_id;
}
