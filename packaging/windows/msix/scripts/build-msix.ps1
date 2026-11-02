<#
.SYNOPSIS
    Builds the Microsoft Store MSIX package of IP Scanner Deluxe.

.DESCRIPTION
    Stages the Store build of IPScannerDeluxe.exe (cargo feature "store") with
    the logo assets, license texts and the manifest rendered from
    AppxManifest.template.xml, then packs it with makeappx.exe from the
    Windows SDK.

    The MSIX version is derived from the workspace version in Cargo.toml
    (x.y.z -> x.y.z.0). Package identity values come from store-identity.json.

    The package is unsigned unless -CertificateThumbprint is given. Unsigned
    is what Partner Center expects: the Store signs the package after
    certification. Test signing is only for installing the package locally.

.EXAMPLE
    .\packaging\windows\msix\scripts\build-msix.ps1 -Build

.EXAMPLE
    .\packaging\windows\msix\scripts\build-msix.ps1 -Build -CertificateThumbprint 0123ABCD...
#>
[CmdletBinding()]
param(
    # Store build of the executable. -Build creates it first.
    [string]$Exe = "target\release\IPScannerDeluxe.exe",
    [string]$OutDir = "dist",
    [ValidateSet("x64", "arm64")]
    [string]$Arch = "x64",
    [string]$IdentityFile = "packaging\windows\msix\store-identity.json",
    # Run `cargo build --release --locked -p ip-scanner-deluxe --features store` first.
    [switch]$Build,
    # Fail instead of warning when store-identity.json still has placeholders.
    [switch]$RequireStoreIdentity,
    # Optional: sign with this certificate from Cert:\CurrentUser\My (local tests only).
    [string]$CertificateThumbprint
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
$msixDir = Join-Path $root "packaging\windows\msix"

function Find-SdkTool([string]$Name) {
    $cmd = Get-Command $Name -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    $hostArch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { @("arm64", "x64") } else { @("x64") }
    $tool = Get-ChildItem $kits -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^10\.\d+\.\d+\.\d+$' } |
        Sort-Object { [version]$_.Name } -Descending |
        ForEach-Object { $dir = $_.FullName; $hostArch | ForEach-Object { Join-Path $dir "$_\$Name" } } |
        Where-Object { Test-Path $_ } |
        Select-Object -First 1
    if (-not $tool) { throw "$Name not found. Install the Windows SDK (MSIX packaging tools)." }
    return $tool
}

function Invoke-Tool([string]$Tool, [string[]]$Arguments) {
    & $Tool @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$(Split-Path $Tool -Leaf) failed with exit code $LASTEXITCODE" }
}

Push-Location $root
try {
    # Version: [workspace.package] version in Cargo.toml -> Major.Minor.Patch.0
    $line = Select-String -Path "Cargo.toml" -Pattern '^version = "(\d+)\.(\d+)\.(\d+)"' | Select-Object -First 1
    if (-not $line) { throw "No plain x.y.z workspace version in Cargo.toml (MSIX needs four numeric parts)." }
    $g = $line.Matches[0].Groups
    $semver = "$($g[1].Value).$($g[2].Value).$($g[3].Value)"
    $version = "$semver.0"

    # Identity
    $id = Get-Content $IdentityFile -Raw | ConvertFrom-Json
    foreach ($key in "IdentityName", "Publisher", "PublisherDisplayName") {
        if (-not $id.$key) { throw "$IdentityFile is missing $key" }
    }
    if ("$($id.IdentityName) $($id.Publisher) $($id.PublisherDisplayName)" -match "PLACEHOLDER") {
        $msg = "$IdentityFile still contains PLACEHOLDER values; this package can be tested locally but not submitted to the Store."
        if ($RequireStoreIdentity) { throw $msg }
        Write-Warning $msg
    }

    if ($Build) {
        Invoke-Tool "cargo" @("build", "--release", "--locked", "-p", "ip-scanner-deluxe", "--features", "store")
    }
    if (-not (Test-Path $Exe)) { throw "$Exe not found. Build it with --features store or pass -Build." }
    # build.rs writes this marker only for --features store; a portable build must never be packaged.
    $comments = (Get-Item $Exe).VersionInfo.Comments
    if ($comments -ne "Microsoft Store build") {
        throw "$Exe is not a Store build (version info Comments = '$comments'). Build with --features store."
    }

    # Stage
    $stage = Join-Path $OutDir "msix\IPScannerDeluxe"
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force (Join-Path $stage "Assets") | Out-Null
    Copy-Item $Exe (Join-Path $stage "IPScannerDeluxe.exe")
    Copy-Item (Join-Path $msixDir "Assets\*.png") (Join-Path $stage "Assets")
    Copy-Item "LICENSE-MIT", "LICENSE-APACHE", "THIRD-PARTY-NOTICES.txt" $stage

    $esc = { param($v) [System.Security.SecurityElement]::Escape([string]$v) }
    $manifest = (Get-Content (Join-Path $msixDir "AppxManifest.template.xml") -Raw).
        Replace("@IDENTITY_NAME@", (& $esc $id.IdentityName)).
        Replace("@PUBLISHER@", (& $esc $id.Publisher)).
        Replace("@PUBLISHER_DISPLAY_NAME@", (& $esc $id.PublisherDisplayName)).
        Replace("@VERSION@", $version).
        Replace("@ARCH@", $Arch)
    if ($manifest -match "@[A-Z_]+@") { throw "Unreplaced token $($Matches[0]) in the manifest template." }
    $manifestPath = Join-Path $stage "AppxManifest.xml"
    $manifestFull = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($manifestPath)
    [System.IO.File]::WriteAllText($manifestFull, $manifest, (New-Object System.Text.UTF8Encoding $false))
    [xml]$manifest | Out-Null  # well-formed?

    if (Get-ChildItem $stage -Recurse -Filter "portable.flag") { throw "portable.flag must not be part of the Store package." }

    # Resource index, so the targetsize/altform-unplated icon variants are used.
    $makepri = Find-SdkTool "makepri.exe"
    $priConfig = Join-Path $OutDir "msix\priconfig.xml"
    Invoke-Tool $makepri @("createconfig", "/cf", $priConfig, "/dq", "en-US", "/o")
    Invoke-Tool $makepri @("new", "/pr", $stage, "/cf", $priConfig, "/mn", $manifestPath, "/of", (Join-Path $stage "resources.pri"), "/o")

    # Pack
    $makeappx = Find-SdkTool "makeappx.exe"
    $msix = Join-Path $OutDir "IPScannerDeluxe-$semver-$Arch-store.msix"
    Invoke-Tool $makeappx @("pack", "/o", "/h", "SHA256", "/d", $stage, "/p", $msix)

    if ($CertificateThumbprint) {
        $signtool = Find-SdkTool "signtool.exe"
        Invoke-Tool $signtool @("sign", "/fd", "SHA256", "/sha1", $CertificateThumbprint, "/s", "My", $msix)
        Write-Host "Test-signed $msix (local testing only, not for distribution)."
    }

    Write-Host "MSIX $version ($Arch): $msix"
    if ($env:GITHUB_OUTPUT) {
        "msix=$msix" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
        "version=$version" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
    }
}
finally {
    Pop-Location
}
