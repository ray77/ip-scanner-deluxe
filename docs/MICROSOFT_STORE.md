# IP Scanner Deluxe in the Microsoft Store

IP Scanner Deluxe ships in two ways on Windows: the GitHub release (portable
ZIP and plain `.exe`) and a Microsoft Store package (MSIX). Both are built
from the same source code and the same license, **MIT OR Apache-2.0**.

```text
GitHub                                  Microsoft Store
├── source code (MIT OR Apache-2.0)     └── IP Scanner Deluxe
├── IPScannerDeluxe.exe                     ├── MSIX, signed by Microsoft
├── portable ZIP (with portable.flag)       ├── automatic Store updates
├── Linux / macOS packages                  ├── same source code
├── SHA256SUMS.txt                          └── MIT OR Apache-2.0
└── build provenance attestations
```

## Architecture decision

The Store version is the existing Win32 program, packaged as a **full-trust
classic desktop app**:

```xml
<Application Id="IPScannerDeluxe" Executable="IPScannerDeluxe.exe"
    uap10:RuntimeBehavior="packagedClassicApp"
    uap10:TrustLevel="mediumIL">
...
<rescap:Capability Name="runFullTrust" />
```

It is deliberately **not** an AppContainer/UWP app. The scanner uses the IP
Helper API (`IcmpSendEcho`, `SendARP`, `GetIpNetTable`), Winsock TCP/UDP
sockets and `InitiateSystemShutdownExW`, and it starts external clients
(`mstsc`, `ssh`, the browser). An AppContainer would block or restrict these.
Packaging as a classic app keeps every feature unchanged: IPv4/IPv6 scan,
ping, ARP/MAC, vendor lookup, reverse DNS, NetBIOS, LLMNR, mDNS, SNMP, port
scan, HTTP/HTTPS/SMB/RDP/SSH detection, Wake-on-LAN and remote shutdown.

Nothing was added that needs elevation: no Npcap/WinPcap, no drivers, no
services, no raw sockets on Windows, no autostart entries. Normal scanning
works as a standard user, in the Store version as in the portable one.

## Portable vs. Store

|                    | GitHub / portable                                  | Microsoft Store                              |
|--------------------|----------------------------------------------------|----------------------------------------------|
| Build              | `cargo build --release --locked -p ip-scanner-deluxe` | same with `--features store`             |
| Package            | `.exe`, ZIP with `portable.flag`                    | `.msix` without `portable.flag`              |
| Data location      | next to the exe if `portable.flag` exists, else `%APPDATA%\IPScannerDeluxe` | always `%APPDATA%\IPScannerDeluxe` (redirected by Windows into the package data folder) |
| Updates            | manual download from GitHub                        | Microsoft Store                              |
| Signature          | unsigned (+ GitHub attestation, SHA-256 sums)      | signed by Microsoft                          |
| Uninstall          | delete the folder                                  | Settings > Apps; removes the app data        |

The `store` feature changes only:

- `AppPaths::detect(false)`: `portable.flag` is ignored, so the program never
  writes into the read-only install folder (`crates/storage/src/paths.rs`).
- The About dialog shows "Updates are delivered automatically through the
  Microsoft Store."
- The log line at start says "Store build", and the exe's version info
  carries the marker `Comments = Microsoft Store build`, which
  `build-msix.ps1` checks so a portable build is never packaged.

There is no update checker in IP Scanner Deluxe, so there is nothing to turn
off: the Store version never downloads executables or points users to GitHub
to update itself.

### Optional: build without remote shutdown

Remote shutdown/restart stays in the Store version. Should certification
object to it, build with

```powershell
cargo build --release --locked -p ip-scanner-deluxe --features store-no-remote-shutdown
```

(which implies `store`). The shutdown and restart entries then disappear from
the context menu and the commands are ignored. Do not use it unless there is
a concrete Store reason.

## Build commands

```bash
cargo test --workspace --locked
cargo build --release --locked -p ip-scanner-deluxe                   # GitHub / portable
cargo build --release --locked -p ip-scanner-deluxe --features store  # Store
```

