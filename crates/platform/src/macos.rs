//! macOS specifics: ARP table via the routing socket sysctl
//! (`CTL_NET, PF_ROUTE, 0, AF_INET, NET_RT_FLAGS, RTF_LLINFO`).

use std::io;
use std::net::{IpAddr, Ipv4Addr};
use vendor_db::MacAddr;

const CTL_NET: libc::c_int = 4;
const PF_ROUTE: libc::c_int = 17;
const NET_RT_FLAGS: libc::c_int = 2;
const RTF_LLINFO: libc::c_int = 0x400;

fn roundup(a: usize) -> usize {
    // Apple aligns routing socket addresses to 4 bytes.
    if a == 0 {
        4
    } else {
        1 + ((a - 1) | 3)
    }
}

pub(crate) fn neighbor_table() -> io::Result<Vec<(IpAddr, MacAddr)>> {
    let mut mib = [CTL_NET, PF_ROUTE, 0, libc::AF_INET, NET_RT_FLAGS, RTF_LLINFO];
    let mut needed: libc::size_t = 0;
    // SAFETY: sysctl with null buffer only writes the required size.
    let rc = unsafe { libc::sysctl(mib.as_mut_ptr(), 6, std::ptr::null_mut(), &mut needed, std::ptr::null_mut(), 0) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    if needed == 0 {
        return Ok(Vec::new());
    }
    // Table may grow between the two calls.
    needed += needed / 2 + 512;
    let mut buf = vec![0u8; needed];
    // SAFETY: buffer is `needed` bytes long; sysctl updates `needed`.
    let rc = unsafe { libc::sysctl(mib.as_mut_ptr(), 6, buf.as_mut_ptr().cast(), &mut needed, std::ptr::null_mut(), 0) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    buf.truncate(needed);
    Ok(parse_rt_dump(&buf))
}

pub(crate) fn parse_rt_dump(buf: &[u8]) -> Vec<(IpAddr, MacAddr)> {
    let hdr = std::mem::size_of::<libc::rt_msghdr>();
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 2 <= buf.len() {
        let msglen = u16::from_ne_bytes([buf[pos], buf[pos + 1]]) as usize;
        if msglen == 0 || pos + msglen > buf.len() {
            break;
        }
        let msg = &buf[pos..pos + msglen];
        pos += msglen;
        if msg.len() < hdr + 8 {
            continue;
        }
        // sockaddr_inarp: len, family, port(2), addr(4)
        let sin = &msg[hdr..];
        let sin_len = sin[0] as usize;
        if sin[1] as i32 != libc::AF_INET || sin.len() < 8 {
            continue;
        }
        let ip = Ipv4Addr::new(sin[4], sin[5], sin[6], sin[7]);
        // sockaddr_dl: len, family, index(2), type, nlen, alen, slen, data...
        let sdl_off = roundup(sin_len);
        let Some(sdl) = sin.get(sdl_off..) else { continue };
        if sdl.len() < 8 || sdl[1] as i32 != libc::AF_LINK {
            continue;
        }
        let nlen = sdl[5] as usize;
        let alen = sdl[6] as usize;
        if alen != 6 {
            continue; // incomplete entry
        }
        let Some(a) = sdl.get(8 + nlen..8 + nlen + 6) else { continue };
        let mac = MacAddr([a[0], a[1], a[2], a[3], a[4], a[5]]);
        if !mac.is_unusable() {
            out.push((IpAddr::V4(ip), mac));
        }
    }
    out
}
