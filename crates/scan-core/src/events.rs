use crate::model::HostRecord;
use std::net::IpAddr;

/// Messages from the scan engine to the GUI.
#[derive(Debug, Clone)]
pub enum ScanEvent {
    ScanStarted {
        total_targets: u64,
    },
    /// A host answered; enrichment (names, services) is still running.
    HostDiscovered {
        host: Box<HostRecord>,
    },
    /// Final or updated information for a host.
    HostUpdated {
        host: Box<HostRecord>,
    },
    Progress {
        completed: u64,
        total: u64,
        current: Option<IpAddr>,
    },
    ScanPaused,
    ScanResumed,
    ScanFinished,
    ScanCancelled,
    Warning(ScanWarning),
}

/// Non-fatal problem during a scan; the UI translates [`ScanWarning::key`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScanWarning {
    #[error("ICMP raw socket not available, falling back to TCP detection")]
    IcmpFallback,
    #[error("mDNS not available: {0}")]
    MdnsUnavailable(String),
}

impl ScanWarning {
    /// Translation key (`ui::i18n`); details are `{e}`.
    pub fn key(&self) -> &'static str {
        match self {
            Self::IcmpFallback => "diag.icmp_fallback",
            Self::MdnsUnavailable(_) => "warn.mdns",
        }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::IcmpFallback => "",
            Self::MdnsUnavailable(e) => e,
        }
    }
}
