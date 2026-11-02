use std::path::{Path, PathBuf};

/// Platform specific locations, or the program directory in portable mode
/// (`portable.flag` next to the executable, only if `allow_portable`).
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub portable: bool,
    pub settings_dir: PathBuf,
    pub database_dir: PathBuf,
    pub favorites_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppPaths {
    /// Store/MSIX builds pass `allow_portable = false`: the install directory is read-only there.
    pub fn detect(allow_portable: bool) -> Self {
        if let Some(dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).filter(|_| allow_portable) {
            if dir.join("portable.flag").exists() {
                return Self::in_dir(&dir, true);
            }
        }
        let base = Self::platform_dir().unwrap_or_else(|| PathBuf::from(".ip-scanner-deluxe"));
        Self::in_dir(&base, false)
    }

    fn platform_dir() -> Option<PathBuf> {
        let b = directories::BaseDirs::new()?;
        #[cfg(windows)]
        {
            Some(b.config_dir().join("IPScannerDeluxe")) // %APPDATA%
        }
        #[cfg(target_os = "macos")]
        {
            Some(b.data_dir().join("IPScannerDeluxe")) // ~/Library/Application Support
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            Some(b.data_dir().join("ip-scanner-deluxe")) // ~/.local/share
        }
    }

    pub fn in_dir(base: &Path, portable: bool) -> Self {
        AppPaths {
            portable,
            settings_dir: base.join("settings"),
            database_dir: base.join("database"),
            favorites_dir: base.join("favorites"),
            log_dir: base.join("logs"),
        }
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        for d in [&self.settings_dir, &self.database_dir, &self.favorites_dir, &self.log_dir] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }

    pub fn store_file(&self) -> PathBuf {
        self.database_dir.join("scanner.redb")
    }

    /// User supplied vendor database (overrides the bundled one).
    pub fn vendor_db_file(&self) -> PathBuf {
        self.database_dir.join("vendor.redb")
    }

    /// Additional user fingerprint rules.
    pub fn fingerprint_file(&self) -> PathBuf {
        self.settings_dir.join("fingerprints.json")
    }

    pub fn log_file(&self) -> PathBuf {
        self.log_dir.join("ip-scanner-deluxe.log")
    }
}
