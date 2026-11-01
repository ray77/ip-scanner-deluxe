//! Screenshot mode for visual regression tests and documentation:
//! `IPScannerDeluxe --demo --theme dark --expand --screenshot out.png`

use std::io::Write;
use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct CliOptions {
    pub demo: bool,
    pub expand: bool,
    pub dark: Option<bool>,
    pub screenshot: Option<PathBuf>,
    pub size: Option<[f32; 2]>,
    pub lang_en: bool,
    pub dialog: Option<String>,
    pub write_icon: Option<PathBuf>,
}

pub fn parse_args() -> CliOptions {
    let mut o = CliOptions::default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--demo" => o.demo = true,
            "--expand" => o.expand = true,
            "--english" => o.lang_en = true,
            "--theme" => o.dark = args.next().map(|t| t == "dark"),
            "--screenshot" => o.screenshot = args.next().map(PathBuf::from),
            "--dialog" => o.dialog = args.next(),
            "--write-icon" => o.write_icon = args.next().map(PathBuf::from),
            "--size" => {
                o.size = args.next().and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some([w.parse().ok()?, h.parse().ok()?])
                })
            }
            _ => {}
        }
    }
    o
}

/// Hosts matching the reference screenshot.
pub fn demo_hosts() -> Vec<scan_core::HostRecord> {
    use scan_core::{DeviceClass, HostRecord, HostStatus, ServiceRecord, ServiceType};
    use vendor_db::MacAddr;
    let mk = |ip: &str, name: &str, vendor: &str, mac: &str, class: DeviceClass| {
        let mut h = HostRecord::new(ip.parse::<IpAddr>().unwrap_or(IpAddr::from([0, 0, 0, 0])), HostStatus::Alive);
        h.hostname = Some(name.into());
        h.vendor = Some(vendor.into());
        h.mac = MacAddr::parse(mac).ok();
        h.device_class = class;
        h.latency_ms = Some(1.2);
        h
    };
    let mut router = mk("192.168.0.90", "fritz.box", "AVM GmbH", "C8:0E:14:12:7A:4B", DeviceClass::Router);
    router.model = Some("FRITZ!Box".into());
    router.fingerprint_confidence = 96;
    router.services.push(ServiceRecord::new(ServiceType::Http, Some(80), "HTTP, FRITZ!Box").with_uri("http://192.168.0.90/"));
    let mut pc = mk("192.168.0.96", "PC01", "Hewlett Packard", "48:0F:CF:5A:21:0C", DeviceClass::Desktop);
    pc.services.push(ServiceRecord::new(ServiceType::Shares, Some(445), "Freigaben"));
    let mut sw = mk("192.168.0.102", "Switch", "Hewlett Packard", "CC:3E:5F:4D:91:80", DeviceClass::Switch);
    sw.services.push(ServiceRecord::new(ServiceType::Http, Some(80), "HTTP, HP 1820-24G Switch J9980A"));
    sw.services.push(ServiceRecord::new(ServiceType::Ssh, Some(22), "SSH"));
    let mut pr = mk("192.168.0.105", "NPI3F2A1B", "HP Inc.", "3C:52:82:3F:2A:1B", DeviceClass::Printer);
    pr.model = Some("HP Color LaserJet 9500".into());
    pr.services.push(ServiceRecord::new(ServiceType::Http, Some(80), "HTTP, HP Color LaserJet 9500"));
    pr.services.push(ServiceRecord::new(ServiceType::JetDirect, Some(9100), "JetDirect"));
    let mut pat = mk("192.168.0.110", "PATRICK", "GIGA-BYTE TECHNOLOGY CO.,LTD.", "00:1F:D0:83:47:2E", DeviceClass::Desktop);
    pat.comment = "Büro".into();
    for (t, p, n) in [
        (ServiceType::Https, 443, "HTTPS, IIS Windows Server"),
        (ServiceType::Ftp, 21, "FTP, Microsoft FTP Service"),
        (ServiceType::Rdp, 3389, "RDP"),
        (ServiceType::Shares, 445, "Freigaben"),
    ] {
        pat.services.push(ServiceRecord::new(t, Some(p), n));
    }
    vec![router, pc, sw, pr, pat]
}

/// Minimal PNG encoder (stored deflate blocks) so no image crate is needed.
pub fn write_png(path: &std::path::Path, width: usize, height: usize, rgba: &[u8]) -> std::io::Result<()> {
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for b in data {
            c ^= u32::from(*b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = kind.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    }
    let mut raw = Vec::with_capacity((width * 4 + 1) * height);
    for y in 0..height {
        raw.push(0);
        raw.extend_from_slice(&rgba[y * width * 4..(y + 1) * width * 4]);
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        z.push(u8::from(last));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for x in &raw {
        a = (a + u32::from(*x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    std::fs::File::create(path)?.write_all(&out)
}
