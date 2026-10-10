pub const APP_CSS: &str = r##"
:root {
    --bg-primary: #07090e;
    --bg-secondary: #0c1017;
    --bg-card: rgba(16, 22, 34, 0.75);
    --border-subtle: rgba(255, 255, 255, 0.08);
    --border-active: rgba(56, 189, 248, 0.4);
    --text-primary: #f8fafc;
    --text-secondary: #94a3b8;
    --text-muted: #64748b;
    --accent-cyan: #38bdf8;
    --accent-emerald: #10b981;
    --accent-purple: #a855f7;
    --accent-amber: #f59e0b;
    --accent-rose: #f43f5e;
    --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    --font-mono: 'JetBrains Mono', 'Cascadia Code', Consolas, monospace;
}

* {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    user-select: none;
    -webkit-user-drag: none;
}

body, html {
    width: 100%;
    height: 100%;
    background-color: var(--bg-primary);
    color: var(--text-primary);
    font-family: var(--font-sans);
    overflow: hidden;
    -webkit-font-smoothing: antialiased;
}

#main-app-container {
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    position: relative;
    background: radial-gradient(circle at 50% 50%, #0d1527 0%, #060911 100%);
}

/* ── Splash Screen ────────────────────────────────────────────── */
.splash-screen-container {
    position: absolute;
    inset: 0;
    background: #05070c;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    flex-direction: column;
}

.splash-ambient-radiance {
    position: absolute;
    width: 500px;
    height: 500px;
    background: radial-gradient(circle, rgba(56, 189, 248, 0.15) 0%, transparent 70%);
    filter: blur(40px);
    pointer-events: none;
}

.splash-branding-box {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 440px;
}

.splash-logo-mark {
    margin-bottom: 24px;
    animation: pulseLogo 3s infinite ease-in-out;
}

@keyframes pulseLogo {
    0%, 100% { transform: scale(1); opacity: 0.9; }
    50% { transform: scale(1.05); opacity: 1; filter: drop-shadow(0 0 16px rgba(56, 189, 248, 0.5)); }
}

.splash-brand-title {
    font-size: 38px;
    font-weight: 700;
    letter-spacing: 0.2em;
    color: #ffffff;
    margin-bottom: 6px;
    text-shadow: 0 0 20px rgba(56, 189, 248, 0.4);
}

.splash-brand-subtitle {
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.16em;
    color: var(--accent-cyan);
    margin-bottom: 4px;
}

.splash-brand-parent {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.2em;
    color: var(--text-muted);
    margin-bottom: 36px;
}

.splash-progress-track {
    width: 320px;
    height: 4px;
    background: rgba(255, 255, 255, 0.08);
    border-radius: 999px;
    overflow: hidden;
    margin-bottom: 16px;
}

.splash-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #0284c7, #38bdf8);
    border-radius: 999px;
    transition: width 0.15s ease-out;
    box-shadow: 0 0 10px #38bdf8;
}

.splash-telemetry-row {
    width: 320px;
    display: flex;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    letter-spacing: 0.06em;
}

/* ── 3D Assistant Core Experience ─────────────────────────────── */
.core-experience-wrapper {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
    pointer-events: auto;
}

.osmoo-three-canvas-holder {
    width: 100%;
    height: 100%;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
}

.osmoo-three-canvas-holder canvas {
    width: 100% !important;
    height: 100% !important;
    outline: none;
}

.core-telemetry-hud {
    position: absolute;
    bottom: 96px;
    display: flex;
    flex-direction: column;
    align-items: center;
    pointer-events: none;
    gap: 8px;
}

.core-state-badge {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 18px;
    background: rgba(15, 23, 42, 0.7);
    border: 1px solid;
    border-radius: 999px;
    backdrop-filter: blur(12px);
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.12em;
    transition: all 0.3s ease;
}

.core-state-pulsar {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    animation: pulsarGlow 1.8s infinite ease-in-out;
}

