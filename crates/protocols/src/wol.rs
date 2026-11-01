//! Wake-on-LAN magic packets.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use vendor_db::MacAddr;

/// `FF FF FF FF FF FF` followed by 16 repetitions of the MAC.
pub fn magic_packet(mac: &MacAddr) -> [u8; 102] {
    let mut p = [0xffu8; 102];
    for i in 0..16 {
        p[6 + i * 6..12 + i * 6].copy_from_slice(&mac.0);
    }
    p
}

/// Send the magic packet to each broadcast address and port.
pub fn send(mac: &MacAddr, broadcasts: &[Ipv4Addr], ports: &[u16]) -> std::io::Result<usize> {
    let sock = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    sock.set_broadcast(true)?;
    let pkt = magic_packet(mac);
    let mut sent = 0;
    let mut last_err = None;
    for b in broadcasts {
        for p in ports {
            match sock.send_to(&pkt, SocketAddr::from((*b, *p))) {
                Ok(_) => sent += 1,
                Err(e) => last_err = Some(e),
            }
        }
    }
    match (sent, last_err) {
        (0, Some(e)) => Err(e),
        _ => Ok(sent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_layout() {
        let mac = MacAddr([0x00, 0x1f, 0xd0, 0x12, 0x34, 0x56]);
        let p = magic_packet(&mac);
        assert_eq!(&p[..6], &[0xff; 6]);
        for i in 0..16 {
            assert_eq!(&p[6 + i * 6..12 + i * 6], &mac.0);
        }
    }
}
