//! Network protocol probes used by the scanner. All of them talk only to
//! hosts in the scanned range; nothing is sent to the internet.

pub mod banner;
pub mod dns;
pub mod http;
pub mod ipp;
pub mod llmnr;
pub mod mdns;
pub mod netbios;
pub mod snmp;
pub mod tls;
pub mod wol;

use std::time::Duration;

/// Run a future with a timeout, mapping elapsed to `None`.
pub async fn with_timeout<T, F>(d: Duration, f: F) -> Option<T>
where
    F: std::future::Future<Output = Option<T>>,
{
    tokio::time::timeout(d, f).await.ok().flatten()
}
