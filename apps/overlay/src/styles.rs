pub const GLOBAL_STYLES: &str = r#"
<style>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500;600;700&display=swap');

:root {
    --bg-base: #06090f;
    --bg-surface: rgba(11, 17, 29, 0.72);
    --bg-surface-elevated: rgba(17, 26, 44, 0.82);
    --bg-glass: rgba(14, 23, 38, 0.6);
    --border-subtle: rgba(255, 255, 255, 0.08);
    --border-active: rgba(56, 189, 248, 0.35);
    --border-accent: rgba(56, 189, 248, 0.2);
    --text-primary: #f8fafc;
    --text-secondary: #94a3b8;
    --text-muted: #64748b;
    --accent-core: #38bdf8;
    --accent-emerald: #10b981;
    --accent-amber: #f59e0b;
    --accent-crimson: #ef4444;
    --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    --font-mono: 'JetBrains Mono', monospace;
    --ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);
}

* {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    user-select: none;
    -webkit-font-smoothing: antialiased;
}

html, body {
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    background: transparent !important;
    font-family: var(--font-sans);
    color: var(--text-primary);
    pointer-events: none;
}

.clickable {
    pointer-events: auto !important;
}

/* Base Root Container */
.voxy-app-root {
    position: relative;
    width: 100vw;
    height: 100vh;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: center;
    padding: 24px 36px;
    background: radial-gradient(circle at 50% 50%, rgba(6, 12, 24, 0.4) 0%, rgba(3, 7, 14, 0.78) 100%);
    transition: opacity 0.35s var(--ease-out-expo), background 0.35s ease;
}

.voxy-app-root.collapsed {
    background: transparent !important;
}

/* ==========================================================================
   LAYER 1: PRESENCE — THE LIVING CORE ENTITY
   ========================================================================== */
.presence-container {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 380px;
    height: 380px;
    display: flex;
    justify-content: center;
    align-items: center;
    cursor: pointer;
    transition: all 0.45s var(--ease-out-expo);
}

.voxy-app-root.collapsed .presence-container {
    width: 130px;
    height: 130px;
    transform: translate(-50%, -50%) scale(0.72);
    opacity: 0.88;
}

.presence-canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
}

.presence-halo {
    position: absolute;
    width: 240px;
    height: 240px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(56, 189, 248, 0.12) 0%, rgba(56, 189, 248, 0) 70%);
    transition: all 0.5s ease;
    pointer-events: none;
    animation: gentleBreathe 6s ease-in-out infinite;
}

.state-listening .presence-halo {
    background: radial-gradient(circle, rgba(16, 185, 129, 0.22) 0%, rgba(16, 185, 129, 0) 70%);
    transform: scale(1.15);
}

.state-thinking .presence-halo {
    background: radial-gradient(circle, rgba(245, 158, 11, 0.22) 0%, rgba(245, 158, 11, 0) 70%);
    transform: scale(1.1);
}

.state-speaking .presence-halo {
    background: radial-gradient(circle, rgba(14, 165, 233, 0.28) 0%, rgba(14, 165, 233, 0) 70%);
}

.state-executing .presence-halo {
    background: radial-gradient(circle, rgba(249, 115, 22, 0.25) 0%, rgba(249, 115, 22, 0) 70%);
}

.state-error .presence-halo {
    background: radial-gradient(circle, rgba(239, 68, 68, 0.28) 0%, rgba(239, 68, 68, 0) 70%);
}

@keyframes gentleBreathe {
    0%, 100% { transform: scale(0.96); opacity: 0.7; }
    50% { transform: scale(1.04); opacity: 1; }
}

.core-badge-container {
    position: absolute;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    pointer-events: none;
    z-index: 2;
}

.core-title {
    font-family: var(--font-sans);
    font-size: 19px;
    font-weight: 700;
    letter-spacing: 2px;
    color: var(--text-primary);
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.8);
    transition: font-size 0.3s ease;
}

.voxy-app-root.collapsed .core-title {
    font-size: 13px;
    letter-spacing: 1px;
}

.core-subtitle {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 1.5px;
    color: var(--text-secondary);
    margin-top: 3px;
    text-transform: uppercase;
}

.voxy-app-root.collapsed .core-subtitle {
    display: none;
}

