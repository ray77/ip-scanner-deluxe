//! Linux specifics: neighbor table from the kernel (`/proc/net/arp`).

use std::io;
use std::net::{IpAddr, Ipv4Addr};
use vendor_db::MacAddr;

const ATF_COM: u32 = 0x2;

pub(crate) fn parse_proc_arp(text: &str) -> Vec<(IpAddr, MacAddr)> {
    text.lines()
        .skip(1)
        .filter_map(|l| {
            let cols: Vec<&str> = l.split_whitespace().collect();
            if cols.len() < 4 {
                return None;
            }
            let ip: Ipv4Addr = cols[0].parse().ok()?;
            let flags = u32::from_str_radix(cols[2].trim_start_matches("0x"), 16).ok()?;
            if flags & ATF_COM == 0 {
                return None;
            }
            let mac = MacAddr::parse(cols[3]).ok()?;
            (!mac.is_unusable()).then_some((IpAddr::V4(ip), mac))
        })
        .collect()
}

pub(crate) fn neighbor_table() -> io::Result<Vec<(IpAddr, MacAddr)>> {
    let text = std::fs::read_to_string("/proc/net/arp")?;
    Ok(parse_proc_arp(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses() {
        let t = "IP address       HW type     Flags       HW address            Mask     Device\n\
192.168.1.1      0x1         0x2         00:11:22:33:44:55     *        eth0\n\
192.168.1.7      0x1         0x0         00:00:00:00:00:00     *        eth0\n";
        let r = parse_proc_arp(t);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1.to_string(), "00:11:22:33:44:55");
    }
}
