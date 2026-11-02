<#
.SYNOPSIS
    Creates a self-signed certificate for test-signing the MSIX locally.

.DESCRIPTION
    FOR DEVELOPMENT AND TESTS ONLY. The certificate is never distributed and
    is not a public code signing certificate. Store packages are signed by
    Microsoft after certification.

    The certificate (with its private key) stays in Cert:\CurrentUser\My; no
    private key file is written. Its subject equals the Publisher in
    store-identity.json, as MSIX requires. Only the public part is exported
    as a .cer file so Windows can be told to trust it.

.EXAMPLE
    .\packaging\windows\msix\scripts\new-test-certificate.ps1
#>
[CmdletBinding()]
param(
    [string]$IdentityFile = "packaging\windows\msix\store-identity.json",
    # Public certificate only. Default lies outside the repository.
    [string]$CerFile = (Join-Path $env:USERPROFILE "IPScannerDeluxe-msix-test.cer")
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..\..")).Path
Push-Location $root
try { $id = Get-Content $IdentityFile -Raw | ConvertFrom-Json } finally { Pop-Location }

$cert = New-SelfSignedCertificate `
    -Type Custom `
    -Subject $id.Publisher `
    -KeyUsage DigitalSignature `
    -FriendlyName "IP Scanner Deluxe MSIX test certificate (not for distribution)" `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddMonths(6) `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")

Export-Certificate -Cert $cert -FilePath $CerFile | Out-Null

Write-Host "Test certificate: $($cert.Subject)"
Write-Host "Thumbprint:       $($cert.Thumbprint)"
Write-Host "Public part:      $CerFile"
Write-Host ""
Write-Host "Trust it (elevated PowerShell, test machines only):"
Write-Host "  Import-Certificate -FilePath `"$CerFile`" -CertStoreLocation Cert:\LocalMachine\TrustedPeople"
Write-Host "Sign the package:"
Write-Host "  .\packaging\windows\msix\scripts\build-msix.ps1 -Build -CertificateThumbprint $($cert.Thumbprint)"
if ($env:GITHUB_OUTPUT) {
    "thumbprint=$($cert.Thumbprint)" | Out-File -FilePath $env:GITHUB_OUTPUT -Append -Encoding utf8
}
