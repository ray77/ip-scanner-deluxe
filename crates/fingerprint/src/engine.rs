use crate::builtin::{apple_model, builtin_rules};
use crate::rules::{CompiledRule, DeviceClass, DeviceFingerprint, RuleError};
use std::collections::BTreeMap;

/// Everything the scanner learned about one host.
#[derive(Debug, Clone, Default)]
pub struct FingerprintInput {
    pub vendor: Option<String>,
    pub hostnames: Vec<String>,
    pub open_ports: Vec<u16>,
    /// mDNS service types without domain, e.g. `_ipp._tcp`.
    pub mdns_services: Vec<String>,
    /// Merged mDNS TXT records (lower-case keys).
    pub mdns_txt: BTreeMap<String, String>,
    pub http_titles: Vec<String>,
    pub http_servers: Vec<String>,
    pub tls_subjects: Vec<String>,
    pub banners: Vec<String>,
    pub ipp_make_model: Option<String>,
    pub snmp_sys_descr: Option<String>,
    pub netbios_name: Option<String>,
    pub is_gateway: bool,
}

/// Outcome of fingerprinting.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FingerprintMatch {
    pub rule_id: Option<String>,
    pub device_class: DeviceClass,
    pub model: Option<String>,
    pub os: Option<String>,
    /// 0..=100
    pub confidence: u8,
    /// Human readable reasons (e.g. "Hersteller", "mDNS _ipp._tcp").
    pub evidence: Vec<String>,
}

// Score weights, see spec "Confidence Score".
const W_VENDOR: u32 = 30;
const W_HOSTNAME: u32 = 20;
const W_PORT: u32 = 10;
const W_PORT_MAX: u32 = 20;
const W_MDNS: u32 = 20;
const W_HTTP_TITLE: u32 = 30;
const W_HTTP_SERVER: u32 = 15;
const W_TLS: u32 = 20;
const W_BANNER: u32 = 20;
const W_MODEL: u32 = 50;
const W_CORROBORATION: u32 = 10;

/// TXT keys that carry a model description, in order of preference.
const TXT_MODEL_KEYS: [&str; 6] = ["ty", "usb_mdl", "md", "product", "model", "modelname"];

pub struct FingerprintEngine {
    rules: Vec<CompiledRule>,
}

impl Default for FingerprintEngine {
    fn default() -> Self {
        Self::with_builtin()
    }
}

impl FingerprintEngine {
    pub fn new(rules: Vec<DeviceFingerprint>) -> Result<Self, RuleError> {
        let rules = rules.into_iter().map(CompiledRule::new).collect::<Result<Vec<_>, _>>()?;
        Ok(FingerprintEngine { rules })
    }

    /// Engine with the shipped rules. Invalid rules are skipped (logged).
    pub fn with_builtin() -> Self {
        Self::with_builtin_and(Vec::new())
    }

    /// Built-in rules followed by user rules; invalid ones are skipped.
    pub fn with_builtin_and(extra: Vec<DeviceFingerprint>) -> Self {
        let rules = builtin_rules()
            .into_iter()
            .chain(extra)
            .filter_map(|r| match CompiledRule::new(r) {
                Ok(c) => Some(c),
                Err(e) => {
                    tracing::warn!("skipping fingerprint rule: {e}");
                    None
                }
            })
            .collect();
        FingerprintEngine { rules }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn rules(&self) -> impl Iterator<Item = &DeviceFingerprint> {
        self.rules.iter().map(|r| &r.rule)
    }

    pub fn identify(&self, input: &FingerprintInput) -> FingerprintMatch {
        let mut best: Option<FingerprintMatch> = None;
        for rule in &self.rules {
            if let Some(m) = evaluate(rule, input) {
                if best.as_ref().is_none_or(|b| m.confidence > b.confidence) {
                    best = Some(m);
                }
            }
        }
        let mut result = best.unwrap_or_else(|| heuristic(input));
        refine_model(&mut result, input);
        // Evidence announced by the device (banners, headers) beats rule hints.
        if let Some(os) = guess_os(input) {
            result.os = Some(os);
        }
        result
    }
}

fn any_match(re: &Option<regex::Regex>, values: &[String]) -> Option<String> {
    let re = re.as_ref()?;
    values.iter().find(|v| re.is_match(v)).cloned()
}

fn model_sources(input: &FingerprintInput) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(m) = &input.ipp_make_model {
        v.push(m.clone());
    }
    if let Some(s) = &input.snmp_sys_descr {
        v.push(s.clone());
    }
    for k in TXT_MODEL_KEYS {
        if let Some(t) = input.mdns_txt.get(k) {
            v.push(t.clone());
        }
    }
    v
}

