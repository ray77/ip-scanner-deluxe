//! Export (CSV, HTML, JSON, XML) and import of result and favorite lists.

use crate::state::{class_label, fmt_time, vendor_label};
use anyhow::{bail, Context, Result};
use scan_core::{HostRecord, HostStatus};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::Path;
use storage::FavoriteDevice;
use ui::commands::ExportFormat;
use vendor_db::MacAddr;

/// One exported row (spec field list).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportRow {
    #[serde(rename = "Status")]
    pub status: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "IP")]
    pub ip: String,
    #[serde(rename = "MAC")]
    pub mac: String,
    #[serde(rename = "Vendor")]
    pub vendor: String,
    #[serde(rename = "DeviceClass")]
    pub device_class: String,
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "Confidence")]
    pub confidence: String,
    #[serde(rename = "Latency")]
    pub latency: String,
    #[serde(rename = "Services")]
    pub services: String,
    #[serde(rename = "Comment")]
    pub comment: String,
    #[serde(rename = "LastSeen")]
    pub last_seen: String,
}

pub fn status_str(s: HostStatus) -> &'static str {
    match s {
        HostStatus::Alive | HostStatus::Scanning => "Alive",
        HostStatus::NoResponse => "NoResponse",
        HostStatus::Unknown => "Unknown",
    }
}

pub fn to_row(h: &HostRecord, name: &str) -> ExportRow {
    ExportRow {
        status: status_str(h.status).into(),
        name: name.into(),
        ip: h.ip.to_string(),
        mac: h.mac.map(|m| m.to_string()).unwrap_or_default(),
        vendor: vendor_label(h),
        device_class: class_label(h),
        model: h.model.clone().unwrap_or_default(),
        confidence: if h.fingerprint_confidence > 0 { h.fingerprint_confidence.to_string() } else { String::new() },
        latency: h.latency_ms.map(|l| format!("{l:.1}")).unwrap_or_default(),
        services: h.services.iter().map(|s| s.display_name.clone()).collect::<Vec<_>>().join("; "),
        comment: h.comment.clone(),
        last_seen: fmt_time(h.last_seen),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

pub fn write_csv(rows: &[ExportRow], bom: bool) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    if bom {
        out.extend_from_slice(b"\xEF\xBB\xBF");
    }
    let mut w = csv::WriterBuilder::new().delimiter(b';').from_writer(&mut out);
    for r in rows {
        w.serialize(r)?;
    }
    w.flush()?;
    drop(w);
    Ok(out)
}

pub fn write_html(rows: &[ExportRow]) -> String {
    let mut s = String::from(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><title>IP Scanner Deluxe</title>\n<style>body{font-family:Segoe UI,Tahoma,sans-serif;font-size:13px}table{border-collapse:collapse}th,td{border:1px solid #ccc;padding:3px 8px;text-align:left}th{background:#f2f2f2}</style></head><body>\n<table>\n<tr>",
    );
    for h in ["Status", "Name", "IP", "MAC", "Vendor", "DeviceClass", "Model", "Confidence", "Latency", "Services", "Comment", "LastSeen"] {
        s.push_str(&format!("<th>{h}</th>"));
    }
    s.push_str("</tr>\n");
    for r in rows {
        s.push_str("<tr>");
        for v in [
            &r.status,
            &r.name,
            &r.ip,
            &r.mac,
            &r.vendor,
            &r.device_class,
            &r.model,
            &r.confidence,
            &r.latency,
            &r.services,
            &r.comment,
            &r.last_seen,
        ] {
            s.push_str(&format!("<td>{}</td>", esc(v)));
        }
        s.push_str("</tr>\n");
    }
    s.push_str("</table>\n</body></html>\n");
    s
}

pub fn write_xml(rows: &[ExportRow]) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<scan>\n");
    for r in rows {
        s.push_str("  <host>\n");
        for (k, v) in [
            ("status", &r.status),
            ("name", &r.name),
            ("ip", &r.ip),
            ("mac", &r.mac),
            ("vendor", &r.vendor),
            ("deviceclass", &r.device_class),
            ("model", &r.model),
            ("confidence", &r.confidence),
            ("latency", &r.latency),
            ("services", &r.services),
            ("comment", &r.comment),
            ("lastseen", &r.last_seen),
        ] {
            s.push_str(&format!("    <{k}>{}</{k}>\n", esc(v)));
        }
        s.push_str("  </host>\n");
    }
    s.push_str("</scan>\n");
    s
}

/// Full-fidelity JSON (all host details) so lists can be loaded again.
pub fn write_json(hosts: &[HostRecord]) -> Result<String> {
    Ok(serde_json::to_string_pretty(hosts)?)
}

pub fn export(path: &Path, fmt: ExportFormat, hosts: &[(HostRecord, String)], bom: bool) -> Result<()> {
    let rows: Vec<ExportRow> = hosts.iter().map(|(h, n)| to_row(h, n)).collect();
    let data = match fmt {
        ExportFormat::Csv => write_csv(&rows, bom)?,
        ExportFormat::Html => write_html(&rows).into_bytes(),
        ExportFormat::Xml => write_xml(&rows).into_bytes(),
        ExportFormat::Json => write_json(&hosts.iter().map(|(h, _)| h.clone()).collect::<Vec<_>>())?.into_bytes(),
    };
    std::fs::write(path, data).with_context(|| path.display().to_string())
}

// ---- minimal XML reader for our own formats ------------------------------

fn unesc(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn elements<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(a) = rest.find(&open) {
        let after = &rest[a + open.len()..];
        let Some(b) = after.find(&close) else { break };
        out.push(&after[..b]);
        rest = &after[b + close.len()..];
    }
    out
}

fn field(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let a = block.find(&open)? + open.len();
    let b = block[a..].find(&close)? + a;
    let v = unesc(block[a..b].trim());
    (!v.is_empty()).then_some(v)
}

pub fn favorites_to_xml(favs: &[FavoriteDevice]) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<favorites>\n");
    for f in favs {
        s.push_str("  <device>\n");
        let opt = |o: Option<String>| o.unwrap_or_default();
        for (k, v) in [
            ("name", opt(f.name.clone())),
            ("ip", opt(f.ip.map(|i| i.to_string()))),
            ("mac", opt(f.mac.map(|m| m.to_string()))),
            ("vendor", opt(f.vendor.clone())),
            ("comment", f.comment.clone()),
        ] {
            s.push_str(&format!("    <{k}>{}</{k}>\n", esc(&v)));
        }
        s.push_str("  </device>\n");
    }
    s.push_str("</favorites>\n");
    s
}

