# OSMOO Engineering Audit: Limitations, Blockers & Unverified Claims

**Audit Date:** 2026-10-20  
**Target Repository:** `C:\Users\gaurav\Downloads\VOXY-COMMERCIAL`  
**Purpose:** Honest disclosure of scope boundaries, environmental blockers, and items requiring live user infrastructure.

---

## 1. Unverified Claims & Explicit Boundaries

1. **Physical Multi-Microphone Spatial Beamforming**:
   - **Status**: **NOT VERIFIED ON HARDWARE**
   - **Reason**: The host environment possesses a standard single-channel capture endpoint. True geometry-aware delay-and-sum beamforming requires multi-channel circular or linear microphone arrays with calibrated inter-element distances and sample-synchronized ADC channels.
   - **What IS Verified**: 80Hz high-pass filter, energy-envelope noise gate with hysteresis, soft peak limiter, and acoustic echo suppression (attenuating microphone capture when assistant speech is active).

2. **Live Cloud AI Billing & External Network Requests**:
   - **Status**: **PARTIALLY VERIFIED**
   - **Reason**: Cloud providers (OpenAI, Anthropic, Cartesia, Groq) require valid, funded API keys and network connectivity. The automated test suite verifies that:
     - Cloud providers fail closed when API keys are missing.
     - Credential storage uses DPAPI user-session encryption.
     - Tool calls validate URLs against SSRF.
   - **What is NOT Verified**: Live token generation, remote TTS streaming from Cartesia servers, and monthly billing accumulation.

3. **Multi-Hour Continuous Interactive GUI Usage**:
   - **Status**: **PARTIALLY VERIFIED**
   - **Reason**: `OSMOO.exe` builds cleanly and executes CLI commands (`--help`, `--status`) and automated Dioxus state tests (WebGL particle budgets, onboarding step progression, settings form persistence). However, manual continuous human interaction over an extended session (e.g., mouse hovering over Three.js companion models, window dragging across multiple physical 4K monitors) requires an active human desktop operator.

4. **Hardware TPM 2.0 PCR Sealing**:
   - **Status**: **NOT IMPLEMENTED / NOT APPLICABLE**
   - **Reason**: Session encryption is implemented via Windows DPAPI (`CryptProtectData`), which seals secrets to the **Windows logon user account**, not hardware TPM PCR registers. UI documentation and banners accurately designate this as Windows DPAPI user-bound encryption rather than claiming TPM hardware sealing.

---

## 2. Non-Fatal Warnings in Test Builds

- In `tests/backend_command_test.rs`, two unused import warnings (`std::sync::Arc`, `voxy_desktop_ui::fixtures::PROVIDER_CATALOG`) were identified and cleaned up.
- In `cargo check --workspace --tests`, all 77 workspace packages compile with exit code 0.
