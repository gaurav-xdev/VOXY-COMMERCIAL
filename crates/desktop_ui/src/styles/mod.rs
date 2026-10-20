pub const APP_CSS: &str = r##"
:root {
    /* Brand Foundation: Warm Ivory, Deep Onyx, Oxblood, Warm Metallic Neutral, Soft Stone */
    --osmoo-bg-deep: #07080b;
    --osmoo-bg-surface: #0e1017;
    --osmoo-bg-card: rgba(18, 22, 32, 0.76);
    --osmoo-bg-machined: #12141a;

    --osmoo-ivory: #f4f0e8;
    --osmoo-soft-stone: #dcd5c8;
    --osmoo-metallic: #b9a995;
    --osmoo-onyx: #201b1b;
    --osmoo-oxblood: #632c35;

    /* Functional status colors */
    --status-idle: #b9a995;
    --status-listening: #10b981;
    --status-thinking: #a855f7;
    --status-speaking: #38bdf8;
    --status-error: #ef4444;
    --status-offline: #64748b;

    /* Borders & Accents */
    --border-machined: rgba(244, 240, 232, 0.09);
    --border-active: rgba(185, 169, 149, 0.45);
    --border-glow: rgba(244, 240, 232, 0.16);

    /* Text */
    --text-primary: #f4f0e8;
    --text-secondary: #a9a397;
    --text-muted: #6e6960;

    /* Typography */
    --font-sans: 'Plus Jakarta Sans', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
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
    background-color: var(--osmoo-bg-deep);
    color: var(--text-primary);
    font-family: var(--font-sans);
    overflow: hidden;
    -webkit-font-smoothing: antialiased;
}

#main-app-container {
    width: 100vw;
    height: 100vh;
    min-width: 720px;
    display: flex;
    flex-direction: column;
    position: relative;
    background: radial-gradient(circle at 50% 40%, #11141e 0%, #06070a 100%);
}

/* ── Doppelrand (Double-Bezel) Machined Card Architecture ───── */
.machined-card {
    background: var(--osmoo-bg-card);
    border: 1px solid var(--border-machined);
    border-radius: 20px;
    padding: 2px;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.65), 0 1px 3px rgba(0, 0, 0, 0.4);
    backdrop-filter: blur(24px);
    transition: all 0.3s cubic-bezier(0.32, 0.72, 0, 1);
}

.machined-card-inner {
    background: linear-gradient(180deg, rgba(22, 27, 39, 0.6) 0%, rgba(13, 16, 24, 0.8) 100%);
    border-radius: calc(20px - 2px);
    box-shadow: inset 0 1px 1px rgba(244, 240, 232, 0.08);
    padding: 24px;
}

/* ── Buttons & Interactive Controls ──────────────────────────── */
.hud-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 10px 18px;
    border-radius: 9999px;
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.02em;
    cursor: pointer;
    transition: all 0.25s cubic-bezier(0.32, 0.72, 0, 1);
    border: 1px solid var(--border-machined);
    background: rgba(244, 240, 232, 0.04);
    color: var(--text-primary);
}

.hud-btn:hover:not(:disabled) {
    background: rgba(244, 240, 232, 0.09);
    border-color: var(--border-active);
    transform: translateY(-1px);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
}

.hud-btn:active:not(:disabled) {
    transform: scale(0.98);
}

.hud-btn.primary {
    background: var(--osmoo-ivory);
    color: var(--osmoo-onyx);
    border-color: var(--osmoo-ivory);
    box-shadow: 0 0 16px rgba(244, 240, 232, 0.2);
}

.hud-btn.primary:hover:not(:disabled) {
    background: #ffffff;
    box-shadow: 0 0 24px rgba(244, 240, 232, 0.35);
}

.hud-btn.accent {
    background: var(--osmoo-oxblood);
    color: #ffffff;
    border-color: rgba(244, 240, 232, 0.2);
}

.hud-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.hud-icon-btn {
    width: 38px;
    height: 38px;
    border-radius: 9999px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: rgba(244, 240, 232, 0.05);
    border: 1px solid var(--border-machined);
    color: var(--text-primary);
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.32, 0.72, 0, 1);
}

.hud-icon-btn:hover:not(:disabled) {
    background: rgba(244, 240, 232, 0.12);
    border-color: var(--border-active);
    transform: translateY(-1px);
}

.hud-icon-btn.active {
    background: var(--osmoo-metallic);
    color: var(--osmoo-onyx);
    border-color: var(--osmoo-metallic);
}

