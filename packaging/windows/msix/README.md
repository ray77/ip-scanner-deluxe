# MSIX package (Microsoft Store)

This folder builds the Microsoft Store package of IP Scanner Deluxe: the
normal Win32 executable, packaged as a full-trust classic desktop app
(`packagedClassicApp`, `mediumIL`, `runFullTrust`). Background, release
process and Partner Center steps: [docs/MICROSOFT_STORE.md](../../../docs/MICROSOFT_STORE.md).

```text
packaging/windows/msix/
├── AppxManifest.template.xml     manifest with @TOKENS@
├── store-identity.json           package identity (Partner Center values)
├── Assets/                       logos, generated from packaging/icons/ip-scanner-deluxe.png
├── STORE_CERTIFICATION_NOTES.md  text for the certification notes field
└── scripts/
    ├── build-msix.ps1            builds dist/IPScannerDeluxe-<version>-x64-store.msix
    └── new-test-certificate.ps1  self-signed certificate for local tests
```

> **The local test certificate is only for development and testing. It is
> never distributed and is not a public code signing certificate.** Store
> packages are signed by Microsoft after certification.

## 1. Requirements

- Windows 10 version 2004 (build 19041) or later, or Windows 11
- Rust (stable) with the MSVC toolchain, as for the normal Windows build
- PowerShell 7 (`pwsh`) or Windows PowerShell 5.1

## 2. Windows SDK / MSIX tools

The script needs `makeappx.exe`, `makepri.exe` and (for test signing)
`signtool.exe` from the Windows SDK. Install the "Windows 11 SDK" with the
Visual Studio Installer or from
<https://developer.microsoft.com/windows/downloads/windows-sdk/>. The script
finds the tools in `C:\Program Files (x86)\Windows Kits\10\bin\<version>\x64`
or on `PATH`.

## 3. Release build

The Store build uses the cargo feature `store`. It ignores `portable.flag`,
always stores data in the user profile and shows that updates come from the
Store.

```powershell
cargo build --release --locked -p ip-scanner-deluxe --features store
```

## 4. Build the MSIX

From the repository root:

```powershell
.\packaging\windows\msix\scripts\build-msix.ps1            # uses target\release\IPScannerDeluxe.exe
.\packaging\windows\msix\scripts\build-msix.ps1 -Build     # runs the cargo build first
```

The result is `dist\IPScannerDeluxe-<version>-x64-store.msix` (unsigned).
The script

- takes the version from `Cargo.toml` (`0.1.2` becomes `0.1.2.0`),
- fills the manifest from `store-identity.json`,
- refuses an executable built without `--features store`,
- fails if a `portable.flag` ended up in the package,
- adds `LICENSE-MIT`, `LICENSE-APACHE` and `THIRD-PARTY-NOTICES.txt`.

While `store-identity.json` still has placeholders the script warns;
`-RequireStoreIdentity` turns the warning into an error.

## 5. Create a local test certificate

```powershell
.\packaging\windows\msix\scripts\new-test-certificate.ps1
```

This creates a self-signed certificate whose subject equals `Publisher` in
`store-identity.json` (MSIX requires that). The private key stays in your
certificate store (`Cert:\CurrentUser\My`); no `.pfx` file is written. Only
the public part is exported to `%USERPROFILE%\IPScannerDeluxe-msix-test.cer`.
The script prints the thumbprint.

Create a new certificate whenever `Publisher` changes.

## 6. Trust the test certificate

In an elevated PowerShell, on test machines only:

```powershell
Import-Certificate -FilePath "$env:USERPROFILE\IPScannerDeluxe-msix-test.cer" -CertStoreLocation Cert:\LocalMachine\TrustedPeople
```

## 7. Sign the test MSIX

```powershell
.\packaging\windows\msix\scripts\build-msix.ps1 -Build -CertificateThumbprint <thumbprint>
```

Never upload a test-signed package to Partner Center or publish it anywhere.

## 8. Install

Double-click the `.msix`, or:

```powershell
Add-AppxPackage .\dist\IPScannerDeluxe-0.1.2-x64-store.msix
```

The app appears in the Start menu as "IP Scanner Deluxe". It uses
`%APPDATA%\IPScannerDeluxe` like the installed GitHub build; Windows
redirects these writes into the package's private data folder
(`%LOCALAPPDATA%\Packages\<PackageFamilyName>\LocalCache`), so the Store
version and a GitHub build on the same machine keep separate settings.

## 9. Remove

```powershell
Get-AppxPackage *IPScannerDeluxe* | Remove-AppxPackage
```

This also removes the package's settings and logs. To remove the test
certificate:

```powershell
Get-ChildItem Cert:\CurrentUser\My | Where-Object FriendlyName -like "IP Scanner Deluxe MSIX test*" | Remove-Item
Get-ChildItem Cert:\LocalMachine\TrustedPeople | Where-Object Subject -eq "<Publisher>" | Remove-Item   # elevated
```

## 10. Insert the Partner Center values

After reserving the app name in Partner Center, open
**Apps and games > IP Scanner Deluxe > Product management > Product identity**
and copy the values into `store-identity.json`:

| store-identity.json    | Partner Center field                      | Example form              |
|------------------------|-------------------------------------------|---------------------------|
| `IdentityName`         | Package/Identity/Name                     | `12345Name.IPScannerDeluxe` |
| `Publisher`            | Package/Identity/Publisher                | `CN=XXXXXXXX-XXXX-...`    |
| `PublisherDisplayName` | Package/Properties/PublisherDisplayName   | your publisher name       |
| `PackageFamilyName`    | Package Family Name (PFN)                 | documentation only        |
| `StoreProductId`       | Store ID                                  | documentation only        |

These values are public. Commit them; they are the only place the identity is
defined. Then build with `-RequireStoreIdentity` to make sure no placeholder
is left.
