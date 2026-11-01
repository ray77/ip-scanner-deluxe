//! Opening hosts with local clients (browser, SSH, RDP, FTP, file manager).
//!
//! This is the only place that starts external programs, and only on an
//! explicit user action. Templates use `{ip}`, `{host}` and `{port}`.

use serde::{Deserialize, Serialize};
use std::io;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ClientKind {
    Http,
    Https,
    Ftp,
    Ssh,
    Telnet,
    Rdp,
    Vnc,
    Smb,
}

impl ClientKind {
    pub const ALL: [ClientKind; 8] = [
        ClientKind::Http,
        ClientKind::Https,
        ClientKind::Ftp,
        ClientKind::Ssh,
        ClientKind::Telnet,
        ClientKind::Rdp,
        ClientKind::Vnc,
        ClientKind::Smb,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ClientKind::Http => "HTTP",
            ClientKind::Https => "HTTPS",
            ClientKind::Ftp => "FTP",
            ClientKind::Ssh => "SSH",
            ClientKind::Telnet => "Telnet",
            ClientKind::Rdp => "RDP",
            ClientKind::Vnc => "VNC",
            ClientKind::Smb => "SMB",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            ClientKind::Http => 80,
            ClientKind::Https => 443,
            ClientKind::Ftp => 21,
            ClientKind::Ssh => 22,
            ClientKind::Telnet => 23,
            ClientKind::Rdp => 3389,
            ClientKind::Vnc => 5900,
            ClientKind::Smb => 445,
        }
    }
}

/// Default command template per platform. Empty = system URL handler.
pub fn default_template(kind: ClientKind) -> &'static str {
    match kind {
        ClientKind::Http | ClientKind::Https | ClientKind::Ftp | ClientKind::Vnc => "",
        #[cfg(windows)]
        ClientKind::Ssh => "cmd /c start \"\" ssh {ip} -p {port}",
        #[cfg(windows)]
        ClientKind::Telnet => "cmd /c start \"\" telnet {ip} {port}",
        #[cfg(windows)]
        ClientKind::Rdp => "mstsc /v:{ip}:{port}",
        #[cfg(windows)]
        ClientKind::Smb => "explorer \\\\{ip}\\",
        #[cfg(target_os = "linux")]
        ClientKind::Ssh => "x-terminal-emulator -e ssh {ip} -p {port}",
        #[cfg(target_os = "linux")]
        ClientKind::Telnet => "x-terminal-emulator -e telnet {ip} {port}",
        #[cfg(target_os = "linux")]
        ClientKind::Rdp => "xfreerdp /v:{ip}:{port}",
        #[cfg(not(any(windows, target_os = "linux")))]
        ClientKind::Ssh | ClientKind::Telnet | ClientKind::Rdp => "",
        #[cfg(not(windows))]
        ClientKind::Smb => "",
    }
}

/// URL used when no command template is configured.
pub fn url_for(kind: ClientKind, host: &str, port: u16) -> String {
    let h = if host.contains(':') && !host.starts_with('[') { format!("[{host}]") } else { host.to_string() };
    let with_port = |scheme: &str, default: u16| {
        if port == default {
            format!("{scheme}://{h}/")
        } else {
            format!("{scheme}://{h}:{port}/")
        }
    };
    match kind {
        ClientKind::Http => with_port("http", 80),
        ClientKind::Https => with_port("https", 443),
        ClientKind::Ftp => with_port("ftp", 21),
        ClientKind::Ssh => format!("ssh://{h}:{port}"),
        ClientKind::Telnet => format!("telnet://{h}:{port}"),
        ClientKind::Rdp => format!("rdp://full%20address=s:{h}:{port}"),
        ClientKind::Vnc => format!("vnc://{h}:{port}"),
        ClientKind::Smb => format!("smb://{h}/"),
    }
}

/// Only allow characters that can never be interpreted by a shell or
/// command parser (hostnames come from the network).
pub fn safe_host(s: &str) -> Option<&str> {
    (!s.is_empty() && s.len() <= 253 && s.chars().all(|c| c.is_ascii_alphanumeric() || ".-_:[]%".contains(c))).then_some(s)
}

/// Split a template into program and arguments (supports double quotes).
pub fn split_template(t: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut had_quotes = false;
    for c in t.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                had_quotes = true;
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() || had_quotes {
                    out.push(std::mem::take(&mut cur));
                }
                had_quotes = false;
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() || had_quotes {
        out.push(cur);
    }
    out
}

/// Open `host` with the client for `kind`.
pub fn open_client(kind: ClientKind, ip: &str, host: Option<&str>, port: u16, template: Option<&str>) -> io::Result<()> {
    let ip = safe_host(ip).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid address"))?;
    let host = host.and_then(safe_host).unwrap_or(ip);
    let template = template.filter(|t| !t.trim().is_empty()).unwrap_or(default_template(kind));
    if template.trim().is_empty() {
        return open::that_detached(url_for(kind, ip, port));
    }
    let parts: Vec<String> = split_template(template)
        .into_iter()
        .map(|p| p.replace("{ip}", ip).replace("{host}", host).replace("{port}", &port.to_string()))
        .collect();
    let (prog, args) = parts.split_first().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty template"))?;
    Command::new(prog).args(args).spawn().map(|_| ())
}

/// Open a plain URL (e.g. a `uri` announced via mDNS) in the default handler.
pub fn open_url(url: &str) -> io::Result<()> {
    let ok = ["http://", "https://", "ftp://", "smb://", "ssh://", "vnc://"].iter().any(|p| url.starts_with(p));
    if !ok {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsupported URL scheme"));
    }
    open::that_detached(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_templates() {
        assert_eq!(split_template("cmd /c start \"\" ssh {ip}"), vec!["cmd", "/c", "start", "", "ssh", "{ip}"]);
        assert_eq!(
            split_template("\"C:\\Program Files\\PuTTY\\putty.exe\" -ssh {ip}"),
            vec!["C:\\Program Files\\PuTTY\\putty.exe", "-ssh", "{ip}"]
        );
    }

    #[test]
    fn host_sanitizing() {
        assert!(safe_host("192.168.0.1").is_some());
        assert!(safe_host("nas.fritz.box").is_some());
        assert!(safe_host("evil & calc").is_none());
        assert!(safe_host("a;rm -rf").is_none());
    }

    #[test]
    fn urls() {
        assert_eq!(url_for(ClientKind::Http, "10.0.0.1", 80), "http://10.0.0.1/");
        assert_eq!(url_for(ClientKind::Https, "10.0.0.1", 8443), "https://10.0.0.1:8443/");
        assert_eq!(url_for(ClientKind::Smb, "nas", 445), "smb://nas/");
    }
}
