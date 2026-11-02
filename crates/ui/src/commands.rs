use platform::launch::ClientKind;
use std::net::IpAddr;
use storage::{AppTheme, Column, Language};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Html,
    Json,
    Xml,
}

impl ExportFormat {
    pub fn ext(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Html => "html",
            ExportFormat::Json => "json",
            ExportFormat::Xml => "xml",
        }
    }
}

/// Actions requested by the UI; executed by the application.
#[derive(Debug, Clone, PartialEq)]
pub enum UiCommand {
    // File
    NewSearch,
    LoadList,
    SaveAs(ExportFormat),
    ImportFavorites,
    ExportFavorites,
    Quit,
    // View
    ExpandAll,
    CollapseAll,
    ToggleAliveOnly,
    ToggleShowOffline,
    ToggleShowUnknown,
    ToggleColumn(Column),
    SetTheme(AppTheme),
    ToggleAlternating,
    ToggleToolbarIpC,
    ToggleToolbarExpand,
    // Settings
    OpenOptions,
    OpenMacDb,
    ImportMacDb,
    OpenProfiles,
    OpenFingerprints,
    SetLanguage(Language),
    // Help
    About,
    Diagnose,
    OpenLog,
    // Toolbar
    StartStop,
    TogglePause,
    FillLocalSubnet,
    FillClassC,
    FocusSearch,
    // Host actions
    OpenDefault(IpAddr, Option<usize>),
    OpenService(IpAddr, usize),
    OpenClient(IpAddr, ClientKind, u16),
    OpenShares(IpAddr),
    Ping(IpAddr),
    Traceroute(IpAddr),
    PortInfo(IpAddr),
    Properties(IpAddr),
    Rescan(IpAddr),
    AddFavorite(IpAddr),
    RemoveFavorite(IpAddr),
    EditAlias(IpAddr),
    EditComment(IpAddr),
    CopyText(String),
    WakeOnLan(IpAddr),
    Shutdown(IpAddr),
    Reboot(IpAddr),
    ScanFavorites,
}
