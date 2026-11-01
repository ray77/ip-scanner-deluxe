//! LLMNR (RFC 4795) reverse lookup sent as unicast to the target.

use crate::dns::{unicast_ptr_query, CLASS_IN};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

pub async fn reverse_lookup(ip: IpAddr, timeout: Duration) -> Option<String> {
    unicast_ptr_query(ip, SocketAddr::new(ip, 5355), CLASS_IN, timeout).await
}