@keyframes pulsarGlow {
    0%, 100% { transform: scale(1); opacity: 0.7; }
    50% { transform: scale(1.3); opacity: 1; }
}

.core-subtext-stream {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    letter-spacing: 0.08em;
}

/* ── HUD Root Layer ───────────────────────────────────────────── */
.hud-root-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 20;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 24px;
}

.hud-top-bar {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    pointer-events: auto;
}

.hud-brand-pill {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    background: rgba(15, 23, 42, 0.6);
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    backdrop-filter: blur(12px);
}

.hud-brand-dot {
    width: 6px;
    height: 6px;
    background-color: var(--accent-cyan);
    border-radius: 50%;
    box-shadow: 0 0 6px var(--accent-cyan);
}

.hud-brand-name {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: #ffffff;
}

.hud-brand-sub {
    font-family: var(--font-mono);
    font-size: 9px;
    color: var(--text-muted);
    letter-spacing: 0.06em;
    padding-left: 6px;
    border-left: 1px solid rgba(255, 255, 255, 0.1);
}

.hud-telemetry-cluster {
    display: flex;
    align-items: center;
    gap: 8px;
}

.telemetry-pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    background: rgba(15, 23, 42, 0.5);
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    backdrop-filter: blur(8px);
}

.telemetry-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
}

.telemetry-text {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-secondary);
    letter-spacing: 0.06em;
}

.hud-controls-cluster {
    display: flex;
    align-items: center;
    gap: 8px;
}

.hud-icon-btn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid var(--border-subtle);
    background: rgba(15, 23, 42, 0.6);
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    backdrop-filter: blur(10px);
    transition: all 0.2s ease;
}

.hud-icon-btn:hover {
    color: #ffffff;
    border-color: rgba(255, 255, 255, 0.25);
    background: rgba(30, 41, 59, 0.8);
    transform: translateY(-1px);
}

.hud-icon-btn.active {
    color: var(--accent-emerald);
    border-color: rgba(16, 185, 129, 0.4);
    box-shadow: 0 0 10px rgba(16, 185, 129, 0.25);
}

.hud-icon-btn.emergency:hover {
    color: var(--accent-rose);
    border-color: rgba(244, 63, 94, 0.5);
    box-shadow: 0 0 12px rgba(244, 63, 94, 0.3);
}

/* ── HUD Bottom Action Bar ────────────────────────────────────── */
.hud-bottom-actions {
    align-self: center;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 18px;
    background: rgba(15, 23, 42, 0.65);
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    backdrop-filter: blur(12px);
    pointer-events: auto;
}

.action-hint {
    display: flex;
    align-items: center;
    gap: 6px;
}

.kbd-key {
    padding: 2px 6px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 600;
    color: var(--accent-cyan);
}

.hint-text {
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    color: var(--text-muted);
}

.action-divider {
    color: rgba(255, 255, 255, 0.2);
}

/* ── Secondary Conversation Drawer ────────────────────────────── */
.hud-drawer-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(4px);
    z-index: 40;
    pointer-events: auto;
}

.hud-history-drawer {
    position: fixed;
    top: 0;
    right: 0;
    width: 440px;
    height: 100vh;
    background: rgba(11, 15, 24, 0.94);
    border-left: 1px solid var(--border-subtle);
    backdrop-filter: blur(24px);
    display: flex;
    flex-direction: column;
    z-index: 50;
    transform: translateX(100%);
    transition: transform 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    pointer-events: auto;
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.6);
}

.hud-history-drawer.open {
    transform: translateX(0);
}

.drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border-subtle);
}

.drawer-title {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.12em;
    color: var(--accent-cyan);
}

.drawer-close-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 20px;
    cursor: pointer;
    line-height: 1;
}

.drawer-close-btn:hover {
    color: #ffffff;
}

.drawer-turns-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.drawer-empty {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-muted);
    text-align: center;
    margin-top: 40px;
}

