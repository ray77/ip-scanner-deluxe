use scan_core::{class_c_range, parse_targets};
use std::net::IpAddr;

fn ips(s: &str) -> Vec<IpAddr> {
    parse_targets(s).unwrap().iter().collect()
}

#[test]
fn single_address() {
    assert_eq!(ips("192.168.0.1"), vec!["192.168.0.1".parse::<IpAddr>().unwrap()]);
}

#[test]
fn last_octet_range() {
    let v = ips("192.168.0.1-254");
    assert_eq!(v.len(), 254);
    assert_eq!(v[0].to_string(), "192.168.0.1");
    assert_eq!(v[253].to_string(), "192.168.0.254");
}

#[test]
fn full_range() {
    let v = ips("192.168.0.1-192.168.0.254");
    assert_eq!(v.len(), 254);
    let cross = ips("192.168.0.250-192.168.1.5");
    assert_eq!(cross.len(), 12);
}

#[test]
fn cidr() {
    let v = ips("192.168.0.0/24");
    assert_eq!(v.len(), 254);
    assert_eq!(v[0].to_string(), "192.168.0.1");
    assert_eq!(ips("10.0.0.0/24").len(), 254);
    assert_eq!(ips("10.0.0.5/32").len(), 1);
    assert_eq!(ips("10.0.0.4/31").len(), 2);
    assert_eq!(parse_targets("10.0.0.0/16").unwrap().len(), 65534);
}

#[test]
fn lists_and_multiple_ranges() {
    assert_eq!(ips("192.168.0.1,192.168.0.5,192.168.0.10").len(), 3);
    assert_eq!(ips("192.168.0.1-254;10.0.0.1-100").len(), 354);
    // Overlapping ranges are deduplicated.
    assert_eq!(ips("192.168.0.1-10, 192.168.0.5-15").len(), 15);
}

#[test]
fn invalid_ranges() {
    for bad in [
        "",
        "192.168.0",
        "192.168.0.256",
        "192.168.0.10-5",
        "192.168.0.1-300",
        "192.168.0.0/33",
        "hello",
        "10.0.0.0/8",
        "fe80::/64",
        "1.2.3.4-",
    ] {
        assert!(parse_targets(bad).is_err(), "{bad:?} should be rejected");
    }
}

#[test]
fn ipv6_manual() {
    assert_eq!(ips("fe80::1").len(), 1);
    assert_eq!(ips("2001:db8::/124").len(), 16);
}

#[test]
fn class_c_button() {
    assert_eq!(class_c_range("192.168.10.47").as_deref(), Some("192.168.10.0-255"));
    assert_eq!(class_c_range("192.168.0.90-110").as_deref(), Some("192.168.0.0-255"));
    assert_eq!(class_c_range("garbage"), None);
}

#[test]
fn lazy_large_range() {
    let spec = parse_targets("10.0.0.0/16").unwrap();
    // Taking a few does not materialise the range.
    let first: Vec<IpAddr> = spec.iter().take(3).collect();
    assert_eq!(first[2].to_string(), "10.0.0.3");
}
