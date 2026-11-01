//! Native ICMP echo (ping / traceroute) without external programs.
//!
//! - Windows: `IcmpSendEcho` (IP Helper API), works without admin rights.
//! - macOS: unprivileged `SOCK_DGRAM` ICMP sockets, raw sockets as fallback.
//! - Linux: `SOCK_DGRAM` ICMP ("ping sockets", allowed by
//!   `net.ipv4.ping_group_range` on most distributions) with `IP_RECVERR`
//!   for TTL-exceeded messages; raw sockets with `CAP_NET_RAW`.
//!
//! All functions are blocking; use [`echo_async`] from async code.

use std::net::IpAddr;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyKind {
    EchoReply,
    TimeExceeded,
    Unreachable(u8),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcmpReply {
    pub from: IpAddr,
    pub rtt: Duration,
    pub kind: ReplyKind,
}

#[derive(Debug, thiserror::Error)]
pub enum IcmpError {
    #[error("timeout")]
    Timeout,
    #[error("ICMP not permitted: {0}")]
    NotPermitted(String),
    #[error("ICMP not supported for this address family")]
    Unsupported,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcmpCapability {
    /// Works without privileges (ping sockets / IcmpSendEcho).
    Unprivileged,
    /// Raw sockets (process is privileged).
    Raw,
    Unavailable(String),
}

impl IcmpCapability {
    pub fn is_available(&self) -> bool {
        !matches!(self, IcmpCapability::Unavailable(_))
    }
}

/// Detected once per process.
pub fn capability() -> IcmpCapability {
    static CAP: OnceLock<IcmpCapability> = OnceLock::new();
    CAP.get_or_init(detect).clone()
}

/// Whether raw sockets can be opened (diagnostics only).
pub fn raw_socket_available() -> bool {
    #[cfg(unix)]
    {
        use socket2::{Domain, Protocol, Socket, Type};
        Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)).is_ok()
    }
    #[cfg(windows)]
    {
        false
    }
}

#[cfg(unix)]
fn detect() -> IcmpCapability {
    use socket2::{Domain, Protocol, Socket, Type};
    if Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::ICMPV4)).is_ok() {
        IcmpCapability::Unprivileged
    } else if Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)).is_ok() {
        IcmpCapability::Raw
    } else {
        IcmpCapability::Unavailable(std::io::Error::last_os_error().to_string())
    }
}

#[cfg(windows)]
fn detect() -> IcmpCapability {
    if crate::windows::icmp_handle_ok() {
        IcmpCapability::Unprivileged
    } else {
        IcmpCapability::Unavailable("IcmpCreateFile failed".into())
    }
}

/// Send one echo request and wait for the matching reply (or a TTL
/// exceeded / unreachable message caused by it).
pub fn echo(target: IpAddr, ttl: Option<u8>, timeout: Duration, seq: u16) -> Result<IcmpReply, IcmpError> {
    #[cfg(unix)]
    {
        unix::echo(target, ttl, timeout, seq)
    }
    #[cfg(windows)]
    {
        let _ = seq;
        match target {
            IpAddr::V4(v4) => crate::windows::icmp_echo(v4, ttl, timeout),
            IpAddr::V6(_) => Err(IcmpError::Unsupported),
        }
    }
}

pub async fn echo_async(target: IpAddr, ttl: Option<u8>, timeout: Duration, seq: u16) -> Result<IcmpReply, IcmpError> {
    tokio::task::spawn_blocking(move || echo(target, ttl, timeout, seq))
        .await
        .map_err(|e| IcmpError::Io(std::io::Error::other(e.to_string())))?
}

/// Standard payload (same length as Windows ping).
pub(crate) const PAYLOAD: &[u8; 32] = b"abcdefghijklmnopqrstuvwabcdefghi";

