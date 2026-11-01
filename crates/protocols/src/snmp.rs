//! Read-only SNMPv2c GET of the system group. Only used when the user has
//! explicitly configured a community; no community lists are tried.

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnmpSystem {
    pub sys_descr: Option<String>,
    pub sys_object_id: Option<String>,
    pub sys_name: Option<String>,
    pub sys_location: Option<String>,
}

const OIDS: [&[u32]; 4] = [
    &[1, 3, 6, 1, 2, 1, 1, 1, 0], // sysDescr
    &[1, 3, 6, 1, 2, 1, 1, 2, 0], // sysObjectID
    &[1, 3, 6, 1, 2, 1, 1, 5, 0], // sysName
    &[1, 3, 6, 1, 2, 1, 1, 6, 0], // sysLocation
];

fn enc_len(out: &mut Vec<u8>, n: usize) {
    if n < 0x80 {
        out.push(n as u8);
    } else if n < 0x100 {
        out.extend_from_slice(&[0x81, n as u8]);
    } else {
        out.extend_from_slice(&[0x82, (n >> 8) as u8, n as u8]);
    }
}

fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut v = vec![tag];
    enc_len(&mut v, content.len());
    v.extend_from_slice(content);
    v
}

fn enc_int(v: i64) -> Vec<u8> {
    let mut bytes = v.to_be_bytes().to_vec();
    while bytes.len() > 1 && ((bytes[0] == 0 && bytes[1] & 0x80 == 0) || (bytes[0] == 0xff && bytes[1] & 0x80 != 0)) {
        bytes.remove(0);
    }
    tlv(0x02, &bytes)
}

fn enc_oid(oid: &[u32]) -> Vec<u8> {
    let mut c = vec![(oid[0] * 40 + oid[1]) as u8];
    for &arc in &oid[2..] {
        let mut stack = vec![(arc & 0x7f) as u8];
        let mut a = arc >> 7;
        while a > 0 {
            stack.push(((a & 0x7f) as u8) | 0x80);
            a >>= 7;
        }
        c.extend(stack.iter().rev());
    }
    tlv(0x06, &c)
}

pub fn build_get(community: &str, request_id: i32) -> Vec<u8> {
    let mut vbs = Vec::new();
    for oid in OIDS {
        let mut vb = enc_oid(oid);
        vb.extend_from_slice(&[0x05, 0x00]);
        vbs.extend(tlv(0x30, &vb));
    }
    let mut pdu = enc_int(i64::from(request_id));
    pdu.extend(enc_int(0));
    pdu.extend(enc_int(0));
    pdu.extend(tlv(0x30, &vbs));
    let mut msg = enc_int(1); // version: v2c
    msg.extend(tlv(0x04, community.as_bytes()));
    msg.extend(tlv(0xA0, &pdu));
    tlv(0x30, &msg)
}

/// Read one TLV: (tag, content, rest).
fn read_tlv(b: &[u8]) -> Option<(u8, &[u8], &[u8])> {
    let tag = *b.first()?;
    let first = *b.get(1)? as usize;
    let (len, hdr) = if first & 0x80 == 0 {
        (first, 2)
    } else {
        let n = first & 0x7f;
        if n == 0 || n > 3 {
            return None;
        }
        let mut l = 0usize;
        for i in 0..n {
            l = (l << 8) | *b.get(2 + i)? as usize;
        }
        (l, 2 + n)
    };
    let content = b.get(hdr..hdr + len)?;
    Some((tag, content, &b[hdr + len..]))
}

fn dec_oid(c: &[u8]) -> String {
    let Some(first) = c.first() else { return String::new() };
    let mut parts = vec![u32::from(*first / 40), u32::from(*first % 40)];
    let mut acc = 0u32;
    for &b in &c[1..] {
        acc = (acc << 7) | u32::from(b & 0x7f);
        if b & 0x80 == 0 {
            parts.push(acc);
            acc = 0;
        }
    }
    parts.iter().map(u32::to_string).collect::<Vec<_>>().join(".")
}

pub fn parse_response(b: &[u8]) -> Option<SnmpSystem> {
    let (t, msg, _) = read_tlv(b)?;
    if t != 0x30 {
        return None;
    }
    let (_, _version, rest) = read_tlv(msg)?;
    let (_, _community, rest) = read_tlv(rest)?;
    let (pdu_tag, pdu, _) = read_tlv(rest)?;
    if pdu_tag != 0xA2 {
        return None;
    }
    let (_, _rid, rest) = read_tlv(pdu)?;
    let (_, err, rest) = read_tlv(rest)?;
    if err.iter().any(|b| *b != 0) {
        return None;
    }
    let (_, _idx, rest) = read_tlv(rest)?;
    let (_, mut vbs, _) = read_tlv(rest)?;
    let mut sys = SnmpSystem::default();
    while !vbs.is_empty() {
        let (_, vb, rest) = read_tlv(vbs)?;
        vbs = rest;
        let (_, oid, rest) = read_tlv(vb)?;
        let (vt, val, _) = read_tlv(rest)?;
        let oid = dec_oid(oid);
        let text = match vt {
            0x04 => String::from_utf8_lossy(val).trim().to_string(),
            0x06 => dec_oid(val),
            _ => continue,
        };
        if text.is_empty() {
            continue;
        }
        match oid.as_str() {
            "1.3.6.1.2.1.1.1.0" => sys.sys_descr = Some(text),
            "1.3.6.1.2.1.1.2.0" => sys.sys_object_id = Some(text),
            "1.3.6.1.2.1.1.5.0" => sys.sys_name = Some(text),
            "1.3.6.1.2.1.1.6.0" => sys.sys_location = Some(text),
            _ => {}
        }
    }
    Some(sys)
}

pub async fn get_system(ip: IpAddr, community: &str, timeout: Duration) -> Option<SnmpSystem> {
    let bind = if ip.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
    let sock = tokio::net::UdpSocket::bind(bind).await.ok()?;
    let req = build_get(community, 0x5253);
    sock.send_to(&req, SocketAddr::new(ip, 161)).await.ok()?;
    let mut buf = vec![0u8; 4096];
    let (n, from) = tokio::time::timeout(timeout, sock.recv_from(&mut buf)).await.ok()?.ok()?;
    if from.ip() != ip {
        return None;
    }
    parse_response(&buf[..n])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oid_roundtrip() {
        let e = enc_oid(&[1, 3, 6, 1, 4, 1, 11, 2, 3, 7, 11, 160]);
        let (_, c, _) = read_tlv(&e).unwrap();
        assert_eq!(dec_oid(c), "1.3.6.1.4.1.11.2.3.7.11.160");
    }

    #[test]
    fn parses_get_response() {
        let mut vb = enc_oid(OIDS[0]);
        vb.extend(tlv(0x04, b"HP J9019B Switch 2510-24"));
        let vbs = tlv(0x30, &vb);
        let mut pdu = enc_int(0x5253);
        pdu.extend(enc_int(0));
        pdu.extend(enc_int(0));
        pdu.extend(tlv(0x30, &vbs));
        let mut msg = enc_int(1);
        msg.extend(tlv(0x04, b"public"));
        msg.extend(tlv(0xA2, &pdu));
        let resp = tlv(0x30, &msg);
        let s = parse_response(&resp).unwrap();
        assert_eq!(s.sys_descr.as_deref(), Some("HP J9019B Switch 2510-24"));
        assert!(build_get("public", 1).len() > 40);
    }
}
