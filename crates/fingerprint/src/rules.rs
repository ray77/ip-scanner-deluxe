use regex::Regex;
use serde::{Deserialize, Serialize};

/// Coarse device category, drives the icon and the "Gerätetyp" column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default, PartialOrd, Ord)]
pub enum DeviceClass {
    Desktop,
    Laptop,
    Server,
    VirtualMachine,
    Router,
    Firewall,
    Switch,
    AccessPoint,
    Printer,
    Nas,
    Camera,
    Television,
    MediaPlayer,
    Smartphone,
    Tablet,
    IoT,
    HomeAutomation,
    GameConsole,
    UPS,
    #[default]
    Unknown,
}

impl DeviceClass {
    pub const ALL: [DeviceClass; 20] = [
        DeviceClass::Desktop,
        DeviceClass::Laptop,
        DeviceClass::Server,
        DeviceClass::VirtualMachine,
        DeviceClass::Router,
        DeviceClass::Firewall,
        DeviceClass::Switch,
        DeviceClass::AccessPoint,
        DeviceClass::Printer,
        DeviceClass::Nas,
        DeviceClass::Camera,
        DeviceClass::Television,
        DeviceClass::MediaPlayer,
        DeviceClass::Smartphone,
        DeviceClass::Tablet,
        DeviceClass::IoT,
        DeviceClass::HomeAutomation,
        DeviceClass::GameConsole,
        DeviceClass::UPS,
        DeviceClass::Unknown,
    ];

    /// Stable identifier, used as translation key suffix and in exports.
    pub fn key(self) -> &'static str {
        match self {
            DeviceClass::Desktop => "desktop",
            DeviceClass::Laptop => "laptop",
            DeviceClass::Server => "server",
            DeviceClass::VirtualMachine => "vm",
            DeviceClass::Router => "router",
            DeviceClass::Firewall => "firewall",
            DeviceClass::Switch => "switch",
            DeviceClass::AccessPoint => "access_point",
            DeviceClass::Printer => "printer",
            DeviceClass::Nas => "nas",
            DeviceClass::Camera => "camera",
            DeviceClass::Television => "tv",
            DeviceClass::MediaPlayer => "media_player",
            DeviceClass::Smartphone => "smartphone",
            DeviceClass::Tablet => "tablet",
            DeviceClass::IoT => "iot",
            DeviceClass::HomeAutomation => "home_automation",
            DeviceClass::GameConsole => "game_console",
            DeviceClass::UPS => "ups",
            DeviceClass::Unknown => "unknown",
        }
    }
}

/// A fingerprint rule. All matchers are optional; a rule only applies when
/// at least one non-port indicator matches and all `required_ports` are open.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DeviceFingerprint {
    pub id: String,
    /// Case-insensitive substrings of the OUI vendor name.
    pub vendor_contains: Vec<String>,
    pub hostname_regex: Option<String>,
    pub required_ports: Vec<u16>,
    pub optional_ports: Vec<u16>,
    /// The rule is rejected if any of these ports is open.
    pub forbidden_ports: Vec<u16>,
    pub mdns_services: Vec<String>,
    pub http_title_regex: Option<String>,
    pub http_server_regex: Option<String>,
    pub tls_subject_regex: Option<String>,
    /// Matched against service banners (SSH, FTP, Telnet ...).
    pub banner_regex: Option<String>,
    /// Matched against IPP printer-make-and-model, SNMP sysDescr and mDNS
    /// TXT model fields.
    pub model_regex: Option<String>,
    pub device_class: DeviceClass,
    pub model_hint: Option<String>,
    pub os_hint: Option<String>,
    /// Bonus added when the rule applies.
    pub score: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    #[error("rule {id}: invalid regex in {field}: {source}")]
    Regex { id: String, field: &'static str, source: regex::Error },
    #[error("invalid rule file: {0}")]
    Json(#[from] serde_json::Error),
}

/// Rule with pre-compiled regexes.
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub rule: DeviceFingerprint,
    pub vendor_lc: Vec<String>,
    pub hostname: Option<Regex>,
    pub http_title: Option<Regex>,
    pub http_server: Option<Regex>,
    pub tls_subject: Option<Regex>,
    pub banner: Option<Regex>,
    pub model: Option<Regex>,
}

fn compile(id: &str, field: &'static str, re: &Option<String>) -> Result<Option<Regex>, RuleError> {
    match re {
        None => Ok(None),
        Some(s) => Regex::new(s).map(Some).map_err(|source| RuleError::Regex { id: id.to_string(), field, source }),
    }
}

impl CompiledRule {
    pub fn new(rule: DeviceFingerprint) -> Result<Self, RuleError> {
        let id = rule.id.clone();
        Ok(CompiledRule {
            vendor_lc: rule.vendor_contains.iter().map(|v| v.to_lowercase()).collect(),
            hostname: compile(&id, "hostname_regex", &rule.hostname_regex)?,
            http_title: compile(&id, "http_title_regex", &rule.http_title_regex)?,
            http_server: compile(&id, "http_server_regex", &rule.http_server_regex)?,
            tls_subject: compile(&id, "tls_subject_regex", &rule.tls_subject_regex)?,
            banner: compile(&id, "banner_regex", &rule.banner_regex)?,
            model: compile(&id, "model_regex", &rule.model_regex)?,
            rule,
        })
    }
}

/// Parse a JSON array of rules.
pub fn parse_rules(json: &str) -> Result<Vec<DeviceFingerprint>, RuleError> {
    Ok(serde_json::from_str(json)?)
}
