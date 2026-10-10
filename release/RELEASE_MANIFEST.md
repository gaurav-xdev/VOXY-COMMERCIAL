# OSMOO — Final Release Verification Manifest

**Product:** OSMOO (Operating System Machine Optimization Operator)  
**Parent Organization:** Osmiora Computational Systems  
**Domain / Website:** https://osmoo.in  
**Commit Revision:** `031d6b08c5246e87a9b868c4af5e8d6ca226ae1b`  
**Architecture:** Windows x86_64 (MSVC)  
**Build Profile:** `release` [optimized]  
**Timestamp:** 2026-10-10T11:35:30+05:30  

---

## 1. Verified Release Artifacts

| Artifact Name | Size (Bytes) | SHA-256 Checksum | Delivery Status |
| :--- | :---: | :--- | :--- |
| `OSMOO.exe` | 8,654,848 | `DD6245A87E835952C402754010DE9C9ADC13AEE55B6C36ED1B069AEE12546518` | Release Binary Verified |
| `voxy-daemon.exe` | 8,019,968 | `DAB24F411911F705B7E547B5722D0729DF6D4C98996DA2F61699808C40F18472` | Release Binary Verified |
| `voxy-overlay.exe` | 3,875,840 | `27C0EE08AE60D09673C6DD73D6DA0DD09C68F4090F77A21A98F94C20EC1FA817` | Release Binary Verified |
| `OSMOO-v1.0.0-Portable-x64.zip` | 8,933,270 | `02B3EFEE562F84AA5A7F4C6D1416D0B99F0F6AA7015FD06B9E235F12F9157B07` | Packaged & Checksum Verified |
| `OSMOO-1.0.0-Setup-x64.exe` | 5,536,745 | `7462797C255DBC5C5D543BD82F24C9872165A58B9A1DD254F469FC448287147B` | Compiled NSIS Windows Setup Verified |
| `OSMOO-Setup-x64.exe` | 5,536,745 | `7462797C255DBC5C5D543BD82F24C9872165A58B9A1DD254F469FC448287147B` | Standard Setup Alias Verified |

---

## 2. Test Execution Verification Baseline

- **Total Automated Workspace Tests:** 2,638 passed, 0 failed across 180 test suites.
- **Authentication & Security Suite (`voxy-api-server`):** 8 passed, 0 failed.
- **Commercial Database & Schema Migration 106 (`voxy-database`):** 1 passed, 0 failed.
- **Autonomous Coding Harness & IDE Launcher (`voxy-harness`):** 12 passed, 0 failed.
- **Persistent Scoped Memory System (`voxy-memory`):** 60 passed, 0 failed.
- **Dynamic Skills & Activation Boundary Engine (`voxy-skills`):** 17 passed, 0 failed.
- **Named Pipe IPC Zero-Network Protocol (`voxy-ipc`):** 5 passed, 0 failed.
- **Desktop Runtime Autolaunch Registry (`voxy-desktop-runtime`):** 2 passed, 0 failed.
- **Desktop UI Bridge & Session Serialization (`voxy-desktop-ui`):** 3 passed, 0 failed.
