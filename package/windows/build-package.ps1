# VOXY Windows MSIX / Sparse Packaging Script
param (
    [switch]$Release
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "           VOXY WINDOWS PACKAGE GENERATOR                   " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$buildConfig = if ($Release) { "release" } else { "debug" }
Write-Host "[1] Building voxy-daemon ($buildConfig)..." -ForegroundColor Yellow

if ($Release) {
    cargo build --bin voxy-daemon --release
} else {
    cargo build --bin voxy-daemon
}

$binDir = "target\$buildConfig"
$exePath = "$binDir\voxy-daemon.exe"
if (-not (Test-Path $exePath)) {
    throw "Build output not found at: $exePath"
}
Write-Host "  -> Binary verified: $exePath" -ForegroundColor Green

$sdkDir = "C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64"
$makeappx = "$sdkDir\makeappx.exe"
$signtool = "$sdkDir\signtool.exe"

if (-not (Test-Path $makeappx)) {
    throw "makeappx.exe not found at: $makeappx"
}

$outDir = "package\windows\out"
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
$stagingDir = "$outDir\staging"
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
New-Item -ItemType Directory -Path "$stagingDir\Assets" -Force | Out-Null

Write-Host "[2] Staging packaging artifacts..." -ForegroundColor Yellow
Copy-Item "package\windows\AppxManifest.xml" "$stagingDir\AppxManifest.xml" -Force
Copy-Item "package\windows\Assets\*" "$stagingDir\Assets\" -Force
Copy-Item $exePath "$stagingDir\voxy-daemon.exe" -Force

$msixPath = "$outDir\VOXY.VoiceDaemon.msix"
Write-Host "[3] Creating MSIX package: $msixPath..." -ForegroundColor Yellow
& $makeappx pack /d $stagingDir /p $msixPath /o /nv

Write-Host "[4] Checking Code Signing Certificate..." -ForegroundColor Yellow
$cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -like "*VOXY-AI-LocalDev*" }
if (-not $cert) {
    Write-Host "  -> Generating local developer certificate..." -ForegroundColor Gray
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=VOXY-AI-LocalDev" -CertStoreLocation Cert:\CurrentUser\My
}

Write-Host "  -> Signing package with cert thumbprint: $($cert.Thumbprint)..." -ForegroundColor Gray
& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $msixPath

Write-Host "`n[5] Registering package in Developer Mode..." -ForegroundColor Yellow
$manifestResolved = (Resolve-Path "$stagingDir\AppxManifest.xml").Path
$externalResolved = (Resolve-Path $stagingDir).Path
Add-AppxPackage -Register $manifestResolved -ExternalLocation $externalResolved
Write-Host "  -> Successfully registered VOXY package with ExternalLocation!" -ForegroundColor Green

Write-Host "`n============================================================" -ForegroundColor Green
Write-Host "           VOXY PACKAGE BUILT & REGISTERED                  " -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Green
Write-Host "Package Path: $msixPath"
Write-Host "Registered Identity: VOXY.VoiceDaemon"
Write-Host "Capabilities: runFullTrust, microphone"
Write-Host "`nVerification Command:"
Write-Host "  Get-AppxPackage | Where-Object { `$_.Name -like '*VOXY*' }"