```powershell
.\packaging\windows\msix\scripts\build-msix.ps1 -Build   # -> dist\IPScannerDeluxe-<version>-x64-store.msix
```

Step-by-step local packaging, test certificate, install and removal:
[packaging/windows/msix/README.md](../packaging/windows/msix/README.md).

## Versioning

The only version is `version` in `[workspace.package]` of `Cargo.toml`.
`build-msix.ps1` turns `x.y.z` into the MSIX version `x.y.z.0`; the Store
requires the fourth part to be 0. Pre-release versions such as `0.2.0-beta`
cannot be packaged as MSIX. Each Store submission needs a higher version than
the previous one, so a Store update always goes together with a new release.

## Signing

- **Store:** upload the *unsigned* MSIX. After certification Microsoft signs
  the package with its own certificate. No commercial code signing
  certificate is needed and none is stored anywhere in this project.
- **Local tests:** `new-test-certificate.ps1` creates a self-signed
  certificate in the current user's certificate store, used with
  `build-msix.ps1 -CertificateThumbprint`. It is for development only and is
  never distributed.
- **CI:** the `build-msix` job creates a throwaway certificate on the runner
  to prove the package installs, then deletes it. The uploaded artifact is
  the unsigned package.
- **GitHub releases:** `SHA256SUMS.txt` and GitHub artifact attestations
  (`gh attestation verify <file> --repo ray77/ip-scanner-deluxe`) document
  where a download came from. They do not replace Authenticode.

The Microsoft signature only says that Microsoft distributed and verified the
package. It does not change the copyright (Dominik Schmidt) or the license
(MIT OR Apache-2.0).

## Licensing

- Project: **MIT OR Apache-2.0**, at the user's choice (`LICENSE-MIT`,
  `LICENSE-APACHE`). Copyright © 2026 Dominik Schmidt.
- The About dialog shows copyright, both licenses (each opens its full text),
  the source code link and a "Third-Party Licenses" button. All texts are
  embedded in the executable, so they are available in every distribution,
  including the MSIX where the install folder is not browsable. The MSIX and
  the ZIP/tar.gz/deb packages also contain the files.
