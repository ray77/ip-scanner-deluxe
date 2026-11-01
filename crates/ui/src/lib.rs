//! Presentation layer. Widgets render state and return [`UiCommand`]s; the
//! application crate executes them. No networking happens here.

pub mod commands;
pub mod i18n;
pub mod icons;
pub mod menus;
pub mod table;
pub mod theme;
pub mod toolbar;
pub mod widgets;

pub use commands::UiCommand;
pub use i18n::{set_language, t};
pub use theme::{apply_theme, install_fonts, ThemePalette};
