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
    Warning(String),
}
