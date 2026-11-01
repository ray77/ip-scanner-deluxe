//! Built-in fingerprint rules, shipped in `assets/fingerprints/builtin.json`.

use crate::rules::{parse_rules, DeviceFingerprint};

pub const BUILTIN_RULES_JSON: &str = include_str!("../../../assets/fingerprints/builtin.json");

pub fn builtin_rules() -> Vec<DeviceFingerprint> {
    match parse_rules(BUILTIN_RULES_JSON) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("built-in fingerprint rules invalid: {e}");
            Vec::new()
        }
    }
}

/// Map Apple hardware model identifiers (mDNS `_device-info._tcp` TXT
/// `model=`) to a readable family and class. Only used when the device
/// itself announced the identifier.
pub fn apple_model(identifier: &str) -> Option<(&'static str, crate::DeviceClass)> {
    use crate::DeviceClass::*;
    let table: [(&str, &str, crate::DeviceClass); 12] = [
        ("MacBookPro", "MacBook Pro", Laptop),
        ("MacBookAir", "MacBook Air", Laptop),
        ("MacBook", "MacBook", Laptop),
        ("iMac", "iMac", Desktop),
        ("Macmini", "Mac mini", Desktop),
        ("MacPro", "Mac Pro", Desktop),
        ("Mac", "Mac", Unknown),
        ("iPhone", "iPhone", Smartphone),
        ("iPad", "iPad", Tablet),
        ("AppleTV", "Apple TV", MediaPlayer),
        ("AudioAccessory", "HomePod", MediaPlayer),
        ("RackMac", "Xserve", Server),
    ];
    table.iter().find(|(prefix, _, _)| identifier.starts_with(prefix)).map(|(_, name, class)| (*name, *class))
}
