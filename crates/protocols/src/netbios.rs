//! NetBIOS Name Service node status (NBSTAT) query, RFC 1002 §4.2.17.
//! Yields the machine name, workgroup, file-server flag and often the MAC.

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use vendor_db::MacAddr;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetbiosInfo {
    pub name: Option<String>,
    pub workgroup: Option<String>,
    /// `<20>` server service registered: file/printer sharing is active.
    pub file_server: bool,
    pub mac: Option<MacAddr>,
    pub names: Vec<(String, u8, bool)>,
}

pub fn build_nbstat_request(id: u16) -> Vec<u8> {
    let mut p = Vec::with_capacity(50);
    p.extend_from_slice(&id.to_be_bytes());
    p.extend_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    // Encoded wildcard name "*" padded with NULs (first-level encoding).
    p.push(0x20);
    let mut name = [0u8; 16];
    name[0] = b'*';
    for b in name {
        p.push(b'A' + (b >> 4));
        p.push(b'A' + (b & 0x0f));
    }
    p.push(0x00);
    p.extend_from_slice(&[0x00, 0x21, 0x00, 0x01]); // NBSTAT, IN
    p
}

pub fn parse_nbstat_response(buf: &[u8], id: u16) -> Option<NetbiosInfo> {
    if buf.len() < 57 || u16::from_be_bytes([buf[0], buf[1]]) != id {
        return None;
    }
    // Skip header (12) + answer name.
    let mut pos = 12;
    let first = *buf.get(pos)?;
    if first & 0xC0 == 0xC0 {
        pos += 2;
    } else {
        while *buf.get(pos)? != 0 {
            pos += 1 + *buf.get(pos)? as usize;
        }
        pos += 1;
    }
    let rtype = u16::from_be_bytes([*buf.get(pos)?, *buf.get(pos + 1)?]);
    if rtype != 0x21 {
        return None;
    }
    pos += 2 + 2 + 4 + 2; // type, class, ttl, rdlength
    let count = *buf.get(pos)? as usize;
    pos += 1;
    let mut info = NetbiosInfo::default();
    for _ in 0..count {
        let entry = buf.get(pos..pos + 18)?;
        let name = String::from_utf8_lossy(&entry[..15]).trim_end().to_string();
        let suffix = entry[15];
        let flags = u16::from_be_bytes([entry[16], entry[17]]);
        let group = flags & 0x8000 != 0;
        match (suffix, group) {
            (0x00, false) if info.name.is_none() => info.name = Some(name.clone()),
            (0x00, true) if info.workgroup.is_none() => info.workgroup = Some(name.clone()),
            (0x20, false) => info.file_server = true,
            _ => {}
        }
        info.names.push((name, suffix, group));
        pos += 18;
    }
    if let Some(mac) = buf.get(pos..pos + 6) {
        let m = MacAddr([mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]]);
        if !m.is_unusable() {
            info.mac = Some(m);
        }
    }
    if info.name.is_none() {
        info.name = info.names.iter().find(|n| !n.2).map(|n| n.0.clone());
    }
    Some(info)
}

pub async fn query(ip: IpAddr, timeout: Duration) -> Option<NetbiosInfo> {
    let IpAddr::V4(_) = ip else { return None };
    let sock = tokio::net::UdpSocket::bind("0.0.0.0:0").await.ok()?;
    let id = 0x4e42 ^ (std::process::id() as u16);
    let req = build_nbstat_request(id);
    let target = SocketAddr::new(ip, 137);
    sock.send_to(&req, target).await.ok()?;
    let mut buf = vec![0u8; 1024];
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let (n, from) = tokio::time::timeout_at(deadline, sock.recv_from(&mut buf)).await.ok()?.ok()?;
        if from.ip() != ip {
            continue;
        }
        if let Some(info) = parse_nbstat_response(&buf[..n], id) {
            return Some(info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_layout() {
        let r = build_nbstat_request(0x1234);
        assert_eq!(r.len(), 50);
        assert_eq!(&r[13..15], b"CK"); // '*' = 0x2A -> 'C','K'
        assert_eq!(&r[15..17], b"AA");
    }

    #[test]
    fn parses_response() {
        let mut r = build_nbstat_request(0x1234);
        r[2] = 0x84;
        r[5] = 0; // QDCOUNT 0
        r[7] = 1; // ANCOUNT 1
        let mut entries = Vec::new();
        let mut push = |name: &str, suffix: u8, flags: u16| {
            let mut n = format!("{name:<15}").into_bytes();
            n.truncate(15);
            entries.extend_from_slice(&n);
            entries.push(suffix);
            entries.extend_from_slice(&flags.to_be_bytes());
        };
        push("PATRICK", 0x00, 0x0400);
        push("WORKGROUP", 0x00, 0x8400);
        push("PATRICK", 0x20, 0x0400);
        let mut resp = r[..12].to_vec();
        resp.extend_from_slice(&r[12..46]); // name
        resp.extend_from_slice(&[0x00, 0x21, 0x00, 0x01, 0, 0, 0, 0]);
        let rdlen = 1 + entries.len() + 6;
        resp.extend_from_slice(&(rdlen as u16).to_be_bytes());
        resp.push(3);
        resp.extend_from_slice(&entries);
        resp.extend_from_slice(&[0x00, 0x1F, 0xD0, 0x12, 0x34, 0x56]);
        let info = parse_nbstat_response(&resp, 0x1234).unwrap();
        assert_eq!(info.name.as_deref(), Some("PATRICK"));
        assert_eq!(info.workgroup.as_deref(), Some("WORKGROUP"));
        assert!(info.file_server);
        assert_eq!(info.mac.unwrap().to_string(), "00:1F:D0:12:34:56");
    }
}
