//! Import of the public IEEE Registration Authority registries.
//!
//! Source files (all public, see README "OUI-Datenbank"):
//! - MA-L: <https://standards-oui.ieee.org/oui/oui.csv>
//! - MA-M: <https://standards-oui.ieee.org/oui28/mam.csv>
//! - MA-S: <https://standards-oui.ieee.org/oui36/oui36.csv>
//!
//! Every file has the columns `Registry,Assignment,Organization Name,Organization Address`.

use crate::lookup::{key_for, META, OUI_PREFIXES, VENDORS, VENDOR_ADDRESSES};
use crate::{Result, VendorDbError};
use redb::Database;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// One registry assignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OuiRecord {
    pub prefix_bits: u8,
    pub prefix: u64,
    pub organization: String,
    pub address: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportStats {
    pub ma_l: usize,
    pub ma_m: usize,
    pub ma_s: usize,
    pub skipped: usize,
    pub vendors: usize,
}

impl ImportStats {
    pub fn total(&self) -> usize {
        self.ma_l + self.ma_m + self.ma_s
    }
}

fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// IEEE addresses end with `<COUNTRY> <POSTCODE>` or just `<COUNTRY>`.
fn country_from_address(addr: &str) -> Option<String> {
    let tokens: Vec<&str> = addr.split_whitespace().collect();
    tokens.iter().rev().take(3).find(|t| t.len() == 2 && t.chars().all(|c| c.is_ascii_uppercase())).map(|t| t.to_string())
}

/// Parse one IEEE registry CSV (MA-L, MA-M or MA-S).
pub fn parse_registry_csv<R: Read>(reader: R) -> Result<(Vec<OuiRecord>, usize)> {
    let mut rdr = csv::ReaderBuilder::new().has_headers(true).flexible(true).from_reader(reader);
    let mut out = Vec::new();
    let mut skipped = 0;
    for row in rdr.records() {
        let row = row?;
        let registry = row.get(0).unwrap_or("").trim();
        let assignment = row.get(1).unwrap_or("").trim();
        let org = clean(row.get(2).unwrap_or(""));
        let addr = clean(row.get(3).unwrap_or(""));
        let bits = match (registry, assignment.len()) {
            ("MA-L", 6) => 24,
            ("MA-M", 7) => 28,
            ("MA-S", 9) | ("IAB", 9) => 36,
            _ => {
                skipped += 1;
                continue;
            }
        };
        let Ok(prefix) = u64::from_str_radix(assignment, 16) else {
            skipped += 1;
            continue;
        };
        if org.is_empty() {
            skipped += 1;
            continue;
        }
        out.push(OuiRecord {
            prefix_bits: bits,
            prefix,
            country: country_from_address(&addr),
            address: (!addr.is_empty()).then_some(addr),
            organization: org,
        });
    }
    Ok((out, skipped))
}

/// Write records into a (new or existing) vendor database, replacing its
/// previous content.
pub fn write_records(db: &Database, records: &[OuiRecord], version: &str) -> Result<ImportStats> {
    let mut stats = ImportStats::default();
    let txn = db.begin_write()?;
    {
        // Start from a clean slate so removed assignments disappear.
        let _ = txn.delete_table(OUI_PREFIXES);
        let _ = txn.delete_table(VENDORS);
        let _ = txn.delete_table(VENDOR_ADDRESSES);
        let mut prefixes = txn.open_table(OUI_PREFIXES)?;
        let mut vendors = txn.open_table(VENDORS)?;
        let mut addresses = txn.open_table(VENDOR_ADDRESSES)?;
        let mut ids: HashMap<(&str, Option<&str>), u32> = HashMap::new();
        for r in records {
            let next = ids.len() as u32;
            let id = *ids.entry((r.organization.as_str(), r.address.as_deref())).or_insert_with(|| next);
            if id == next {
                vendors.insert(id, r.organization.as_str())?;
                if let Some(a) = &r.address {
                    addresses.insert(id, a.as_str())?;
                }
            }
            prefixes.insert(key_for(r.prefix_bits, r.prefix), id)?;
            match r.prefix_bits {
                24 => stats.ma_l += 1,
                28 => stats.ma_m += 1,
                _ => stats.ma_s += 1,
            }
        }
        stats.vendors = ids.len();
        let mut meta = txn.open_table(META)?;
        meta.insert("version", version)?;
        meta.insert("ma_l", stats.ma_l.to_string().as_str())?;
        meta.insert("ma_m", stats.ma_m.to_string().as_str())?;
        meta.insert("ma_s", stats.ma_s.to_string().as_str())?;
        meta.insert("source", "IEEE Registration Authority (MA-L, MA-M, MA-S)")?;
    }
    txn.commit()?;
    Ok(stats)
}

/// Import the given registry CSV files into a new database at `out`.
pub fn import_csv_files<P: AsRef<Path>>(inputs: &[P], out: &Path, version: &str) -> Result<ImportStats> {
    let mut records = Vec::new();
    let mut skipped = 0;
    for p in inputs {
        let f = std::fs::File::open(p.as_ref())?;
        let (mut r, s) = parse_registry_csv(std::io::BufReader::new(f))?;
        records.append(&mut r);
        skipped += s;
    }
    if records.is_empty() {
        return Err(VendorDbError::InvalidRow("no registry rows found".into()));
    }
    if out.exists() {
        std::fs::remove_file(out)?;
    }
    let mut db = Database::create(out)?;
    let mut stats = write_records(&db, &records, version)?;
    stats.skipped = skipped;
    // Compaction keeps the bundled file small.
    let _ = db.compact();
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ieee_rows() {
        let csv = "Registry,Assignment,Organization Name,Organization Address\n\
MA-L,3C5282,Hewlett Packard,11445 Compaq Center Drive Houston  US 77070 \n\
MA-M,C85CE27,SYNERGY SYSTEMS AND SOLUTIONS,\"A1526, GREEN FIELDS COLONY Faridabad HARYANA IN 121001 \"\n\
MA-S,8C1F64AFA,\"DATA ELECTRONIC DEVICES, INC\",32 NORTHWESTERN DR SALEM NH US 03079 \n\
CID,0A1B2C,Something,\n";
        let (r, skipped) = parse_registry_csv(csv.as_bytes()).unwrap();
        assert_eq!(skipped, 1);
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].prefix_bits, 24);
        assert_eq!(r[0].prefix, 0x3C5282);
        assert_eq!(r[0].country.as_deref(), Some("US"));
        assert_eq!(r[1].prefix_bits, 28);
        assert_eq!(r[1].country.as_deref(), Some("IN"));
        assert_eq!(r[2].prefix_bits, 36);
        assert_eq!(r[2].organization, "DATA ELECTRONIC DEVICES, INC");
    }
}