fn evaluate(rule: &CompiledRule, input: &FingerprintInput) -> Option<FingerprintMatch> {
    let r = &rule.rule;
    if r.required_ports.iter().any(|p| !input.open_ports.contains(p)) {
        return None;
    }
    if r.forbidden_ports.iter().any(|p| input.open_ports.contains(p)) {
        return None;
    }

    let mut score = 0u32;
    let mut strong = false;
    let mut strong_hits = 0u32;
    let mut evidence = Vec::new();

    if !rule.vendor_lc.is_empty() {
        let vendor = input.vendor.as_deref().unwrap_or("").to_lowercase();
        if !vendor.is_empty() && rule.vendor_lc.iter().any(|v| vendor.contains(v.as_str())) {
            score += W_VENDOR;
            strong = true;
            evidence.push(format!("vendor:{}", input.vendor.as_deref().unwrap_or("")));
        }
    }
    if let Some(h) = any_match(&rule.hostname, &input.hostnames) {
        score += W_HOSTNAME;
        strong = true;
        evidence.push(format!("hostname:{h}"));
    }
    if !r.required_ports.is_empty() {
        score += W_PORT;
        evidence.push(format!("ports:{:?}", r.required_ports));
    }
    let optional_hits = r.optional_ports.iter().filter(|p| input.open_ports.contains(p)).count() as u32;
    if optional_hits > 0 {
        score += (optional_hits * W_PORT).min(W_PORT_MAX);
        evidence.push(format!("ports:{optional_hits}"));
    }
    let mdns_hit = r.mdns_services.iter().find(|s| input.mdns_services.iter().any(|m| m.eq_ignore_ascii_case(s)));
    if let Some(s) = mdns_hit {
        score += W_MDNS;
        strong = true;
        evidence.push(format!("mdns:{s}"));
    }
    if let Some(t) = any_match(&rule.http_title, &input.http_titles) {
        score += W_HTTP_TITLE;
        strong = true;
        evidence.push(format!("http-title:{t}"));
    }
    if let Some(s) = any_match(&rule.http_server, &input.http_servers) {
        score += W_HTTP_SERVER;
        strong = true;
        evidence.push(format!("http-server:{s}"));
    }
    if let Some(s) = any_match(&rule.tls_subject, &input.tls_subjects) {
        score += W_TLS;
        strong = true;
        evidence.push(format!("tls:{s}"));
    }
    if let Some(b) = any_match(&rule.banner, &input.banners) {
        score += W_BANNER;
        strong = true;
        evidence.push(format!("banner:{b}"));
    }
    let model_from_device = any_match(&rule.model, &model_sources(input));
    if let Some(m) = &model_from_device {
        score += W_MODEL;
        strong = true;
        evidence.push(format!("model:{m}"));
    }

    if !strong {
        return None;
    }
    // Independent indicators corroborate each other.
    strong_hits += evidence.iter().filter(|e| !e.starts_with("ports:")).count() as u32;
    score += strong_hits.saturating_sub(1) * W_CORROBORATION;
    // A vendor alone is weak evidence for a *class* of device; require a
    // second indicator unless the rule is vendor-only by design (score bonus
    // marks those, e.g. virtual machine NIC vendors).
    let indicator_count = evidence.len();
    if indicator_count < 2 && r.score == 0 {
        score = score.min(29);
    }
    score += r.score;

    Some(FingerprintMatch {
        rule_id: Some(r.id.clone()),
        device_class: r.device_class,
        model: model_from_device.or_else(|| r.model_hint.clone()),
        os: r.os_hint.clone(),
        confidence: score.min(100) as u8,
        evidence,
    })
}