.chat-turn {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.04);
}

.chat-turn.user {
    background: rgba(56, 189, 248, 0.06);
    border-color: rgba(56, 189, 248, 0.15);
}

.chat-turn.assistant {
    background: rgba(16, 185, 129, 0.05);
    border-color: rgba(16, 185, 129, 0.12);
}

.turn-meta {
    display: flex;
    justify-content: space-between;
    font-family: var(--font-mono);
    font-size: 10px;
}

.turn-sender {
    font-weight: 600;
    color: var(--accent-cyan);
}

.turn-time {
    color: var(--text-muted);
}

.turn-body {
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-primary);
    word-break: break-word;
}

.drawer-input-row {
    padding: 18px 24px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    gap: 10px;
    background: rgba(8, 12, 19, 0.8);
}

/* ── Form Controls & Shared UI ────────────────────────────────── */
.hud-input {
    flex: 1;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 13px;
    color: #ffffff;
    outline: none;
    transition: all 0.2s ease;
}

.hud-input:focus {
    border-color: var(--accent-cyan);
    background: rgba(255, 255, 255, 0.08);
}

.hud-select {
    width: 100%;
    background: #0f172a;
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 10px 14px;
    font-size: 13px;
    color: #ffffff;
    outline: none;
}

.hud-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    border: 1px solid transparent;
    transition: all 0.2s ease;
}

.hud-btn.primary {
    background: #0284c7;
    color: #ffffff;
}

.hud-btn.primary:hover {
    background: #0369a1;
}

.hud-btn.secondary {
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-secondary);
    border-color: var(--border-subtle);
}

.hud-btn.secondary:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
}

/* ── Onboarding Wizard Card ───────────────────────────────────── */
.onboarding-wizard-overlay {
    position: fixed;
    inset: 0;
    background: rgba(3, 5, 10, 0.85);
    backdrop-filter: blur(20px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.wizard-card {
    width: 580px;
    background: rgba(13, 18, 28, 0.95);
    border: 1px solid var(--border-subtle);
    border-radius: 16px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 25px 50px rgba(0, 0, 0, 0.6);
}

.wizard-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 28px;
    border-bottom: 1px solid var(--border-subtle);
}

.wizard-step-pills {
    display: flex;
    gap: 8px;
}

.step-pill {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-muted);
}

.step-pill.active {
    background: var(--accent-cyan);
    color: #07090e;
    font-weight: 700;
    border-color: var(--accent-cyan);
}

.step-pill.completed {
    background: rgba(16, 185, 129, 0.15);
    color: var(--accent-emerald);
    border-color: rgba(16, 185, 129, 0.4);
}

.wizard-brand-badge {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    color: var(--text-muted);
}

.wizard-body {
    padding: 28px;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.wizard-title {
    font-size: 20px;
    font-weight: 600;
    color: #ffffff;
}

.wizard-desc {
    font-size: 13px;
    color: var(--text-secondary);
    line-height: 1.5;
}

.wizard-footer {
    padding: 18px 28px;
    border-top: 1px solid var(--border-subtle);
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(9, 13, 21, 0.8);
}

.provider-selector-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
}

.provider-card {
    padding: 14px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-subtle);
    cursor: pointer;
    display: flex;
    flex-direction: column;
    gap: 6px;
    transition: all 0.2s ease;
}

.provider-card:hover {
    border-color: rgba(255, 255, 255, 0.2);
}

.provider-card.selected {
    border-color: var(--accent-cyan);
    background: rgba(56, 189, 248, 0.08);
}

.provider-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.provider-name {
    font-size: 13px;
    font-weight: 600;
    color: #ffffff;
}

.provider-card-sub {
    font-size: 11px;
    color: var(--text-muted);
    line-height: 1.4;
}

.model-discovery-box {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    background: rgba(15, 23, 42, 0.5);
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
}

.discovery-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.refresh-btn {
    background: none;
    border: none;
    color: var(--accent-cyan);
    font-family: var(--font-mono);
    font-size: 11px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 4px;
}

