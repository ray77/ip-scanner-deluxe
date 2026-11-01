//! Windows specifics via the IP Helper API (no `arp.exe` / `ping.exe`).

use crate::icmp::{IcmpError, IcmpReply, ReplyKind, PAYLOAD};
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};
use vendor_db::MacAddr;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::NetworkManagement::IpHelper::{
    GetIpNetTable, IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, SendARP, ICMP_ECHO_REPLY, IP_OPTION_INFORMATION, MIB_IPNETTABLE,
};

const NO_ERROR: u32 = 0;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const IP_SUCCESS: u32 = 0;
const IP_DEST_NET_UNREACHABLE: u32 = 11002;
const IP_DEST_HOST_UNREACHABLE: u32 = 11003;
const IP_DEST_PROT_UNREACHABLE: u32 = 11004;
const IP_DEST_PORT_UNREACHABLE: u32 = 11005;
const IP_REQ_TIMED_OUT: u32 = 11010;
const IP_TTL_EXPIRED_TRANSIT: u32 = 11013;

fn ipv4_to_u32(ip: Ipv4Addr) -> u32 {
    // IPAddr is in network byte order.
    u32::from_ne_bytes(ip.octets())
}

fn u32_to_ipv4(v: u32) -> Ipv4Addr {
    Ipv4Addr::from(v.to_ne_bytes())
}

/// Active ARP request through `SendARP`.
pub(crate) fn send_arp(ip: Ipv4Addr) -> io::Result<Option<MacAddr>> {
    let mut mac = [0u8; 8];
    let mut len: u32 = mac.len() as u32;
    // SAFETY: buffer and length are valid for the call.
    let rc = unsafe { SendARP(ipv4_to_u32(ip), 0, mac.as_mut_ptr().cast(), &mut len) };
    if rc != NO_ERROR || len != 6 {
        return Ok(None);
    }
    let m = MacAddr([mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]]);
    Ok((!m.is_unusable()).then_some(m))
}

pub(crate) fn neighbor_table() -> io::Result<Vec<(IpAddr, MacAddr)>> {
    let mut size: u32 = 0;
    // SAFETY: querying the required size with a null buffer.
    let rc = unsafe { GetIpNetTable(std::ptr::null_mut(), &mut size, 0) };
    if rc != ERROR_INSUFFICIENT_BUFFER && rc != NO_ERROR {
        return Err(io::Error::from_raw_os_error(rc as i32));
    }
    let mut buf = vec![0u8; size as usize + 1024];
    let mut size = buf.len() as u32;
    // SAFETY: buffer has `size` bytes.
    let rc = unsafe { GetIpNetTable(buf.as_mut_ptr().cast::<MIB_IPNETTABLE>(), &mut size, 0) };
    if rc != NO_ERROR {
        return Err(io::Error::from_raw_os_error(rc as i32));
    }
    // Layout: dwNumEntries, then MIB_IPNETROW { index, physlen, phys[8], addr, type } (24 bytes)
    let count = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let off = 4 + i * 24;
        let Some(row) = buf.get(off..off + 24) else { break };
        let physlen = u32::from_ne_bytes([row[4], row[5], row[6], row[7]]);
        let addr = u32::from_ne_bytes([row[16], row[17], row[18], row[19]]);
        let ty = u32::from_ne_bytes([row[20], row[21], row[22], row[23]]);
        if physlen != 6 || ty == 2 {
            continue; // invalid entry
        }
        let mac = MacAddr([row[8], row[9], row[10], row[11], row[12], row[13]]);
        if !mac.is_unusable() {
            out.push((IpAddr::V4(u32_to_ipv4(addr)), mac));
        }
    }
    Ok(out)
}

pub(crate) fn icmp_handle_ok() -> bool {
    // SAFETY: plain handle creation/close.
    unsafe {
        let h = IcmpCreateFile();
        if h == INVALID_HANDLE_VALUE {
            return false;
        }
        IcmpCloseHandle(h);
        true
    }
}

