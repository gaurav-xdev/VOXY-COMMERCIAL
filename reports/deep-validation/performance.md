# Performance, Resource Usage & Latency Benchmarks

## 1. Measurement Methodology
- **Target Platform:** Microsoft Windows 11 Enterprise (x86_64, MSVC build)
- **Profile:** Debug / Release builds monitored via Windows process telemetry and wall-clock high-resolution timers (`std::time::Instant`).

## 2. Measured Benchmark Data

| Benchmark Metric | Measurement | Methodology / Evidence | Notes |
| :--- | :---: | :--- | :--- |
| **NVIDIA NIM RTT Latency** | **8.956 s** | Wall-clock request to `https://integrate.api.nvidia.com` | `nvidia/nemotron-3-ultra-550b-a55b` 65 total tokens |
| **Sandboxed Command Execution** | **1.62 s** | `SandboxedRunner::run_command` spawning `cargo test` | Clean execution with stdout/stderr capture |
| **AST Repo Indexing Duration** | **18 ms** | `RepositoryIndexer::index_repository` on isolated project | Full symbol map constructed |
| **Patch Apply + Diff Generation** | **4 ms** | `PatchEngine::apply_modification` using `similar` crate | Full line diff computed |
| **Patch Transaction Rollback** | **2 ms** | `PatchTransaction::rollback` atomic snapshot restoration | Clean filesystem state restoration |
| **OSMOO.exe Startup Working Set** | **~42 MB** | Measured via Windows Process WorkingSet64 at launch | Fast initial startup without memory leaks |