/// Fallback when no rule applies: classify by well known port combinations.
fn heuristic(input: &FingerprintInput) -> FingerprintMatch {
    let has = |p: u16| input.open_ports.contains(&p);
    let mut m = FingerprintMatch::default();
    if has(9100) || has(631) || has(515) || input.mdns_services.iter().any(|s| s.contains("ipp") || s.contains("printer")) {
        m.device_class = DeviceClass::Printer;
        m.confidence = 45;
        m.evidence.push("printer ports".into());
    } else if input.is_gateway {
        m.device_class = DeviceClass::Router;
        m.confidence = 50;
        m.evidence.push("default gateway".into());
    } else if has(3389) && has(445) {
        m.device_class = DeviceClass::Desktop;
        m.confidence = 35;
        m.evidence.push("rdp+smb".into());
    } else if has(445) && (has(5000) || has(5001)) {
        m.device_class = DeviceClass::Nas;
        m.confidence = 35;
        m.evidence.push("smb+web admin".into());
    } else if has(554) {
        m.device_class = DeviceClass::Camera;
        m.confidence = 30;
        m.evidence.push("rtsp".into());
    } else if has(22) && (has(80) || has(443)) && !has(445) {
        m.device_class = DeviceClass::Server;
        m.confidence = 25;
        m.evidence.push("ssh+http".into());
    }
    m
}

fn txt_model(input: &FingerprintInput) -> Option<String> {
    TXT_MODEL_KEYS.iter().find_map(|k| input.mdns_txt.get(*k)).filter(|s| !s.trim().is_empty()).cloned()
}

/// Prefer models announced by the device itself over rule hints.
fn refine_model(m: &mut FingerprintMatch, input: &FingerprintInput) {
    if let Some(ipp) = input.ipp_make_model.as_ref().filter(|s| !s.trim().is_empty()) {
        m.model = Some(ipp.trim().to_string());
        if m.device_class == DeviceClass::Unknown {
            m.device_class = DeviceClass::Printer;
        }
        m.confidence = m.confidence.max(40).saturating_add(50).min(100);
        m.evidence.push("ipp".into());
        return;
    }
    let vendor_apple = input.vendor.as_deref().is_some_and(|v| v.to_lowercase().contains("apple"));
    if let Some(id) = input.mdns_txt.get("model") {
        if let Some((name, class)) = apple_model(id) {
            if vendor_apple || input.mdns_services.iter().any(|s| s == "_device-info._tcp") {
                m.model = Some(format!("{name} ({id})"));
                if class != DeviceClass::Unknown {
                    m.device_class = class;
                }
                m.confidence = m.confidence.saturating_add(30).min(95);
                m.evidence.push("mdns model".into());
                return;
            }
        }
    }
    if m.model.is_none() {
        m.model = txt_model(input);
    }
}

