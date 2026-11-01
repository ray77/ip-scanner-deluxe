use std::time::Duration;
fn main() {
    let t: std::net::IpAddr = std::env::args().nth(1).unwrap_or("1.1.1.1".into()).parse().unwrap();
    for ttl in [None, Some(1), Some(2), Some(3)] {
        println!("{t} ttl={ttl:?}: {:?}", platform::icmp::echo(t, ttl, Duration::from_millis(1500), 7));
    }
}
