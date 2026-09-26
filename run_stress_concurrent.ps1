$env:LIBCLANG_PATH="C:\tools\llvm\bin"
$env:CMAKE="C:\tools\cmake2\cmake-3.31.6-windows-x86_64\bin\cmake.exe"
$env:CC=$env:CXX="C:\Program Files\LLVM\bin\clang-cl.exe"

$threads = @(1, 2, 4, 8, 12)
$iterations = 15

foreach ($t in $threads) {
    Write-Host "=== THREADS=$t CONCURRENT ===" -ForegroundColor Cyan
    $env:VOXY_WHISPER_THREADS = $t
    $result = cargo run --example stt_stress -p voxy-whisper --features whisper-engine -- $t concurrent $iterations 2>&1
    $result | Select-String -Pattern "Testing|Partial|Final|SUMMARY|error|thread|panicked" | ForEach-Object { Write-Host $_ }
    Write-Host ""
}