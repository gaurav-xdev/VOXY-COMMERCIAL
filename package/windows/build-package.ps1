# VOXY COM Windows MSIX & Distribution Package Generator
# Complies with Production Hardening Standards (Fail-Closed Signing Architecture)
param (
    [ValidateSet("Development", "Production")]
    [string]$Environment = "Development",
    [switch]$Release,
    [switch]$SkipBuild,
    [switch]$Register,
    [string]$CertificatePath,
    [string]$CertificateBase64,
    [string]$CertificatePassword,
    [string]$PublisherSubject,
    [string]$TimestampServer = "http://timestamp.digicert.com"
)

$ErrorActionPreference = "Stop"

# Auto-promote to Production environment if Release switch or CI environment indicates
if ($Release -and ($Environment -eq "Development") -and ($env:VOXY_RELEASE_CHANNEL -eq "production" -or $env:GITHUB_REF -like "refs/tags/v*")) {
    $Environment = "Production"
}

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "         VOXY COM WINDOWS PACKAGE GENERATOR ($Environment)  " -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

$buildConfig = if ($Release -or $Environment -eq "Production") { "release" } else { "debug" }
$binDir = "target\$buildConfig"
$daemonExe = "$binDir\voxy-daemon.exe"
$overlayExe = "$binDir\voxy-overlay.exe"

# 1. Build Binaries
if (-not $SkipBuild) {
    Write-Host "[1/7] Building voxy-daemon and voxy-overlay ($buildConfig)..." -ForegroundColor Yellow
    if ($buildConfig -eq "release") {
        cargo build --bin voxy-daemon --bin voxy-overlay --release
    } else {
        cargo build --bin voxy-daemon --bin voxy-overlay
    }
} else {
    Write-Host "[1/7] Skipping build step (using existing binaries in $binDir)..." -ForegroundColor Gray
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
Write-Host "[2/7] Locating Windows SDK Packaging Tools..." -ForegroundColor Yellow
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

# 3. Resolve & Validate Code Signing Certificate
Write-Host "[3/7] Resolving and Validating Code Signing Credentials..." -ForegroundColor Yellow

# Fallback to standard environment variables if parameters not supplied directly
if (-not $CertificatePath -and $env:VOXY_CODE_SIGN_PFX_PATH) {
    $CertificatePath = $env:VOXY_CODE_SIGN_PFX_PATH
}
if (-not $CertificateBase64) {
    if ($env:VOXY_CODE_SIGN_CERT_BASE64) {
        $CertificateBase64 = $env:VOXY_CODE_SIGN_CERT_BASE64
    } elseif ($env:WINDOWS_CERTIFICATE) {
        $CertificateBase64 = $env:WINDOWS_CERTIFICATE
    }
}
if (-not $CertificatePassword) {
    if ($env:VOXY_CODE_SIGN_PASSWORD) {
        $CertificatePassword = $env:VOXY_CODE_SIGN_PASSWORD
    } elseif ($env:WINDOWS_CERTIFICATE_PASSWORD) {
        $CertificatePassword = $env:WINDOWS_CERTIFICATE_PASSWORD
    }
}
if (-not $PublisherSubject -and $env:VOXY_CODE_SIGN_PUBLISHER) {
    $PublisherSubject = $env:VOXY_CODE_SIGN_PUBLISHER
}

$certPfxFile = $null
$isTemporaryCertFile = $false
$activeCert = $null
$activeSubject = $null

if ($CertificateBase64) {
    Write-Host "  -> Decoding certificate from base64 secret payload..." -ForegroundColor Gray
    $rawBytes = [System.Convert]::FromBase64String($CertificateBase64)
    $tempPfxPath = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), "voxy_codesign_$([System.Guid]::NewGuid()).pfx")
    [System.IO.File]::WriteAllBytes($tempPfxPath, $rawBytes)
    $certPfxFile = $tempPfxPath
    $isTemporaryCertFile = $true
} elseif ($CertificatePath -and (Test-Path $CertificatePath)) {
    $certPfxFile = (Resolve-Path $CertificatePath).Path
    Write-Host "  -> Using certificate file from: $certPfxFile" -ForegroundColor Gray
}

