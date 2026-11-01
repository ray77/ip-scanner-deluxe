//! Local device fingerprinting.
//!
//! A MAC/OUI only tells which company made the network interface. This crate
//! combines that vendor with hostnames, open ports, mDNS services and TXT
//! records, HTTP and TLS metadata, IPP and SNMP answers to derive a *probable*
//! device class and model together with a transparent confidence score.

pub mod builtin;
pub mod engine;
pub mod rules;

pub use engine::{FingerprintEngine, FingerprintInput, FingerprintMatch};
pub use rules::{DeviceClass, DeviceFingerprint, RuleError};

/// Map a confidence value to its qualitative band.
pub fn confidence_band(confidence: u8) -> ConfidenceBand {
    match confidence {
        0..=29 => ConfidenceBand::Unknown,
        30..=49 => ConfidenceBand::Low,
        50..=69 => ConfidenceBand::Probable,
        70..=89 => ConfidenceBand::VeryProbable,
        _ => ConfidenceBand::Confirmed,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceBand {
    Unknown,
    Low,
    Probable,
    VeryProbable,
    Confirmed,
}
