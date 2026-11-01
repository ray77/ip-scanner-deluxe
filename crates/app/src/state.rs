//! Scan results, annotations and favorites plus the derived table rows.

use chrono::{DateTime, Local};
use scan_core::model::ip_sort_key;
use scan_core::{HostRecord, HostStatus, ServiceType};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::SystemTime;
use storage::store::host_key;
use storage::{Annotation, AppSettings, Column, FavoriteDevice, HistoryChange, SortState, Store};
use ui::i18n::{t, tf};
use ui::table::{HostRow, ServiceRow};

pub fn fmt_time(t: Option<SystemTime>) -> String {
    t.map(|t| DateTime::<Local>::from(t).format("%d.%m.%Y %H:%M:%S").to_string()).unwrap_or_default()
}

pub fn class_label(h: &HostRecord) -> String {
    if h.device_class == fingerprint::DeviceClass::Unknown {
        if h.fingerprint_rule.as_deref() == Some("apple") {
            return t("class.apple").to_string();
        }
        return String::new();
    }
    t(&format!("class.{}", h.device_class.key())).to_string()
}

pub fn vendor_label(h: &HostRecord) -> String {
    if h.randomized_mac {
        t("vendor.randomized").to_string()
    } else {
        h.vendor.clone().unwrap_or_default()
    }
}

#[derive(Default)]
pub struct Data {
    pub hosts: Vec<HostRecord>,
    index: HashMap<IpAddr, usize>,
    pub annotations: HashMap<String, Annotation>,
    pub favorites: Vec<FavoriteDevice>,
    pub changes: HashMap<IpAddr, HistoryChange>,
    pub first_seen: HashMap<IpAddr, SystemTime>,
    pub last_scan: Option<SystemTime>,
    pub dirty: bool,
}

impl Data {
    pub fn load(store: Option<&Store>) -> Self {
        let mut d = Data { dirty: true, ..Default::default() };
        if let Some(s) = store {
            d.annotations = s.annotations().unwrap_or_default().into_iter().collect();
            d.favorites = s.favorites().unwrap_or_default();
        }
        d
    }

    pub fn clear(&mut self) {
        self.hosts.clear();
        self.index.clear();
        self.changes.clear();
        self.dirty = true;
    }

    pub fn get(&self, ip: IpAddr) -> Option<&HostRecord> {
        self.index.get(&ip).map(|i| &self.hosts[*i])
    }

    pub fn get_mut(&mut self, ip: IpAddr) -> Option<&mut HostRecord> {
        self.dirty = true;
        match self.index.get(&ip) {
            Some(i) => Some(&mut self.hosts[*i]),
            None => None,
        }
    }

    pub fn annotation_for(&self, h: &HostRecord) -> Option<&Annotation> {
        self.annotations.get(&host_key(h)).or_else(|| self.annotations.get(&format!("ip:{}", h.ip)))
    }

    pub fn is_favorite(&self, h: &HostRecord) -> bool {
        let key = host_key(h);
        self.favorites.iter().any(|f| f.id() == key || (f.mac.is_none() && f.ip == Some(h.ip)))
    }

    /// Insert or update a host. Keeps the previous record's details when the
    /// new one is only a "discovered" placeholder.
    pub fn upsert(&mut self, mut h: HostRecord) {
        if let Some(a) = self.annotation_for(&h) {
            h.comment = a.comment.clone();
        }
        h.is_favorite = self.is_favorite(&h);
        if let Some(i) = self.index.get(&h.ip).copied() {
            let old = &self.hosts[i];
            if h.status == HostStatus::Scanning && old.status == HostStatus::Alive {
                // Keep showing previous data while re-scanning.
                let mut merged = old.clone();
                merged.status = HostStatus::Scanning;
                merged.latency_ms = h.latency_ms.or(merged.latency_ms);
                self.hosts[i] = merged;
            } else {
                self.hosts[i] = h;
            }
        } else {
            self.index.insert(h.ip, self.hosts.len());
            self.hosts.push(h);
        }
        self.dirty = true;
    }

    /// Record history for final alive hosts.
    pub fn record_history(&mut self, ip: IpAddr, store: Option<&Store>) {
        let Some(store) = store else { return };
        let Some(h) = self.get(ip).cloned() else { return };
        if h.status != HostStatus::Alive {
            return;
        }
        if let Ok((change, entry)) = store.record_seen(&h) {
            if let Some(fs) = entry.first_seen {
                self.first_seen.insert(ip, fs);
            }
            self.changes.insert(ip, change);
        }
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        let mut a = 0;
        let mut d = 0;
        let mut u = 0;
        for h in &self.hosts {
            match h.status {
                HostStatus::Alive | HostStatus::Scanning => a += 1,
                HostStatus::NoResponse => d += 1,
                HostStatus::Unknown => u += 1,
            }
        }
        (a, d, u)
    }

