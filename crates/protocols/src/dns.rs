//! Minimal DNS message encoding/decoding (RFC 1035) used for mDNS legacy
//! unicast and LLMNR reverse lookups, plus system reverse DNS.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;

pub const TYPE_A: u16 = 1;
pub const TYPE_PTR: u16 = 12;
pub const TYPE_TXT: u16 = 16;
pub const TYPE_AAAA: u16 = 28;
pub const TYPE_SRV: u16 = 33;
pub const CLASS_IN: u16 = 1;
/// mDNS "unicast response requested" bit in the question class.
pub const CLASS_QU: u16 = 0x8000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RData {
    A(Ipv4Addr),
    Aaaa(Ipv6Addr),
    Ptr(String),
    Txt(Vec<String>),
    Srv { priority: u16, weight: u16, port: u16, target: String },
    Other(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    pub name: String,
    pub rtype: u16,
    pub data: RData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub id: u16,
    pub flags: u16,
    pub records: Vec<Record>,
}

/// `d.c.b.a.in-addr.arpa` / nibble format for IPv6.
pub fn reverse_name(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            format!("{}.{}.{}.{}.in-addr.arpa", o[3], o[2], o[1], o[0])
        }
        IpAddr::V6(v6) => {
            let mut s = String::with_capacity(72);
            for b in v6.octets().iter().rev() {
                s.push_str(&format!("{:x}.{:x}.", b & 0xf, b >> 4));
            }
            s.push_str("ip6.arpa");
            s
        }
    }
}

fn encode_name(out: &mut Vec<u8>, name: &str) {
    for label in name.trim_end_matches('.').split('.') {
        if label.is_empty() {
            continue;
        }
        let bytes = label.as_bytes();
        let len = bytes.len().min(63);
        out.push(len as u8);
        out.extend_from_slice(&bytes[..len]);
    }
    out.push(0);
}

/// Build a single-question query.
pub fn build_query(id: u16, name: &str, qtype: u16, qclass: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(&id.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes()); // flags: standard query
    out.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    out.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // AN/NS/AR
    encode_name(&mut out, name);
    out.extend_from_slice(&qtype.to_be_bytes());
    out.extend_from_slice(&qclass.to_be_bytes());
    out
}

fn read_u16(buf: &[u8], pos: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*buf.get(pos)?, *buf.get(pos + 1)?]))
}

/// Decode a possibly compressed name. Returns (name, position after name).
pub fn read_name(buf: &[u8], mut pos: usize) -> Option<(String, usize)> {
    let mut labels: Vec<String> = Vec::new();
    let mut end = None;
    let mut jumps = 0;
    loop {
        let len = *buf.get(pos)? as usize;
        if len == 0 {
            pos += 1;
            break;
        }
        if len & 0xC0 == 0xC0 {
            let ptr = ((len & 0x3F) << 8) | *buf.get(pos + 1)? as usize;
            if end.is_none() {
                end = Some(pos + 2);
            }
            jumps += 1;
            if jumps > 32 || ptr >= buf.len() {
                return None;
            }
            pos = ptr;
            continue;
        }
        let label = buf.get(pos + 1..pos + 1 + len)?;
        labels.push(String::from_utf8_lossy(label).into_owned());
        pos += 1 + len;
        if labels.len() > 128 {
            return None;
        }
    }
    Some((labels.join("."), end.unwrap_or(pos)))
}

/// Parse a response, returning answer, authority and additional records.
pub fn parse_message(buf: &[u8]) -> Option<Message> {
    if buf.len() < 12 {
        return None;
    }
    let id = read_u16(buf, 0)?;
    let flags = read_u16(buf, 2)?;
    let qd = read_u16(buf, 4)? as usize;
    let rr = read_u16(buf, 6)? as usize + read_u16(buf, 8)? as usize + read_u16(buf, 10)? as usize;
    let mut pos = 12;
    for _ in 0..qd {
        let (_, p) = read_name(buf, pos)?;
        pos = p + 4;
    }
    let mut records = Vec::new();
    for _ in 0..rr.min(256) {
        let (name, p) = read_name(buf, pos)?;
        let rtype = read_u16(buf, p)?;
        let rdlen = read_u16(buf, p + 8)? as usize;
        let rd_start = p + 10;
        let rdata = buf.get(rd_start..rd_start + rdlen)?;
        let data = match rtype {
            TYPE_A if rdlen == 4 => RData::A(Ipv4Addr::new(rdata[0], rdata[1], rdata[2], rdata[3])),
            TYPE_AAAA if rdlen == 16 => {
                let mut o = [0u8; 16];
                o.copy_from_slice(rdata);
                RData::Aaaa(Ipv6Addr::from(o))
            }
            TYPE_PTR => RData::Ptr(read_name(buf, rd_start)?.0),
            TYPE_TXT => {
                let mut strings = Vec::new();
                let mut i = 0;
                while i < rdata.len() {
                    let l = rdata[i] as usize;
                    if let Some(s) = rdata.get(i + 1..i + 1 + l) {
                        strings.push(String::from_utf8_lossy(s).into_owned());
                    }
                    i += 1 + l;
                }
                RData::Txt(strings)
            }
            TYPE_SRV if rdlen >= 7 => RData::Srv {
                priority: read_u16(buf, rd_start)?,
                weight: read_u16(buf, rd_start + 2)?,
                port: read_u16(buf, rd_start + 4)?,
                target: read_name(buf, rd_start + 6)?.0,
            },
            t => RData::Other(t),
        };
        records.push(Record { name, rtype, data });
        pos = rd_start + rdlen;
    }
    Some(Message { id, flags, records })
}

