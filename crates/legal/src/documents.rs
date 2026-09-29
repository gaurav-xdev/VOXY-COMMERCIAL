//! Canonical Production Legal Documents & Version Disclosures for VOXY COM.
//!
//! Complies with Master Goal Sections 27, 28, 29, 30:
//! - Discloses real data flows (Local models, Cloud AI routing, Whisper STT)
//! - Discloses Dodo Payments checkout architecture and zero-PCI-card retention
//! - Explicit GDPR & CCPA user rights (access, export, erasure)
//! - 100% User ownership of all code, content, and automation output

pub const CURRENT_PRIVACY_POLICY_VERSION: &str = "2026-10-01";
pub const CURRENT_TERMS_OF_SERVICE_VERSION: &str = "2026-10-01";
pub const CURRENT_AI_DISCLOSURE_VERSION: &str = "2026-10-01";

/// Canonical Production Privacy Policy for VOXY COM.
pub const PRIVACY_POLICY_MARKDOWN: &str = r#"# VOXY COM — Production Privacy Policy
**Version:** 2026-10-01  
**Effective Date:** October 1, 2026  
**Contact:** privacy@voxy.ai

At VOXY, we are committed to respecting your privacy, protecting your personal data, and providing transparent disclosures regarding how data is processed within VOXY COM ("the Software").

---

### 1. Data Architecture & Processing Model
VOXY COM operates under a hybrid local-first architecture:
* **Local Processing:** By default, microphone voice detection, wake-word recognition, acoustic echo cancellation, local LLMs (via Ollama / local weights), and local speech synthesis run on your physical machine. Audio captured during local processing is never uploaded or transmitted to external servers.
* **Cloud AI Providers:** If you select Cloud or Auto routing modes, specific prompt text, code snippets, or user inquiries are securely transmitted over TLS 1.3 to authorized third-party AI providers (e.g., Anthropic, OpenAI, Groq) solely to generate conversational responses or execute automation plans. VOXY COM does not permit AI providers to train foundation models on your proprietary code or private queries.
* **Audio Telemetry:** Real-time RMS audio energy levels are calculated locally in memory for visualizer animations and immediately discarded. VOXY COM does not record, retain, or store your raw voice recordings without your explicit, separate opt-in consent.

---

### 2. Billing & Payment Information
All commercial subscriptions, hosted checkouts, invoices, and billing lifecycle operations are processed directly through **Dodo Payments** (https://dodopayments.com).
* **Payment Security:** Dodo Payments is a certified PCI-DSS compliant payment processor.
* **Zero Card Retention:** VOXY servers and daemon applications NEVER receive, process, or store your credit card numbers, debit card numbers, expiration dates, or CVV/CVC security codes.
* **Stored Billing Metadata:** VOXY retains only your Dodo Customer ID, active Subscription ID, product tier, and renewal timestamps in order to provision feature entitlements.

---

### 3. Telemetry & Error Reporting
* **System Telemetry:** We collect high-level diagnostic data (operating system version, CPU architecture, GPU availability, and crash traces) solely to maintain software stability.
* **Opt-Out Control:** You may disable anonymous diagnostic reporting at any time via Settings -> Privacy -> Telemetry.

---

### 4. Your Rights under GDPR, CCPA/CPRA & Global Privacy Laws
Regardless of your geographic location, VOXY grants you comprehensive data rights:
1. **Right to Access:** You may review all account, session, and subscription records associated with your identity.
2. **Right to Data Portability (Export):** You may download a machine-readable JSON archive of all your stored data via Settings or our API (`GET /privacy/export`).
3. **Right to Erasure ("Right to be Forgotten"):** You may permanently erase your account, identity records, sessions, consent history, and audit trails via Settings or our API (`POST /privacy/erase`).
4. **Right to Withdraw Consent:** You may revoke optional consents (e.g., telemetry, cloud routing) at any time.

---

### 5. Security Safeguards
We implement defense-in-depth security including Argon2id password hashing, 256-bit cryptographically random session tokens, HMAC-SHA256 webhook validation, and strict Windows Named Pipe access control.
"#;

/// Canonical Terms of Service for VOXY COM.
pub const TERMS_OF_SERVICE_MARKDOWN: &str = r#"# VOXY COM — Commercial Terms of Service
**Version:** 2026-10-01  
**Effective Date:** October 1, 2026  
**Contact:** legal@voxy.ai

These Terms of Service ("Terms") govern your access to and use of VOXY COM ("the Software"). By creating an account, downloading, or using the Software, you agree to be bound by these Terms.

---

### 1. Commercial License & Scope of Use
Subject to payment of applicable subscription fees, VOXY grants you a non-exclusive, non-transferable commercial license to install, operate, and utilize VOXY COM on supported Windows devices.

---

### 2. Intellectual Property & 100% User Ownership
* **User Ownership of Output:** You retain 100% exclusive intellectual property ownership of all source code, software, repositories, documentation, spreadsheets, and artifacts generated, modified, or authored by VOXY during your sessions.
* **No Claim by VOXY:** VOXY makes no copyright, patent, or proprietary claim over your projects, codebases, or automation workflows.

---

### 3. Computer Control & Autonomous Operations Disclaimer
* **Human Oversight:** VOXY provides automated mouse, keyboard, and system interaction capabilities ("Computer Control"). You acknowledge that automated operations carry inherent operational risks.
* **Confirmation Policies:** High-risk operations (file deletion, formatting, financial submissions) enforce mandatory confirmation prompts.
* **Emergency Stop:** A prominent Emergency Stop control (and ESC shortcut) is provided at all times. You agree to maintain active supervision when authorizing VOXY to perform autonomous actions on your computer.

---

### 4. Limitation of Liability
To the maximum extent permitted by applicable law, in no event shall VOXY or its contributors be liable for any indirect, incidental, punitive, or consequential damages arising from the use or inability to use the Software.
"#;

/// Verifies that all mandatory privacy requirements are disclosed in the text.
pub fn verify_privacy_policy_completeness(policy: &str) -> Vec<&'static str> {
    let mut missing = Vec::new();

    let checks: &[(&str, &str)] = &[
        ("AI Providers", "Cloud AI Providers"),
        ("Local vs Cloud Processing", "Local Processing"),
        ("Audio Telemetry / Zero Storage", "raw voice recordings"),
        ("Dodo Payments Disclosure", "Dodo Payments"),
        ("Zero Card Retention", "Zero Card Retention"),
        ("GDPR / CCPA Rights", "GDPR, CCPA/CPRA"),
        ("Right to Erasure", "Right to Erasure"),
        ("Data Portability / Export", "Data Portability"),
        ("Contact Email", "privacy@voxy.ai"),
    ];

    for (name, needle) in checks {
        if !policy.contains(needle) {
            missing.push(*name);
        }
    }

    missing
}
