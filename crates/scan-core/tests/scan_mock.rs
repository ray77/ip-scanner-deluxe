//! Scan engine against a mock network (no real LAN required).

use fingerprint::{DeviceClass, FingerprintEngine};
use platform::interfaces::{InterfaceKind, NetworkInterface};
use protocols::http::HttpInfo;
use protocols::ipp::IppInfo;
use protocols::mdns::MdnsService;
use protocols::netbios::NetbiosInfo;
use protocols::snmp::SnmpSystem;
use scan_core::discovery::IcmpOutcome;
use scan_core::{parse_targets, HostRecord, HostStatus, NetProbe, ScanConfig, ScanControl, ScanEvent, Scanner, ServiceType, TcpOutcome};
use std::collections::HashMap;
use std::future::Future;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use vendor_db::import::OuiRecord;
use vendor_db::{MacAddr, VendorDb};

#[derive(Default, Clone)]
struct MockHost {
    icmp: bool,
    ports: Vec<u16>,
    closed_ports_respond: bool,
    mac: Option<MacAddr>,
    rdns: Option<String>,
    netbios: Option<NetbiosInfo>,
    http: HashMap<u16, HttpInfo>,
    ipp: Option<IppInfo>,
    mdns: Vec<MdnsService>,
}

struct MockNet {
    hosts: HashMap<IpAddr, MockHost>,
    dead_rdns: HashMap<IpAddr, String>,
    arp_reliable: bool,
    delay: Duration,
    icmp_calls: AtomicUsize,
}

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

fn lan() -> NetworkInterface {
    NetworkInterface {
        index: 1,
        name: "eth0".into(),
        display_name: "Ethernet".into(),
        kind: InterfaceKind::Ethernet,
        virtual_tag: None,
        ipv4: vec![("192.168.0.50".parse().unwrap(), 24)],
        ipv6: vec![],
        mac: Some(MacAddr::parse("00:11:22:00:00:50").unwrap()),
        gateway: Some(ip("192.168.0.1")),
        gateway_mac: None,
        is_up: true,
        is_default: true,
    }
}

impl NetProbe for MockNet {
    fn interface_for(&self, _ip: IpAddr) -> Option<NetworkInterface> {
        Some(lan())
    }
    fn icmp(&self, ip: IpAddr, _t: Duration) -> impl Future<Output = IcmpOutcome> + Send {
        self.icmp_calls.fetch_add(1, Ordering::Relaxed);
        let r = self.hosts.get(&ip).is_some_and(|h| h.icmp);
        let d = self.delay;
        async move {
            tokio::time::sleep(d).await;
            if r {
                IcmpOutcome::Reply(Duration::from_micros(1500))
            } else {
                IcmpOutcome::NoReply
            }
        }
    }
    fn tcp(&self, ip: IpAddr, port: u16, _t: Duration) -> impl Future<Output = TcpOutcome> + Send {
        let r = match self.hosts.get(&ip) {
            Some(h) if h.ports.contains(&port) => TcpOutcome::Open(Duration::from_millis(2)),
            Some(h) if h.closed_ports_respond => TcpOutcome::Refused(Duration::from_millis(2)),
            _ => TcpOutcome::NoResponse,
        };
        async move { r }
    }
    fn mac(&self, iface: Option<&NetworkInterface>, ip: IpAddr) -> impl Future<Output = Option<MacAddr>> + Send {
        // Like the real resolver: only on-link targets.
        let on_link = iface.is_some_and(|i| i.contains(ip));
        let m = if on_link { self.hosts.get(&ip).and_then(|h| h.mac) } else { None };
        async move { m }
    }
    fn reverse_dns(&self, ip: IpAddr, _t: Duration) -> impl Future<Output = Option<String>> + Send {
        let r = self.hosts.get(&ip).and_then(|h| h.rdns.clone()).or_else(|| self.dead_rdns.get(&ip).cloned());
        async move { r }
    }
    async fn mdns_name(&self, _ip: IpAddr, _t: Duration) -> Option<String> {
        None
    }
    async fn llmnr_name(&self, _ip: IpAddr, _t: Duration) -> Option<String> {
        None
    }
    fn netbios(&self, ip: IpAddr, _t: Duration) -> impl Future<Output = Option<NetbiosInfo>> + Send {
        let r = self.hosts.get(&ip).and_then(|h| h.netbios.clone());
        async move { r }
    }
    fn http(&self, ip: IpAddr, port: u16, _tls: bool, _t: Duration) -> impl Future<Output = Option<HttpInfo>> + Send {
        let r = self.hosts.get(&ip).and_then(|h| h.http.get(&port).cloned());
        async move { r }
    }
    fn banner(&self, _ip: IpAddr, port: u16, _t: Duration) -> impl Future<Output = Option<String>> + Send {
        let r = (port == 22).then(|| "SSH-2.0-OpenSSH_8.4p1 Raspbian-5".to_string());
        async move { r }
    }
    fn ipp(&self, ip: IpAddr, _port: u16, _t: Duration) -> impl Future<Output = Option<IppInfo>> + Send {
        let r = self.hosts.get(&ip).and_then(|h| h.ipp.clone());
        async move { r }
    }
    async fn snmp(&self, _ip: IpAddr, _c: &str, _t: Duration) -> Option<SnmpSystem> {
        None
    }
    fn touch(&self, _ip: IpAddr) {}
    fn arp_reliable(&self) -> bool {
        self.arp_reliable
    }
    fn mdns_begin(&self) -> Option<String> {
        None
    }
    fn mdns_services(&self, ip: IpAddr) -> Vec<MdnsService> {
        self.hosts.get(&ip).map(|h| h.mdns.clone()).unwrap_or_default()
    }
    fn mdns_addresses(&self) -> Vec<IpAddr> {
        self.hosts.iter().filter(|(_, h)| !h.mdns.is_empty()).map(|(i, _)| *i).collect()
    }
    fn mdns_end(&self) {}
}

