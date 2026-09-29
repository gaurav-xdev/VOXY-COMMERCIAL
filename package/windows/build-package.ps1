# VOXY COM Windows MSIX & Distribution Package Generator
param (
    [switch]$Release,
    [switch]$SkipBuild,
    [switch]$Register
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "         VOXY COM WINDOWS PRODUCTION PACKAGE GENERATOR      " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$buildConfig = if ($Release) { "release" } else { "debug" }
$binDir = "target\$buildConfig"
$daemonExe = "$binDir\voxy-daemon.exe"
$overlayExe = "$binDir\voxy-overlay.exe"

# 1. Build Binaries
if (-not $SkipBuild) {
    Write-Host "[1/6] Building voxy-daemon and voxy-overlay ($buildConfig)..." -ForegroundColor Yellow
    if ($Release) {
        cargo build --bin voxy-daemon --bin voxy-overlay --release
    } else {
        cargo build --bin voxy-daemon --bin voxy-overlay
    }
} else {
    Write-Host "[1/6] Skipping build step (using existing binaries in $binDir)..." -ForegroundColor Gray
}

if (-not (Test-Path $daemonExe)) {
    throw "Daemon binary not found at: $daemonExe"
}
if (-not (Test-Path $overlayExe)) {
    throw "Overlay binary not found at: $overlayExe"
}
Write-Host "  -> Daemon binary verified: $daemonExe" -ForegroundColor Green
Write-Host "  -> Overlay binary verified: $overlayExe" -ForegroundColor Green

# 2. Locate Windows SDK Tools
Write-Host "[2/6] Locating Windows SDK Packaging Tools..." -ForegroundColor Yellow
$sdkTools = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe" -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1

$makeappx = if ($sdkTools) { 
    $sdkTools.FullName 
} else { 
    $m = Get-Command makeappx.exe -ErrorAction SilentlyContinue
    if ($m) { $m.Source } else { $null }
}
if (-not $makeappx -or -not (Test-Path $makeappx)) {
    throw "makeappx.exe not found in Windows Kits or PATH"
}
$sdkDir = Split-Path $makeappx
$signtool = Join-Path $sdkDir "signtool.exe"
if (-not (Test-Path $signtool)) {
    $s = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($s) { $signtool = $s.Source }
    if (-not $signtool -or -not (Test-Path $signtool)) {
        throw "signtool.exe not found in $sdkDir or PATH"
    }
}
Write-Host "  -> makeappx: $makeappx" -ForegroundColor Green
Write-Host "  -> signtool: $signtool" -ForegroundColor Green

# 3. Stage Packaging Artifacts
$outDir = "package\windows\out"
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
$stagingDir = "$outDir\staging"
if (Test-Path $stagingDir) {
    Remove-Item -Path $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
New-Item -ItemType Directory -Path "$stagingDir\Assets" -Force | Out-Null

Write-Host "[3/6] Staging package artifacts in $stagingDir..." -ForegroundColor Yellow
Copy-Item "package\windows\AppxManifest.xml" "$stagingDir\AppxManifest.xml" -Force
Copy-Item "package\windows\Assets\*" "$stagingDir\Assets\" -Force
Copy-Item $daemonExe "$stagingDir\voxy-daemon.exe" -Force
Copy-Item $overlayExe "$stagingDir\voxy-overlay.exe" -Force

# Create convenient launcher script
$launcherContent = @"
@echo off
echo Starting VOXY COM AI Operating Companion...
start "" "%~dp0voxy-daemon.exe"
start "" "%~dp0voxy-overlay.exe"
"@
Set-Content -Path "$stagingDir\run-voxy.bat" -Value $launcherContent -Encoding ASCII

# 4. Create MSIX Package
$msixPath = "$outDir\VOXY.Commercial.msix"
Write-Host "[4/6] Creating MSIX package: $msixPath..." -ForegroundColor Yellow
& $makeappx pack /d $stagingDir /p $msixPath /o /nv

# 5. Check or Create Code Signing Certificate & Sign Package
Write-Host "[5/6] Checking Code Signing Certificate..." -ForegroundColor Yellow
$cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -like "*VOXY-AI-LocalDev*" } | Select-Object -First 1
if (-not $cert) {
    Write-Host "  -> Generating local developer signing certificate (CN=VOXY-AI-LocalDev)..." -ForegroundColor Gray
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=VOXY-AI-LocalDev" -CertStoreLocation Cert:\CurrentUser\My
}
Write-Host "  -> Signing MSIX with certificate thumbprint: $($cert.Thumbprint)..." -ForegroundColor Gray
& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $msixPath

# 6. Create Portable Standalone Distribution ZIP
Write-Host "[6/6] Generating Portable Distribution ZIP..." -ForegroundColor Yellow
$portableDir = "$outDir\VOXY-COM-Portable-x64"
if (Test-Path $portableDir) {
    Remove-Item -Path $portableDir -Recurse -Force
}
New-Item -ItemType Directory -Path $portableDir -Force | Out-Null
New-Item -ItemType Directory -Path "$portableDir\Assets" -Force | Out-Null
Copy-Item "$stagingDir\voxy-daemon.exe" "$portableDir\" -Force
Copy-Item "$stagingDir\voxy-overlay.exe" "$portableDir\" -Force
Copy-Item "$stagingDir\run-voxy.bat" "$portableDir\" -Force
Copy-Item "$stagingDir\Assets\*" "$portableDir\Assets\" -Force

$readmeText = @"
VOXY COM - Commercial AI Operating Companion
=============================================
Version: 1.0.0 (Production Release)
Architecture: Windows x64

Quick Start:
  1. Double click 'run-voxy.bat' to launch both VOXY Daemon & Overlay.
  2. Or run 'voxy-daemon.exe' in a background terminal, then 'voxy-overlay.exe'.

Security & Privacy:
  - All local audio buffers are stored in volatile memory and zeroized upon release.
  - Computer control operations requiring elevated privileges prompt for user confirmation.
  - Emergency Stop can be triggered at any time via the Overlay or Ctrl+C in Daemon console.
"@
Set-Content -Path "$portableDir\README.txt" -Value $readmeText -Encoding ASCII

# Generate SHA256 Checksums
$daemonHash = (Get-FileHash "$portableDir\voxy-daemon.exe" -Algorithm SHA256).Hash
$overlayHash = (Get-FileHash "$portableDir\voxy-overlay.exe" -Algorithm SHA256).Hash
$checksums = @"
$daemonHash  voxy-daemon.exe
$overlayHash  voxy-overlay.exe
"@
Set-Content -Path "$portableDir\SHA256SUMS.txt" -Value $checksums -Encoding ASCII

$portableZip = "$outDir\VOXY-COM-Portable-x64.zip"
if (Test-Path $portableZip) {
    Remove-Item -Path $portableZip -Force
}
Compress-Archive -Path "$portableDir\*" -DestinationPath $portableZip -Force

# Optional Registration
if ($Register) {
    Write-Host "`nRegistering MSIX package in Developer Mode..." -ForegroundColor Yellow
    $manifestResolved = (Resolve-Path "$stagingDir\AppxManifest.xml").Path
    $externalResolved = (Resolve-Path $stagingDir).Path
    Add-AppxPackage -Register $manifestResolved -ExternalLocation $externalResolved
    Write-Host "  -> Successfully registered VOXY COM package!" -ForegroundColor Green
}

Write-Host "`n============================================================" -ForegroundColor Green
Write-Host "     VOXY COM PACKAGING COMPLETED SUCCESSFULLY              " -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Green
Write-Host "MSIX Package:     $msixPath"
Write-Host "Portable ZIP:     $portableZip"
Write-Host "Package Identity: VOXY.Commercial"
Write-Host "Applications:     voxy-overlay.exe (GUI), voxy-daemon.exe (Runtime)"
Write-Host "Daemon SHA256:    $daemonHash"
Write-Host "Overlay SHA256:   $overlayHash"
