//! Remote shutdown / restart. Only Windows targets are supported, through
//! the Windows remote shutdown API when this program runs on Windows.
//! Callers must always ask the user for confirmation first.

use std::future::Future;
use std::io;

#[derive(Debug, Clone)]
pub struct ShutdownOptions {
    pub message: String,
    pub timeout_secs: u32,
    pub force: bool,
}

impl Default for ShutdownOptions {
    fn default() -> Self {
        ShutdownOptions { message: String::new(), timeout_secs: 30, force: false }
    }
}

pub trait RemotePowerProvider: Send + Sync {
    fn is_supported(&self) -> bool;
    fn shutdown(&self, target: &str, options: ShutdownOptions) -> impl Future<Output = io::Result<()>> + Send;
    fn reboot(&self, target: &str, options: ShutdownOptions) -> impl Future<Output = io::Result<()>> + Send;
    fn abort(&self, target: &str) -> impl Future<Output = io::Result<()>> + Send;
}

/// Provider using the operating system's facilities.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemPowerProvider;

#[allow(unused_variables)]
fn run(target: String, options: ShutdownOptions, reboot: bool) -> io::Result<()> {
    if !target.chars().all(|c| c.is_ascii_alphanumeric() || ".-_:".contains(c)) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid target name"));
    }
    #[cfg(windows)]
    {
        crate::windows::remote_shutdown(&target, &options.message, options.timeout_secs, options.force, reboot)
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::new(io::ErrorKind::Unsupported, "remote shutdown is only available on Windows"))
    }
}

impl RemotePowerProvider for SystemPowerProvider {
    fn is_supported(&self) -> bool {
        cfg!(windows)
    }

    fn shutdown(&self, target: &str, options: ShutdownOptions) -> impl Future<Output = io::Result<()>> + Send {
        let t = target.to_string();
        async move { tokio::task::spawn_blocking(move || run(t, options, false)).await.map_err(|e| io::Error::other(e.to_string()))? }
    }

    fn reboot(&self, target: &str, options: ShutdownOptions) -> impl Future<Output = io::Result<()>> + Send {
        let t = target.to_string();
        async move { tokio::task::spawn_blocking(move || run(t, options, true)).await.map_err(|e| io::Error::other(e.to_string()))? }
    }

    #[allow(unused_variables)]
    fn abort(&self, target: &str) -> impl Future<Output = io::Result<()>> + Send {
        let t = target.to_string();
        async move {
            #[cfg(windows)]
            {
                tokio::task::spawn_blocking(move || crate::windows::abort_shutdown(&t))
                    .await
                    .map_err(|e| io::Error::other(e.to_string()))?
            }
            #[cfg(not(windows))]
            {
                Err(io::Error::new(io::ErrorKind::Unsupported, "not supported"))
            }
        }
    }
}
