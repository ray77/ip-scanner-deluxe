//! Developer tasks.
//!
//! ```text
//! cargo xtask import-oui <file.csv>...   build assets/vendor.redb from IEEE CSVs
//! cargo xtask build-vendor-db            same, using assets/oui/{oui,mam,oui36}.csv
//! cargo xtask bundle-macos [target]      release build -> dist/IP Scanner Deluxe.app (+ .zip)
//! ```

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

fn today() -> String {
    // Days since epoch -> civil date (Howard Hinnant's algorithm), avoids a chrono dependency.
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let z = secs / 86_400 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

fn import(inputs: &[PathBuf]) -> Result<()> {
    let root = workspace_root();
    let out = root.join("assets").join("vendor.redb");
    for i in inputs {
        if !i.exists() {
            bail!("input file not found: {}", i.display());
        }
    }
    let version = format!("IEEE {}", today());
    let stats = vendor_db::import_csv_files(inputs, &out, &version).with_context(|| format!("importing into {}", out.display()))?;
    println!(
        "wrote {} ({} MA-L, {} MA-M, {} MA-S, {} vendors, {} rows skipped)",
        out.display(),
        stats.ma_l,
        stats.ma_m,
        stats.ma_s,
        stats.vendors,
        stats.skipped
    );
    Ok(())
}

fn run(cmd: &mut Command) -> Result<()> {
    let status = cmd.status().with_context(|| format!("starting {cmd:?}"))?;
    if !status.success() {
        bail!("{cmd:?} failed with {status}");
    }
    Ok(())
}

fn workspace_version(root: &Path) -> Result<String> {
    let toml = std::fs::read_to_string(root.join("Cargo.toml"))?;
    toml.lines()
        .find_map(|l| l.strip_prefix("version = ").map(|v| v.trim_matches('"').to_string()))
        .context("no workspace version in Cargo.toml")
}

/// Fresh release build assembled into a signed (ad-hoc) macOS app bundle.
fn bundle_macos(target: Option<&str>) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("bundle-macos needs macOS (sips, iconutil, codesign)");
    }
    let root = workspace_root();
    let mut build = Command::new(env!("CARGO"));
    build.current_dir(&root).args(["build", "--release", "--locked", "-p", "ip-scanner-deluxe"]);
    if let Some(t) = target {
        build.args(["--target", t]);
    }
    run(&mut build)?;
    let bin = match target {
        Some(t) => root.join("target").join(t).join("release").join("IPScannerDeluxe"),
        None => root.join("target").join("release").join("IPScannerDeluxe"),
    };

    let dist = root.join("dist");
    let app = dist.join("IP Scanner Deluxe.app");
    if app.exists() {
        std::fs::remove_dir_all(&app).with_context(|| format!("removing {}", app.display()))?;
    }
    let macos = app.join("Contents").join("MacOS");
    let resources = app.join("Contents").join("Resources");
    std::fs::create_dir_all(&macos)?;
    std::fs::create_dir_all(&resources)?;
    std::fs::copy(&bin, macos.join("IPScannerDeluxe")).with_context(|| format!("copying {}", bin.display()))?;

    let plist = std::fs::read_to_string(root.join("packaging/macos/Info.plist"))?.replace("@VERSION@", &workspace_version(&root)?);
    std::fs::write(app.join("Contents").join("Info.plist"), plist)?;

    let icon = root.join("packaging/icons/ip-scanner-deluxe.png");
    let iconset = dist.join("IPScannerDeluxe.iconset");
    std::fs::create_dir_all(&iconset)?;
    for s in [16u32, 32, 128, 256] {
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            let px = (s * scale).to_string();
            let out = iconset.join(format!("icon_{s}x{s}{suffix}.png"));
            run(Command::new("sips").args(["-z", &px, &px]).arg(&icon).arg("--out").arg(&out).stdout(std::process::Stdio::null()))?;
        }
    }
    run(Command::new("iconutil").args(["-c", "icns"]).arg(&iconset).arg("-o").arg(resources.join("IPScannerDeluxe.icns")))?;
    std::fs::remove_dir_all(&iconset)?;

    // Ad-hoc signature so macOS can attribute the Local Network permission.
    run(Command::new("codesign").args(["--force", "--deep", "--sign", "-"]).arg(&app))?;
    let zip = dist.join("IPScannerDeluxe-macos.zip");
    let _ = std::fs::remove_file(&zip);
    run(Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(&app).arg(&zip))?;
    println!("wrote {} and {}", app.display(), zip.display());
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("import-oui") => {
            let files: Vec<PathBuf> = args[1..].iter().map(PathBuf::from).collect();
            if files.is_empty() {
                bail!("usage: cargo xtask import-oui <oui.csv> [mam.csv] [oui36.csv]");
            }
            import(&files)
        }
        Some("build-vendor-db") => {
            let dir = workspace_root().join("assets").join("oui");
            let files: Vec<PathBuf> = ["oui.csv", "mam.csv", "oui36.csv"].iter().map(|f| dir.join(f)).filter(|p| p.exists()).collect();
            if files.is_empty() {
                bail!("no IEEE CSV files in {}", dir.display());
            }
            import(&files)
        }
        Some("bundle-macos") => bundle_macos(args.get(1).map(String::as_str)),
        _ => {
            eprintln!("tasks:\n  import-oui <csv>...\n  build-vendor-db\n  bundle-macos [target]");
            Ok(())
        }
    }
}
