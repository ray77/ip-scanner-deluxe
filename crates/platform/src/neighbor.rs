//! MAC resolution through the operating system neighbor (ARP) table.
//!
//! MAC addresses are only meaningful on the local layer 2 segment. For
//! routed targets `None` is returned; the gateway's MAC is never assigned to
//! a remote host.

use crate::interfaces::NetworkInterface;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use vendor_db::MacAddr;

pub trait NeighborResolver: Send + Sync {
    fn resolve_mac(&self, interface: Option<&NetworkInterface>, ip: IpAddr) -> impl Future<Output = io::Result<Option<MacAddr>>> + Send;
}

/// Read the full OS neighbor table (IPv4).
pub fn neighbor_table() -> io::Result<Vec<(IpAddr, MacAddr)>> {
    #[cfg(target_os = "linux")]
    {
        crate::linux::neighbor_table()
    }
    #[cfg(target_os = "macos")]
    {
        crate::macos::neighbor_table()
    }
    #[cfg(windows)]
    {
        crate::windows::neighbor_table()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        Err(io::Error::new(io::ErrorKind::Unsupported, "neighbor table not supported"))
    }
}

/// Resolver backed by the OS: Windows uses `SendARP` (actively ARPs),
/// Linux and macOS read the kernel neighbor cache, which is filled by the
/// discovery traffic (ICMP/TCP) sent just before.
type TableSnapshot = Option<(Instant, HashMap<IpAddr, MacAddr>)>;

#[derive(Clone, Default)]
pub struct SystemNeighborResolver {
    cache: Arc<Mutex<TableSnapshot>>,
}

const CACHE_TTL: Duration = Duration::from_millis(300);

impl SystemNeighborResolver {
    pub fn new() -> Self {
        Self::default()
    }

    fn lookup_table(&self, ip: IpAddr) -> io::Result<Option<MacAddr>> {
        let mut guard = self.cache.lock();
        let fresh = guard.as_ref().is_some_and(|(t, _)| t.elapsed() < CACHE_TTL);
        if !fresh {
            let table: HashMap<IpAddr, MacAddr> = neighbor_table()?.into_iter().collect();
            *guard = Some((Instant::now(), table));
        }
        Ok(guard.as_ref().and_then(|(_, t)| t.get(&ip).copied()))
    }

    /// Force the next lookup to re-read the table.
    pub fn invalidate(&self) {
        *self.cache.lock() = None;
    }
}

impl NeighborResolver for SystemNeighborResolver {
    fn resolve_mac(&self, interface: Option<&NetworkInterface>, ip: IpAddr) -> impl Future<Output = io::Result<Option<MacAddr>>> + Send {
        let on_link = interface.is_some_and(|i| i.contains(ip));
        let own = interface.and_then(|i| i.ipv4.iter().any(|(a, _)| IpAddr::V4(*a) == ip).then_some(i.mac).flatten());
        let this = self.clone();
        async move {
            if let Some(m) = own {
                return Ok(Some(m));
            }
            if !on_link {
                return Ok(None);
            }
            #[cfg(windows)]
            {
                if let IpAddr::V4(v4) = ip {
                    let r = tokio::task::spawn_blocking(move || crate::windows::send_arp(v4))
                        .await
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    if let Ok(Some(m)) = r {
                        return Ok(Some(m));
                    }
                }
            }
            let r = tokio::task::spawn_blocking(move || this.lookup_table(ip)).await.map_err(|e| io::Error::other(e.to_string()))?;
            r.map(|m| m.filter(|m| !m.is_unusable()))
        }
    }
}
