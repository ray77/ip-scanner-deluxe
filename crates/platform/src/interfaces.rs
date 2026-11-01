use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use vendor_db::MacAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterfaceKind {
    Ethernet,
    WiFi,
    Vpn,
    Bridge,
    Virtual,
    Loopback,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub index: u32,
    /// OS name (`en0`, `eth0`, `{GUID}` on Windows).
    pub name: String,
    /// Human friendly name (`Ethernet`, `WLAN`), falls back to `name`.
    pub display_name: String,
    pub kind: InterfaceKind,
    /// Detected virtual product (Docker, Hyper-V, VMware ...), only a label.
    pub virtual_tag: Option<String>,
    /// IPv4 addresses with prefix length.
    pub ipv4: Vec<(Ipv4Addr, u8)>,
    pub ipv6: Vec<Ipv6Addr>,
    pub mac: Option<MacAddr>,
    pub gateway: Option<IpAddr>,
    pub gateway_mac: Option<MacAddr>,
    pub is_up: bool,
    pub is_default: bool,
}

pub fn prefix_to_mask(prefix: u8) -> Ipv4Addr {
    if prefix == 0 {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::from(u32::MAX << (32 - u32::from(prefix.min(32))))
    }
}

impl NetworkInterface {
    /// Primary IPv4 address and prefix.
    pub fn primary_ipv4(&self) -> Option<(Ipv4Addr, u8)> {
        self.ipv4.iter().find(|(a, _)| !a.is_link_local() && !a.is_loopback()).or(self.ipv4.first()).copied()
    }

    pub fn netmask(&self) -> Option<Ipv4Addr> {
        self.primary_ipv4().map(|(_, p)| prefix_to_mask(p))
    }

    /// True if `ip` is inside one of the interface's IPv4 subnets (on-link,
    /// MAC addresses can be resolved directly).
    pub fn contains(&self, ip: IpAddr) -> bool {
        let IpAddr::V4(t) = ip else {
            let IpAddr::V6(t6) = ip else { return false };
            // Link-local or same /64 as one of our addresses.
            return (t6.segments()[0] & 0xffc0) == 0xfe80 || self.ipv6.iter().any(|a| a.segments()[..4] == t6.segments()[..4]);
        };
        self.ipv4.iter().any(|(a, p)| {
            let m = u32::from(prefix_to_mask(*p));
            (u32::from(*a) & m) == (u32::from(t) & m)
        })
    }

    /// Directed broadcast addresses of all IPv4 subnets.
    pub fn broadcasts(&self) -> Vec<Ipv4Addr> {
        self.ipv4.iter().filter(|(_, p)| *p < 31).map(|(a, p)| Ipv4Addr::from(u32::from(*a) | !u32::from(prefix_to_mask(*p)))).collect()
    }

    /// Scan range of the primary subnet, e.g. `192.168.178.1-254`.
    /// Large subnets are clamped to the /16 around our address.
    pub fn subnet_range(&self) -> Option<String> {
        let (addr, prefix) = self.primary_ipv4()?;
        let prefix = prefix.clamp(16, 30);
        let mask = u32::from(prefix_to_mask(prefix));
        let net = u32::from(addr) & mask;
        let bcast = net | !mask;
        let first = Ipv4Addr::from(net + 1);
        let last = Ipv4Addr::from(bcast - 1);
        let (f, l) = (first.octets(), last.octets());
        if f[..3] == l[..3] {
            Some(format!("{first}-{}", l[3]))
        } else {
            Some(format!("{first}-{last}"))
        }
    }
}

fn detect_virtual(name: &str, desc: &str, mac: Option<&MacAddr>) -> Option<String> {
    let s = format!("{name} {desc}").to_lowercase();
    let tags = [
        ("hyper-v", "Hyper-V"),
        ("vethernet", "Hyper-V"),
        ("docker", "Docker"),
        ("veth", "Docker"),
        ("br-", "Docker"),
        ("vmware", "VMware"),
        ("vmnet", "VMware"),
        ("virtualbox", "VirtualBox"),
        ("vboxnet", "VirtualBox"),
        ("tailscale", "Tailscale"),
        ("wireguard", "WireGuard"),
        ("wg", "WireGuard"),
        ("openvpn", "OpenVPN"),
        ("tap-windows", "OpenVPN"),
        ("utun", "utun"),
        ("zerotier", "ZeroTier"),
        ("bridge", "Bridge"),
    ];
    for (needle, tag) in tags {
        let hit = if needle.len() <= 3 { s.split_whitespace().any(|w| w.starts_with(needle)) } else { s.contains(needle) };
        if hit {
            return Some(tag.to_string());
        }
    }
    let mac = mac?;
    let o = mac.0;
    match (o[0], o[1], o[2]) {
        (0x00, 0x15, 0x5d) => Some("Hyper-V".into()),
        (0x00, 0x50, 0x56) | (0x00, 0x0c, 0x29) => Some("VMware".into()),
        (0x08, 0x00, 0x27) | (0x0a, 0x00, 0x27) => Some("VirtualBox".into()),
        _ => None,
    }
}

