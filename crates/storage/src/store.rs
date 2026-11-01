use redb::{Database, ReadableDatabase, ReadableTable, TableDefinition};
use scan_core::HostRecord;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::net::IpAddr;
use std::path::Path;
use std::time::SystemTime;
use vendor_db::MacAddr;

const SETTINGS: TableDefinition<&str, &str> = TableDefinition::new("settings");
const FAVORITES: TableDefinition<&str, &str> = TableDefinition::new("favorites");
const ALIASES: TableDefinition<&str, &str> = TableDefinition::new("user_device_aliases");
const HISTORY: TableDefinition<&str, &str> = TableDefinition::new("scan_history");

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database: {0}")]
    Db(#[from] redb::Error),
    #[error("serialization: {0}")]
    Json(#[from] serde_json::Error),
}

macro_rules! from_redb {
    ($($t:ty),*) => { $(impl From<$t> for StoreError { fn from(e: $t) -> Self { StoreError::Db(e.into()) } })* };
}
from_redb!(redb::DatabaseError, redb::TransactionError, redb::TableError, redb::StorageError, redb::CommitError);

pub type Result<T> = std::result::Result<T, StoreError>;

/// A saved device ("Favoriten"). MAC is the preferred identifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FavoriteDevice {
    pub name: Option<String>,
    pub ip: Option<IpAddr>,
    pub mac: Option<MacAddr>,
    pub vendor: Option<String>,
    pub comment: String,
    pub last_seen: Option<SystemTime>,
}

impl FavoriteDevice {
    pub fn id(&self) -> String {
        device_key(self.mac.as_ref(), self.ip)
    }

    pub fn from_host(h: &HostRecord) -> Self {
        FavoriteDevice {
            name: h.hostname.clone(),
            ip: Some(h.ip),
            mac: h.mac,
            vendor: h.vendor.clone(),
            comment: h.comment.clone(),
            last_seen: h.last_seen,
        }
    }
}

