//! Scan engine. Independent of the GUI: results are streamed as
//! [`ScanEvent`]s over a channel while the scan runs.

pub mod coordinator;
pub mod discovery;
pub mod events;
pub mod model;
pub mod ports;
pub mod target;

pub use coordinator::{ScanConfig, ScanControl, ScanProfile, Scanner};
pub use discovery::{NetProbe, SystemProbe, TcpOutcome};
pub use events::{ScanEvent, ScanWarning};
pub use model::{HostNames, HostRecord, HostStatus, ServiceRecord, ServiceType, TransportProtocol};
pub use target::{class_c_range, parse_targets, RangeError, RangeErrorKind, TargetSpec};

pub use fingerprint::DeviceClass;
pub use vendor_db::MacAddr;