    pub fn set_annotation(&mut self, ip: IpAddr, store: Option<&Store>, f: impl FnOnce(&mut Annotation)) {
        let Some(h) = self.get(ip).cloned() else { return };
        let key = host_key(&h);
        let mut a = self.annotations.get(&key).cloned().unwrap_or_default();
        f(&mut a);
        if let Some(s) = store {
            if let Err(e) = s.set_annotation(&key, &a) {
                tracing::warn!("saving annotation failed: {e}");
            }
        }
        if let Some(hm) = self.get_mut(ip) {
            hm.comment = a.comment.clone();
        }
        self.annotations.insert(key, a);
        self.dirty = true;
    }

    pub fn display_name(&self, h: &HostRecord) -> String {
        if let Some(alias) = self.annotation_for(h).and_then(|a| a.alias.clone()) {
            return alias;
        }
        h.hostname.clone().unwrap_or_else(|| h.ip.to_string())
    }

    fn row_for(&self, h: &HostRecord) -> HostRow {
        let mut cells = vec![String::new(); Column::ALL.len()];
        let name = self.display_name(h);
        let mac = h.mac.map(|m| m.to_string()).unwrap_or_default();
        let set = |cells: &mut Vec<String>, c: Column, v: String| cells[c as usize] = v;
        set(&mut cells, Column::Status, t(&format!("status.{}", h.status.key())).to_string());
        set(&mut cells, Column::Name, name.clone());
        set(&mut cells, Column::Ip, h.ip.to_string());
        set(&mut cells, Column::Vendor, vendor_label(h));
        set(&mut cells, Column::Mac, mac.clone());
        set(&mut cells, Column::Comment, h.comment.clone());
        set(&mut cells, Column::DeviceType, class_label(h));
        set(&mut cells, Column::Model, h.model.clone().unwrap_or_default());
        set(&mut cells, Column::Os, h.os.clone().unwrap_or_default());
        set(&mut cells, Column::Latency, h.latency_ms.map(|l| format!("{l:.1} ms")).unwrap_or_default());
        let svc_names: Vec<String> = h
            .services
            .iter()
            .filter(|s| s.service_type != ServiceType::MdnsService)
            .map(|s| s.display_name.split(',').next().unwrap_or("").to_string())
            .collect();
        set(&mut cells, Column::Services, svc_names.join(", "));
        set(
            &mut cells,
            Column::Confidence,
            if h.fingerprint_confidence > 0 { format!("{} %", h.fingerprint_confidence) } else { String::new() },
        );
        set(&mut cells, Column::LastScan, fmt_time(self.last_scan.filter(|_| h.status != HostStatus::Unknown)));
        set(&mut cells, Column::LastSeen, fmt_time(h.last_seen));
        set(&mut cells, Column::Interface, h.interface.clone().unwrap_or_default());

        let mut tooltip = Vec::new();
        if let Some(m) = &h.model {
            tooltip.push(format!("{}: {m}", t("col.model")));
        }
        if !class_label(h).is_empty() {
            tooltip.push(format!("{}: {} ({} %)", t("col.device_type"), class_label(h), h.fingerprint_confidence));
        }
        if h.randomized_mac {
            tooltip.push(t("vendor.randomized_long").to_string());
        }
        match self.changes.get(&h.ip) {
            Some(HistoryChange::New) => tooltip.push(t("hist.new").to_string()),
            Some(HistoryChange::IpChanged { old }) => tooltip.push(tf("hist.ip_changed", &[("old", &old.to_string())])),
            Some(HistoryChange::HostnameChanged { old }) => tooltip.push(tf("hist.name_changed", &[("old", old)])),
            _ => {}
        }

        HostRow {
            ip: h.ip,
            status: h.status,
            class: h.device_class,
            alive: matches!(h.status, HostStatus::Alive | HostStatus::Scanning),
            favorite: h.is_favorite,
            has_mac: h.mac.is_some(),
            name,
            mac,
            cells,
            services: h
                .services
                .iter()
                .map(|s| ServiceRow {
                    service_type: s.service_type.clone(),
                    label: s.display_name.clone(),
                    port: s.port,
                    uri: s.uri.clone(),
                })
                .collect(),
            tooltip: tooltip.join("\n"),
        }
    }

    fn matches(&self, h: &HostRecord, row: &HostRow, q: &str) -> bool {
        if q.is_empty() {
            return true;
        }
        let hay = [
            row.name.as_str(),
            row.cell(Column::Ip),
            row.cell(Column::Vendor),
            row.mac.as_str(),
            row.cell(Column::DeviceType),
            row.cell(Column::Model),
            row.cell(Column::Os),
            row.cell(Column::Comment),
            h.hostname.as_deref().unwrap_or(""),
        ];
        if hay.iter().any(|s| s.to_lowercase().contains(q)) {
            return true;
        }
        let q_mac: String = q.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if q_mac.len() >= 4 && row.mac.replace(':', "").to_lowercase().contains(&q_mac) {
            return true;
        }
        h.all_hostnames.iter().any(|n| n.to_lowercase().contains(q)) || h.services.iter().any(|s| s.display_name.to_lowercase().contains(q))
    }

