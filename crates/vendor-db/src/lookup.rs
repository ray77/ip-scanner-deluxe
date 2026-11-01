use crate::import::{write_records, OuiRecord};
use crate::mac::MacAddr;
use crate::Result;
use redb::backends::InMemoryBackend;
use redb::{Database, ReadableDatabase, StorageBackend, TableDefinition};
use std::path::Path;

pub(crate) const OUI_PREFIXES: TableDefinition<u64, u32> = TableDefinition::new("oui_prefixes");
pub(crate) const VENDORS: TableDefinition<u32, &str> = TableDefinition::new("vendors");
pub(crate) const VENDOR_ADDRESSES: TableDefinition<u32, &str> = TableDefinition::new("vendor_addresses");
pub(crate) const META: TableDefinition<&str, &str> = TableDefinition::new("meta");

/// Assignment lengths, longest first, for longest-prefix match.
const PREFIX_BITS: [u8; 3] = [36, 28, 24];

pub(crate) fn key_for(bits: u8, prefix: u64) -> u64 {
    (u64::from(bits) << 40) | prefix
}

/// Result of a vendor lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VendorLookup {
    /// Registered assignment found.
    Vendor { name: String, address: Option<String>, prefix_bits: u8 },
    /// Locally administered (randomized / private) address. No vendor is
    /// claimed for these.
    Randomized,
    /// Universally administered but not registered (or DB missing).
    Unknown,
}

impl VendorLookup {
    pub fn vendor_name(&self) -> Option<&str> {
        match self {
            VendorLookup::Vendor { name, .. } => Some(name),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VendorDbInfo {
    pub version: String,
    pub source: String,
    pub ma_l: usize,
    pub ma_m: usize,
    pub ma_s: usize,
}

impl VendorDbInfo {
    pub fn total(&self) -> usize {
        self.ma_l + self.ma_m + self.ma_s
    }
}

/// Read access to the vendor database.
pub struct VendorDb {
    db: Database,
    info: VendorDbInfo,
}

impl std::fmt::Debug for VendorDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VendorDb").field("info", &self.info).finish()
    }
}

impl VendorDb {
    /// Open a vendor database file.
    pub fn open(path: &Path) -> Result<Self> {
        let db = Database::open(path)?;
        Self::from_database(db)
    }

    /// Load a database image (e.g. the bundled `vendor.redb`) into memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let backend = InMemoryBackend::new();
        backend.set_len(bytes.len() as u64)?;
        backend.write(0, bytes)?;
        let db = Database::builder().create_with_backend(backend)?;
        Self::from_database(db)
    }

    /// Build an in-memory database from records (tests, custom imports).
    pub fn from_records(records: &[OuiRecord], version: &str) -> Result<Self> {
        let db = Database::builder().create_with_backend(InMemoryBackend::new())?;
        write_records(&db, records, version)?;
        Self::from_database(db)
    }

    /// An empty database: every lookup yields `Unknown`/`Randomized`.
    pub fn empty() -> Result<Self> {
        Self::from_records(&[], "none")
    }

    fn from_database(db: Database) -> Result<Self> {
        let mut info = VendorDbInfo::default();
        let txn = db.begin_read()?;
        if let Ok(meta) = txn.open_table(META) {
            let get = |k: &str| -> Option<String> { meta.get(k).ok().flatten().map(|v| v.value().to_string()) };
            info.version = get("version").unwrap_or_default();
            info.source = get("source").unwrap_or_default();
            info.ma_l = get("ma_l").and_then(|v| v.parse().ok()).unwrap_or(0);
            info.ma_m = get("ma_m").and_then(|v| v.parse().ok()).unwrap_or(0);
            info.ma_s = get("ma_s").and_then(|v| v.parse().ok()).unwrap_or(0);
        }
        drop(txn);
        Ok(VendorDb { db, info })
    }

    pub fn info(&self) -> &VendorDbInfo {
        &self.info
    }

    /// Longest-prefix-match lookup (MA-S 36 bit, MA-M 28 bit, MA-L 24 bit).
    pub fn lookup(&self, mac: &MacAddr) -> VendorLookup {
        if mac.is_locally_administered() {
            return VendorLookup::Randomized;
        }
        self.lookup_registered(mac).unwrap_or(VendorLookup::Unknown)
    }