/// Send a PTR query for `ip` to `server` and return the first PTR target.
pub async fn unicast_ptr_query(ip: IpAddr, server: std::net::SocketAddr, qclass: u16, timeout: Duration) -> Option<String> {
    let bind: std::net::SocketAddr = if server.is_ipv4() { (Ipv4Addr::UNSPECIFIED, 0).into() } else { (Ipv6Addr::UNSPECIFIED, 0).into() };
    let sock = tokio::net::UdpSocket::bind(bind).await.ok()?;
    let id = (std::process::id() as u16) ^ (ip_hash(ip) as u16);
    let rname = reverse_name(ip);
    let q = build_query(id, &rname, TYPE_PTR, qclass);
    sock.send_to(&q, server).await.ok()?;
    let mut buf = vec![0u8; 1500];
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let (n, from) = tokio::time::timeout_at(deadline, sock.recv_from(&mut buf)).await.ok()?.ok()?;
        if from.ip() != server.ip() {
            continue;
        }
        let Some(msg) = parse_message(&buf[..n]) else { continue };
        // mDNS responders use id 0 in multicast responses, echo it otherwise.
        if msg.id != id && msg.id != 0 {
            continue;
        }
        for r in msg.records {
            if let RData::Ptr(target) = r.data {
                if r.name.eq_ignore_ascii_case(&rname) && !target.is_empty() {
                    return Some(target.trim_end_matches('.').to_string());
                }
            }
        }
    }
}

fn ip_hash(ip: IpAddr) -> u32 {
    match ip {
        IpAddr::V4(v) => u32::from(v),
        IpAddr::V6(v) => v.octets().iter().fold(0u32, |a, b| a.rotate_left(5) ^ u32::from(*b)),
    }
}

/// Reverse DNS through the operating system resolver (getnameinfo).
pub async fn reverse_dns(ip: IpAddr, timeout: Duration) -> Option<String> {
    let task = tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&ip).ok());
    let name = tokio::time::timeout(timeout, task).await.ok()?.ok()??;
    let name = name.trim_end_matches('.').to_string();
    if name.is_empty() || name.parse::<IpAddr>().is_ok() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_names() {
        assert_eq!(reverse_name("192.168.0.110".parse().unwrap()), "110.0.168.192.in-addr.arpa");
        let v6 = reverse_name("2001:db8::1".parse().unwrap());
        assert!(v6.starts_with("1.0.0.0."));
        assert!(v6.ends_with("8.b.d.0.1.0.0.2.ip6.arpa"));
    }

    #[test]
    fn query_roundtrip_with_compression() {
        let q = build_query(0x1234, "110.0.168.192.in-addr.arpa", TYPE_PTR, CLASS_IN);
        // Build a response: copy question, add answer with pointer to question name.
        let mut r = q.clone();
        r[2] = 0x84; // response, authoritative
        r[7] = 1; // ANCOUNT
        r.extend_from_slice(&[0xC0, 12]); // pointer to name at offset 12
        r.extend_from_slice(&TYPE_PTR.to_be_bytes());
        r.extend_from_slice(&CLASS_IN.to_be_bytes());
        r.extend_from_slice(&120u32.to_be_bytes());
        let mut rdata = Vec::new();
        encode_name(&mut rdata, "patrick.local");
        r.extend_from_slice(&(rdata.len() as u16).to_be_bytes());
        r.extend_from_slice(&rdata);
        let m = parse_message(&r).unwrap();
        assert_eq!(m.id, 0x1234);
        assert_eq!(m.records.len(), 1);
        assert_eq!(m.records[0].name, "110.0.168.192.in-addr.arpa");
        assert_eq!(m.records[0].data, RData::Ptr("patrick.local".into()));
    }

    #[test]
    fn pointer_loop_rejected() {
        let mut buf = vec![0u8; 12];
        buf.extend_from_slice(&[0xC0, 12]);
        assert!(read_name(&buf, 12).is_none());
    }
}
