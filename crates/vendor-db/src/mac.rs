use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// A 48 bit IEEE 802 MAC address.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct MacAddr(pub [u8; 6]);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid MAC address: {0}")]
pub struct MacParseError(pub String);

impl MacAddr {
    pub const BROADCAST: MacAddr = MacAddr([0xff; 6]);

    pub const fn new(bytes: [u8; 6]) -> Self {
        MacAddr(bytes)
    }

    pub fn octets(&self) -> [u8; 6] {
        self.0
    }

    /// The address as a 48 bit integer (big endian).
    pub fn to_u64(&self) -> u64 {
        self.0.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
    }

    pub fn from_u64(v: u64) -> Self {
        let mut b = [0u8; 6];
        for (i, byte) in b.iter_mut().enumerate() {
            *byte = ((v >> (8 * (5 - i))) & 0xff) as u8;
        }
        MacAddr(b)
    }

    /// The upper `bits` bits of the address.
    pub fn prefix(&self, bits: u8) -> u64 {
        debug_assert!(bits <= 48);
        self.to_u64() >> (48 - u32::from(bits))
    }

    /// Locally administered bit (U/L bit) set: randomized/private or
    /// software assigned address, the OUI does not identify a vendor.
    pub fn is_locally_administered(&self) -> bool {
        self.0[0] & 0b10 != 0
    }

    pub fn is_multicast(&self) -> bool {
        self.0[0] & 0b1 != 0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == [0; 6]
    }

    /// True for addresses that are meaningless as a device identifier.
    pub fn is_unusable(&self) -> bool {
        self.is_zero() || *self == Self::BROADCAST || self.is_multicast()
    }

    /// Parse a MAC address in any common notation:
    /// `00:11:22:33:44:55`, `00-11-22-33-44-55`, `0011.2233.4455`,
    /// `001122334455`. Single-digit groups (`0:1:2:3:4:5`) are accepted
    /// for colon/dash notation as produced by some systems.
    pub fn parse(s: &str) -> Result<Self, MacParseError> {
        let t = s.trim();
        let err = || MacParseError(s.to_string());
        let mut out = [0u8; 6];

        if t.contains(':') || t.contains('-') {
            let parts: Vec<&str> = t.split([':', '-']).collect();
            if parts.len() != 6 {
                return Err(err());
            }
            for (i, p) in parts.iter().enumerate() {
                if p.is_empty() || p.len() > 2 {
                    return Err(err());
                }
                out[i] = u8::from_str_radix(p, 16).map_err(|_| err())?;
            }
            return Ok(MacAddr(out));
        }

        let hex: String = if t.contains('.') {
            let parts: Vec<&str> = t.split('.').collect();
            if parts.len() != 3 || parts.iter().any(|p| p.len() != 4) {
                return Err(err());
            }
            parts.concat()
        } else {
            t.to_string()
        };
        if hex.len() != 12 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(err());
        }
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| err())?;
        }
        Ok(MacAddr(out))
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = self.0;
        write!(f, "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", b[0], b[1], b[2], b[3], b[4], b[5])
    }
}

impl fmt::Debug for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MacAddr({self})")
    }
}

impl FromStr for MacAddr {
    type Err = MacParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        MacAddr::parse(s)
    }
}

impl From<[u8; 6]> for MacAddr {
    fn from(b: [u8; 6]) -> Self {
        MacAddr(b)
    }
}

impl Serialize for MacAddr {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for MacAddr {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        MacAddr::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_notations() {
        let expect = MacAddr([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        for s in ["00:11:22:33:44:55", "00-11-22-33-44-55", "0011.2233.4455", "001122334455", " 00:11:22:33:44:55 ", "0:11:22:33:44:55"] {
            assert_eq!(MacAddr::parse(s).unwrap(), expect, "{s}");
        }
        assert_eq!(MacAddr::parse("aa:bb:cc:dd:ee:ff").unwrap().to_string(), "AA:BB:CC:DD:EE:FF");
    }

    #[test]
    fn rejects_garbage() {
        for s in ["", "00:11:22:33:44", "00:11:22:33:44:55:66", "0011.2233.445", "00112233445G", "001:11:22:33:44:55", "hello"] {
            assert!(MacAddr::parse(s).is_err(), "{s}");
        }
    }

    #[test]
    fn u64_roundtrip_and_prefix() {
        let m = MacAddr::parse("3C:52:82:12:34:56").unwrap();
        assert_eq!(MacAddr::from_u64(m.to_u64()), m);
        assert_eq!(m.prefix(24), 0x3C5282);
        assert_eq!(m.prefix(28), 0x3C52821);
        assert_eq!(m.prefix(36), 0x3C5282123);
    }

    #[test]
    fn local_admin_bit() {
        assert!(MacAddr::parse("02:42:ac:11:00:02").unwrap().is_locally_administered());
        assert!(MacAddr::parse("DA:A1:19:00:00:01").unwrap().is_locally_administered());
        assert!(MacAddr::parse("F6:00:00:00:00:01").unwrap().is_locally_administered());
        assert!(!MacAddr::parse("3C:52:82:00:00:01").unwrap().is_locally_administered());
        assert!(!MacAddr::parse("00:1F:D0:00:00:01").unwrap().is_locally_administered());
    }
}