.hud-icon-btn.emergency {
    color: #fca5a5;
    border-color: rgba(239, 68, 68, 0.3);
}

.hud-icon-btn.emergency:hover {
    background: rgba(239, 68, 68, 0.2);
    border-color: var(--status-error);
}

/* ── Top Floating Navigation & Telemetry ─────────────────────── */
.hud-root-layer {
    position: absolute;
    inset: 0;
    pointer-events: none;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 20px 28px;
    z-index: 10;
}

.hud-top-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    pointer-events: auto;
    width: 100%;
}

.hud-brand-pill {
    display: flex;
    align-items: center;
    gap: 12px;
    background: rgba(14, 16, 23, 0.85);
    border: 1px solid var(--border-machined);
    padding: 8px 18px 8px 12px;
    border-radius: 9999px;
    backdrop-filter: blur(16px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
}

.brand-helical-mark {
    color: var(--osmoo-ivory);
    filter: drop-shadow(0 0 8px rgba(244, 240, 232, 0.4));
}

.hud-brand-name {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.15em;
    color: var(--osmoo-ivory);
}

.hud-brand-sub {
    font-size: 10px;
    letter-spacing: 0.14em;
    color: var(--osmoo-metallic);
    font-family: var(--font-mono);
    border-left: 1px solid var(--border-machined);
    padding-left: 10px;
}

.hud-telemetry-cluster {
    display: flex;
    align-items: center;
    gap: 10px;
}

.telemetry-pill {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(14, 16, 23, 0.85);
    border: 1px solid var(--border-machined);
    padding: 7px 14px;
    border-radius: 9999px;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    color: var(--text-secondary);
    backdrop-filter: blur(16px);
}

.telemetry-dot {
    width: 7px;
    height: 7px;
    border-radius: 9999px;
    box-shadow: 0 0 6px currentColor;
}

.hud-controls-cluster {
    display: flex;
    align-items: center;
    gap: 8px;
}

/* ── Mode Switcher & Lower Floating Dock ─────────────────────── */
.hud-bottom-dock {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    width: 100%;
    max-width: 680px;
    margin: 0 auto;
    pointer-events: auto;
}

.mode-toggle-segmented {
    display: flex;
    align-items: center;
    background: rgba(14, 16, 23, 0.9);
    border: 1px solid var(--border-machined);
    border-radius: 9999px;
    padding: 3px;
    backdrop-filter: blur(20px);
}

.mode-segment-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 16px;
    border-radius: 9999px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    background: transparent;
    border: none;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.32, 0.72, 0, 1);
}

.mode-segment-btn.active {
    background: var(--osmoo-ivory);
    color: var(--osmoo-onyx);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.3);
}

.hud-input-capsule {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    background: rgba(14, 16, 23, 0.9);
    border: 1px solid var(--border-machined);
    border-radius: 9999px;
    padding: 8px 12px 8px 20px;
    backdrop-filter: blur(24px);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.5);
    transition: all 0.2s cubic-bezier(0.32, 0.72, 0, 1);
}

.hud-input-capsule:focus-within {
    border-color: var(--border-active);
    box-shadow: 0 12px 36px rgba(185, 169, 149, 0.15);
}

.hud-text-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 14px;
    font-family: var(--font-sans);
}

.hud-text-input::placeholder {
    color: var(--text-muted);
}

/* ── Slide-in Conversation Drawer ────────────────────────────── */
.hud-drawer-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    z-index: 90;
    pointer-events: auto;
}

.hud-drawer-panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 420px;
    background: #0b0d14;
    border-left: 1px solid var(--border-machined);
    display: flex;
    flex-direction: column;
    z-index: 100;
    pointer-events: auto;
    box-shadow: -16px 0 40px rgba(0, 0, 0, 0.7);
    animation: drawerSlideIn 0.35s cubic-bezier(0.32, 0.72, 0, 1);
}

@keyframes drawerSlideIn {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
}

.drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 24px;
    border-bottom: 1px solid var(--border-machined);
}

.drawer-title {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--osmoo-ivory);
    font-family: var(--font-mono);
}

