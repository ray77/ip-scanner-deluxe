//! Port profiles and port → service mapping.

use crate::model::ServiceType;

/// Quick host discovery set (TCP fallback when ICMP is filtered).
pub const DISCOVERY_PORTS: &[u16] = &[22, 80, 443, 445, 3389, 8080, 8443, 9100, 135, 139, 53, 5000];

pub const FAST_PORTS: &[u16] = &[21, 22, 23, 53, 80, 139, 443, 445, 515, 548, 631, 3389, 5000, 5001, 5900, 8080, 8443, 9100];

const STANDARD_EXTRA: &[u16] = &[
    25, 110, 135, 143, 389, 554, 636, 993, 995, 1400, 1433, 1521, 1883, 2375, 2376, 3000, 3306, 4899, 5353, 5432, 5672, 5985, 5986, 8000,
    8006, 8008, 8009, 8123, 8883, 9200, 9443, 27017, 49000,
];

const ACCURATE_EXTRA: &[u16] = &[
    81, 88, 111, 427, 873, 1080, 1194, 2049, 3260, 4443, 5060, 5601, 6379, 6443, 7000, 8001, 8002, 8081, 8088, 8181, 8291, 8888, 9000,
    9090, 9091, 10000, 32400, 37777, 49443,
];

pub fn fast_ports() -> Vec<u16> {
    FAST_PORTS.to_vec()
}

pub fn standard_ports() -> Vec<u16> {
    let mut v: Vec<u16> = FAST_PORTS.iter().chain(STANDARD_EXTRA).copied().collect();
    v.sort_unstable();
    v.dedup();
    v
}

pub fn accurate_ports() -> Vec<u16> {
    let mut v = standard_ports();
    v.extend_from_slice(ACCURATE_EXTRA);
    v.sort_unstable();
    v.dedup();
    v
}

/// Rejected entry of a user port list; the UI translates [`PortError::key`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    #[error("Invalid port: {0}")]
    Invalid(String),
    #[error("Invalid port range: {0}")]
    InvalidRange(String),
    #[error("Port 0 is invalid")]
    Zero,
}

impl PortError {
    /// Translation key (`ui::i18n`); the offending entry is `{t}`.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "ports.invalid",
            Self::InvalidRange(_) => "ports.invalid_range",
            Self::Zero => "ports.zero",
        }
    }

    pub fn token(&self) -> &str {
        match self {
            Self::Invalid(t) | Self::InvalidRange(t) => t,
            Self::Zero => "0",
        }
    }
}

/// Parse a user port list like `22, 80, 8000-8010`.
pub fn parse_port_list(s: &str) -> Result<Vec<u16>, PortError> {
    let mut out = Vec::new();
    for tok in s.split([',', ';', ' ']).map(str::trim).filter(|t| !t.is_empty()) {
        if let Some((a, b)) = tok.split_once('-') {
            let a: u16 = a.trim().parse().map_err(|_| PortError::Invalid(tok.into()))?;
            let b: u16 = b.trim().parse().map_err(|_| PortError::Invalid(tok.into()))?;
            if a == 0 || b < a || (b - a) > 4096 {
                return Err(PortError::InvalidRange(tok.into()));
            }
            out.extend(a..=b);
        } else {
            let p: u16 = tok.parse().map_err(|_| PortError::Invalid(tok.into()))?;
            if p == 0 {
                return Err(PortError::Zero);
            }
            out.push(p);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// How a port should be probed for metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeKind {
    Http,
    Https,
    Banner,
    Ipp,
    None,
}

/// Well-known service for a TCP port.
pub fn service_for_port(port: u16) -> (ServiceType, &'static str, ProbeKind) {
    use ProbeKind::*;
    use ServiceType as S;
    match port {
        21 => (S::Ftp, "FTP", Banner),
        22 => (S::Ssh, "SSH", Banner),
        23 => (S::Telnet, "Telnet", Banner),
        25 | 587 => (S::Smtp, "SMTP", Banner),
        53 => (S::Dns, "DNS", None),
        80 | 81 | 8000 | 8008 | 8080 | 8081 | 8088 | 8888 | 3000 | 5000 | 9000 | 9090 | 10000 | 1400 | 49000 | 32400 | 7000 | 8001
        | 9091 | 8181 => (S::Http, "HTTP", Http),
        443 | 4443 | 5001 | 8443 | 9443 | 8006 | 49443 | 6443 | 8002 => (S::Https, "HTTPS", Https),
        8123 => (S::HomeAssistant, "Home Assistant", Http),
        110 => (S::Pop3, "POP3", Banner),
        143 => (S::Imap, "IMAP", Banner),
        993 => (S::Imap, "IMAPS", None),
        995 => (S::Pop3, "POP3S", None),
        135 => (S::Other, "MS-RPC", None),
        139 | 445 => (S::Smb, "SMB", None),
        389 => (S::Ldap, "LDAP", None),
        636 => (S::Ldap, "LDAPS", None),
        515 => (S::Lpd, "LPD", None),
        548 => (S::Afp, "AFP", None),
        554 => (S::Rtsp, "RTSP", None),
        631 => (S::Ipp, "IPP", Ipp),
        9100 => (S::JetDirect, "JetDirect", None),
        1433 => (S::MsSql, "MS SQL Server", None),
        1521 => (S::Oracle, "Oracle DB", None),
        1883 => (S::Mqtt, "MQTT", None),
        8883 => (S::Mqtt, "MQTT (TLS)", None),
        2375 => (S::Docker, "Docker API", Http),
        2376 => (S::Docker, "Docker API (TLS)", None),
        3306 => (S::MySql, "MySQL/MariaDB", Banner),
        3389 => (S::Rdp, "RDP", None),
        4899 => (S::Radmin, "Radmin", None),
        5432 => (S::PostgreSql, "PostgreSQL", None),
        5672 => (S::Amqp, "AMQP", None),
        5900 => (S::Vnc, "VNC", Banner),
        5985 => (S::WinRm, "WinRM", None),
        5986 => (S::WinRm, "WinRM (HTTPS)", None),
        6379 => (S::Redis, "Redis", None),
        9200 => (S::Elasticsearch, "Elasticsearch", Http),
        27017 => (S::MongoDb, "MongoDB", None),
        _ => (S::Other, "", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles() {
        assert_eq!(fast_ports().len(), 18);
        assert!(standard_ports().contains(&27017));
        assert!(standard_ports().len() > fast_ports().len());
        assert!(accurate_ports().len() > standard_ports().len());
    }

    #[test]
    fn custom_ports() {
        assert_eq!(parse_port_list("22, 80,8000-8002").unwrap(), vec![22, 80, 8000, 8001, 8002]);
        assert!(parse_port_list("0").is_err());
        assert!(parse_port_list("abc").is_err());
        assert!(parse_port_list("100-50").is_err());
    }
}