fn guess_os(input: &FingerprintInput) -> Option<String> {
    let banners = input.banners.join(" ").to_lowercase();
    for (needle, os) in [
        ("raspbian", "Raspberry Pi OS"),
        ("ubuntu", "Ubuntu Linux"),
        ("debian", "Debian Linux"),
        ("freebsd", "FreeBSD"),
        ("microsoft", "Windows"),
        ("windows", "Windows"),
    ] {
        if banners.contains(needle) {
            return Some(os.to_string());
        }
    }
    let servers = input.http_servers.join(" ").to_lowercase();
    if servers.contains("microsoft-iis") {
        return Some("Windows".into());
    }
    let has = |p: u16| input.open_ports.contains(&p);
    if has(3389) || (has(135) && has(445)) {
        return Some("Windows".into());
    }
    if input.mdns_services.iter().any(|s| s == "_companion-link._tcp" || s == "_airplay._tcp")
        && input.vendor.as_deref().is_some_and(|v| v.to_lowercase().contains("apple"))
    {
        return Some("Apple".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> FingerprintEngine {
        let e = FingerprintEngine::with_builtin();
        assert!(e.rule_count() > 20, "builtin rules must load");
        e
    }

    fn input() -> FingerprintInput {
        FingerprintInput::default()
    }

    #[test]
    fn hp_printer_with_ipp_model() {
        let mut i = input();
        i.vendor = Some("Hewlett Packard".into());
        i.open_ports = vec![80, 443, 631, 9100];
        i.mdns_services = vec!["_ipp._tcp".into(), "_printer._tcp".into()];
        i.ipp_make_model = Some("HP Color LaserJet 9500".into());
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::Printer);
        assert_eq!(m.model.as_deref(), Some("HP Color LaserJet 9500"));
        assert!(m.confidence >= 90, "{m:?}");
    }

    #[test]
    fn hp_switch_without_printer_ports() {
        let mut i = input();
        i.vendor = Some("Hewlett Packard".into());
        i.open_ports = vec![22, 80, 443];
        i.http_titles = vec!["HP ProCurve Switch 2810-24G".into()];
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::Switch, "{m:?}");
        assert!(m.confidence >= 50);
    }

    #[test]
    fn raspberry_pi() {
        let mut i = input();
        i.vendor = Some("Raspberry Pi Trading Ltd".into());
        i.hostnames = vec!["raspberrypi.local".into()];
        i.open_ports = vec![22];
        i.banners = vec!["SSH-2.0-OpenSSH_8.4p1 Raspbian-5+deb11u1".into()];
        let m = engine().identify(&i);
        assert_eq!(m.rule_id.as_deref(), Some("raspberry-pi"));
        assert_eq!(m.os.as_deref(), Some("Raspberry Pi OS"));
        assert!(m.confidence >= 70, "{m:?}");
    }

    #[test]
    fn apple_without_evidence_is_not_a_macbook() {
        let mut i = input();
        i.vendor = Some("Apple, Inc.".into());
        let m = engine().identify(&i);
        assert!(m.model.is_none(), "{m:?}");
        assert!(m.confidence < 50, "{m:?}");
    }

    #[test]
    fn apple_with_device_info() {
        let mut i = input();
        i.vendor = Some("Apple, Inc.".into());
        i.mdns_services = vec!["_device-info._tcp".into(), "_companion-link._tcp".into()];
        i.mdns_txt.insert("model".into(), "MacBookPro18,3".into());
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::Laptop);
        assert_eq!(m.model.as_deref(), Some("MacBook Pro (MacBookPro18,3)"));
    }

    #[test]
    fn fritzbox() {
        let mut i = input();
        i.vendor = Some("AVM Audiovisuelles Marketing und Computersysteme GmbH".into());
        i.hostnames = vec!["fritz.box".into()];
        i.open_ports = vec![53, 80, 443, 49000];
        i.http_titles = vec!["FRITZ!Box".into()];
        i.is_gateway = true;
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::Router);
        assert_eq!(m.model.as_deref(), Some("FRITZ!Box"));
        assert!(m.confidence >= 90, "{m:?}");
    }

    #[test]
    fn synology() {
        let mut i = input();
        i.vendor = Some("Synology Incorporated".into());
        i.open_ports = vec![139, 445, 5000, 5001];
        i.http_titles = vec!["Synology DiskStation - nas".into()];
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::Nas);
        assert!(m.confidence >= 90, "{m:?}");
    }

    #[test]
    fn home_assistant() {
        let mut i = input();
        i.open_ports = vec![8123];
        i.mdns_services = vec!["_home-assistant._tcp".into()];
        i.http_titles = vec!["Home Assistant".into()];
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::HomeAutomation);
        assert!(m.confidence >= 70, "{m:?}");
    }

    #[test]
    fn heuristic_gateway() {
        let mut i = input();
        i.is_gateway = true;
        i.open_ports = vec![80];
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::Router);
    }

    #[test]
    fn vm_vendor() {
        let mut i = input();
        i.vendor = Some("VMware, Inc.".into());
        let m = engine().identify(&i);
        assert_eq!(m.device_class, DeviceClass::VirtualMachine);
    }
}
