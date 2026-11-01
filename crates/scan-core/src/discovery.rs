//! Network access used by the coordinator, behind the [`NetProbe`] trait so
//! the engine can be tested against a mock network.

use parking_lot::Mutex;
use platform::interfaces::{interface_for, list_interfaces, NetworkInterface};
use platform::neighbor::{NeighborResolver, SystemNeighborResolver};
use protocols::http::HttpInfo;
use protocols::ipp::IppInfo;
use protocols::mdns::{MdnsBrowser, MdnsService, DEFAULT_SERVICE_TYPES};
use protocols::netbios::NetbiosInfo;
use protocols::snmp::SnmpSystem;
use std::future::Future;
use std::io::ErrorKind;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use vendor_db::MacAddr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TcpOutcome {
    Open(Duration),
    /// RST received: host is alive, port closed.
    Refused(Duration),
    NoResponse,
}

impl TcpOutcome {
    pub fn host_alive(&self) -> bool {
        !matches!(self, TcpOutcome::NoResponse)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IcmpOutcome {
    Reply(Duration),
    NoReply,
    /// ICMP cannot be used (no permission); TCP fallback only.
    Unavailable(String),
}

/// Everything the scanner needs from the network.
pub trait NetProbe: Send + Sync + 'static {
    fn interface_for(&self, ip: IpAddr) -> Option<NetworkInterface>;
    fn icmp(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = IcmpOutcome> + Send;
    fn tcp(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = TcpOutcome> + Send;
    fn mac(&self, iface: Option<&NetworkInterface>, ip: IpAddr) -> impl Future<Output = Option<MacAddr>> + Send;
    fn reverse_dns(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send;
    fn mdns_name(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send;
    fn llmnr_name(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send;
    fn netbios(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<NetbiosInfo>> + Send;
    fn http(&self, ip: IpAddr, port: u16, tls: bool, timeout: Duration) -> impl Future<Output = Option<HttpInfo>> + Send;
    fn banner(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = Option<String>> + Send;
    fn ipp(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = Option<IppInfo>> + Send;
    fn snmp(&self, ip: IpAddr, community: &str, timeout: Duration) -> impl Future<Output = Option<SnmpSystem>> + Send;
    /// Send a harmless datagram so the OS resolves the target's MAC (ARP).
    fn touch(&self, ip: IpAddr);
    /// True when an absent neighbor entry reliably means "host absent"
    /// (the neighbor table is readable and populated).
    fn arp_reliable(&self) -> bool;
    /// Start DNS-SD browsing for the duration of a scan.
    fn mdns_begin(&self) -> Option<String>;
    fn mdns_services(&self, ip: IpAddr) -> Vec<MdnsService>;
    fn mdns_addresses(&self) -> Vec<IpAddr>;
    fn mdns_end(&self);
}

/// The real network.
pub struct SystemProbe {
    interfaces: Vec<NetworkInterface>,
    neighbors: SystemNeighborResolver,
    browser: Mutex<Option<MdnsBrowser>>,
    arp_reliable: Mutex<Option<bool>>,
}

impl Default for SystemProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemProbe {
    pub fn new() -> Self {
        SystemProbe {
            interfaces: list_interfaces(),
            neighbors: SystemNeighborResolver::new(),
            browser: Mutex::new(None),
            arp_reliable: Mutex::new(None),
        }
    }

    pub fn with_interfaces(interfaces: Vec<NetworkInterface>) -> Self {
        SystemProbe { interfaces, ..Self::new() }
    }
}

pub(crate) fn elapsed_since(start: Instant) -> Duration {
    start.elapsed()
}

impl NetProbe for SystemProbe {
    fn interface_for(&self, ip: IpAddr) -> Option<NetworkInterface> {
        interface_for(ip, &self.interfaces).cloned()
    }

    fn icmp(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = IcmpOutcome> + Send {
        let cap = platform::icmp::capability();
        async move {
            if let platform::IcmpCapability::Unavailable(e) = cap {
                return IcmpOutcome::Unavailable(e);
            }
            match platform::icmp::echo_async(ip, None, timeout, 1).await {
                Ok(r) if r.kind == platform::ReplyKind::EchoReply => IcmpOutcome::Reply(r.rtt),
                Ok(_) | Err(platform::IcmpError::Timeout) => IcmpOutcome::NoReply,
                Err(platform::IcmpError::NotPermitted(e)) => IcmpOutcome::Unavailable(e),
                Err(_) => IcmpOutcome::NoReply,
            }
        }
    }

    async fn tcp(&self, ip: IpAddr, port: u16, timeout: Duration) -> TcpOutcome {
        let start = Instant::now();
        match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(SocketAddr::new(ip, port))).await {
            Ok(Ok(_s)) => TcpOutcome::Open(elapsed_since(start)),
            Ok(Err(e)) if e.kind() == ErrorKind::ConnectionRefused => TcpOutcome::Refused(elapsed_since(start)),
            _ => TcpOutcome::NoResponse,
        }
    }

    fn mac(&self, iface: Option<&NetworkInterface>, ip: IpAddr) -> impl Future<Output = Option<MacAddr>> + Send {
        let fut = self.neighbors.resolve_mac(iface, ip);
        async move { fut.await.ok().flatten() }
    }

    fn reverse_dns(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        protocols::dns::reverse_dns(ip, timeout)
    }

    fn mdns_name(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        protocols::mdns::reverse_lookup(ip, timeout)
    }

    fn llmnr_name(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        protocols::llmnr::reverse_lookup(ip, timeout)
    }

    fn netbios(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<NetbiosInfo>> + Send {
        protocols::netbios::query(ip, timeout)
    }

    fn http(&self, ip: IpAddr, port: u16, tls: bool, timeout: Duration) -> impl Future<Output = Option<HttpInfo>> + Send {
        protocols::http::probe(ip, port, tls, timeout)
    }

    fn banner(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        protocols::banner::grab(ip, port, None, timeout)
    }

    fn ipp(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = Option<IppInfo>> + Send {
        protocols::ipp::get_printer_attributes(ip, port, timeout)
    }

    fn snmp(&self, ip: IpAddr, community: &str, timeout: Duration) -> impl Future<Output = Option<SnmpSystem>> + Send {
        let c = community.to_string();
        async move { protocols::snmp::get_system(ip, &c, timeout).await }
    }

    fn touch(&self, ip: IpAddr) {
        let bind: SocketAddr = if ip.is_ipv4() { ([0, 0, 0, 0], 0).into() } else { (std::net::Ipv6Addr::UNSPECIFIED, 0).into() };
        if let Ok(s) = std::net::UdpSocket::bind(bind) {
            // Never stall the async worker on a slow send path.
            let _ = s.set_nonblocking(true);
            // Port 9 (discard): no service answers, the packet only forces ARP.
            let _ = s.send_to(&[0u8], SocketAddr::new(ip, 9));
        }
    }

    fn arp_reliable(&self) -> bool {
        if cfg!(windows) {
            return true; // SendARP actively resolves
        }
        let mut g = self.arp_reliable.lock();
        *g.get_or_insert_with(|| platform::neighbor::neighbor_table().map(|t| !t.is_empty()).unwrap_or(false))
    }

    fn mdns_begin(&self) -> Option<String> {
        *self.arp_reliable.lock() = None;
        let (b, warn) = MdnsBrowser::start(DEFAULT_SERVICE_TYPES);
        *self.browser.lock() = Some(b);
        warn
    }

    fn mdns_services(&self, ip: IpAddr) -> Vec<MdnsService> {
        self.browser.lock().as_ref().map(|b| b.services_for(ip)).unwrap_or_default()
    }

    fn mdns_addresses(&self) -> Vec<IpAddr> {
        self.browser.lock().as_ref().map(|b| b.known_addresses()).unwrap_or_default()
    }

    fn mdns_end(&self) {
        if let Some(mut b) = self.browser.lock().take() {
            b.stop();
        }
    }
}

impl<P: NetProbe> NetProbe for Arc<P> {
    fn interface_for(&self, ip: IpAddr) -> Option<NetworkInterface> {
        (**self).interface_for(ip)
    }
    fn icmp(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = IcmpOutcome> + Send {
        (**self).icmp(ip, timeout)
    }
    fn tcp(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = TcpOutcome> + Send {
        (**self).tcp(ip, port, timeout)
    }
    fn mac(&self, iface: Option<&NetworkInterface>, ip: IpAddr) -> impl Future<Output = Option<MacAddr>> + Send {
        (**self).mac(iface, ip)
    }
    fn reverse_dns(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        (**self).reverse_dns(ip, timeout)
    }
    fn mdns_name(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        (**self).mdns_name(ip, timeout)
    }
    fn llmnr_name(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        (**self).llmnr_name(ip, timeout)
    }
    fn netbios(&self, ip: IpAddr, timeout: Duration) -> impl Future<Output = Option<NetbiosInfo>> + Send {
        (**self).netbios(ip, timeout)
    }
    fn http(&self, ip: IpAddr, port: u16, tls: bool, timeout: Duration) -> impl Future<Output = Option<HttpInfo>> + Send {
        (**self).http(ip, port, tls, timeout)
    }
    fn banner(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = Option<String>> + Send {
        (**self).banner(ip, port, timeout)
    }
    fn ipp(&self, ip: IpAddr, port: u16, timeout: Duration) -> impl Future<Output = Option<IppInfo>> + Send {
        (**self).ipp(ip, port, timeout)
    }
    fn snmp(&self, ip: IpAddr, community: &str, timeout: Duration) -> impl Future<Output = Option<SnmpSystem>> + Send {
        (**self).snmp(ip, community, timeout)
    }
    fn touch(&self, ip: IpAddr) {
        (**self).touch(ip)
    }
    fn arp_reliable(&self) -> bool {
        (**self).arp_reliable()
    }
    fn mdns_begin(&self) -> Option<String> {
        (**self).mdns_begin()
    }
    fn mdns_services(&self, ip: IpAddr) -> Vec<MdnsService> {
        (**self).mdns_services(ip)
    }
    fn mdns_addresses(&self) -> Vec<IpAddr> {
        (**self).mdns_addresses()
    }
    fn mdns_end(&self) {
        (**self).mdns_end()
    }
}