pub(crate) fn icmp_echo(target: Ipv4Addr, ttl: Option<u8>, timeout: Duration) -> Result<IcmpReply, IcmpError> {
    // SAFETY: handle lifetime is limited to this function.
    let handle = unsafe { IcmpCreateFile() };
    if handle == INVALID_HANDLE_VALUE {
        return Err(IcmpError::Io(io::Error::last_os_error()));
    }
    let opts = IP_OPTION_INFORMATION { Ttl: ttl.unwrap_or(128), Tos: 0, Flags: 0, OptionsSize: 0, OptionsData: std::ptr::null_mut() };
    let reply_size = std::mem::size_of::<ICMP_ECHO_REPLY>() + PAYLOAD.len() + 8 + 64;
    let mut reply = vec![0u8; reply_size];
    let start = Instant::now();
    // SAFETY: all pointers reference live buffers of the given sizes.
    let n = unsafe {
        IcmpSendEcho(
            handle,
            ipv4_to_u32(target),
            PAYLOAD.as_ptr().cast(),
            PAYLOAD.len() as u16,
            &opts,
            reply.as_mut_ptr().cast(),
            reply_size as u32,
            timeout.as_millis().clamp(1, u32::MAX as u128) as u32,
        )
    };
    let elapsed = start.elapsed();
    // SAFETY: handle from IcmpCreateFile.
    unsafe { IcmpCloseHandle(handle) };
    if n == 0 {
        return Err(IcmpError::Timeout);
    }
    // SAFETY: on success the buffer starts with an ICMP_ECHO_REPLY.
    let r: ICMP_ECHO_REPLY = unsafe { std::ptr::read_unaligned(reply.as_ptr().cast()) };
    let from = IpAddr::V4(u32_to_ipv4(r.Address));
    let rtt = if r.RoundTripTime == 0 { elapsed } else { Duration::from_millis(u64::from(r.RoundTripTime)) };
    let kind = match r.Status {
        IP_SUCCESS => ReplyKind::EchoReply,
        IP_TTL_EXPIRED_TRANSIT => ReplyKind::TimeExceeded,
        IP_DEST_NET_UNREACHABLE => ReplyKind::Unreachable(0),
        IP_DEST_HOST_UNREACHABLE => ReplyKind::Unreachable(1),
        IP_DEST_PROT_UNREACHABLE => ReplyKind::Unreachable(2),
        IP_DEST_PORT_UNREACHABLE => ReplyKind::Unreachable(3),
        IP_REQ_TIMED_OUT => return Err(IcmpError::Timeout),
        _ => return Err(IcmpError::Timeout),
    };
    Ok(IcmpReply { from, rtt, kind })
}

/// `InitiateSystemShutdownExW` against a remote machine. Uses the
/// credentials of the current Windows session; nothing is stored.
pub(crate) fn remote_shutdown(machine: &str, message: &str, timeout_secs: u32, force: bool, reboot: bool) -> io::Result<()> {
    use windows_sys::Win32::System::Shutdown::InitiateSystemShutdownExW;
    let to_wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let m = to_wide(&format!("\\\\{machine}"));
    let msg = to_wide(message);
    const SHTDN_REASON_FLAG_PLANNED: u32 = 0x8000_0000;
    // SAFETY: both strings are NUL terminated UTF-16 buffers.
    let ok = unsafe {
        InitiateSystemShutdownExW(m.as_ptr(), msg.as_ptr(), timeout_secs, i32::from(force), i32::from(reboot), SHTDN_REASON_FLAG_PLANNED)
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn abort_shutdown(machine: &str) -> io::Result<()> {
    use windows_sys::Win32::System::Shutdown::AbortSystemShutdownW;
    let m: Vec<u16> = format!("\\\\{machine}").encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: NUL terminated UTF-16 buffer.
    let ok = unsafe { AbortSystemShutdownW(m.as_ptr()) };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