.state-pill {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 1.5px;
    padding: 3px 10px;
    border-radius: 9999px;
    margin-top: 8px;
    transition: all 0.3s ease;
    border: 1px solid var(--border-subtle);
    background: var(--bg-surface);
}

/* ==========================================================================
   LAYER 2: CONTEXT — STATUS & LIVE FEEDBACK
   ========================================================================== */
.top-nav-bar {
    width: 100%;
    max-width: 1320px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 18px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    backdrop-filter: blur(24px) saturate(180%);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
    transition: all 0.4s var(--ease-out-expo);
}

.voxy-app-root.collapsed .top-nav-bar {
    opacity: 0;
    transform: translateY(-24px);
    pointer-events: none;
}

.nav-brand {
    display: flex;
    align-items: center;
    gap: 10px;
}

.brand-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent-core);
    box-shadow: 0 0 8px var(--accent-core);
}

.brand-label {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 1.5px;
    color: var(--text-primary);
}

.nav-metrics {
    display: flex;
    align-items: center;
    gap: 10px;
}

.metric-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    padding: 4px 10px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-subtle);
}

.sensor-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 600;
    cursor: pointer;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    transition: all 0.2s ease;
}

.sensor-toggle.active {
    color: var(--accent-emerald);
    border-color: rgba(16, 185, 129, 0.3);
    background: rgba(16, 185, 129, 0.08);
}

.sensor-toggle:hover {
    background: rgba(255, 255, 255, 0.08);
}

/* Bottom Contextual Subtitle & Action Bar */
.bottom-context-bar {
    width: 100%;
    max-width: 1320px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: all 0.4s var(--ease-out-expo);
}

.voxy-app-root.collapsed .bottom-context-bar {
    opacity: 0;
    transform: translateY(24px);
    pointer-events: none;
}

.live-transcript-strip {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 18px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    backdrop-filter: blur(24px) saturate(180%);
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.45);
}

.transcript-tag {
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 1px;
    padding: 3px 8px;
    border-radius: 4px;
    background: rgba(56, 189, 248, 0.1);
    color: var(--accent-core);
    border: 1px solid rgba(56, 189, 248, 0.25);
    white-space: nowrap;
}

.transcript-text {
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-primary);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

/* ==========================================================================
   COMPUTER CONTROL & EMERGENCY STOP BANNER
   ========================================================================== */
.computer-control-banner {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 18px;
    background: rgba(249, 115, 22, 0.12);
    border: 1px solid rgba(249, 115, 22, 0.35);
    border-radius: 10px;
    backdrop-filter: blur(20px);
}

.control-target-info {
    display: flex;
    align-items: center;
    gap: 12px;
}

.control-pulse {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent-amber);
    box-shadow: 0 0 10px var(--accent-amber);
    animation: pulseControl 1.2s infinite;
}

@keyframes pulseControl {
    0%, 100% { opacity: 0.8; transform: scale(1); }
    50% { opacity: 1; transform: scale(1.3); }
}

.control-title {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 1px;
    color: #fed7aa;
}

.control-action {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-primary);
}

.emergency-stop-btn {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 1.5px;
    padding: 7px 18px;
    background: #dc2626;
    color: #ffffff;
    border: 1px solid #ef4444;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
    box-shadow: 0 0 16px rgba(220, 38, 38, 0.5);
    transition: all 0.2s ease;
}

.emergency-stop-btn:hover {
    background: #ef4444;
    box-shadow: 0 0 24px rgba(239, 68, 68, 0.8);
    transform: scale(1.02);
}

/* ==========================================================================
   LAYER 3: DEPTH — EXPANDABLE DRAWER & SECONDARY SURFACE
   ========================================================================== */
.drawer-toggle-btn {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 6px 14px;
    border-radius: 6px;
    background: var(--bg-surface);
    border: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.2s ease;
}

.drawer-toggle-btn:hover {
    background: var(--bg-surface-elevated);
    color: var(--text-primary);
    border-color: var(--border-active);
}

