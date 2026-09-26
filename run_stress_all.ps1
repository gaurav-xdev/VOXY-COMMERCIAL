$env:LIBCLANG_PATH="C:\tools\llvm\bin"
$env:CMAKE="C:\tools\cmake2\cmake-3.31.6-windows-x86_64\bin\cmake.exe"
$env:CC=$env:CXX="C:\Program Files\LLVM\bin\clang-cl.exe"

$threads = @(1, 2, 4, 8, 12)
$iterations = 15

# Sequential
Write-Host "=== SEQUENTIAL MODE ===" -ForegroundColor Green
foreach ($t in $threads) {
    Write-Host "--- THREADS=$t ---" -ForegroundColor Cyan
    $env:VOXY_WHISPER_THREADS = $t
    $result = cargo run --example stt_stress -p voxy-whisper --features whisper-engine -- $t sequential $iterations 2>&1
    $result | Select-String -Pattern "Testing|Partial|Final|SUMMARY|error|panicked" | ForEach-Object { Write-Host $_ }
    Write-Host ""
}

# Rapid
Write-Host "=== RAPID MODE ===" -ForegroundColor Green
foreach ($t in $threads) {
    Write-Host "--- THREADS=$t ---" -ForegroundColor Cyan
    $env:VOXY_WHISPER_THREADS = $t
    $result = cargo run --example stt_stress -p voxy-whisper --features whisper-engine -- $t rapid $iterations 2>&1
    $result | Select-String -Pattern "Testing|Rapid|SUMMARY|error|panicked" | ForEach-Object { Write-Host $_ }
    Write-Host ""
}