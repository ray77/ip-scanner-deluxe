//! Local persistence. Everything stays on this machine.

pub mod paths;
pub mod settings;
pub mod store;

pub use paths::AppPaths;
pub use settings::{AppSettings, AppTheme, Column, Language, SortState};
pub use store::{Annotation, FavoriteDevice, HistoryChange, HistoryEntry, Store, StoreError};
