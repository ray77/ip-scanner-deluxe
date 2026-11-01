use fingerprint::DeviceClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::SystemTime;
use vendor_db::MacAddr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum HostStatus {
    Alive,
    NoResponse,
    Unknown,
    Scanning,
}

impl HostStatus {
    pub fn key(self) -> &'static str {
        match self {
            HostStatus::Alive => "alive",
            HostStatus::NoResponse => "no_response",
            HostStatus::Unknown => "unknown",
            HostStatus::Scanning => "scanning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ServiceType {
    Http,
    Https,
    Ftp,
    Ssh,
    Telnet,
    Smtp,
    Pop3,
    Imap,
    Dns,
    Smb,
    Shares,
    Ipp,
    JetDirect,
    Lpd,
    Rdp,
    Vnc,
    Radmin,
    WinRm,
    PostgreSql,
    MySql,
    MsSql,
    Oracle,
    MongoDb,
    Redis,
    Elasticsearch,
    Ldap,
    Mqtt,
    Amqp,
    Docker,
    Rtsp,
    HomeAssistant,
    SynologyDsm,
    Afp,
    Snmp,
    MdnsService,
    Other,
}

/// One detected service (a child row in the tree view).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceRecord {
    pub port: Option<u16>,
    pub protocol: TransportProtocol,
    pub service_type: ServiceType,
    /// Text shown in the tree, e.g. `HTTP, FRITZ!Box`.
    pub display_name: String,
    /// Address the "open" action uses (`http://…`, `\\host\`).
    pub uri: Option<String>,
    pub metadata: BTreeMap<String, String>,
}

impl ServiceRecord {
    pub fn new(service_type: ServiceType, port: Option<u16>, display_name: impl Into<String>) -> Self {
        ServiceRecord {
            port,
            protocol: TransportProtocol::Tcp,
            service_type,
            display_name: display_name.into(),
            uri: None,
            metadata: BTreeMap::new(),
        }
    }

    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    pub fn meta(mut self, k: &str, v: impl Into<String>) -> Self {
        self.metadata.insert(k.to_string(), v.into());
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostNames {
    pub reverse_dns: Option<String>,
    pub mdns: Vec<String>,
    pub netbios: Option<String>,
    pub llmnr: Option<String>,
    pub workgroup: Option<String>,
}

fn strip_local(s: &str) -> String {
    s.trim_end_matches('.').trim_end_matches(".local").to_string()
}

impl HostNames {
    /// Preferred display name: reverse DNS, mDNS, NetBIOS, LLMNR.
    pub fn best(&self) -> Option<String> {
        self.reverse_dns
            .clone()
            .or_else(|| self.mdns.first().map(|m| strip_local(m)))
            .or_else(|| self.netbios.clone())
            .or_else(|| self.llmnr.as_deref().map(strip_local))
    }

    pub fn all(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        let mut push = |s: &str| {
            if !s.is_empty() && !v.iter().any(|e| e.eq_ignore_ascii_case(s)) {
                v.push(s.to_string());
            }
        };
        if let Some(r) = &self.reverse_dns {
            push(r);
        }
        for m in &self.mdns {
            push(m.trim_end_matches('.'));
        }
        if let Some(n) = &self.netbios {
            push(n);
        }
        if let Some(l) = &self.llmnr {
            push(l);
        }
        v
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostRecord {
    pub status: HostStatus,
    pub ip: IpAddr,
    pub mac: Option<MacAddr>,
    pub hostname: Option<String>,
    pub all_hostnames: Vec<String>,
    pub names: HostNames,
    pub vendor: Option<String>,
    /// MAC is locally administered (randomized/private).
    pub randomized_mac: bool,
    pub device_class: DeviceClass,
    pub model: Option<String>,
    pub os: Option<String>,
    pub fingerprint_confidence: u8,
    pub fingerprint_rule: Option<String>,
    pub latency_ms: Option<f32>,
    pub services: Vec<ServiceRecord>,
    pub open_ports: Vec<u16>,
    pub comment: String,
    pub interface: Option<String>,
    pub is_gateway: bool,
    pub first_seen: Option<SystemTime>,
    pub last_seen: Option<SystemTime>,
    pub is_favorite: bool,
}

impl HostRecord {
    pub fn new(ip: IpAddr, status: HostStatus) -> Self {
        HostRecord {
            status,
            ip,
            mac: None,
            hostname: None,
            all_hostnames: Vec::new(),
            names: HostNames::default(),
            vendor: None,
            randomized_mac: false,
            device_class: DeviceClass::Unknown,
            model: None,
            os: None,
            fingerprint_confidence: 0,
            fingerprint_rule: None,
            latency_ms: None,
            services: Vec::new(),
            open_ports: Vec::new(),
            comment: String::new(),
            interface: None,
            is_gateway: false,
            first_seen: None,
            last_seen: None,
            is_favorite: false,
        }
    }

    /// Stable identifier: MAC when known, otherwise the IP.
    pub fn device_id(&self) -> String {
        match &self.mac {
            Some(m) => m.to_string(),
            None => format!("ip:{}", self.ip),
        }
    }

    pub fn display_name(&self) -> String {
        self.hostname.clone().unwrap_or_else(|| self.ip.to_string())
    }

    pub fn has_service(&self, t: &ServiceType) -> bool {
        self.services.iter().any(|s| &s.service_type == t)
    }
}

/// Sort key that orders IP addresses numerically (IPv4 before IPv6).
pub fn ip_sort_key(ip: &IpAddr) -> (u8, u128) {
    match ip {
        IpAddr::V4(v) => (4, u128::from(u32::from(*v))),
        IpAddr::V6(v) => (6, u128::from(*v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_sorting_is_numeric() {
        let mut v: Vec<IpAddr> = ["192.168.0.10", "192.168.0.2", "10.0.0.1", "::1"].iter().map(|s| s.parse().unwrap()).collect();
        v.sort_by_key(ip_sort_key);
        let s: Vec<String> = v.iter().map(|i| i.to_string()).collect();
        assert_eq!(s, ["10.0.0.1", "192.168.0.2", "192.168.0.10", "::1"]);
    }

    #[test]
    fn best_name_order() {
        let mut n = HostNames { netbios: Some("PATRICK".into()), mdns: vec!["patrick.local".into()], ..Default::default() };
        assert_eq!(n.best().as_deref(), Some("patrick"));
        n.reverse_dns = Some("patrick.fritz.box".into());
        assert_eq!(n.best().as_deref(), Some("patrick.fritz.box"));
    }
}
