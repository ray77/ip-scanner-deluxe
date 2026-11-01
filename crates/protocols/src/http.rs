//! Small HTTP/1.x client for device metadata: status, Server header, page
//! title and redirect target. At most 64 KiB are read per response.

use crate::tls::{handshake, CertInfo};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

pub const MAX_RESPONSE: usize = 64 * 1024;
const USER_AGENT: &str = concat!("IPScannerDeluxe/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpInfo {
    pub status: Option<u16>,
    pub server: Option<String>,
    pub title: Option<String>,
    pub location: Option<String>,
    pub realm: Option<String>,
    pub tls: Option<CertInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl RawResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Parse a raw HTTP/1.x response (headers + possibly chunked body).
pub fn parse_response(raw: &[u8]) -> Option<RawResponse> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(&raw[..split]);
    let mut lines = head.split("\r\n");
    let status_line = lines.next()?;
    let mut parts = status_line.split_whitespace();
    let proto = parts.next()?;
    if !proto.starts_with("HTTP/") {
        return None;
    }
    let status: u16 = parts.next()?.parse().ok()?;
    let headers: Vec<(String, String)> =
        lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let mut body = raw[split + 4..].to_vec();
    let chunked = headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.to_lowercase().contains("chunked"));
    if chunked {
        body = dechunk(&body);
    }
    Some(RawResponse { status, headers, body })
}

fn dechunk(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut pos = 0;
    while pos < data.len() {
        let Some(eol) = data[pos..].windows(2).position(|w| w == b"\r\n") else { break };
        let line = String::from_utf8_lossy(&data[pos..pos + eol]);
        let size_str = line.split(';').next().unwrap_or("").trim();
        let Ok(size) = usize::from_str_radix(size_str, 16) else { break };
        pos += eol + 2;
        if size == 0 {
            break;
        }
        let end = (pos + size).min(data.len());
        out.extend_from_slice(&data[pos..end]);
        pos = end + 2;
    }
    out
}

fn decode_entities(s: &str) -> String {
    let mut out = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");
    // Numeric entities (&#33; &#x21;)
    while let Some(start) = out.find("&#") {
        let Some(len) = out[start..].find(';') else { break };
        let ent = &out[start + 2..start + len];
        let code = if let Some(hex) = ent.strip_prefix(['x', 'X']) { u32::from_str_radix(hex, 16).ok() } else { ent.parse().ok() };
        let rep = code.and_then(char::from_u32).map(String::from).unwrap_or_default();
        out.replace_range(start..start + len + 1, &rep);
    }
    out
}

/// Extract `<title>` from an HTML body.
pub fn extract_title(body: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(body);
    let lower = text.to_lowercase();
    let start = lower.find("<title")?;
    let open_end = start + lower[start..].find('>')? + 1;
    let close = open_end + lower[open_end..].find("</title")?;
    let title = decode_entities(&text[open_end..close]);
    let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
    (!title.is_empty()).then(|| title.chars().take(200).collect())
}

async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(s: &mut S, req: &[u8]) -> Option<Vec<u8>> {
    s.write_all(req).await.ok()?;
    s.flush().await.ok()?;
    let mut buf = Vec::with_capacity(8192);
    let mut chunk = [0u8; 8192];
    loop {
        match s.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() >= MAX_RESPONSE {
                    buf.truncate(MAX_RESPONSE);
                    break;
                }
            }
            Err(_) => break,
        }
    }
    (!buf.is_empty()).then_some(buf)
}

/// Perform one request and return raw bytes (+ certificate for TLS).
pub async fn request(ip: IpAddr, port: u16, tls: bool, req: &[u8], timeout: Duration) -> Option<(Vec<u8>, Option<CertInfo>)> {
    let fut = async {
        let tcp = TcpStream::connect(SocketAddr::new(ip, port)).await.ok()?;
        let _ = tcp.set_nodelay(true);
        if tls {
            let (mut s, cert) = handshake(tcp, ip).await?;
            let data = exchange(&mut s, req).await;
            Some((data.unwrap_or_default(), cert))
        } else {
            let mut s = tcp;
            Some((exchange(&mut s, req).await?, None))
        }
    };
    tokio::time::timeout(timeout, fut).await.ok().flatten()
}