.drawer-content {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.chat-bubble {
    padding: 12px 16px;
    border-radius: 14px;
    font-size: 13px;
    line-height: 1.6;
    max-width: 88%;
}

.chat-bubble.assistant {
    background: rgba(22, 27, 39, 0.85);
    border: 1px solid var(--border-machined);
    color: var(--text-primary);
    align-self: flex-start;
}

.chat-bubble.user {
    background: var(--osmoo-oxblood);
    color: #ffffff;
    align-self: flex-end;
}

.chat-sender-label {
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--osmoo-metallic);
    margin-bottom: 4px;
    letter-spacing: 0.08em;
}

/* ── Startup Splash Screen ───────────────────────────────────── */
.splash-screen-container {
    position: absolute;
    inset: 0;
    background: #050609;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    flex-direction: column;
}

.splash-ambient-radiance {
    position: absolute;
    width: 520px;
    height: 520px;
    background: radial-gradient(circle, rgba(185, 169, 149, 0.12) 0%, transparent 70%);
    filter: blur(50px);
    pointer-events: none;
}

.splash-branding-box {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 480px;
}

.splash-logo-mark {
    margin-bottom: 28px;
    color: var(--osmoo-ivory);
    animation: helicalPulse 4s infinite ease-in-out;
}

@keyframes helicalPulse {
    0%, 100% { transform: scale(1) rotate(0deg); opacity: 0.9; }
    50% { transform: scale(1.04) rotate(12deg); opacity: 1; filter: drop-shadow(0 0 20px rgba(244, 240, 232, 0.5)); }
}

.splash-brand-title {
    font-size: 42px;
    font-weight: 700;
    letter-spacing: 0.24em;
    color: var(--osmoo-ivory);
    margin-bottom: 8px;
}

.splash-brand-subtitle {
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.18em;
    color: var(--osmoo-metallic);
    margin-bottom: 4px;
}

.splash-brand-parent {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    color: var(--text-muted);
    margin-bottom: 32px;
}

.splash-progress-track {
    width: 320px;
    height: 3px;
    background: rgba(244, 240, 232, 0.08);
    border-radius: 9999px;
    overflow: hidden;
    margin-bottom: 14px;
}

.splash-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--osmoo-metallic) 0%, var(--osmoo-ivory) 100%);
    border-radius: 9999px;
    transition: width 0.3s ease-out;
}

.splash-telemetry-row {
    display: flex;
    justify-content: space-between;
    width: 320px;
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.12em;
    color: var(--text-muted);
}

/* ── 3D Viewport & Fallback ──────────────────────────────────── */
#osmoo-viewport-container {
    width: 100%;
    height: 100%;
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
}

.webgl-fallback-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    color: var(--osmoo-metallic);
}

.fallback-companion-silhouette {
    width: 160px;
    height: 220px;
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
}

.fallback-robot-head {
    width: 90px;
    height: 90px;
    border-radius: 9999px;
    background: radial-gradient(circle at 35% 35%, #2a2e3d 0%, #101218 80%);
    border: 1px solid var(--border-machined);
    box-shadow: 0 0 20px rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
}

.fallback-pill-eye {
    width: 10px;
    height: 18px;
    border-radius: 9999px;
    background: #ffffff;
    box-shadow: 0 0 10px #ffffff;
}

.fallback-robot-body {
    width: 100px;
    height: 130px;
    border-radius: 50px 50px 70px 70px;
    background: radial-gradient(circle at 40% 30%, #222533 0%, #0d0e14 80%);
    border: 1px solid var(--border-machined);
    margin-top: 10px;
}

/* ── Onboarding & Settings Modals ────────────────────────────── */
.modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.75);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 24px;
}

.modal-container {
    width: 100%;
    max-width: 760px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
}

.modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 20px 28px;
    border-bottom: 1px solid var(--border-machined);
}

.modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 28px;
}

.modal-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 28px;
    border-top: 1px solid var(--border-machined);
    background: rgba(14, 16, 23, 0.5);
}

/* Form inputs */
.hud-input-field {
    width: 100%;
    background: rgba(14, 16, 23, 0.8);
    border: 1px solid var(--border-machined);
    border-radius: 10px;
    padding: 12px 16px;
    color: var(--text-primary);
    font-size: 13px;
    font-family: var(--font-sans);
    outline: none;
    transition: border-color 0.2s ease;
}

.hud-input-field:focus {
    border-color: var(--border-active);
}

.form-label {
    display: block;
    font-size: 12px;
    font-weight: 600;
    color: var(--osmoo-soft-stone);
    margin-bottom: 8px;
    letter-spacing: 0.04em;
}

.form-help {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 6px;
}
"##;
