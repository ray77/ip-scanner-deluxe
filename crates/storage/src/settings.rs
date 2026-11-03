use platform::launch::ClientKind;
use scan_core::ScanConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppTheme {
    #[default]
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Language {
    /// Follow the system locale.
    #[default]
    System,
    German,
    English,
    Spanish,
    Italian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Column {
    Status,
    Name,
    Ip,
    Vendor,
    Mac,
    Comment,
    DeviceType,
    Model,
    Os,
    Latency,
    Services,
    Confidence,
    LastScan,
    LastSeen,
    Interface,
}

impl Column {
    pub const ALL: [Column; 15] = [
        Column::Status,
        Column::Name,
        Column::Ip,
        Column::Vendor,
        Column::Mac,
        Column::Comment,
        Column::DeviceType,
        Column::Model,
        Column::Os,
        Column::Latency,
        Column::Services,
        Column::Confidence,
        Column::LastScan,
        Column::LastSeen,
        Column::Interface,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Column::Status => "col.status",
            Column::Name => "col.name",
            Column::Ip => "col.ip",
            Column::Vendor => "col.vendor",
            Column::Mac => "col.mac",
            Column::Comment => "col.comment",
            Column::DeviceType => "col.device_type",
            Column::Model => "col.model",
            Column::Os => "col.os",
            Column::Latency => "col.latency",
            Column::Services => "col.services",
            Column::Confidence => "col.confidence",
            Column::LastScan => "col.last_scan",
            Column::LastSeen => "col.last_seen",
            Column::Interface => "col.interface",
        }
    }

    /// Default width at the 653 px reference window.
    pub fn default_width(self) -> f32 {
        match self {
            Column::Status => 52.0,
            Column::Name => 128.0,
            Column::Ip => 100.0,
            Column::Vendor => 124.0,
            Column::Mac => 118.0,
            Column::Comment => 96.0,
            Column::DeviceType => 100.0,
            Column::Model => 150.0,
            Column::Os => 100.0,
            Column::Latency => 80.0,
            Column::Services => 160.0,
            Column::Confidence => 80.0,
            Column::LastScan => 130.0,
            Column::LastSeen => 130.0,
            Column::Interface => 100.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortState {
    pub column: Column,
    pub ascending: bool,
    /// User picked the sort explicitly (otherwise IP order during scans).
    pub user: bool,
}

impl Default for SortState {
    fn default() -> Self {
        SortState { column: Column::Ip, ascending: true, user: false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme: AppTheme,
    pub language: Language,
    pub window_size: Option<[f32; 2]>,
    pub window_pos: Option<[f32; 2]>,
    /// Visible columns in display order.
    pub columns: Vec<Column>,
    pub column_widths: BTreeMap<Column, f32>,
    pub sort: SortState,
    pub scan: ScanConfig,
    pub custom_ports: String,
    pub last_range: String,
    pub range_history: Vec<String>,
    pub last_interface: Option<String>,
    pub alternating_rows: bool,
    pub show_alive_only: bool,
    pub show_offline: bool,
    pub show_unknown: bool,
    pub clear_on_scan: bool,
    pub client_templates: BTreeMap<ClientKind, String>,
    pub csv_bom: bool,
    pub toolbar_ip_c: bool,
    pub toolbar_expand: bool,
    pub hide_virtual_interfaces: bool,
    pub wol_port: u16,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            theme: AppTheme::Light,
            language: Language::System,
            window_size: None,
            window_pos: None,
            columns: vec![Column::Status, Column::Name, Column::Ip, Column::Vendor, Column::Mac, Column::Comment],
            column_widths: BTreeMap::new(),
            sort: SortState::default(),
            scan: ScanConfig::default(),
            custom_ports: String::new(),
            last_range: String::new(),
            range_history: Vec::new(),
            last_interface: None,
            alternating_rows: false,
            show_alive_only: false,
            show_offline: true,
            show_unknown: false,
            clear_on_scan: true,
            client_templates: BTreeMap::new(),
            csv_bom: false,
            toolbar_ip_c: true,
            toolbar_expand: true,
            hide_virtual_interfaces: false,
            wol_port: 9,
        }
    }
}

impl AppSettings {
    pub fn column_width(&self, c: Column) -> f32 {
        self.column_widths.get(&c).copied().unwrap_or_else(|| c.default_width())
    }

    pub fn push_history(&mut self, range: &str) {
        let r = range.trim().to_string();
        if r.is_empty() {
            return;
        }
        self.range_history.retain(|h| h != &r);
        self.range_history.insert(0, r);
        self.range_history.truncate(15);
    }
}