fn host_header(ip: IpAddr, port: u16, tls: bool) -> String {
    let h = match ip {
        IpAddr::V4(v) => v.to_string(),
        IpAddr::V6(v) => format!("[{v}]"),
    };
    if (tls && port == 443) || (!tls && port == 80) {
        h
    } else {
        format!("{h}:{port}")
    }
}

pub fn get_request(ip: IpAddr, port: u16, tls: bool, path: &str) -> Vec<u8> {
    format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nUser-Agent: {USER_AGENT}\r\nAccept: text/html,*/*;q=0.8\r\nConnection: close\r\n\r\n",
        host_header(ip, port, tls)
    )
    .into_bytes()
}

/// Probe `/` and follow up to two same-host redirects for the title.
pub async fn probe(ip: IpAddr, port: u16, tls: bool, timeout: Duration) -> Option<HttpInfo> {
    let mut info = HttpInfo::default();
    let mut path = "/".to_string();
    for _ in 0..3 {
        let (raw, cert) = request(ip, port, tls, &get_request(ip, port, tls, &path), timeout).await?;
        if info.tls.is_none() {
            info.tls = cert;
        }
        let Some(resp) = parse_response(&raw) else {
            // TLS handshake succeeded but no HTTP: still report the port.
            return (info.tls.is_some()).then_some(info);
        };
        if info.status.is_none() {
            info.status = Some(resp.status);
            info.server = resp.header("server").map(str::to_string);
            info.location = resp.header("location").map(str::to_string);
        }
        if info.realm.is_none() {
            info.realm = resp
                .header("www-authenticate")
                .and_then(|v| v.split("realm=").nth(1))
                .map(|r| r.trim_matches('"').split('"').next().unwrap_or("").to_string())
                .filter(|r| !r.is_empty());
        }
        if let Some(t) = extract_title(&resp.body) {
            info.title = Some(t);
            break;
        }
        if !(300..400).contains(&resp.status) {
            break;
        }
        // Follow relative or same-host redirects only.
        let Some(loc) = resp.header("location") else { break };
        let next = if loc.starts_with('/') {
            loc.to_string()
        } else if let Some(rest) = loc.split("://").nth(1) {
            let (host, p) = rest.split_once('/').unwrap_or((rest, ""));
            let host_only = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
            if host_only.trim_matches(['[', ']']) != ip.to_string() {
                break;
            }
            if loc.starts_with("https") != tls {
                break;
            }
            format!("/{p}")
        } else {
            break;
        };
        if next == path {
            break;
        }
        path = next;
    }
    if info.title.is_none() {
        info.title = info.realm.clone();
    }
    Some(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_response() {
        let raw = b"HTTP/1.1 200 OK\r\nServer: lighttpd\r\nContent-Type: text/html\r\n\r\n<html><head><TITLE>\n FRITZ!Box &amp; Co </TITLE></head></html>";
        let r = parse_response(raw).unwrap();
        assert_eq!(r.status, 200);
        assert_eq!(r.header("SERVER"), Some("lighttpd"));
        assert_eq!(extract_title(&r.body).as_deref(), Some("FRITZ!Box & Co"));
    }

    #[test]
    fn parses_chunked() {
        let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\n<title>\r\n9\r\nSynology!\r\n8\r\n</title>\r\n0\r\n\r\n";
        let r = parse_response(raw).unwrap();
        assert_eq!(extract_title(&r.body).as_deref(), Some("Synology!"));
    }

    #[test]
    fn numeric_entities() {
        assert_eq!(decode_entities("A&#33;B&#x21;"), "A!B!");
    }

    #[test]
    fn host_headers() {
        let ip: IpAddr = "192.168.0.1".parse().unwrap();
        assert_eq!(host_header(ip, 80, false), "192.168.0.1");
        assert_eq!(host_header(ip, 8080, false), "192.168.0.1:8080");
        assert_eq!(host_header(ip, 443, true), "192.168.0.1");
    }
}
