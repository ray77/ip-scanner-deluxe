//! Command line scan for manual testing: `cargo run -p scan-core --example scan -- 192.168.0.1-254`
use fingerprint::FingerprintEngine;
use scan_core::{parse_targets, ScanConfig, ScanControl, ScanEvent, Scanner, SystemProbe};
use std::sync::Arc;
use vendor_db::VendorDb;

#[tokio::main]
async fn main() {
    let range = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1".into());
    let spec = parse_targets(&range).expect("valid range");
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/vendor.redb")).unwrap_or_default();
    let vendors = Arc::new(VendorDb::from_bytes(&bytes).unwrap_or_else(|_| VendorDb::empty().expect("db")));
    let scanner = Scanner::new(Arc::new(SystemProbe::new()), vendors, Arc::new(FingerprintEngine::with_builtin()));
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let start = std::time::Instant::now();
    let task = tokio::spawn(async move { scanner.run(spec, ScanConfig::default(), tx, Arc::new(|| {}), ScanControl::new()).await });
    while let Some(ev) = rx.recv().await {
        match ev {
            ScanEvent::HostUpdated { host } => println!(
                "{:<16} {:?} name={:?} mac={:?} vendor={:?} class={:?} conf={} services={:?}",
                host.ip,
                host.status,
                host.hostname,
                host.mac.map(|m| m.to_string()),
                host.vendor,
                host.device_class,
                host.fingerprint_confidence,
                host.services.iter().map(|s| s.display_name.clone()).collect::<Vec<_>>()
            ),
            ScanEvent::Warning(w) => println!("warning: {w}"),
            ScanEvent::ScanFinished => println!("finished in {:.1}s", start.elapsed().as_secs_f32()),
            _ => {}
        }
    }
    let _ = task.await;
}
