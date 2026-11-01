//! Service banner grabbing (first line a server sends after connect).

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Read the greeting of a service. For protocols that need a client hello
/// first (none of the defaults), `hello` can be supplied.
pub async fn grab(ip: IpAddr, port: u16, hello: Option<&[u8]>, timeout: Duration) -> Option<String> {
    let fut = async {
        let mut s = TcpStream::connect(SocketAddr::new(ip, port)).await.ok()?;
        if let Some(h) = hello {
            s.write_all(h).await.ok()?;
        }
        let mut buf = [0u8; 512];
        let n = s.read(&mut buf).await.ok()?;
        sanitize(&buf[..n], port)
    };
    tokio::time::timeout(timeout, fut).await.ok().flatten()
}

/// Turn raw greeting bytes into a printable single line.
pub fn sanitize(raw: &[u8], port: u16) -> Option<String> {
    // MySQL/MariaDB handshake: [len:3][seq:1][proto:1][version\0]
    if port == 3306 && raw.len() > 6 && raw[4] == 0x0a {
        let v: String = raw[5..].iter().take_while(|b| **b != 0).map(|b| *b as char).collect();
        return (!v.is_empty()).then(|| format!("MySQL {v}"));
    }
    // Telnet negotiation (IAC sequences) is skipped.
    let mut out = String::new();
    let mut i = 0;
    while i < raw.len() {
        let b = raw[i];
        if b == 0xff {
            i += 3;
            continue;
        }
        if b == b'\n' || b == b'\r' {
            if !out.trim().is_empty() {
                break;
            }
        } else if (0x20..0x7f).contains(&b) {
            out.push(b as char);
        }
        i += 1;
    }
    let t = out.trim().to_string();
    (!t.is_empty()).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes() {
        assert_eq!(sanitize(b"SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13\r\n", 22).as_deref(), Some("SSH-2.0-OpenSSH_9.6p1 Ubuntu-3ubuntu13"));
        assert_eq!(sanitize(b"220 FRITZ!Box FTP server ready.\r\n", 21).as_deref(), Some("220 FRITZ!Box FTP server ready."));
        assert_eq!(sanitize(&[0xff, 0xfd, 0x18, b'l', b'o', b'g', b'i', b'n', b':'], 23).as_deref(), Some("login:"));
        let mysql = [0x4a, 0, 0, 0, 0x0a, b'8', b'.', b'0', 0, 1, 2];
        assert_eq!(sanitize(&mysql, 3306).as_deref(), Some("MySQL 8.0"));
    }
}