#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;
    for chunk in data.chunks(2) {
        let word = if chunk.len() == 2 { u16::from_be_bytes([chunk[0], chunk[1]]) } else { u16::from_be_bytes([chunk[0], 0]) };
        sum += u32::from(word);
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn build_echo(v6: bool, id: u16, seq: u16) -> Vec<u8> {
    let mut p = Vec::with_capacity(8 + PAYLOAD.len());
    p.push(if v6 { 128 } else { 8 });
    p.push(0);
    p.extend_from_slice(&[0, 0]);
    p.extend_from_slice(&id.to_be_bytes());
    p.extend_from_slice(&seq.to_be_bytes());
    p.extend_from_slice(PAYLOAD);
    if !v6 {
        let c = checksum(&p);
        p[2..4].copy_from_slice(&c.to_be_bytes());
    }
    p
}

/// Classify a received ICMP message (without IP header). Returns the kind
/// if it refers to our echo (`id` is ignored when `check_id` is false).
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn classify(v6: bool, icmp: &[u8], id: u16, seq: u16, check_id: bool) -> Option<ReplyKind> {
    if icmp.len() < 8 {
        return None;
    }
    let (t_reply, t_exceeded, t_unreach, t_request) = if v6 { (129, 3, 1, 128) } else { (0, 11, 3, 8) };
    let matches = |hdr: &[u8]| -> bool {
        hdr.len() >= 8 && u16::from_be_bytes([hdr[6], hdr[7]]) == seq && (!check_id || u16::from_be_bytes([hdr[4], hdr[5]]) == id)
    };
    let t = icmp[0];
    if t == t_reply {
        return matches(icmp).then_some(ReplyKind::EchoReply);
    }
    if t == t_exceeded || t == t_unreach {
        let inner = &icmp[8..];
        let inner_icmp = if v6 {
            inner.get(40..)?
        } else {
            let ihl = usize::from(inner.first()? & 0x0f) * 4;
            inner.get(ihl..)?
        };
        if inner_icmp.first() != Some(&t_request) || !matches(inner_icmp) {
            return None;
        }
        return Some(if t == t_exceeded { ReplyKind::TimeExceeded } else { ReplyKind::Unreachable(icmp[1]) });
    }
    None
}

#[cfg(unix)]
mod unix {
    use super::*;
    use socket2::{Domain, Protocol, SockAddr, Socket, Type};
    use std::mem::MaybeUninit;
    use std::net::SocketAddr;
    use std::os::fd::AsRawFd;
    use std::sync::atomic::{AtomicU16, Ordering};
    use std::time::Instant;

    static NEXT_ID: AtomicU16 = AtomicU16::new(0);

    fn open(v6: bool) -> Result<(Socket, bool), IcmpError> {
        let (domain, proto) = if v6 { (Domain::IPV6, Protocol::ICMPV6) } else { (Domain::IPV4, Protocol::ICMPV4) };
        match Socket::new(domain, Type::DGRAM, Some(proto)) {
            Ok(s) => Ok((s, false)),
            Err(e1) => match Socket::new(domain, Type::RAW, Some(proto)) {
                Ok(s) => Ok((s, true)),
                Err(_) => Err(IcmpError::NotPermitted(e1.to_string())),
            },
        }
    }

    pub(super) fn echo(target: IpAddr, ttl: Option<u8>, timeout: Duration, seq: u16) -> Result<IcmpReply, IcmpError> {
        let v6 = target.is_ipv6();
        let (sock, raw) = open(v6)?;
        if let Some(t) = ttl {
            if v6 {
                sock.set_unicast_hops_v6(u32::from(t))?;
            } else {
                sock.set_ttl_v4(u32::from(t))?;
            }
        }
        #[cfg(target_os = "linux")]
        let use_errqueue = !raw;
        #[cfg(target_os = "linux")]
        if use_errqueue {
            linux_err::enable(&sock, v6);
        }
        // Linux ping sockets rewrite the identifier to the local port.
        let check_id = raw || cfg!(not(target_os = "linux"));
        let id = (std::process::id() as u16).wrapping_mul(31).wrapping_add(NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let pkt = build_echo(v6, id, seq);
        let dest = SockAddr::from(SocketAddr::new(target, 0));
        let start = Instant::now();
        sock.send_to(&pkt, &dest)?;
        let deadline = start + timeout;
        let fd = sock.as_raw_fd();
        let mut buf = [MaybeUninit::<u8>::uninit(); 2048];
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err(IcmpError::Timeout);
            }
            let ms = (deadline - now).as_millis().clamp(1, i32::MAX as u128) as i32;
            let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
            // SAFETY: pfd is a valid pollfd for the lifetime of the call.
            let rc = unsafe { libc::poll(&mut pfd, 1, ms) };
            if rc < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e.into());
            }
            if rc == 0 {
                return Err(IcmpError::Timeout);
            }
            #[cfg(target_os = "linux")]
            if use_errqueue && pfd.revents & libc::POLLERR != 0 {
                if let Some((from, kind, err_seq)) = linux_err::read(fd, v6) {
                    if err_seq == seq {
                        return Ok(IcmpReply { from, rtt: start.elapsed(), kind });
                    }
                }
                continue;
            }
            if pfd.revents & libc::POLLIN == 0 {
                continue;
            }
            let (n, addr) = match sock.recv_from(&mut buf) {
                Ok(r) => r,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            };
            // SAFETY: recv_from initialised the first n bytes.
            let data: &[u8] = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), n) };
            let Some(from) = addr.as_socket().map(|a| a.ip()) else { continue };
            // IPv4 raw sockets (and macOS ping sockets) include the IP header.
            let icmp = if !v6 && data.first().is_some_and(|b| b >> 4 == 4) {
                let ihl = usize::from(data[0] & 0x0f) * 4;
                match data.get(ihl..) {
                    Some(d) => d,
                    None => continue,
                }
            } else {
                data
            };
            if let Some(kind) = classify(v6, icmp, id, seq, check_id) {
                if kind == ReplyKind::EchoReply && from != target {
                    continue;
                }
                return Ok(IcmpReply { from, rtt: start.elapsed(), kind });
            }
        }
    }

    #[cfg(target_os = "linux")]
    mod linux_err {
        //! `IP_RECVERR` error queue parsing for unprivileged ping sockets.
        use super::super::ReplyKind;
        use socket2::Socket;
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
        use std::os::fd::AsRawFd;

        const SOL_IP: libc::c_int = 0;
        const IP_RECVERR: libc::c_int = 11;
        const SOL_IPV6: libc::c_int = 41;
        const IPV6_RECVERR: libc::c_int = 25;
        const SO_EE_ORIGIN_ICMP: u8 = 2;
        const SO_EE_ORIGIN_ICMP6: u8 = 3;

        pub(super) fn enable(sock: &Socket, v6: bool) {
            let one: libc::c_int = 1;
            let (level, opt) = if v6 { (SOL_IPV6, IPV6_RECVERR) } else { (SOL_IP, IP_RECVERR) };
            // SAFETY: valid fd and option pointer/length.
            unsafe {
                libc::setsockopt(
                    sock.as_raw_fd(),
                    level,
                    opt,
                    (&one as *const libc::c_int).cast(),
                    std::mem::size_of::<libc::c_int>() as libc::socklen_t,
                );
            }
        }

        /// Returns (offender, kind, sequence of the original echo).
        pub(super) fn read(fd: libc::c_int, v6: bool) -> Option<(IpAddr, ReplyKind, u16)> {
            let mut data = [0u8; 576];
            let mut control = [0u8; 512];
            let mut iov = libc::iovec { iov_base: data.as_mut_ptr().cast(), iov_len: data.len() };
            // SAFETY: zeroed msghdr is a valid initial state.
            let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
            msg.msg_iov = &mut iov;
            msg.msg_iovlen = 1;
            msg.msg_control = control.as_mut_ptr().cast();
            msg.msg_controllen = control.len() as _;
            // SAFETY: msg points to valid buffers.
            let n = unsafe { libc::recvmsg(fd, &mut msg, libc::MSG_ERRQUEUE | libc::MSG_DONTWAIT) };
            if n < 8 {
                return None;
            }
            let seq = u16::from_be_bytes([data[6], data[7]]);
            // SAFETY: CMSG macros walk the control buffer filled by recvmsg.
            unsafe {
                let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
                while !cmsg.is_null() {
                    let c = &*cmsg;
                    let want = if v6 { (SOL_IPV6, IPV6_RECVERR) } else { (SOL_IP, IP_RECVERR) };
                    if (c.cmsg_level, c.cmsg_type) == want {
                        let p = libc::CMSG_DATA(cmsg);
                        let ee = std::slice::from_raw_parts(p, 16);
                        let origin = ee[4];
                        let ty = ee[5];
                        let code = ee[6];
                        if origin != SO_EE_ORIGIN_ICMP && origin != SO_EE_ORIGIN_ICMP6 {
                            return None;
                        }
                        let off = std::slice::from_raw_parts(p.add(16), if v6 { 24 } else { 8 });
                        let from = if v6 {
                            let mut a = [0u8; 16];
                            a.copy_from_slice(&off[8..24]);
                            IpAddr::V6(Ipv6Addr::from(a))
                        } else {
                            IpAddr::V4(Ipv4Addr::new(off[4], off[5], off[6], off[7]))
                        };
                        let kind = match (v6, ty) {
                            (false, 11) | (true, 3) => ReplyKind::TimeExceeded,
                            _ => ReplyKind::Unreachable(code),
                        };
                        return Some((from, kind, seq));
                    }
                    cmsg = libc::CMSG_NXTHDR(&msg, cmsg);
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_valid() {
        let p = build_echo(false, 0x1234, 7);
        assert_eq!(checksum(&p), 0);
    }

    #[test]
    fn classify_reply_and_exceeded() {
        let mut reply = build_echo(false, 0x1234, 7);
        reply[0] = 0;
        assert_eq!(classify(false, &reply, 0x1234, 7, true), Some(ReplyKind::EchoReply));
        assert_eq!(classify(false, &reply, 0x9999, 7, true), None);
        assert_eq!(classify(false, &reply, 0x9999, 7, false), Some(ReplyKind::EchoReply));

        let orig = build_echo(false, 0x1234, 7);
        let mut te = vec![11, 0, 0, 0, 0, 0, 0, 0];
        let mut ip = vec![0x45u8; 1];
        ip.extend_from_slice(&[0u8; 19]);
        te.extend_from_slice(&ip);
        te.extend_from_slice(&orig[..8]);
        assert_eq!(classify(false, &te, 0x1234, 7, true), Some(ReplyKind::TimeExceeded));
    }
}
