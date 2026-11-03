#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! IP Scanner Deluxe – entry point.

mod app;
mod commands;
mod dialogs;
mod export;
mod licenses;
mod screenshot;
mod state;
mod tools;

use std::sync::Mutex;
use storage::{AppPaths, AppSettings, Store};

/// Microsoft Store / MSIX build (`--features store`): no portable mode, updates via the Store.
pub const STORE_BUILD: bool = cfg!(feature = "store");
/// Remote shutdown/restart can be compiled out of Store builds (`--features store-no-remote-shutdown`).
pub const REMOTE_POWER: bool = !cfg!(feature = "store-no-remote-shutdown");

const LOG_MAX_BYTES: u64 = 5 * 1024 * 1024;

fn init_logging(paths: &AppPaths) {
    let file = paths.log_file();
    if std::fs::metadata(&file).map(|m| m.len() > LOG_MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&file, paths.log_dir.join("ip-scanner-deluxe.1.log"));
    }
    let filter = tracing_subscriber::EnvFilter::try_from_env("IPSD_LOG").unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    match std::fs::OpenOptions::new().create(true).append(true).open(&file) {
        Ok(f) => {
            let _ = tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(Mutex::new(f)).try_init();
        }
        Err(_) => {
            let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
        }
    }
}

/// Application icon (monitor with IP pin), decoded once from the embedded PNG.
pub fn app_icon() -> std::sync::Arc<egui::IconData> {
    static ICON: std::sync::OnceLock<std::sync::Arc<egui::IconData>> = std::sync::OnceLock::new();
    ICON.get_or_init(|| {
        std::sync::Arc::new(
            eframe::icon_data::from_png_bytes(include_bytes!("../../../packaging/icons/ip-scanner-deluxe.png")).unwrap_or_default(),
        )
    })
    .clone()
}

/// The application icon as a texture (uploaded once per context), for the About dialog.
pub fn app_icon_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("app_icon_texture");
    if let Some(tex) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return tex;
    }
    let icon = app_icon();
    let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
    let tex = ctx.load_texture("app_icon", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

fn main() -> eframe::Result<()> {
    let cli = screenshot::parse_args();
    let paths = AppPaths::detect(!STORE_BUILD);
    if let Err(e) = paths.ensure() {
        eprintln!("cannot create data directories: {e}");
    }
    init_logging(&paths);
    tracing::info!(
        "IP Scanner Deluxe {} starting ({} {}{})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if STORE_BUILD { ", Store build" } else { "" }
    );

    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("scan-worker").max_blocking_threads(512).build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("cannot start async runtime: {e}");
            std::process::exit(1);
        }
    };

    // Read window geometry before the window exists.
    let geometry: AppSettings = if cli.demo || cli.screenshot.is_some() {
        AppSettings::default()
    } else {
        Store::open(&paths.store_file()).map(|s| s.load_settings()).unwrap_or_default()
    };
    let size = cli.size.or(geometry.window_size).unwrap_or([653.0, 473.0]);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("IP Scanner Deluxe")
        .with_app_id("ip-scanner-deluxe")
        .with_inner_size([size[0].max(650.0), size[1].max(450.0)])
        .with_min_inner_size([650.0, 450.0])
        .with_icon(app_icon());
    if let Some(p) = geometry.window_pos {
        viewport = viewport.with_position(p);
    }
    // Language for the startup error dialog; the app applies it again from the settings.
    ui::set_language(geometry.language);
    drop(geometry);
    let options = eframe::NativeOptions { viewport, centered: true, persist_window: false, ..Default::default() };
    let log_file = paths.log_file();
    let interactive = cli.screenshot.is_none();
    let result = run_ui(options, rt, paths, cli);
    if let Err(e) = &result {
        tracing::error!("cannot open the main window: {e}");
        if interactive {
            startup_error_dialog(&e.to_string(), &log_file);
        }
    }
    result
}

/// Opens the main window. OpenGL first; on Windows without a usable OpenGL
/// driver (virtual machines, Windows Server, some RDP sessions) the window
/// falls back to Direct3D 12, which also runs on the WARP software rasterizer.
fn run_ui(options: eframe::NativeOptions, rt: tokio::runtime::Runtime, paths: AppPaths, cli: screenshot::CliOptions) -> eframe::Result<()> {
    // Shared so a failed renderer (which fails before creating the app) leaves it for the next one.
    let state = std::rc::Rc::new(std::cell::RefCell::new(Some((rt, paths, cli))));
    let creator = |state: std::rc::Rc<std::cell::RefCell<Option<_>>>| -> eframe::AppCreator<'static> {
        Box::new(move |cc| {
            let (rt, paths, cli) = state.borrow_mut().take().ok_or("application already started")?;
            Ok(Box::new(app::App::new(cc, rt, paths, cli)))
        })
    };
    #[cfg(windows)]
    let fallback = eframe::NativeOptions { renderer: eframe::Renderer::Wgpu, ..options.clone() };
    let glow = eframe::NativeOptions { renderer: eframe::Renderer::Glow, ..options };
    let result = eframe::run_native("IP Scanner Deluxe", glow, creator(state.clone()));
    #[cfg(windows)]
    if let Err(e) = &result {
        if state.borrow().is_some() {
            tracing::warn!("OpenGL renderer unavailable ({e}), falling back to Direct3D 12");
            return eframe::run_native("IP Scanner Deluxe", fallback, creator(state));
        }
    }
    result
}

/// Release builds on Windows have no console, so startup errors would vanish silently.
fn startup_error_dialog(error: &str, log_file: &std::path::Path) {
    let text = ui::i18n::tf("startup.error", &[("error", error), ("log", &log_file.display().to_string())]);
    eprintln!("{text}");
    let _ = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("IP Scanner Deluxe")
        .set_description(text)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}