pub fn favorites_from_xml(xml: &str) -> Result<Vec<FavoriteDevice>> {
    let devs = elements(xml, "device");
    if devs.is_empty() && !xml.contains("<favorites") {
        bail!("keine Favoritenliste");
    }
    Ok(devs
        .into_iter()
        .map(|d| FavoriteDevice {
            name: field(d, "name"),
            ip: field(d, "ip").and_then(|s| s.parse().ok()),
            mac: field(d, "mac").and_then(|s| MacAddr::parse(&s).ok()),
            vendor: field(d, "vendor"),
            comment: field(d, "comment").unwrap_or_default(),
            last_seen: None,
        })
        .filter(|f| f.ip.is_some() || f.mac.is_some())
        .collect())
}

/// Load a list saved as JSON (full) or XML/CSV (summary).
pub fn load_list(path: &Path) -> Result<Vec<HostRecord>> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    if text.trim_start().starts_with('[') {
        let mut hosts: Vec<HostRecord> = serde_json::from_str(text)?;
        for h in &mut hosts {
            if h.status == HostStatus::Scanning {
                h.status = HostStatus::Alive;
            }
        }
        return Ok(hosts);
    }
    let parse_status = |s: &str| match s {
        "Alive" => HostStatus::Alive,
        "NoResponse" => HostStatus::NoResponse,
        _ => HostStatus::Unknown,
    };
    let from_fields = |ip: Option<String>,
                       status: Option<String>,
                       name: Option<String>,
                       mac: Option<String>,
                       vendor: Option<String>,
                       comment: Option<String>|
     -> Option<HostRecord> {
        let ip: IpAddr = ip?.parse().ok()?;
        let mut h = HostRecord::new(ip, parse_status(status.as_deref().unwrap_or("")));
        h.hostname = name.filter(|n| n != &ip.to_string());
        h.mac = mac.and_then(|m| MacAddr::parse(&m).ok());
        h.vendor = vendor;
        h.comment = comment.unwrap_or_default();
        Some(h)
    };
    if text.contains("<scan>") {
        return Ok(elements(text, "host")
            .into_iter()
            .filter_map(|b| {
                from_fields(field(b, "ip"), field(b, "status"), field(b, "name"), field(b, "mac"), field(b, "vendor"), field(b, "comment"))
            })
            .collect());
    }
    // CSV
    let mut rdr = csv::ReaderBuilder::new().delimiter(b';').flexible(true).from_reader(text.as_bytes());
    let mut out = Vec::new();
    for r in rdr.deserialize::<ExportRow>() {
        let r = r?;
        let some = |s: String| (!s.is_empty()).then_some(s);
        if let Some(h) = from_fields(Some(r.ip), Some(r.status), some(r.name), some(r.mac), some(r.vendor), some(r.comment)) {
            out.push(h);
        }
    }
    if out.is_empty() {
        bail!("keine Einträge gefunden");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> HostRecord {
        let mut h = HostRecord::new("192.168.0.96".parse().unwrap(), HostStatus::Alive);
        h.mac = MacAddr::parse("3C:52:82:11:22:33").ok();
        h.vendor = Some("Hewlett Packard".into());
        h.hostname = Some("PC01".into());
        h.comment = "Büro <2>".into();
        h
    }

    #[test]
    fn csv_has_spec_fields() {
        let rows = vec![to_row(&host(), "PC01")];
        let csv = String::from_utf8(write_csv(&rows, false).unwrap()).unwrap();
        let header = csv.lines().next().unwrap();
        assert_eq!(header, "Status;Name;IP;MAC;Vendor;DeviceClass;Model;Confidence;Latency;Services;Comment;LastSeen");
        assert!(csv.contains("192.168.0.96"));
        let bom = write_csv(&rows, true).unwrap();
        assert_eq!(&bom[..3], b"\xEF\xBB\xBF");
    }

    #[test]
    fn xml_roundtrip_favorites() {
        let f = FavoriteDevice::from_host(&host());
        let xml = favorites_to_xml(std::slice::from_ref(&f));
        let back = favorites_from_xml(&xml).unwrap();
        assert_eq!(back[0].mac, f.mac);
        assert_eq!(back[0].comment, "Büro <2>");
    }

    #[test]
    fn list_roundtrip() {
        let dir = std::env::temp_dir().join(format!("ipsd-exp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for fmt in [ExportFormat::Json, ExportFormat::Xml, ExportFormat::Csv] {
            let p = dir.join(format!("list.{}", fmt.ext()));
            export(&p, fmt, &[(host(), "PC01".into())], true).unwrap();
            let back = load_list(&p).unwrap();
            assert_eq!(back.len(), 1, "{fmt:?}");
            assert_eq!(back[0].ip, host().ip);
            assert_eq!(back[0].mac, host().mac);
        }
        let html = write_html(&[to_row(&host(), "PC01")]);
        assert!(html.contains("&lt;2&gt;"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