.discovery-warning {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--accent-amber);
    line-height: 1.4;
}

.toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 14px;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 8px;
    border: 1px solid var(--border-subtle);
}

.toggle-title {
    font-size: 13px;
    font-weight: 500;
    color: #ffffff;
}

.toggle-desc {
    font-size: 11px;
    color: var(--text-muted);
}

.mic-calibration-panel, .tts-test-panel {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
    background: rgba(15, 23, 42, 0.5);
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
}

.mic-status-row, .tts-test-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.meter-container {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 999px;
    overflow: hidden;
}

.meter-fill {
    height: 100%;
    background: linear-gradient(90deg, #10b981, #38bdf8);
    transition: width 0.08s ease;
}

.complete-summary-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
    background: rgba(15, 23, 42, 0.6);
    border-radius: 10px;
    border: 1px solid var(--border-subtle);
    text-align: left;
}

.summary-row {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
}

.summary-label {
    color: var(--text-muted);
}

.summary-val {
    color: #ffffff;
    font-weight: 500;
}

/* ── Settings Modal ───────────────────────────────────────────── */
.settings-view-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
}

.settings-modal-card {
    width: 600px;
    max-height: 85vh;
    background: rgba(12, 17, 26, 0.96);
    border: 1px solid var(--border-subtle);
    border-radius: 16px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}

.settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border-subtle);
}

.settings-header-title-group {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--accent-cyan);
}

.settings-title {
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.1em;
    color: #ffffff;
}

.settings-content-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 24px;
}

.settings-section-title {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    color: var(--accent-cyan);
    margin-bottom: 12px;
}

.settings-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    border-top: 1px solid var(--border-subtle);
    background: rgba(8, 11, 18, 0.8);
}

.settings-btn-group {
    display: flex;
    gap: 10px;
}

.save-status-msg {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--accent-emerald);
}

/* ── Settings Tier Card & Upgrade CTA ──────────────────────────── */
.tier-status-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 20px;
    background: linear-gradient(135deg, rgba(56, 189, 248, 0.08) 0%, rgba(16, 185, 129, 0.04) 100%);
    border: 1px solid rgba(56, 189, 248, 0.25);
    border-radius: 12px;
    margin-bottom: 12px;
}

.tier-status-left {
    display: flex;
    flex-direction: column;
    gap: 4px;
}

.tier-badge-cluster {
    display: flex;
    align-items: center;
    gap: 8px;
}

.tier-name-badge {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.12em;
    padding: 2px 8px;
    border-radius: 4px;
    text-transform: uppercase;
}

.tier-name-badge.free {
    background: rgba(148, 163, 184, 0.2);
    color: #cbd5e1;
    border: 1px solid rgba(148, 163, 184, 0.3);
}

.tier-name-badge.pro {
    background: rgba(56, 189, 248, 0.2);
    color: #38bdf8;
    border: 1px solid rgba(56, 189, 248, 0.4);
}

.tier-name-badge.enterprise {
    background: rgba(168, 85, 247, 0.2);
    color: #c084fc;
    border: 1px solid rgba(168, 85, 247, 0.4);
}

.tier-account-email {
    font-size: 13px;
    font-weight: 500;
    color: #ffffff;
}

.tier-features-summary {
    font-size: 11px;
    color: var(--text-muted);
}

.hud-btn.upgrade-cta {
    background: linear-gradient(135deg, #0284c7 0%, #0369a1 100%);
    border: 1px solid rgba(56, 189, 248, 0.5);
    color: #ffffff;
    font-weight: 600;
    box-shadow: 0 0 16px rgba(56, 189, 248, 0.3);
}

.hud-btn.upgrade-cta:hover {
    background: linear-gradient(135deg, #0369a1 0%, #075985 100%);
    box-shadow: 0 0 24px rgba(56, 189, 248, 0.5);
}
"##;
