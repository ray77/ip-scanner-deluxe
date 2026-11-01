//! IPP Get-Printer-Attributes (RFC 8011) for exact printer models.

use crate::http::{parse_response, request};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IppInfo {
    pub printer_name: Option<String>,
    pub printer_info: Option<String>,
    pub printer_location: Option<String>,
    pub make_and_model: Option<String>,
    pub state: Option<String>,
}

const REQUESTED: [&str; 5] = ["printer-name", "printer-info", "printer-location", "printer-make-and-model", "printer-state"];

fn attr(out: &mut Vec<u8>, tag: u8, name: &str, value: &[u8]) {
    out.push(tag);
    out.extend_from_slice(&(name.len() as u16).to_be_bytes());
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(value);
}

pub fn build_request(printer_uri: &str) -> Vec<u8> {
    let mut b = vec![0x02, 0x00, 0x00, 0x0B, 0x00, 0x00, 0x00, 0x01, 0x01];
    attr(&mut b, 0x47, "attributes-charset", b"utf-8");
    attr(&mut b, 0x48, "attributes-natural-language", b"en");
    attr(&mut b, 0x45, "printer-uri", printer_uri.as_bytes());
    for (i, a) in REQUESTED.iter().enumerate() {
        // Additional values of a multi-valued attribute have an empty name.
        let name = if i == 0 { "requested-attributes" } else { "" };
        attr(&mut b, 0x44, name, a.as_bytes());
    }
    b.push(0x03);
    b
}

pub fn parse_attributes(body: &[u8]) -> Option<BTreeMap<String, String>> {
    if body.len() < 8 {
        return None;
    }
    let status = u16::from_be_bytes([body[2], body[3]]);
    if status > 0x00FF {
        return None;
    }
    let mut pos = 8;
    let mut out = BTreeMap::new();
    let mut last_name = String::new();
    while pos < body.len() {
        let tag = body[pos];
        pos += 1;
        if tag == 0x03 {
            break;
        }
        if tag < 0x10 {
            continue; // delimiter tag
        }
        let nl = u16::from_be_bytes([*body.get(pos)?, *body.get(pos + 1)?]) as usize;
        pos += 2;
        let name = String::from_utf8_lossy(body.get(pos..pos + nl)?).into_owned();
        pos += nl;
        let vl = u16::from_be_bytes([*body.get(pos)?, *body.get(pos + 1)?]) as usize;
        pos += 2;
        let value = body.get(pos..pos + vl)?;
        pos += vl;
        if !name.is_empty() {
            last_name = name;
        }
        let text = match tag {
            0x21 | 0x23 if vl == 4 => i32::from_be_bytes([value[0], value[1], value[2], value[3]]).to_string(),
            0x35 | 0x36 => {
                // textWithLanguage: lang-len, lang, text-len, text
                let ll = u16::from_be_bytes([*value.first()?, *value.get(1)?]) as usize;
                let t = value.get(2 + ll + 2..).unwrap_or_default();
                String::from_utf8_lossy(t).into_owned()
            }
            _ => String::from_utf8_lossy(value).into_owned(),
        };
        out.entry(last_name.clone()).or_insert(text);
    }
    Some(out)
}

fn state_name(v: &str) -> String {
    match v {
        "3" => "idle".into(),
        "4" => "processing".into(),
        "5" => "stopped".into(),
        other => other.into(),
    }
}

/// Query a printer on port 631. Tries the common resource paths.
pub async fn get_printer_attributes(ip: IpAddr, port: u16, timeout: Duration) -> Option<IppInfo> {
    let host = match ip {
        IpAddr::V4(v) => v.to_string(),
        IpAddr::V6(v) => format!("[{v}]"),
    };
    for path in ["/ipp/print", "/ipp", "/"] {
        let uri = format!("ipp://{host}:{port}{path}");
        let body = build_request(&uri);
        let mut req = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/ipp\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        req.extend_from_slice(&body);
        let Some((raw, _)) = request(ip, port, false, &req, timeout).await else { continue };
        let Some(resp) = parse_response(&raw) else { continue };
        if resp.status != 200 {
            continue;
        }
        let Some(attrs) = parse_attributes(&resp.body) else { continue };
        let get = |k: &str| attrs.get(k).filter(|v| !v.trim().is_empty()).cloned();
        let info = IppInfo {
            printer_name: get("printer-name"),
            printer_info: get("printer-info"),
            printer_location: get("printer-location"),
            make_and_model: get("printer-make-and-model"),
            state: get("printer-state").map(|s| state_name(&s)),
        };
        if info.make_and_model.is_some() || info.printer_name.is_some() {
            return Some(info);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_attributes() {
        let mut resp = vec![0x02, 0x00, 0x00, 0x00, 0, 0, 0, 1, 0x01];
        attr(&mut resp, 0x47, "attributes-charset", b"utf-8");
        resp.push(0x04);
        attr(&mut resp, 0x41, "printer-make-and-model", b"HP Color LaserJet 9500");
        attr(&mut resp, 0x23, "printer-state", &3i32.to_be_bytes());
        resp.push(0x03);
        let a = parse_attributes(&resp).unwrap();
        assert_eq!(a["printer-make-and-model"], "HP Color LaserJet 9500");
        assert_eq!(state_name(&a["printer-state"]), "idle");
        let req = build_request("ipp://1.2.3.4:631/ipp/print");
        assert_eq!(&req[..4], &[0x02, 0x00, 0x00, 0x0B]);
        assert_eq!(*req.last().unwrap(), 0x03);
    }
}
