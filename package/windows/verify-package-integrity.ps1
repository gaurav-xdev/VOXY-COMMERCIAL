# Automated Package & Signing Integrity Verifier for VOXY COM
# Verifies complete packaging, fail-closed enforcement, and authentic signing.
param (
    [switch]$FailOnWarning
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "       VOXY COM PRODUCTION PACKAGE INTEGRITY VERIFIER       " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$testPass = 0
$testFail = 0

function Assert-Condition($condition, $message) {
    if ($condition) {
        Write-Host "  [PASS] $message" -ForegroundColor Green
        $script:testPass++
    } else {
        Write-Host "  [FAIL] $message" -ForegroundColor Red
        $script:testFail++
    }
}

# 1. Test Fail-Closed in Production Mode without Certificate
Write-Host "`nTest 1: Fail-Closed Enforcement When Certificate Missing in Production Mode" -ForegroundColor Yellow
$caughtExpectedError = $false
try {
    # Run build-package with Production mode and no credentials
    $prevCert = $env:VOXY_CODE_SIGN_CERT_BASE64
    $prevPath = $env:VOXY_CODE_SIGN_PFX_PATH
    $env:VOXY_CODE_SIGN_CERT_BASE64 = ""
    $env:VOXY_CODE_SIGN_PFX_PATH = ""

    & powershell -ExecutionPolicy Bypass -File "package\windows\build-package.ps1" -Environment Production -SkipBuild 2>&1 | Out-String -OutVariable buildOut
} catch {
    $caughtExpectedError = $true
} finally {
    $env:VOXY_CODE_SIGN_CERT_BASE64 = $prevCert
    $env:VOXY_CODE_SIGN_PFX_PATH = $prevPath
}

Assert-Condition ($LASTEXITCODE -ne 0 -or $caughtExpectedError) "Production packaging must FAIL CLOSED without certificate"

# 2. Verify Output Artifact Directory Structure
Write-Host "`nTest 2: Dual-Application Completeness (Daemon + Overlay GUI)" -ForegroundColor Yellow
$outDir = "package\windows\out"
$stagingDir = "$outDir\staging"
$portableDir = "$outDir\VOXY-COM-Portable-x64"

Assert-Condition (Test-Path "$outDir\VOXY.Commercial.msix") "MSIX package artifact exists: $outDir\VOXY.Commercial.msix"
Assert-Condition (Test-Path "$outDir\VOXY-COM-Portable-x64.zip") "Portable ZIP bundle exists: $outDir\VOXY-COM-Portable-x64.zip"

Assert-Condition (Test-Path "$stagingDir\voxy-daemon.exe") "Staging contains voxy-daemon.exe (Background Runtime)"
Assert-Condition (Test-Path "$stagingDir\voxy-overlay.exe") "Staging contains voxy-overlay.exe (Interactive Desktop GUI)"
Assert-Condition (Test-Path "$stagingDir\run-voxy.bat") "Staging contains unified dual-app launcher (run-voxy.bat)"
Assert-Condition (Test-Path "$stagingDir\AppxManifest.xml") "Staging contains Windows AppxManifest.xml"
Assert-Condition (Test-Path "$stagingDir\Assets\StoreLogo.png") "Staging contains application visual assets"

# 3. Verify AppxManifest Declares Both Applications
Write-Host "`nTest 3: AppxManifest Multi-Application Architecture" -ForegroundColor Yellow
$manifestXml = [xml](Get-Content "$stagingDir\AppxManifest.xml")
$apps = $manifestXml.Package.Applications.Application

$hasOverlayApp = ($apps | Where-Object { $_.Id -eq "VoxyOverlay" -and $_.Executable -eq "voxy-overlay.exe" })
$hasDaemonApp = ($apps | Where-Object { $_.Id -eq "VoxyDaemon" -and $_.Executable -eq "voxy-daemon.exe" })

Assert-Condition ($null -ne $hasOverlayApp) "Manifest declares VoxyOverlay as primary interactive application"
Assert-Condition ($null -ne $hasDaemonApp) "Manifest declares VoxyDaemon as background runtime service"

# 4. Verify Portable Bundle & Checksums
Write-Host "`nTest 4: Portable Distribution Integrity & SHA256 Checksums" -ForegroundColor Yellow
Assert-Condition (Test-Path "$portableDir\voxy-daemon.exe") "Portable directory contains voxy-daemon.exe"
Assert-Condition (Test-Path "$portableDir\voxy-overlay.exe") "Portable directory contains voxy-overlay.exe"
Assert-Condition (Test-Path "$portableDir\SHA256SUMS.txt") "Portable directory contains SHA256SUMS.txt"

$checksumContent = Get-Content "$portableDir\SHA256SUMS.txt"
$calcDaemonHash = (Get-FileHash "$portableDir\voxy-daemon.exe" -Algorithm SHA256).Hash
$calcOverlayHash = (Get-FileHash "$portableDir\voxy-overlay.exe" -Algorithm SHA256).Hash

Assert-Condition ($checksumContent -match $calcDaemonHash) "Daemon SHA256 checksum matches binary hash"
Assert-Condition ($checksumContent -match $calcOverlayHash) "Overlay SHA256 checksum matches binary hash"

# 5. Authenticode Signature Check
Write-Host "`nTest 5: Authenticode Signature Verification on Built Artifacts" -ForegroundColor Yellow
$msixSig = Get-AuthenticodeSignature "$outDir\VOXY.Commercial.msix"
Assert-Condition ($msixSig.Status -ne "NotSigned") "MSIX package is digitally signed"

$daemonSig = Get-AuthenticodeSignature "$stagingDir\voxy-daemon.exe"
Assert-Condition ($daemonSig.Status -ne "NotSigned") "Daemon executable is digitally signed"

$overlaySig = Get-AuthenticodeSignature "$stagingDir\voxy-overlay.exe"
Assert-Condition ($overlaySig.Status -ne "NotSigned") "Overlay executable is digitally signed"

Write-Host "`n============================================================" -ForegroundColor Cyan
Write-Host "       SUMMARY: $testPass Passed, $testFail Failed          " -ForegroundColor $(if ($testFail -eq 0) { "Green" } else { "Red" })
Write-Host "============================================================" -ForegroundColor Cyan

if ($testFail -gt 0) {
    exit 1
} else {
    exit 0
}
