//! Platform abstraction layer. Everything OS specific lives behind the
//! functions and traits of this crate; `scan-core` contains no `cfg(windows)`.
//!
//! No external programs (`arp`, `ping`, `traceroute`, shells) are used for
//! scanning. Only the user facing "open with client" actions in [`launch`]
//! start the locally configured programs.

pub mod icmp;
pub mod interfaces;
pub mod launch;
pub mod neighbor;
pub mod power;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub use icmp::{IcmpCapability, IcmpError, IcmpReply, ReplyKind};
pub use interfaces::{InterfaceKind, NetworkInterface};
pub use neighbor::{NeighborResolver, SystemNeighborResolver};
pub use power::{RemotePowerProvider, ShutdownOptions};

/// Short OS description for the diagnostics window.
pub fn os_description() -> String {
    format!("{} ({})", std::env::consts::OS, std::env::consts::FAMILY)
}

pub fn arch() -> &'static str {
    std::env::consts::ARCH
}
