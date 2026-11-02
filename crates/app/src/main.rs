#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! IP Scanner Deluxe – entry point.

mod app;
mod commands;
mod dialogs;
mod export;
mod screenshot;
mod state;
mod tools;

use std::sync::Mutex;
use storage::{AppPaths, AppSettings, Store};

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

fn main() -> eframe::Result<()> {
    let cli = screenshot::parse_args();
    let paths = AppPaths::detect();
    if let Err(e) = paths.ensure() {
        eprintln!("cannot create data directories: {e}");
    }
    init_logging(&paths);
    tracing::info!("IP Scanner Deluxe {} starting ({} {})", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH);

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
    drop(geometry);
    let options = eframe::NativeOptions { viewport, centered: true, persist_window: false, ..Default::default() };
    eframe::run_native("IP Scanner Deluxe", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, rt, paths, cli)))))
}
