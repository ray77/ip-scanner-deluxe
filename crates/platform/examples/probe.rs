//! Manual smoke test: `cargo run -p platform --example probe`
use std::time::Duration;

fn main() {
    let ifaces = platform::interfaces::list_interfaces();
    for i in &ifaces {
        println!(
            "{:<10} {:<20} {:?} tag={:?} v4={:?} mac={:?} gw={:?} up={} default={}",
            i.name,
            i.display_name,
            i.kind,
            i.virtual_tag,
            i.ipv4,
            i.mac.map(|m| m.to_string()),
            i.gateway,
            i.is_up,
            i.is_default
        );
    }
    println!("icmp capability: {:?}, raw: {}", platform::icmp::capability(), platform::icmp::raw_socket_available());
    match platform::neighbor::neighbor_table() {
        Ok(t) => {
            println!("neighbor entries: {}", t.len());
            for (ip, mac) in t.iter().take(5) {
                println!("  {ip} -> {mac}");
            }
        }
        Err(e) => println!("neighbor table error: {e}"),
    }
    if let Some(gw) = ifaces.iter().find(|i| i.is_default).and_then(|i| i.gateway) {
        println!("ping {gw}: {:?}", platform::icmp::echo(gw, None, Duration::from_millis(700), 1));
        println!("ttl1 1.1.1.1: {:?}", platform::icmp::echo("1.1.1.1".parse().unwrap(), Some(1), Duration::from_millis(1000), 2));
        println!("route source: {:?}", platform::interfaces::route_source(gw));
    }
}
