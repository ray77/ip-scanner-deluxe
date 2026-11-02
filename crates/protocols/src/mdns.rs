//! mDNS / DNS-SD: per-host reverse lookups (legacy unicast) and a service
//! browser built on `mdns-sd` that runs alongside a scan.

use crate::dns::{unicast_ptr_query, CLASS_IN, CLASS_QU};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Service types browsed by default.
pub const DEFAULT_SERVICE_TYPES: &[&str] = &[
    "_http._tcp",
    "_https._tcp",
    "_ssh._tcp",
    "_sftp-ssh._tcp",
    "_smb._tcp",
    "_afpovertcp._tcp",
    "_ipp._tcp",
    "_ipps._tcp",
    "_printer._tcp",
    "_pdl-datastream._tcp",
    "_scanner._tcp",
    "_uscan._tcp",
    "_airplay._tcp",
    "_raop._tcp",
    "_googlecast._tcp",
    "_workstation._tcp",
    "_device-info._tcp",
    "_companion-link._tcp",
    "_home-assistant._tcp",
    "_hap._tcp",
    "_hue._tcp",
    "_sonos._tcp",
    "_spotify-connect._tcp",
    "_esphomelib._tcp",
    "_rfb._tcp",
    "_adisk._tcp",
    "_amzn-wplay._tcp",
];

/// Reverse lookup by asking the host's own mDNS responder (RFC 6762 §5.5
/// legacy unicast query to port 5353).
pub async fn reverse_lookup(ip: IpAddr, timeout: Duration) -> Option<String> {
    let name = unicast_ptr_query(ip, SocketAddr::new(ip, 5353), CLASS_IN | CLASS_QU, timeout).await?;
    Some(name)
}

/// A resolved DNS-SD service instance.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MdnsService {
    /// e.g. `_ipp._tcp`
    pub service_type: String,
    /// Instance name, e.g. `HP Color LaserJet 9500`
    pub instance: String,
    pub hostname: String,
    pub port: u16,
    pub txt: BTreeMap<String, String>,
}

#[derive(Default)]
struct BrowserState {
    by_ip: HashMap<IpAddr, Vec<MdnsService>>,
}

/// Browses DNS-SD services until stopped and indexes them by address.
pub struct MdnsBrowser {
    daemon: Option<ServiceDaemon>,
    state: Arc<Mutex<BrowserState>>,
    running: Arc<AtomicBool>,
}

fn short_type(ty_domain: &str) -> String {
    ty_domain.trim_end_matches('.').trim_end_matches(".local").to_string()
}

fn instance_name(fullname: &str, ty_domain: &str) -> String {
    let n = fullname.strip_suffix(ty_domain).unwrap_or(fullname);
    n.trim_end_matches('.').replace("\\032", " ").replace("\\.", ".")
}

impl MdnsBrowser {
    /// Start browsing. Failure to open multicast sockets (e.g. sandbox, no
    /// interface) yields an inert browser and the error text.
    pub fn start(service_types: &[&str]) -> (Self, Option<String>) {
        let state = Arc::new(Mutex::new(BrowserState::default()));
        let running = Arc::new(AtomicBool::new(true));
        let daemon = match ServiceDaemon::new() {
            Ok(d) => d,
            Err(e) => return (MdnsBrowser { daemon: None, state, running }, Some(e.to_string())),
        };
        for ty in service_types {
            let full = format!("{}.local.", ty.trim_end_matches('.'));
            let rx = match daemon.browse(&full) {
                Ok(rx) => rx,
                Err(e) => {
                    tracing::debug!("mdns browse {full} failed: {e}");
                    continue;
                }
            };
            let state = state.clone();
            let running = running.clone();
            std::thread::Builder::new()
                .name("mdns-browse".into())
                .spawn(move || {
                    while running.load(Ordering::Relaxed) {
                        match rx.recv_timeout(Duration::from_millis(250)) {
                            Ok(ServiceEvent::ServiceResolved(info)) => {
                                let svc = MdnsService {
                                    service_type: short_type(&info.ty_domain),
                                    instance: instance_name(&info.fullname, &info.ty_domain),
                                    hostname: info.host.trim_end_matches('.').to_string(),
                                    port: info.port,
                                    txt: info.txt_properties.iter().map(|p| (p.key().to_lowercase(), p.val_str().to_string())).collect(),
                                };
                                let mut st = state.lock();
                                for addr in info.addresses.iter() {
                                    let list = st.by_ip.entry(addr.to_ip_addr()).or_default();
                                    if let Some(existing) =
                                        list.iter_mut().find(|s| s.service_type == svc.service_type && s.instance == svc.instance)
                                    {
                                        *existing = svc.clone();
                                    } else {
                                        list.push(svc.clone());
                                    }
                                }
                            }
                            Ok(_) => {}
                            Err(e) => {
                                if e.to_string().contains("disconnected") {
                                    break;
                                }
                            }
                        }
                    }
                })
                .ok();
        }
        (MdnsBrowser { daemon: Some(daemon), state, running }, None)
    }

    /// Services seen so far for an address.
    pub fn services_for(&self, ip: IpAddr) -> Vec<MdnsService> {
        self.state.lock().by_ip.get(&ip).cloned().unwrap_or_default()
    }

    /// All addresses with at least one service.
    pub fn known_addresses(&self) -> Vec<IpAddr> {
        self.state.lock().by_ip.keys().copied().collect()
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(d) = self.daemon.take() {
            let _ = d.shutdown();
        }
    }
}

impl Drop for MdnsBrowser {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(short_type("_ipp._tcp.local."), "_ipp._tcp");
        assert_eq!(instance_name("HP Color LaserJet._ipp._tcp.local.", "_ipp._tcp.local."), "HP Color LaserJet");
    }
}