# FAIL-CLOSED VALIDATION FOR PRODUCTION ENVIRONMENT
if ($Environment -eq "Production") {
    if (-not $certPfxFile) {
        throw "FAIL-CLOSED SECURITY VIOLATION: Production packaging requires an authoritative code signing certificate via VOXY_CODE_SIGN_PFX_PATH or VOXY_CODE_SIGN_CERT_BASE64. Unsigned or development packages cannot be released to production."
    }

    try {
        $activeCert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new(
            $certPfxFile,
            $CertificatePassword,
            [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::Exportable
        )
    } catch {
        if ($isTemporaryCertFile -and (Test-Path $certPfxFile)) { Remove-Item $certPfxFile -Force }
        throw "FAIL-CLOSED SECURITY VIOLATION: Failed to parse production certificate with provided password: $_"
    }

    if ($activeCert.NotAfter -lt (Get-Date)) {
        if ($isTemporaryCertFile -and (Test-Path $certPfxFile)) { Remove-Item $certPfxFile -Force }
        throw "FAIL-CLOSED SECURITY VIOLATION: Production code signing certificate expired on $($activeCert.NotAfter)."
    }

    $activeSubject = $activeCert.Subject
    if ($activeSubject -like "*LocalDev*" -or $activeSubject -like "*VOXY-AI-LocalDev*") {
        if ($isTemporaryCertFile -and (Test-Path $certPfxFile)) { Remove-Item $certPfxFile -Force }
        throw "FAIL-CLOSED SECURITY VIOLATION: Production packaging strictly rejects development identity '$activeSubject'. A commercial CA-signed certificate is required."
    }

    Write-Host "  -> Authoritative Production Publisher Verified: $activeSubject" -ForegroundColor Green
    Write-Host "  -> Certificate Valid Until: $($activeCert.NotAfter)" -ForegroundColor Green
} else {
    # Development Environment: Allow LocalDev cert fallback with clear warning
    if ($certPfxFile) {
        try {
            $activeCert = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($certPfxFile, $CertificatePassword)
            $activeSubject = $activeCert.Subject
            Write-Host "  -> Using provided certificate: $activeSubject" -ForegroundColor Gray
        } catch {
            Write-Warning "Could not load provided cert; falling back to local dev cert: $_"
        }
    }

    if (-not $activeCert) {
        Write-Warning "============================================================"
        Write-Warning "DEVELOPMENT PACKAGING: Using local development certificate."
        Write-Warning "THIS ARTIFACT IS NOT SIGNED FOR COMMERCIAL PRODUCTION USE."
        Write-Warning "============================================================"

        $devCert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -like "*VOXY-AI-LocalDev*" } | Select-Object -First 1
        if (-not $devCert) {
            Write-Host "  -> Creating local self-signed dev certificate (CN=VOXY-AI-LocalDev)..." -ForegroundColor Gray
            $devCert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=VOXY-AI-LocalDev" -CertStoreLocation Cert:\CurrentUser\My
        }
        $activeCert = $devCert
        $activeSubject = $devCert.Subject
    }
}