    /// Visible rows for the result list.
    pub fn result_rows(&self, settings: &AppSettings, query: &str) -> Vec<HostRow> {
        let q = query.trim().to_lowercase();
        let mut v: Vec<(&HostRecord, HostRow)> = self
            .hosts
            .iter()
            .filter(|h| match h.status {
                HostStatus::Alive | HostStatus::Scanning => true,
                HostStatus::NoResponse => settings.show_offline && !settings.show_alive_only,
                HostStatus::Unknown => settings.show_unknown && !settings.show_alive_only,
            })
            .map(|h| (h, self.row_for(h)))
            .filter(|(h, r)| self.matches(h, r, &q))
            .collect();
        sort_rows(&mut v, &settings.sort);
        v.into_iter().map(|(_, r)| r).collect()
    }

    /// Favorites tab: favorites merged with current scan data.
    pub fn favorite_rows(&self, settings: &AppSettings, query: &str) -> Vec<HostRow> {
        let q = query.trim().to_lowercase();
        let recs: Vec<HostRecord> = self
            .favorites
            .iter()
            .filter_map(|f| {
                let live = self.hosts.iter().find(|h| (f.mac.is_some() && h.mac == f.mac) || (f.mac.is_none() && Some(h.ip) == f.ip));
                match live {
                    Some(h) => Some(h.clone()),
                    None => {
                        let mut h = HostRecord::new(f.ip?, HostStatus::Unknown);
                        h.mac = f.mac;
                        h.hostname = f.name.clone();
                        h.vendor = f.vendor.clone();
                        h.comment = f.comment.clone();
                        h.last_seen = f.last_seen;
                        h.is_favorite = true;
                        Some(h)
                    }
                }
            })
            .collect();
        let mut v: Vec<(&HostRecord, HostRow)> = recs
            .iter()
            .map(|h| {
                let mut r = self.row_for(h);
                r.favorite = true;
                (h, r)
            })
            .filter(|(h, r)| self.matches(h, r, &q))
            .collect();
        sort_rows(&mut v, &settings.sort);
        v.into_iter().map(|(_, r)| r).collect()
    }

    pub fn add_favorite(&mut self, ip: IpAddr, store: Option<&Store>) {
        let Some(h) = self.get(ip).cloned() else { return };
        let mut f = FavoriteDevice::from_host(&h);
        if let Some(a) = self.annotation_for(&h) {
            if let Some(alias) = &a.alias {
                f.name = Some(alias.clone());
            }
        }
        if let Some(s) = store {
            let _ = s.upsert_favorite(&f);
        }
        self.favorites.retain(|x| x.id() != f.id());
        self.favorites.push(f);
        if let Some(hm) = self.get_mut(ip) {
            hm.is_favorite = true;
        }
        self.dirty = true;
    }

    pub fn remove_favorite(&mut self, ip: IpAddr, store: Option<&Store>) {
        let key = self.get(ip).map(host_key);
        let ids: Vec<String> = self.favorites.iter().filter(|f| Some(f.id()) == key || f.ip == Some(ip)).map(FavoriteDevice::id).collect();
        for id in &ids {
            if let Some(s) = store {
                let _ = s.remove_favorite(id);
            }
        }
        self.favorites.retain(|f| !ids.contains(&f.id()));
        if let Some(hm) = self.get_mut(ip) {
            hm.is_favorite = false;
        }
        self.dirty = true;
    }
}

fn sort_rows(v: &mut [(&HostRecord, HostRow)], sort: &SortState) {
    let col = if sort.user { sort.column } else { Column::Ip };
    let asc = if sort.user { sort.ascending } else { true };
    v.sort_by(|(ha, ra), (hb, rb)| {
        let ord = match col {
            Column::Ip => ip_sort_key(&ha.ip).cmp(&ip_sort_key(&hb.ip)),
            Column::Status => ha.status.cmp(&hb.status).then(ip_sort_key(&ha.ip).cmp(&ip_sort_key(&hb.ip))),
            Column::Latency => ha.latency_ms.unwrap_or(f32::MAX).total_cmp(&hb.latency_ms.unwrap_or(f32::MAX)),
            Column::Confidence => ha.fingerprint_confidence.cmp(&hb.fingerprint_confidence),
            Column::LastSeen | Column::LastScan => ha.last_seen.cmp(&hb.last_seen),
            Column::Mac => ha.mac.cmp(&hb.mac),
            c => {
                let a = ra.cell(c).to_lowercase();
                let b = rb.cell(c).to_lowercase();
                // Empty values last.
                match (a.is_empty(), b.is_empty()) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => a.cmp(&b),
                }
            }
        };
        let ord = ord.then_with(|| ip_sort_key(&ha.ip).cmp(&ip_sort_key(&hb.ip)));
        if asc {
            ord
        } else {
            ord.reverse()
        }
    });
}
