//! Address range parsing. Targets are generated lazily, so even a /16 does
//! not allocate per-address objects before the scan starts.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Hard upper bound to protect against accidental huge ranges (/8).
pub const MAX_TARGETS: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} ({token})")]
pub struct RangeError {
    pub token: String,
    pub message: String,
}

fn err(token: &str, message: &str) -> RangeError {
    RangeError { token: token.to_string(), message: message.to_string() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetRange {
    V4 { start: u32, end: u32 },
    V6 { start: u128, end: u128 },
}

impl TargetRange {
    pub fn len(&self) -> u64 {
        match self {
            TargetRange::V4 { start, end } => u64::from(end - start) + 1,
            TargetRange::V6 { start, end } => (end - start) as u64 + 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self, ip) {
            (TargetRange::V4 { start, end }, IpAddr::V4(v)) => (*start..=*end).contains(&u32::from(v)),
            (TargetRange::V6 { start, end }, IpAddr::V6(v)) => (*start..=*end).contains(&u128::from(v)),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TargetSpec {
    pub ranges: Vec<TargetRange>,
}

impl TargetSpec {
    pub fn len(&self) -> u64 {
        self.ranges.iter().map(TargetRange::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        self.ranges.iter().any(|r| r.contains(ip))
    }

    /// Lazy iterator over all addresses, duplicates across ranges skipped.
    pub fn iter(&self) -> impl Iterator<Item = IpAddr> + Send + '_ {
        self.ranges.iter().enumerate().flat_map(move |(idx, r)| {
            let earlier = &self.ranges[..idx];
            let it: Box<dyn Iterator<Item = IpAddr> + Send> = match r {
                TargetRange::V4 { start, end } => Box::new((*start..=*end).map(|v| IpAddr::V4(Ipv4Addr::from(v)))),
                TargetRange::V6 { start, end } => Box::new((*start..=*end).map(|v| IpAddr::V6(Ipv6Addr::from(v)))),
            };
            it.filter(move |ip| !earlier.iter().any(|e| e.contains(*ip)))
        })
    }
}

fn parse_v4(s: &str) -> Option<Ipv4Addr> {
    // Reject leading-zero octal ambiguity like "010", accept plain digits.
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 || parts.iter().any(|p| p.is_empty() || p.len() > 3 || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    s.parse().ok()
}

fn parse_token(tok: &str) -> Result<TargetRange, RangeError> {
    // CIDR
    if let Some((addr, prefix)) = tok.split_once('/') {
        let prefix: u32 = prefix.parse().map_err(|_| err(tok, "Ungültige Präfixlänge"))?;
        if let Some(v4) = parse_v4(addr) {
            if prefix > 32 {
                return Err(err(tok, "Präfix muss 0-32 sein"));
            }
            let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
            let net = u32::from(v4) & mask;
            let bcast = net | !mask;
            // Skip network and broadcast address for normal subnets.
            let (s, e) = if prefix <= 30 { (net + 1, bcast - 1) } else { (net, bcast) };
            return Ok(TargetRange::V4 { start: s, end: e });
        }
        if let Ok(v6) = addr.parse::<Ipv6Addr>() {
            if prefix > 128 {
                return Err(err(tok, "Präfix muss 0-128 sein"));
            }
            if prefix < 120 {
                return Err(err(tok, "IPv6-Netze größer als /120 werden nicht durchsucht"));
            }
            let mask = u128::MAX << (128 - prefix);
            let net = u128::from(v6) & mask;
            return Ok(TargetRange::V6 { start: net, end: net | !mask });
        }
        return Err(err(tok, "Ungültige Adresse"));
    }
    // Ranges: a.b.c.d-e or a.b.c.d-w.x.y.z
    if let Some((a, b)) = tok.split_once('-') {
        let start = parse_v4(a).ok_or_else(|| err(tok, "Ungültige Startadresse"))?;
        let end = if let Some(full) = parse_v4(b) {
            full
        } else {
            let last: u8 = b.parse().map_err(|_| err(tok, "Ungültige Endadresse"))?;
            let o = start.octets();
            Ipv4Addr::new(o[0], o[1], o[2], last)
        };
        let (s, e) = (u32::from(start), u32::from(end));
        if e < s {
            return Err(err(tok, "Endadresse liegt vor der Startadresse"));
        }
        return Ok(TargetRange::V4 { start: s, end: e });
    }
    if let Some(v4) = parse_v4(tok) {
        let v = u32::from(v4);
        return Ok(TargetRange::V4 { start: v, end: v });
    }
    if let Ok(v6) = tok.trim_matches(['[', ']']).parse::<Ipv6Addr>() {
        let v = u128::from(v6);
        return Ok(TargetRange::V6 { start: v, end: v });
    }
    Err(err(tok, "Ungültige IP-Adresse oder ungültiger Bereich"))
}

/// Parse user input such as `192.168.0.1-254, 10.0.0.0/24; 10.0.1.5`.
pub fn parse_targets(input: &str) -> Result<TargetSpec, RangeError> {
    let tokens: Vec<&str> = input.split([',', ';', ' ', '\t', '\n']).map(str::trim).filter(|t| !t.is_empty()).collect();
    if tokens.is_empty() {
        return Err(err(input, "Kein Adressbereich angegeben"));
    }
    let mut spec = TargetSpec::default();
    for t in tokens {
        // Allow "192.168.0.1 - 192.168.0.9" with spaces around the dash.
        spec.ranges.push(parse_token(t)?);
    }
    if spec.len() > MAX_TARGETS {
        return Err(err(input, "Bereich zu groß (max. 1.048.576 Adressen)"));
    }
    Ok(spec)
}

/// The "C" toolbar button: classic /24 around the first IPv4 address found
/// in the input, written as `a.b.c.0-255`.
pub fn class_c_range(input: &str) -> Option<String> {
    let first = input.split([',', ';', ' ', '-', '/']).find_map(|t| parse_v4(t.trim()))?;
    let o = first.octets();
    Some(format!("{}.{}.{}.0-255", o[0], o[1], o[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_and_short_range() {
        assert_eq!(parse_targets("192.168.0.1").unwrap().len(), 1);
        assert_eq!(parse_targets("192.168.0.1-254").unwrap().len(), 254);
        assert_eq!(parse_targets("192.168.0.90-110").unwrap().len(), 21);
    }
}