- `THIRD-PARTY-NOTICES.txt` lists every Rust dependency compiled into the
  program (name, version, license, license text with copyright lines). It is
  generated by [cargo-about](https://github.com/EmbarkStudios/cargo-about)
  from `Cargo.lock` (`about.toml`, template in `packaging/licenses/`):

  ```bash
  cargo install cargo-about --locked
  cargo xtask third-party-notices
  ```

  Regenerate and commit it after every dependency change. CI fails when it is
  out of date.
- `deny.toml` + [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
  check every dependency license against an allow list (CI job `licenses`).
  A new license, a license change to something not listed, GPL/AGPL/LGPL-only
  code or a crate from an unknown source fails the build.

Dependency license audit (Cargo.lock, November 2026): all licenses are
permissive (MIT, Apache-2.0, BSD-2/3-Clause, ISC, Zlib, 0BSD, BSL-1.0,
Unlicense, Unicode-3.0). Special cases:

| Crate                  | License                            | Note |
|------------------------|------------------------------------|------|
| `option-ext`           | MPL-2.0                            | file-level copyleft, used unmodified via `directories`; source is on crates.io; notice included |
| `epaint_default_fonts` | (MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0 | fonts bundled by egui; font licenses allow embedding in software; texts included |
| `ring`                 | Apache-2.0 AND ISC                 | notices included |
| `r-efi`                | MIT OR Apache-2.0 OR LGPL-2.1-or-later | used under MIT/Apache-2.0 |

### MAC vendor database (IEEE)

- **Source:** the public IEEE Registration Authority listings MA-L
  (`oui.csv`), MA-M (`mam.csv`) and MA-S (`oui36.csv`) from
  <https://standards-oui.ieee.org/>, stored in `assets/oui/` and compiled into
  `assets/vendor.redb`, which is embedded in the program.
- **Update:** download the three CSV files into `assets/oui/` and run
  `cargo xtask build-vendor-db`. Users can import newer CSV files themselves
  (Settings > MAC database).
- **License:** the IEEE publishes the listings for public lookup without a
  license file. According to the copyright notice of Debian's `ieee-data`
  package, the IEEE does not assert copyright in the public OUI listing and
  does not restrict its distribution, and encourages users to obtain it from
  the IEEE directly. Many open source projects ship derived copies (for
  example Wireshark, arp-scan, nmap).
- **Attribution:** stated in the About dialog ("Vendor data: IEEE
  Registration Authority") and in `THIRD-PARTY-NOTICES.txt`.
- **Store:** redistribution in the Store package is consistent with this
  practice. This is our reading, not legal advice; check the current terms on
  the IEEE site before submission if in doubt.

## Store listing

Position the app as a network administration and diagnostics tool. Do not use
other products' names (for example competing scanners or their makers) in
the name, description or keywords, and do not describe it as a hacking or
security attack tool.

Ready-to-paste texts (English and German), search terms, screenshots
(1366×768) and the 300×300 logo are in
[packaging/windows/store-listing/](../packaging/windows/store-listing/README.md).
Privacy policy: [PRIVACY.md](../PRIVACY.md), URL
<https://github.com/ray77/ip-scanner-deluxe/blob/main/PRIVACY.md>.

## Partner Center steps

1. Register a developer account at <https://partner.microsoft.com/dashboard>
   (an individual account is enough; check the current registration fee).
2. Apps and games > New product > MSIX or PWA app; reserve the name
   "IP Scanner Deluxe".
3. Product management > Product identity: copy `Package/Identity/Name`,
   `Package/Identity/Publisher`, `Package/Properties/PublisherDisplayName`,
   Package Family Name and Store ID into
   `packaging/windows/msix/store-identity.json` and commit. Done: the app is
   `DomySchmidt.IpScannerDeluxe`, Store ID `9NS53FFRBJHW`
   (<https://apps.microsoft.com/detail/9NS53FFRBJHW>). CI builds with
   `-RequireStoreIdentity`, so placeholders cannot come back unnoticed.
4. Start a submission:
   - Pricing and availability: free, markets.
   - Properties: category "Utilities & tools", privacy policy URL,
     website and support contact (see the store listing file).
   - Age ratings questionnaire.
   - Packages: upload `IPScannerDeluxe-<version>-x64-store.msix` from the
     `msix-store-x64` artifact of the tagged CI run.
   - Store listings (English, German): texts, screenshots and logo from
     `packaging/windows/store-listing/`.
   - Submission options > Notes for certification: paste
     `packaging/windows/msix/STORE_CERTIFICATION_NOTES.md`.
   - Restricted capabilities: justify `runFullTrust` (same text).
5. Submit. Certification usually takes a few days.

## Release process

1. Bump `version` in `Cargo.toml`, update `Cargo.lock` (`cargo check`).
2. Run `cargo xtask third-party-notices` and commit the result (the notices
   list the workspace crates with their version, so every version bump
   changes the file; CI fails otherwise).
3. Tag `vX.Y.Z` and push. CI builds all packages, the MSIX, checksums and
   attestations, and creates the GitHub release (English release notes).
4. Download the `msix-store-x64` artifact from the tag's workflow run
   (kept 90 days) and upload it as a new Store submission.

## Known limitations

- The MSIX is built for x64 only. It runs on Windows on ARM through
  emulation; a native arm64 package needs an `aarch64-pc-windows-msvc` build
  (`build-msix.ps1 -Arch arm64`).
- Minimum Windows version for the Store package: Windows 10 version 2004
  (build 19041), required by `uap10:TrustLevel`.
- Store and GitHub builds on the same machine keep separate settings because
  Windows redirects the Store version's AppData writes.
- The Windows firewall may ask once for network access when the first scan
  starts, as with the portable version.
- The CI install test only checks that the package installs and removes
  cleanly; the GUI is tested manually.