# 4. Stage Packaging Artifacts
$outDir = "package\windows\out"
New-Item -ItemType Directory -Path $outDir -Force | Out-Null
$stagingDir = "$outDir\staging"
if (Test-Path $stagingDir) {
    Remove-Item -Path $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null
New-Item -ItemType Directory -Path "$stagingDir\Assets" -Force | Out-Null

Write-Host "[4/7] Staging package artifacts in $stagingDir..." -ForegroundColor Yellow

# Dynamically patch AppxManifest.xml with authoritative Publisher Subject
$manifestContent = Get-Content "package\windows\AppxManifest.xml" -Raw
$escapedSubject = $activeSubject.Replace('"', '&quot;')
$manifestContent = $manifestContent -replace 'Publisher="[^"]*"', "Publisher=`"$escapedSubject`""
Set-Content -Path "$stagingDir\AppxManifest.xml" -Value $manifestContent -Encoding UTF8

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

# 5. Create MSIX Package
$msixPath = "$outDir\VOXY.Commercial.msix"
Write-Host "[5/7] Creating MSIX package: $msixPath..." -ForegroundColor Yellow
& $makeappx pack /d $stagingDir /p $msixPath /o /nv
if ($LASTEXITCODE -ne 0) {
    if ($isTemporaryCertFile -and (Test-Path $certPfxFile)) { Remove-Item $certPfxFile -Force }
    throw "makeappx failed with exit code $LASTEXITCODE"
}

# 6. Sign Binaries and MSIX Package
Write-Host "[6/7] Signing Package Artifacts..." -ForegroundColor Yellow

$signTargetFiles = @(
    "$stagingDir\voxy-daemon.exe",
    "$stagingDir\voxy-overlay.exe",
    $msixPath
)

foreach ($targetFile in $signTargetFiles) {
    Write-Host "  -> Signing $targetFile..." -ForegroundColor Gray
    if ($certPfxFile) {
        $signArgs = @("sign", "/fd", "SHA256")
        if ($TimestampServer) {
            $signArgs += @("/tr", $TimestampServer, "/td", "SHA256")
        }
        $signArgs += @("/f", $certPfxFile)
        if ($CertificatePassword) {
            $signArgs += @("/p", $CertificatePassword)
        }
        $signArgs += $targetFile
        & $signtool @signArgs
    } else {
        # Development thumbprint signing from store
        & $signtool sign /fd SHA256 /sha1 $activeCert.Thumbprint $targetFile
    }

    if ($LASTEXITCODE -ne 0) {
        if ($isTemporaryCertFile -and (Test-Path $certPfxFile)) { Remove-Item $certPfxFile -Force }
        throw "signtool failed to sign $targetFile (exit code $LASTEXITCODE)"
    }
}

# Cleanup temporary certificate file
if ($isTemporaryCertFile -and (Test-Path $certPfxFile)) {
    Remove-Item $certPfxFile -Force
}

# AUTOMATED SIGNATURE VERIFICATION PASS
Write-Host "[7/7] Verifying Authenticode Signatures and Package Integrity..." -ForegroundColor Yellow
$verifyTargets = @(
    @{ Name = "Daemon"; Path = "$stagingDir\voxy-daemon.exe" },
    @{ Name = "Overlay"; Path = "$stagingDir\voxy-overlay.exe" },
    @{ Name = "MSIX Package"; Path = $msixPath }
)

foreach ($target in $verifyTargets) {
    $verifySig = Get-AuthenticodeSignature $target.Path
    if ($verifySig.Status -eq "NotSigned") {
        throw "VERIFICATION FAILED: $($target.Name) artifact $($target.Path) is unsigned!"
    }
    if ($Environment -eq "Production") {
        if ($verifySig.Status -ne "Valid") {
            throw "FAIL-CLOSED VIOLATION: $($target.Name) signature status is '$($verifySig.Status)' ($($verifySig.StatusMessage))!"
        }
        if ($verifySig.SignerCertificate.Subject -like "*LocalDev*") {
            throw "FAIL-CLOSED VIOLATION: Production artifact $($target.Name) was signed with local development certificate!"
        }
        Write-Host "  -> Production Authenticode signature verified on $($target.Name): $($verifySig.SignerCertificate.Subject)" -ForegroundColor Green
    } else {
        Write-Host "  -> Development Authenticode signature verified on $($target.Name): $($verifySig.SignerCertificate.Subject) [Status: $($verifySig.Status)]" -ForegroundColor Yellow
    }
}

# Generate Portable Standalone Distribution ZIP
Write-Host "`nGenerating Portable Standalone Distribution ZIP..." -ForegroundColor Yellow
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
Version: 1.0.0 (Commercial Production Edition)
Architecture: Windows x64
Environment: $Environment
Publisher: $activeSubject

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
$msixHash = (Get-FileHash $msixPath -Algorithm SHA256).Hash
$checksums = @"
$daemonHash  voxy-daemon.exe
$overlayHash  voxy-overlay.exe
$msixHash  VOXY.Commercial.msix
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
Write-Host "     VOXY COM PACKAGING COMPLETED SUCCESSFULLY ($Environment) " -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Green
Write-Host "MSIX Package:     $msixPath"
Write-Host "Portable ZIP:     $portableZip"
Write-Host "Publisher:        $activeSubject"
Write-Host "Applications:     voxy-overlay.exe (GUI), voxy-daemon.exe (Runtime)"
Write-Host "MSIX SHA256:      $msixHash"
Write-Host "Daemon SHA256:    $daemonHash"
Write-Host "Overlay SHA256:   $overlayHash"