.depth-drawer {
    position: fixed;
    right: 24px;
    top: 80px;
    bottom: 80px;
    width: 440px;
    background: var(--bg-surface-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 16px;
    backdrop-filter: blur(32px) saturate(190%);
    box-shadow: 0 12px 48px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    z-index: 100;
    transform: translateX(480px);
    transition: transform 0.4s var(--ease-out-expo), opacity 0.3s ease;
    opacity: 0;
}

.depth-drawer.open {
    transform: translateX(0);
    opacity: 1;
}

.drawer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-subtle);
}

.drawer-tabs {
    display: flex;
    gap: 8px;
    padding: 10px 16px;
    background: rgba(0, 0, 0, 0.2);
    border-bottom: 1px solid var(--border-subtle);
}

.drawer-tab {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 6px 12px;
    border-radius: 6px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.2s ease;
}

.drawer-tab.active {
    background: rgba(255, 255, 255, 0.08);
    border-color: var(--border-subtle);
    color: var(--text-primary);
    font-weight: 600;
}

.drawer-tab:hover:not(.active) {
    color: var(--text-primary);
}

.drawer-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
}

.drawer-body::-webkit-scrollbar {
    width: 6px;
}

.drawer-body::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.1);
    border-radius: 3px;
}

/* Chat Log in Drawer */
.chat-bubble {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 14px;
    border-radius: 10px;
    font-size: 12px;
    line-height: 1.5;
}

.chat-bubble.user {
    align-self: flex-end;
    background: rgba(56, 189, 248, 0.12);
    border: 1px solid rgba(56, 189, 248, 0.25);
    color: #e0f2fe;
    max-width: 85%;
}

.chat-bubble.voxy {
    align-self: flex-start;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-subtle);
    color: var(--text-primary);
    max-width: 90%;
}

.chat-sender {
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 1px;
    color: var(--text-secondary);
}

/* Telemetry Rows */
.telemetry-card {
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border-subtle);
    border-radius: 8px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.telemetry-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-family: var(--font-mono);
    font-size: 11px;
}

.telemetry-key {
    color: var(--text-secondary);
}

.telemetry-val {
    color: var(--text-primary);
    font-weight: 500;
}

.telemetry-val.highlight {
    color: var(--accent-core);
}

/* Tool execution timeline */
.tool-timeline-item {
    display: flex;
    gap: 12px;
    padding: 10px;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border-subtle);
}

.tool-status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    margin-top: 4px;
}

.status-completed { background: var(--accent-emerald); box-shadow: 0 0 6px var(--accent-emerald); }
.status-running { background: var(--accent-amber); box-shadow: 0 0 6px var(--accent-amber); animation: pulseControl 1s infinite; }
.status-pending { background: var(--text-muted); }
.status-failed { background: var(--accent-crimson); box-shadow: 0 0 6px var(--accent-crimson); }

/* ==========================================================================
   DESKTOP MODES: COMPACT, FLOATING, EDGE DOCK, MINIMAL
   ========================================================================== */
.mode-pill-selector {
    display: flex;
    gap: 4px;
    background: rgba(0, 0, 0, 0.3);
    padding: 3px;
    border-radius: 8px;
    border: 1px solid var(--border-subtle);
}

.mode-btn {
    font-family: var(--font-mono);
    font-size: 10px;
    padding: 4px 10px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition: all 0.2s ease;
}

.mode-btn.active {
    background: rgba(255, 255, 255, 0.1);
    color: var(--text-primary);
    font-weight: 600;
}

/* Compact Mode (Floating Capsule) */
.compact-capsule {
    position: fixed;
    top: 24px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 18px;
    background: var(--bg-surface-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 9999px;
    backdrop-filter: blur(28px) saturate(180%);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    z-index: 200;
}

/* Edge Dock (Pinned to Bottom) */
.edge-dock {
    position: fixed;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 8px 20px;
    background: var(--bg-surface-elevated);
    border: 1px solid var(--border-subtle);
    border-radius: 14px;
    backdrop-filter: blur(28px);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.6);
    z-index: 200;
}

/* Minimal Mode */
.minimal-orb {
    position: fixed;
    top: 20px;
    right: 20px;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--bg-surface-elevated);
    border: 1px solid var(--border-subtle);
    backdrop-filter: blur(20px);
    display: flex;
    justify-content: center;
    align-items: center;
    cursor: pointer;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    z-index: 200;
}
</style>
"#;
