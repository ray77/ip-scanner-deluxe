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

/// Procedural application icon (`n`×`n`): monitor with green play triangle.
fn icon(n: usize) -> egui::IconData {
    let k = 64.0 / n as f32;
    let mut rgba = vec![0u8; n * n * 4];
    let mut put = |x: usize, y: usize, c: [u8; 4]| {
        let i = (y * n + x) * 4;
        rgba[i..i + 4].copy_from_slice(&c);
    };
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 * k, y as f32 * k);
            if (6.0..58.0).contains(&fx) && (8.0..46.0).contains(&fy) {
                let inner = (10.0..54.0).contains(&fx) && (12.0..42.0).contains(&fy);
                put(x, y, if inner { [0x2F, 0x8F, 0xE0, 255] } else { [0x45, 0x4B, 0x53, 255] });
            } else if (27.0..37.0).contains(&fx) && (46.0..52.0).contains(&fy) || (18.0..46.0).contains(&fx) && (52.0..56.0).contains(&fy) {
                put(x, y, [0x45, 0x4B, 0x53, 255]);
            }
            // play triangle
            let (px, py) = (fx - 24.0, fy - 17.0);
            if (0.0..=18.0).contains(&px) && py >= px * 0.55 && py <= 20.0 - px * 0.55 {
                put(x, y, [0x6C, 0xC2, 0x4A, 255]);
            }
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

fn main() -> eframe::Result<()> {
    let cli = screenshot::parse_args();
    if let Some(path) = &cli.write_icon {
        // Packaging helper: `IPScannerDeluxe --write-icon icon.png`
        let i = icon(256);
        if let Err(e) = screenshot::write_png(path, i.width as usize, i.height as usize, &i.rgba) {
            eprintln!("cannot write icon: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
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
    let geometry: AppSettings =
        if cli.demo { AppSettings::default() } else { Store::open(&paths.store_file()).map(|s| s.load_settings()).unwrap_or_default() };
    let size = cli.size.or(geometry.window_size).unwrap_or([653.0, 473.0]);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("IP Scanner Deluxe")
        .with_app_id("ip-scanner-deluxe")
        .with_inner_size([size[0].max(650.0), size[1].max(450.0)])
        .with_min_inner_size([650.0, 450.0])
        .with_icon(icon(64));
    if let Some(p) = geometry.window_pos {
        viewport = viewport.with_position(p);
    }
    drop(geometry);
    let options = eframe::NativeOptions { viewport, centered: true, persist_window: false, ..Default::default() };
    eframe::run_native("IP Scanner Deluxe", options, Box::new(move |cc| Ok(Box::new(app::App::new(cc, rt, paths, cli)))))
}
