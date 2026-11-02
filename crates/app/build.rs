//! Records the compiler version for the diagnostics window and, on Windows,
//! embeds the program icon and version info into the .exe.
fn main() {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = std::process::Command::new(rustc)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=IPSD_RUSTC_VERSION={version}");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon = "../../packaging/windows/ip-scanner-deluxe.ico";
        println!("cargo:rerun-if-changed={icon}");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(icon)
            .set("ProductName", "IP Scanner Deluxe")
            .set("FileDescription", "IP Scanner Deluxe")
            .set("CompanyName", "Dominik Schmidt")
            .set("LegalCopyright", "Copyright (C) 2026 Dominik Schmidt. MIT OR Apache-2.0.");
        // build-msix.ps1 refuses executables without this marker (portable builds).
        if std::env::var_os("CARGO_FEATURE_STORE").is_some() {
            res.set("Comments", "Microsoft Store build");
        }
        if let Err(e) = res.compile() {
            panic!("cannot embed Windows resources: {e}");
        }
    }
}