fn vendors() -> Arc<VendorDb> {
    let rec = |p: u64, o: &str| OuiRecord { prefix_bits: 24, prefix: p, organization: o.into(), address: None, country: None };
    Arc::new(
        VendorDb::from_records(
            &[rec(0x3C5282, "Hewlett Packard"), rec(0xB827EB, "Raspberry Pi Foundation"), rec(0x001FD0, "GIGA-BYTE TECHNOLOGY CO.,LTD.")],
            "test",
        )
        .unwrap(),
    )
}

fn network(arp_reliable: bool) -> MockNet {
    let mut hosts = HashMap::new();
    hosts.insert(
        ip("192.168.0.1"),
        MockHost { icmp: true, ports: vec![80], mac: MacAddr::parse("C8:0E:14:00:00:01").ok(), ..Default::default() },
    );
    let mut printer_http = HashMap::new();
    printer_http.insert(80, HttpInfo { status: Some(200), title: Some("HP Color LaserJet 9500".into()), ..Default::default() });
    hosts.insert(
        ip("192.168.0.96"),
        MockHost {
            icmp: true,
            ports: vec![80, 631, 9100],
            mac: MacAddr::parse("3C:52:82:11:22:33").ok(),
            rdns: Some("PC01.fritz.box".into()),
            http: printer_http,
            ipp: Some(IppInfo { make_and_model: Some("HP Color LaserJet 9500".into()), ..Default::default() }),
            ..Default::default()
        },
    );
    // ICMP blocked, only answers ARP + one TCP port.
    hosts.insert(
        ip("192.168.0.110"),
        MockHost {
            icmp: false,
            ports: vec![445, 3389],
            closed_ports_respond: true,
            mac: MacAddr::parse("00:1F:D0:12:34:56").ok(),
            netbios: Some(NetbiosInfo {
                name: Some("PATRICK".into()),
                workgroup: Some("WORKGROUP".into()),
                file_server: true,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    // Randomized MAC phone.
    hosts.insert(ip("192.168.0.120"), MockHost { icmp: true, mac: MacAddr::parse("DA:A1:19:00:00:01").ok(), ..Default::default() });
    // Raspberry Pi with SSH.
    hosts.insert(
        ip("192.168.0.130"),
        MockHost {
            icmp: true,
            ports: vec![22],
            mac: MacAddr::parse("B8:27:EB:00:00:01").ok(),
            rdns: Some("raspberrypi.fritz.box".into()),
            ..Default::default()
        },
    );
    // Routed host (another subnet): MAC must stay unknown.
    hosts.insert(
        ip("10.1.0.5"),
        MockHost { icmp: true, ports: vec![22], mac: MacAddr::parse("C8:0E:14:00:00:01").ok(), ..Default::default() },
    );
    let mut dead_rdns = HashMap::new();
    dead_rdns.insert(ip("192.168.0.200"), "old-laptop.fritz.box".to_string());
    MockNet { hosts, dead_rdns, arp_reliable, delay: Duration::ZERO, icmp_calls: AtomicUsize::new(0) }
}

fn config() -> ScanConfig {
    ScanConfig { resolve_mdns: false, ..Default::default() }
}

async fn run(net: MockNet, range: &str, cfg: ScanConfig) -> Vec<ScanEvent> {
    let scanner = Scanner::new(Arc::new(net), vendors(), Arc::new(FingerprintEngine::with_builtin()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    scanner.run(parse_targets(range).unwrap(), cfg, tx, Arc::new(|| {}), ScanControl::new()).await;
    let mut events = Vec::new();
    while let Ok(e) = rx.try_recv() {
        events.push(e);
    }
    events
}

fn finals(events: &[ScanEvent]) -> HashMap<IpAddr, HostRecord> {
    let mut m = HashMap::new();
    for e in events {
        if let ScanEvent::HostUpdated { host } = e {
            m.insert(host.ip, (**host).clone());
        }
    }
    m
}

#[tokio::test]
async fn scans_subnet_and_streams_results() {
    let events = run(network(true), "192.168.0.1-254", config()).await;
    assert!(matches!(events.first(), Some(ScanEvent::ScanStarted { total_targets: 254 })));
    assert!(matches!(events.last(), Some(ScanEvent::ScanFinished)));
    let progress = events.iter().filter(|e| matches!(e, ScanEvent::Progress { .. })).count();
    assert_eq!(progress, 254);

    // Discovered (spinner) comes before the final update.
    let pos_disc = events.iter().position(|e| matches!(e, ScanEvent::HostDiscovered { host } if host.ip == ip("192.168.0.96"))).unwrap();
    let pos_upd = events.iter().position(|e| matches!(e, ScanEvent::HostUpdated { host } if host.ip == ip("192.168.0.96"))).unwrap();
    assert!(pos_disc < pos_upd);

    let f = finals(&events);
    let alive: Vec<_> = f.values().filter(|h| h.status == HostStatus::Alive).collect();
    assert_eq!(alive.len(), 5, "{:?}", f.keys().collect::<Vec<_>>());

    let printer = &f[&ip("192.168.0.96")];
    assert_eq!(printer.vendor.as_deref(), Some("Hewlett Packard"));
    assert_eq!(printer.hostname.as_deref(), Some("PC01.fritz.box"));
    assert_eq!(printer.device_class, DeviceClass::Printer);
    assert_eq!(printer.model.as_deref(), Some("HP Color LaserJet 9500"));
    assert!(printer.fingerprint_confidence >= 90);
    assert!(printer.services.iter().any(|s| s.service_type == ServiceType::JetDirect));
    assert!(printer.services.iter().any(|s| s.display_name == "HTTP, HP Color LaserJet 9500"));

    let gw = &f[&ip("192.168.0.1")];
    assert!(gw.is_gateway);
    assert_eq!(gw.device_class, DeviceClass::Router);

    let phone = &f[&ip("192.168.0.120")];
    assert!(phone.randomized_mac);
    assert_eq!(phone.vendor, None);

    let pi = &f[&ip("192.168.0.130")];
    assert_eq!(pi.fingerprint_rule.as_deref(), Some("raspberry-pi"));

    // Dead host with a DNS name -> "keine Reaktion".
    assert_eq!(f[&ip("192.168.0.200")].status, HostStatus::NoResponse);
}

#[tokio::test]
async fn icmp_blocked_host_found_via_arp_and_tcp() {
    let events = run(network(true), "192.168.0.110", config()).await;
    let f = finals(&events);
    let h = &f[&ip("192.168.0.110")];
    assert_eq!(h.status, HostStatus::Alive);
    assert_eq!(h.hostname.as_deref(), Some("PATRICK"));
    assert_eq!(h.vendor.as_deref(), Some("GIGA-BYTE TECHNOLOGY CO.,LTD."));
    assert!(h.services.iter().any(|s| s.service_type == ServiceType::Shares && s.uri.as_deref() == Some("\\\\PATRICK\\")));
    assert!(h.services.iter().any(|s| s.service_type == ServiceType::Rdp));

    // Without a usable neighbor table, the TCP fallback still finds it.
    let mut net = network(false);
    net.hosts.get_mut(&ip("192.168.0.110")).unwrap().mac = None;
    let events = run(net, "192.168.0.110", config()).await;
    assert_eq!(finals(&events)[&ip("192.168.0.110")].status, HostStatus::Alive);
}

#[tokio::test]
async fn routed_host_gets_no_mac() {
    let events = run(network(true), "10.1.0.5", config()).await;
    let f = finals(&events);
    let h = &f[&ip("10.1.0.5")];
    assert_eq!(h.status, HostStatus::Alive);
    assert_eq!(h.mac, None, "gateway MAC must never be assigned to routed hosts");
}

#[tokio::test]
async fn cancel_stops_scan() {
    let mut net = network(true);
    net.delay = Duration::from_millis(200);
    let scanner = Scanner::new(Arc::new(net), vendors(), Arc::new(FingerprintEngine::with_builtin()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let control = ScanControl::new();
    let c = control.clone();
    let mut cfg = config();
    cfg.max_host_concurrency = 4;
    let task = tokio::spawn(async move {
        scanner.run(parse_targets("192.168.0.0/24").unwrap(), cfg, tx, Arc::new(|| {}), c).await;
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    control.cancel();
    tokio::time::timeout(Duration::from_secs(2), task).await.expect("scan must stop promptly").unwrap();
    let mut events = Vec::new();
    while let Ok(e) = rx.try_recv() {
        events.push(e);
    }
    assert!(matches!(events.last(), Some(ScanEvent::ScanCancelled)));
    let done = events.iter().filter(|e| matches!(e, ScanEvent::Progress { .. })).count();
    assert!(done < 254, "cancelled scan must not complete all targets ({done})");
}

#[tokio::test]
async fn pause_and_resume() {
    let mut net = network(true);
    net.delay = Duration::from_millis(50);
    let net = Arc::new(net);
    let scanner = Scanner::new(net.clone(), vendors(), Arc::new(FingerprintEngine::with_builtin()));
    let (tx, mut rx) = mpsc::unbounded_channel();
    let control = ScanControl::new();
    let c = control.clone();
    let mut cfg = config();
    cfg.max_host_concurrency = 2;
    let task = tokio::spawn(async move {
        scanner.run(parse_targets("192.168.0.1-20").unwrap(), cfg, tx, Arc::new(|| {}), c).await;
    });
    tokio::time::sleep(Duration::from_millis(120)).await;
    control.pause();
    tokio::time::sleep(Duration::from_millis(150)).await;
    let calls_paused = net.icmp_calls.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(net.icmp_calls.load(Ordering::Relaxed), calls_paused, "no new hosts while paused");
    assert!(calls_paused < 20);
    control.resume();
    tokio::time::timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
    let mut events = Vec::new();
    while let Ok(e) = rx.try_recv() {
        events.push(e);
    }
    assert!(events.iter().any(|e| matches!(e, ScanEvent::ScanPaused)));
    assert!(events.iter().any(|e| matches!(e, ScanEvent::ScanResumed)));
    assert!(matches!(events.last(), Some(ScanEvent::ScanFinished)));
    assert_eq!(net.icmp_calls.load(Ordering::Relaxed), 20, "resume continues where it stopped");
}

#[tokio::test]
async fn mdns_only_host_is_reported() {
    let mut net = network(true);
    net.hosts.insert(
        ip("192.168.0.140"),
        MockHost {
            icmp: false,
            mdns: vec![MdnsService {
                service_type: "_home-assistant._tcp".into(),
                instance: "Home".into(),
                hostname: "homeassistant.local".into(),
                port: 8123,
                txt: Default::default(),
            }],
            ..Default::default()
        },
    );
    let mut cfg = config();
    cfg.resolve_mdns = true;
    let events = run(net, "192.168.0.140", cfg).await;
    let f = finals(&events);
    let h = &f[&ip("192.168.0.140")];
    assert_eq!(h.status, HostStatus::Alive);
    assert_eq!(h.hostname.as_deref(), Some("homeassistant"));
    assert_eq!(h.device_class, DeviceClass::HomeAutomation);
}
