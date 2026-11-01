//! MAC address handling and the offline IEEE OUI vendor database.
//!
//! The database is built from the public IEEE Registration Authority
//! registries (MA-L, MA-M, MA-S) and stored in a `redb` file. Lookups use a
//! longest-prefix match over 36, 28 and 24 bit assignments.

pub mod import;
pub mod lookup;
pub mod mac;

pub use import::{import_csv_files, parse_registry_csv, ImportStats};
pub use lookup::{VendorDb, VendorDbInfo, VendorLookup};
pub use mac::{MacAddr, MacParseError};

/// Errors produced by the vendor database.
#[derive(Debug, thiserror::Error)]
pub enum VendorDbError {
    #[error("database error: {0}")]
    Db(#[from] redb::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("invalid registry row: {0}")]
    InvalidRow(String),
}

macro_rules! impl_from_redb {
    ($($t:ty),*) => {
        $(impl From<$t> for VendorDbError {
            fn from(e: $t) -> Self {
                VendorDbError::Db(e.into())
            }
        })*
    };
}
impl_from_redb!(redb::DatabaseError, redb::TransactionError, redb::TableError, redb::StorageError, redb::CommitError);

pub type Result<T> = std::result::Result<T, VendorDbError>;