fn kind_of(i: &netdev::Interface, tag: &Option<String>) -> InterfaceKind {
    use netdev::prelude::InterfaceType as T;
    if i.is_loopback() {
        return InterfaceKind::Loopback;
    }
    match tag.as_deref() {
        Some("Tailscale" | "WireGuard" | "OpenVPN" | "utun" | "ZeroTier") => return InterfaceKind::Vpn,
        Some("Bridge") => return InterfaceKind::Bridge,
        Some(_) => return InterfaceKind::Virtual,
        None => {}
    }
    if i.is_tun() || i.is_point_to_point() {
        return InterfaceKind::Vpn;
    }
    match i.if_type {
        T::Wireless80211 => InterfaceKind::WiFi,
        T::Ethernet | T::FastEthernetT | T::FastEthernetFx | T::GigabitEthernet => {
            let n = i.name.to_lowercase();
            let f = i.friendly_name.clone().unwrap_or_default().to_lowercase();
            if n.starts_with("wl") || f.contains("wi-fi") || f.contains("wlan") || f.contains("wireless") {
                InterfaceKind::WiFi
            } else if n.starts_with("br") || n.starts_with("bridge") {
                InterfaceKind::Bridge
            } else {
                InterfaceKind::Ethernet
            }
        }
        T::Tunnel | T::Ppp => InterfaceKind::Vpn,
        _ => InterfaceKind::Other,
    }
}

fn convert_mac(m: &netdev::MacAddr) -> Option<MacAddr> {
    let mac = MacAddr(m.octets());
    (!mac.is_zero()).then_some(mac)
}

/// Enumerate interfaces. Loopback is included but flagged.
pub fn list_interfaces() -> Vec<NetworkInterface> {
    let mut out: Vec<NetworkInterface> = netdev::get_interfaces()
        .into_iter()
        .map(|i| {
            let mac = i.mac_addr.as_ref().and_then(convert_mac);
            let desc = i.description.clone().unwrap_or_default();
            let tag = detect_virtual(&i.name, &desc, mac.as_ref());
            let kind = kind_of(&i, &tag);
            let display = i.friendly_name.clone().filter(|s| !s.is_empty()).unwrap_or_else(|| i.name.clone());
            NetworkInterface {
                index: i.index,
                display_name: display,
                kind,
                virtual_tag: tag,
                ipv4: i.ipv4.iter().map(|n| (n.addr(), n.prefix_len())).collect(),
                ipv6: i.ipv6.iter().map(|n| n.addr()).collect(),
                mac,
                gateway: i
                    .gateway
                    .as_ref()
                    .and_then(|g| g.ipv4.first().copied().map(IpAddr::V4).or(g.ipv6.first().copied().map(IpAddr::V6))),
                gateway_mac: i.gateway.as_ref().and_then(|g| convert_mac(&g.mac_addr)),
                is_up: i.is_up(),
                is_default: i.default,
                name: i.name,
            }
        })
        .collect();
    out.sort_by_key(|i| (!i.is_default, i.kind == InterfaceKind::Loopback, i.virtual_tag.is_some(), i.index));
    out
}

/// Interfaces suitable for scanning: up, with IPv4, not loopback.
pub fn scannable_interfaces() -> Vec<NetworkInterface> {
    list_interfaces().into_iter().filter(|i| i.is_up && i.kind != InterfaceKind::Loopback && i.primary_ipv4().is_some()).collect()
}

/// Source address the OS routing table would use to reach `target`
/// (UDP connect does not send packets).
pub fn route_source(target: IpAddr) -> Option<IpAddr> {
    let bind: SocketAddr = match target {
        IpAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
        IpAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    let s = UdpSocket::bind(bind).ok()?;
    s.connect(SocketAddr::new(target, 9)).ok()?;
    s.local_addr().ok().map(|a| a.ip())
}

/// Interface used to reach `target`: on-link subnet first, else the one
/// owning the route's source address.
pub fn interface_for(target: IpAddr, ifaces: &[NetworkInterface]) -> Option<&NetworkInterface> {
    if let Some(i) = ifaces.iter().find(|i| i.kind != InterfaceKind::Loopback && i.contains(target)) {
        return Some(i);
    }
    let src = route_source(target)?;
    ifaces.iter().find(|i| match src {
        IpAddr::V4(v4) => i.ipv4.iter().any(|(a, _)| *a == v4),
        IpAddr::V6(v6) => i.ipv6.contains(&v6),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(addr: &str, prefix: u8) -> NetworkInterface {
        NetworkInterface {
            index: 1,
            name: "eth0".into(),
            display_name: "Ethernet".into(),
            kind: InterfaceKind::Ethernet,
            virtual_tag: None,
            ipv4: vec![(addr.parse().unwrap(), prefix)],
            ipv6: vec![],
            mac: None,
            gateway: None,
            gateway_mac: None,
            is_up: true,
            is_default: true,
        }
    }

    #[test]
    fn subnet_helpers() {
        let i = iface("192.168.178.34", 24);
        assert_eq!(i.subnet_range().as_deref(), Some("192.168.178.1-254"));
        assert_eq!(i.netmask(), Some(Ipv4Addr::new(255, 255, 255, 0)));
        assert_eq!(i.broadcasts(), vec![Ipv4Addr::new(192, 168, 178, 255)]);
        assert!(i.contains("192.168.178.200".parse().unwrap()));
        assert!(!i.contains("192.168.179.1".parse().unwrap()));
        let big = iface("10.1.2.3", 8);
        assert_eq!(big.subnet_range().as_deref(), Some("10.1.0.1-10.1.255.254"));
    }

    #[test]
    fn virtual_detection() {
        assert_eq!(detect_virtual("docker0", "", None).as_deref(), Some("Docker"));
        assert_eq!(detect_virtual("utun3", "", None).as_deref(), Some("utun"));
        assert_eq!(detect_virtual("{GUID}", "Hyper-V Virtual Ethernet Adapter", None).as_deref(), Some("Hyper-V"));
        assert_eq!(detect_virtual("en0", "", None), None);
        assert_eq!(detect_virtual("wlan0", "", None), None);
    }
}