    fn lookup_registered(&self, mac: &MacAddr) -> Option<VendorLookup> {
        let txn = self.db.begin_read().ok()?;
        let prefixes = txn.open_table(OUI_PREFIXES).ok()?;
        for bits in PREFIX_BITS {
            let key = key_for(bits, mac.prefix(bits));
            if let Ok(Some(id)) = prefixes.get(key) {
                let id = id.value();
                let vendors = txn.open_table(VENDORS).ok()?;
                let name = vendors.get(id).ok()??.value().to_string();
                let address = txn.open_table(VENDOR_ADDRESSES).ok().and_then(|t| t.get(id).ok().flatten().map(|v| v.value().to_string()));
                return Some(VendorLookup::Vendor { name, address, prefix_bits: bits });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(bits: u8, prefix: u64, org: &str) -> OuiRecord {
        OuiRecord { prefix_bits: bits, prefix, organization: org.into(), address: None, country: None }
    }

    fn db() -> VendorDb {
        VendorDb::from_records(
            &[
                rec(24, 0x3C5282, "Hewlett Packard"),
                rec(24, 0x8C1F64, "IEEE Registration Authority"),
                rec(36, 0x8C1F64AFA, "DATA ELECTRONIC DEVICES, INC"),
                rec(24, 0xC85CE2, "IEEE Registration Authority"),
                rec(28, 0xC85CE27, "SYNERGY SYSTEMS AND SOLUTIONS"),
            ],
            "test",
        )
        .unwrap()
    }

    fn mac(s: &str) -> MacAddr {
        MacAddr::parse(s).unwrap()
    }

    #[test]
    fn ma_l_lookup() {
        let d = db();
        assert_eq!(d.lookup(&mac("3C:52:82:01:02:03")).vendor_name(), Some("Hewlett Packard"));
        assert_eq!(d.info().ma_l, 3);
        assert_eq!(d.info().ma_m, 1);
        assert_eq!(d.info().ma_s, 1);
    }

    #[test]
    fn longest_prefix_wins() {
        let d = db();
        match d.lookup(&mac("8C:1F:64:AF:A1:23")) {
            VendorLookup::Vendor { name, prefix_bits, .. } => {
                assert_eq!(name, "DATA ELECTRONIC DEVICES, INC");
                assert_eq!(prefix_bits, 36);
            }
            other => panic!("unexpected {other:?}"),
        }
        // Same MA-L block, different MA-S: falls back to the 24 bit owner.
        assert_eq!(d.lookup(&mac("8C:1F:64:00:01:23")).vendor_name(), Some("IEEE Registration Authority"));
        match d.lookup(&mac("C8:5C:E2:7A:BC:DE")) {
            VendorLookup::Vendor { prefix_bits, name, .. } => {
                assert_eq!(prefix_bits, 28);
                assert_eq!(name, "SYNERGY SYSTEMS AND SOLUTIONS");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn randomized_and_unknown() {
        let d = db();
        assert_eq!(d.lookup(&mac("DA:A1:19:01:02:03")), VendorLookup::Randomized);
        assert_eq!(d.lookup(&mac("00:00:5E:00:00:01")), VendorLookup::Unknown);
    }

    #[test]
    fn bytes_roundtrip() {
        let dir = std::env::temp_dir().join(format!("vdb-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("oui.csv");
        std::fs::write(
            &csv,
            "Registry,Assignment,Organization Name,Organization Address\nMA-L,B827EB,Raspberry Pi Foundation,Cambridge GB CB3 0FA\n",
        )
        .unwrap();
        let out = dir.join("vendor.redb");
        let stats = crate::import_csv_files(&[&csv], &out, "t").unwrap();
        assert_eq!(stats.ma_l, 1);
        let bytes = std::fs::read(&out).unwrap();
        let d = VendorDb::from_bytes(&bytes).unwrap();
        assert_eq!(d.lookup(&mac("B8:27:EB:00:00:01")).vendor_name(), Some("Raspberry Pi Foundation"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
