# VOXY Holographic Spatial UI Launcher
# Starts the local MediaPipe gesture tracking service and the voxy-overlay desktop HUD

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "  VOXY HOLOGRAPHIC SPATIAL GESTURE UI // MK-V" -ForegroundColor Cyan
Write-Host "  ZERO AI SLOP - AEROSPACE HUD SPECIFICATION" -ForegroundColor Cyan
Write-Host "==========================================================" -ForegroundColor Cyan

# 1. Start MediaPipe Gesture Service
Write-Host "[1/2] Starting MediaPipe Gesture Tracking Service (Webcam 0)..." -ForegroundColor Yellow
$gestureProc = Start-Process -FilePath "python" -ArgumentList "tools/gesture_service.py", "--port", "18888" -PassThru -NoNewWindow

Start-Sleep -Seconds 3

# 2. Start VOXY Desktop Overlay
Write-Host "[2/2] Launching VOXY Holographic Overlay Window..." -ForegroundColor Green
$overlayProc = Start-Process -FilePath "target\debug\voxy-overlay.exe" -PassThru

Write-Host ""
Write-Host ">>> VOXY Holographic UI Active!" -ForegroundColor Cyan
Write-Host "    - Move hands apart or clap to expand HUD spatially" -ForegroundColor White
Write-Host "    - Bring hands together to collapse HUD to dormant ring" -ForegroundColor White
Write-Host "    - Press [SPACE] or [G] to manually toggle spatial view" -ForegroundColor White
Write-Host "    - Press [1]-[6] to preview voice states (Listening, Thinking, Speaking, etc.)" -ForegroundColor White
Write-Host "    - Press Ctrl+C in this console to terminate both services" -ForegroundColor Yellow
Write-Host ""

try {
    Wait-Process -Id $overlayProc.Id
} finally {
    Write-Host "Stopping background services..." -ForegroundColor Red
    if ($gestureProc -and -not $gestureProc.HasExited) {
        Stop-Process -Id $gestureProc.Id -Force
    }
    if ($overlayProc -and -not $overlayProc.HasExited) {
        Stop-Process -Id $overlayProc.Id -Force
    }
    Write-Host "Services stopped cleanly." -ForegroundColor Green
}