/// User-assigned name and comment for a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Annotation {
    pub alias: Option<String>,
    pub comment: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HistoryEntry {
    pub first_seen: Option<SystemTime>,
    pub last_seen: Option<SystemTime>,
    pub last_ip: Option<IpAddr>,
    pub last_hostname: Option<String>,
    pub last_vendor: Option<String>,
    pub last_latency: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryChange {
    New,
    IpChanged { old: IpAddr },
    HostnameChanged { old: String },
    Unchanged,
}

pub fn device_key(mac: Option<&MacAddr>, ip: Option<IpAddr>) -> String {
    match (mac, ip) {
        (Some(m), _) if !m.is_locally_administered() => m.to_string(),
        (Some(m), Some(ip)) => format!("{m}@{ip}"),
        (Some(m), None) => m.to_string(),
        (None, Some(ip)) => format!("ip:{ip}"),
        (None, None) => String::new(),
    }
}

pub fn host_key(h: &HostRecord) -> String {
    device_key(h.mac.as_ref(), Some(h.ip))
}

pub struct Store {
    db: Database,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::create(path)?;
        let s = Store { db };
        s.init()?;
        Ok(s)
    }

    pub fn in_memory() -> Result<Self> {
        let db = Database::builder().create_with_backend(redb::backends::InMemoryBackend::new())?;
        let s = Store { db };
        s.init()?;
        Ok(s)
    }

    fn init(&self) -> Result<()> {
        let w = self.db.begin_write()?;
        w.open_table(SETTINGS)?;
        w.open_table(FAVORITES)?;
        w.open_table(ALIASES)?;
        w.open_table(HISTORY)?;
        w.commit()?;
        Ok(())
    }

    fn get<T: DeserializeOwned>(&self, table: TableDefinition<&str, &str>, key: &str) -> Result<Option<T>> {
        let r = self.db.begin_read()?;
        let t = r.open_table(table)?;
        match t.get(key)? {
            Some(v) => Ok(Some(serde_json::from_str(v.value())?)),
            None => Ok(None),
        }
    }

    fn put<T: Serialize>(&self, table: TableDefinition<&str, &str>, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        let w = self.db.begin_write()?;
        {
            let mut t = w.open_table(table)?;
            t.insert(key, json.as_str())?;
        }
        w.commit()?;
        Ok(())
    }

    fn delete(&self, table: TableDefinition<&str, &str>, key: &str) -> Result<()> {
        let w = self.db.begin_write()?;
        {
            let mut t = w.open_table(table)?;
            t.remove(key)?;
        }
        w.commit()?;
        Ok(())
    }

    fn all<T: DeserializeOwned>(&self, table: TableDefinition<&str, &str>) -> Result<Vec<(String, T)>> {
        let r = self.db.begin_read()?;
        let t = r.open_table(table)?;
        let mut out = Vec::new();
        for item in t.iter()? {
            let (k, v) = item?;
            match serde_json::from_str(v.value()) {
                Ok(val) => out.push((k.value().to_string(), val)),
                Err(e) => tracing::warn!("skipping corrupt record {}: {e}", k.value()),
            }
        }
        Ok(out)
    }

    // --- settings -------------------------------------------------------
    pub fn load_settings<T: DeserializeOwned + Default>(&self) -> T {
        self.get(SETTINGS, "app").ok().flatten().unwrap_or_default()
    }

    pub fn save_settings<T: Serialize>(&self, s: &T) -> Result<()> {
        self.put(SETTINGS, "app", s)
    }

    // --- favorites ------------------------------------------------------
    pub fn favorites(&self) -> Result<Vec<FavoriteDevice>> {
        Ok(self.all::<FavoriteDevice>(FAVORITES)?.into_iter().map(|(_, f)| f).collect())
    }

    pub fn upsert_favorite(&self, f: &FavoriteDevice) -> Result<()> {
        self.put(FAVORITES, &f.id(), f)
    }

    pub fn remove_favorite(&self, id: &str) -> Result<()> {
        self.delete(FAVORITES, id)
    }

    pub fn is_favorite(&self, id: &str) -> bool {
        self.get::<FavoriteDevice>(FAVORITES, id).ok().flatten().is_some()
    }

    // --- annotations ----------------------------------------------------
    pub fn annotation(&self, id: &str) -> Annotation {
        self.get(ALIASES, id).ok().flatten().unwrap_or_default()
    }

    pub fn annotations(&self) -> Result<Vec<(String, Annotation)>> {
        self.all(ALIASES)
    }

    pub fn set_annotation(&self, id: &str, a: &Annotation) -> Result<()> {
        if a.alias.is_none() && a.comment.is_empty() {
            self.delete(ALIASES, id)
        } else {
            self.put(ALIASES, id, a)
        }
    }

    // --- history --------------------------------------------------------
    pub fn history(&self, id: &str) -> Option<HistoryEntry> {
        self.get(HISTORY, id).ok().flatten()
    }

    /// Record a sighting; returns what changed since the last scan.
    pub fn record_seen(&self, h: &HostRecord) -> Result<(HistoryChange, HistoryEntry)> {
        let id = host_key(h);
        let now = h.last_seen.unwrap_or_else(SystemTime::now);
        let prev = self.history(&id);
        let change = match &prev {
            None => HistoryChange::New,
            Some(p) if p.last_ip.is_some_and(|ip| ip != h.ip) && h.mac.is_some() => {
                HistoryChange::IpChanged { old: p.last_ip.unwrap_or(h.ip) }
            }
            Some(p) => match (&p.last_hostname, &h.hostname) {
                (Some(old), Some(new)) if old != new => HistoryChange::HostnameChanged { old: old.clone() },
                _ => HistoryChange::Unchanged,
            },
        };
        let entry = HistoryEntry {
            first_seen: prev.as_ref().and_then(|p| p.first_seen).or(Some(now)),
            last_seen: Some(now),
            last_ip: Some(h.ip),
            last_hostname: h.hostname.clone().or(prev.as_ref().and_then(|p| p.last_hostname.clone())),
            last_vendor: h.vendor.clone(),
            last_latency: h.latency_ms,
        };
        self.put(HISTORY, &id, &entry)?;
        Ok((change, entry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scan_core::HostStatus;

    fn host(ip: &str, mac: &str) -> HostRecord {
        let mut h = HostRecord::new(ip.parse().unwrap(), HostStatus::Alive);
        h.mac = Some(MacAddr::parse(mac).unwrap());
        h.hostname = Some("pc01".into());
        h
    }

    #[test]
    fn favorites_roundtrip() {
        let s = Store::in_memory().unwrap();
        let f = FavoriteDevice::from_host(&host("192.168.0.96", "3C:52:82:11:22:33"));
        s.upsert_favorite(&f).unwrap();
        assert!(s.is_favorite("3C:52:82:11:22:33"));
        assert_eq!(s.favorites().unwrap(), vec![f]);
        s.remove_favorite("3C:52:82:11:22:33").unwrap();
        assert!(s.favorites().unwrap().is_empty());
    }

    #[test]
    fn annotations_follow_mac() {
        let s = Store::in_memory().unwrap();
        let a = Annotation { alias: Some("Drucker Buchhaltung".into()), comment: "Rack 2".into() };
        s.set_annotation(&host_key(&host("192.168.0.96", "3C:52:82:11:22:33")), &a).unwrap();
        // Same MAC, new IP -> same annotation.
        assert_eq!(s.annotation(&host_key(&host("192.168.0.97", "3C:52:82:11:22:33"))), a);
    }

    #[test]
    fn history_changes() {
        let s = Store::in_memory().unwrap();
        let h = host("192.168.0.96", "3C:52:82:11:22:33");
        assert_eq!(s.record_seen(&h).unwrap().0, HistoryChange::New);
        assert_eq!(s.record_seen(&h).unwrap().0, HistoryChange::Unchanged);
        let moved = host("192.168.0.97", "3C:52:82:11:22:33");
        assert!(matches!(s.record_seen(&moved).unwrap().0, HistoryChange::IpChanged { .. }));
    }

    #[test]
    fn settings_roundtrip() {
        let s = Store::in_memory().unwrap();
        let st = crate::AppSettings { last_range: "192.168.0.1-254".into(), theme: crate::AppTheme::Dark, ..Default::default() };
        s.save_settings(&st).unwrap();
        let back: crate::AppSettings = s.load_settings();
        assert_eq!(back.last_range, "192.168.0.1-254");
        assert_eq!(back.theme, crate::AppTheme::Dark);
    }
}
