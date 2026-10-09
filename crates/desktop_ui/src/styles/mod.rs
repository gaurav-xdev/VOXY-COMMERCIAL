pub const APP_CSS: &str = r#"
:root {
    --bg-primary: #070709;
    --bg-secondary: #0d0d12;
    --bg-tertiary: #14141b;
    --bg-elevated: #1a1a24;
    --bg-hover: #22222e;
    --bg-active: #2b2b3b;
    --text-primary: #ededed;
    --text-secondary: #a1a1aa;
    --text-muted: #52525b;
    --accent-primary: #e4e4e7;
    --accent-secondary: #ffffff;
    --accent-glow: rgba(255, 255, 255, 0.08);
    --border: #1f1f28;
    --border-active: #3f3f46;
    --success: #10b981;
    --warning: #f59e0b;
    --error: #ef4444;
    --info: #71717a;
    --orb-idle: #27272a;
    --orb-listening: #10b981;
    --orb-speaking: #e4e4e7;
    --orb-thinking: #71717a;
    --orb-error: #ef4444;
    --sidebar-width: 280px;
    --header-height: 52px;
    --radius-sm: 4px;
    --radius-md: 8px;
    --radius-lg: 12px;
    --radius-xl: 16px;
    --radius-full: 9999px;
    --shadow-sm: 0 1px 2px rgba(0,0,0,0.5);
    --shadow-md: 0 4px 16px rgba(0,0,0,0.6);
    --shadow-lg: 0 12px 36px rgba(0,0,0,0.8);
    --transition-fast: 0.15s cubic-bezier(0.16, 1, 0.3, 1);
    --transition-normal: 0.3s cubic-bezier(0.16, 1, 0.3, 1);
    --transition-slow: 0.6s cubic-bezier(0.16, 1, 0.3, 1);
}

* {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
}

html, body {
    width: 100%;
    height: 100%;
    overflow: hidden;
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    background: var(--bg-primary);
    color: var(--text-primary);
    font-size: 14px;
    line-height: 1.5;
    -webkit-font-smoothing: antialiased;
}

#main {
    width: 100%;
    height: 100%;
}

::-webkit-scrollbar {
    width: 6px;
    height: 6px;
}
::-webkit-scrollbar-track {
    background: transparent;
}
::-webkit-scrollbar-thumb {
    background: var(--bg-hover);
    border-radius: var(--radius-full);
}
::-webkit-scrollbar-thumb:hover {
    background: var(--bg-active);
}

.app-layout {
    display: flex;
    width: 100%;
    height: 100%;
}

.sidebar {
    width: var(--sidebar-width);
    height: 100%;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    overflow: hidden;
}

.sidebar-header {
    padding: 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 12px;
}

.sidebar-logo {
    width: 32px;
    height: 32px;
    background: var(--accent-primary);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 14px;
    color: white;
}

.sidebar-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
}

.sidebar-version {
    font-size: 11px;
    color: var(--text-muted);
}

.sidebar-nav {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
}

.nav-section {
    margin-bottom: 8px;
}

.nav-section-label {
    padding: 4px 12px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
}

.nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: all var(--transition-fast);
    color: var(--text-secondary);
    font-size: 13px;
    font-weight: 500;
    border: none;
    background: none;
    width: 100%;
    text-align: left;
}

.nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
}

.nav-item.active {
    background: var(--accent-glow);
    color: var(--accent-secondary);
}

.nav-item-icon {
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
}

.sidebar-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--border);
}

.user-info {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px;
    border-radius: var(--radius-md);
    cursor: pointer;
    transition: background var(--transition-fast);
}

.user-info:hover {
    background: var(--bg-hover);
}

.user-avatar {
    width: 32px;
    height: 32px;
    border-radius: var(--radius-full);
    background: var(--bg-tertiary);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    color: var(--text-secondary);
}

.user-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
}

.user-plan {
    font-size: 11px;
    color: var(--text-muted);
}

.main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
}

.content-header {
    height: var(--header-height);
    padding: 0 24px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
}

.content-header h1 {
    font-size: 16px;
    font-weight: 600;
}

.content-body {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
}

.card {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 20px;
    margin-bottom: 16px;
}

.card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
}

.card-title {
    font-size: 15px;
    font-weight: 600;
}

.btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 8px 16px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all var(--transition-fast);
    border: 1px solid transparent;
}

.btn-primary {
    background: var(--accent-primary);
    color: white;
}

.btn-primary:hover {
    background: var(--accent-secondary);
}

.btn-secondary {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border-color: var(--border);
}

.btn-secondary:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
}

.btn-ghost {
    background: transparent;
    color: var(--text-secondary);
}

.btn-ghost:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
}

.btn-danger {
    background: rgba(255, 107, 107, 0.1);
    color: var(--error);
    border-color: rgba(255, 107, 107, 0.2);
}

.btn-danger:hover {
    background: rgba(255, 107, 107, 0.2);
}

.input {
    width: 100%;
    padding: 10px 14px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: 13px;
    transition: border-color var(--transition-fast);
    outline: none;
}

.input:focus {
    border-color: var(--accent-primary);
    box-shadow: 0 0 0 3px var(--accent-glow);
}

.input::placeholder {
    color: var(--text-muted);
}

.toggle {
    position: relative;
    width: 44px;
    height: 24px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-full);
    cursor: pointer;
    transition: background var(--transition-fast);
    border: 1px solid var(--border);
}

.toggle.active {
    background: var(--accent-primary);
    border-color: var(--accent-primary);
}

.toggle::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    background: white;
    border-radius: var(--radius-full);
    transition: transform var(--transition-fast);
}

.toggle.active::after {
    transform: translateX(20px);
}

.status-dot {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-full);
    flex-shrink: 0;
}

.status-dot.healthy { background: var(--success); }
.status-dot.degraded { background: var(--warning); }
.status-dot.unhealthy { background: var(--error); }
.status-dot.unknown { background: var(--text-muted); }

.badge {
    display: inline-flex;
    align-items: center;
    padding: 2px 8px;
    border-radius: var(--radius-full);
    font-size: 11px;
    font-weight: 500;
}

.badge-success { background: rgba(0,210,211,0.15); color: var(--success); }
.badge-warning { background: rgba(254,202,87,0.15); color: var(--warning); }
.badge-error { background: rgba(255,107,107,0.15); color: var(--error); }
.badge-info { background: rgba(84,160,255,0.15); color: var(--info); }

.chat-container {
    display: flex;
    flex-direction: column;
    height: 100%;
}

.chat-messages {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.message {
    display: flex;
    gap: 12px;
    max-width: 80%;
}

.message.user {
    align-self: flex-end;
    flex-direction: row-reverse;
}

.message-avatar {
    width: 32px;
    height: 32px;
    border-radius: var(--radius-full);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    flex-shrink: 0;
}

.message.assistant .message-avatar {
    background: var(--accent-glow);
    color: var(--accent-secondary);
}

.message.user .message-avatar {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
}

.message-bubble {
    padding: 10px 14px;
    border-radius: var(--radius-lg);
    font-size: 13px;
    line-height: 1.6;
}

.message.assistant .message-bubble {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
}

.message.user .message-bubble {
    background: var(--accent-primary);
    color: white;
}

.chat-input-area {
    padding: 16px 24px;
    border-top: 1px solid var(--border);
    display: flex;
    gap: 12px;
    align-items: flex-end;
}

.chat-input-wrapper {
    flex: 1;
    position: relative;
}

.chat-input {
    width: 100%;
    min-height: 44px;
    max-height: 120px;
    padding: 10px 14px;
    padding-right: 48px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    color: var(--text-primary);
    font-size: 13px;
    resize: none;
    outline: none;
    font-family: inherit;
    line-height: 1.5;
}

.chat-input:focus {
    border-color: var(--accent-primary);
}

.send-btn {
    position: absolute;
    right: 8px;
    bottom: 8px;
    width: 32px;
    height: 32px;
    border-radius: var(--radius-md);
    background: var(--accent-primary);
    color: white;
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background var(--transition-fast);
}

.send-btn:hover {
    background: var(--accent-secondary);
}

.voice-controls {
    display: flex;
    gap: 8px;
}

.voice-btn {
    width: 44px;
    height: 44px;
    border-radius: var(--radius-full);
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    transition: all var(--transition-fast);
}

.voice-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
}

.voice-btn.active {
    background: var(--accent-primary);
    color: white;
    border-color: var(--accent-primary);
}

.settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
}

.setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 0;
    border-bottom: 1px solid var(--border);
}

.setting-row:last-child {
    border-bottom: none;
}

.setting-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
}

.setting-desc {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: 2px;
}

.health-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 12px;
}

.health-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
}

.health-card-info {
    flex: 1;
    min-width: 0;
}

.health-card-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
}

.health-card-detail {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 2px;
}

/* ==========================================================================
   OSMOO 3D Volumetric Dynamic Voice Orb & Presence System
   ========================================================================== */

.orb-volumetric-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    position: relative;
    padding: 40px;
    width: 320px;
    height: 320px;
    perspective: 1000px;
}

.orb-radiance-field {
    position: absolute;
    width: 280px;
    height: 280px;
    border-radius: var(--radius-full);
    background: radial-gradient(circle, rgba(99, 102, 241, 0.15) 0%, rgba(16, 185, 129, 0.05) 50%, transparent 70%);
    filter: blur(28px);
    pointer-events: none;
    transition: all 0.8s cubic-bezier(0.16, 1, 0.3, 1);
    z-index: 1;
}

.orb-orbit-ring {
    position: absolute;
    width: 220px;
    height: 220px;
    border-radius: var(--radius-full);
    border: 1px dashed rgba(255, 255, 255, 0.12);
    pointer-events: none;
    animation: rotate-orbit 28s linear infinite;
    z-index: 2;
}

.orb-gyro-ring {
    position: absolute;
    width: 180px;
    height: 180px;
    border-radius: var(--radius-full);
    border: 1px solid rgba(255, 255, 255, 0.08);
    pointer-events: none;
    animation: rotate-orbit-reverse 20s linear infinite;
    z-index: 2;
}

.orb {
    width: 140px;
    height: 140px;
    border-radius: var(--radius-full);
    position: relative;
    cursor: pointer;
    z-index: 5;
    transition: all 0.5s cubic-bezier(0.16, 1, 0.3, 1);
    transform-style: preserve-3d;
    box-shadow: 
        inset 0 0 40px rgba(0, 0, 0, 0.9),
        0 12px 32px rgba(0, 0, 0, 0.7),
        0 0 48px rgba(99, 102, 241, 0.25);
}

.orb:hover {
    transform: scale(1.05);
}

/* Internal Volumetric Orb Components */
.orb-plasma-core {
    position: absolute;
    inset: 12px;
    border-radius: var(--radius-full);
    background: radial-gradient(circle at 35% 35%, rgba(255, 255, 255, 0.3) 0%, rgba(99, 102, 241, 0.4) 40%, rgba(15, 23, 42, 0.9) 100%);
    filter: blur(4px);
    transition: all 0.6s ease;
}

.orb-inner-filament {
    position: absolute;
    inset: 2px;
    border-radius: var(--radius-full);
    background: conic-gradient(from 180deg at 50% 50%, rgba(99, 102, 241, 0.5) 0deg, rgba(16, 185, 129, 0.5) 120deg, rgba(245, 158, 11, 0.4) 240deg, rgba(99, 102, 241, 0.5) 360deg);
    opacity: 0.35;
    animation: filament-spin 12s linear infinite;
    mix-blend-mode: screen;
}

.orb-specular-glint {
    position: absolute;
    top: 14px;
    left: 24px;
    width: 32px;
    height: 18px;
    border-radius: var(--radius-full);
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.7) 0%, rgba(255, 255, 255, 0) 100%);
    transform: rotate(-28deg);
    pointer-events: none;
}

/* Dynamic State Styling */
.orb.state-idle {
    background: radial-gradient(circle at 38% 38%, #27272a 0%, #18181b 60%, #09090b 100%);
    border: 1px solid rgba(255, 255, 255, 0.15);
    box-shadow: 0 0 35px rgba(255, 255, 255, 0.05);
}

.orb.state-listening {
    background: radial-gradient(circle at 38% 38%, #34d399 0%, #059669 50%, #064e3b 100%);
    border: 1px solid rgba(52, 211, 153, 0.6);
    box-shadow: 0 0 60px rgba(16, 185, 129, 0.5), inset 0 0 20px rgba(255, 255, 255, 0.4);
    animation: pulse-listening 1.8s ease-in-out infinite;
}

.orb.state-speaking {
    background: radial-gradient(circle at 38% 38%, #38bdf8 0%, #0284c7 50%, #0c4a6e 100%);
    border: 1px solid rgba(56, 189, 248, 0.7);
    box-shadow: 0 0 65px rgba(14, 165, 233, 0.55), inset 0 0 24px rgba(255, 255, 255, 0.5);
    animation: pulse-speaking 0.9s ease-in-out infinite;
}

.orb.state-thinking {
    background: radial-gradient(circle at 38% 38%, #fbbf24 0%, #d97706 50%, #78350f 100%);
    border: 1px solid rgba(251, 191, 36, 0.6);
    box-shadow: 0 0 55px rgba(245, 158, 11, 0.45);
    animation: thinking-resonance 2.4s ease-in-out infinite;
}

.orb.state-executing {
    background: radial-gradient(circle at 38% 38%, #fb923c 0%, #ea580c 50%, #7c2d12 100%);
    border: 1px solid rgba(251, 146, 60, 0.7);
    box-shadow: 0 0 60px rgba(234, 88, 12, 0.5);
    animation: executing-drive 1.4s linear infinite;
}

.orb.state-waiting {
    background: radial-gradient(circle at 38% 38%, #a78bfa 0%, #7c3aed 50%, #4c1d95 100%);
    border: 1px solid rgba(167, 139, 250, 0.7);
    box-shadow: 0 0 50px rgba(124, 58, 237, 0.45);
    animation: pulse-waiting 1.5s ease-in-out infinite;
}

.orb.state-error {
    background: radial-gradient(circle at 38% 38%, #f87171 0%, #dc2626 50%, #7f1d1d 100%);
    border: 1px solid rgba(248, 113, 113, 0.8);
    box-shadow: 0 0 60px rgba(220, 38, 38, 0.6);
}

/* State Status Pill */
.orb-status-pill-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin-top: 24px;
    z-index: 6;
}

.orb-state-pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px 16px;
    border-radius: var(--radius-full);
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.08em;
    font-weight: 600;
    color: var(--text-primary);
    backdrop-filter: blur(12px);
    transition: all var(--transition-fast);
}

.orb-state-dot {
    width: 6px;
    height: 6px;
    border-radius: var(--radius-full);
    background: var(--accent-primary);
    box-shadow: 0 0 8px var(--accent-primary);
}

.orb-state-subtext {
    font-size: 12px;
    color: var(--text-muted);
    letter-spacing: 0.02em;
}

@keyframes rotate-orbit {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
}

@keyframes rotate-orbit-reverse {
    from { transform: rotate(360deg); }
    to { transform: rotate(0deg); }
}

@keyframes filament-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
}

@keyframes pulse-listening {
    0%, 100% { transform: scale(1); filter: brightness(1); }
    50% { transform: scale(1.08); filter: brightness(1.25); }
}

@keyframes pulse-speaking {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.05); }
}

@keyframes thinking-resonance {
    0%, 100% { transform: rotate(0deg) scale(1); }
    50% { transform: rotate(8deg) scale(1.04); }
}

@keyframes executing-drive {
    0% { transform: scale(1); }
    50% { transform: scale(1.03); }
    100% { transform: scale(1); }
}

@keyframes pulse-waiting {
    0%, 100% { opacity: 0.85; transform: scale(0.98); }
    50% { opacity: 1; transform: scale(1.02); }
}

.empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 60px 24px;
    text-align: center;
}

.empty-state-icon {
    font-size: 48px;
    margin-bottom: 16px;
    opacity: 0.3;
}

.empty-state-title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 8px;
}

.empty-state-desc {
    font-size: 13px;
    color: var(--text-muted);
    max-width: 400px;
}

.download-item {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    margin-bottom: 8px;
}

.download-icon {
    width: 40px;
    height: 40px;
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 18px;
    flex-shrink: 0;
}

.download-info {
    flex: 1;
    min-width: 0;
}

.download-name {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.download-meta {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 2px;
}

.progress-bar {
    width: 100%;
    height: 4px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-full);
    margin-top: 6px;
    overflow: hidden;
}

.progress-fill {
    height: 100%;
    background: var(--accent-primary);
    border-radius: var(--radius-full);
    transition: width var(--transition-normal);
}

.notification-item {
    display: flex;
    gap: 12px;
    padding: 14px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    margin-bottom: 8px;
}

.notification-icon {
    width: 32px;
    height: 32px;
    border-radius: var(--radius-full);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    flex-shrink: 0;
}

.notification-icon.info { background: rgba(84,160,255,0.15); color: var(--info); }
.notification-icon.warning { background: rgba(254,202,87,0.15); color: var(--warning); }
.notification-icon.error { background: rgba(255,107,107,0.15); color: var(--error); }

.notification-content {
    flex: 1;
    min-width: 0;
}

.notification-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-primary);
}

.notification-message {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 2px;
}

.notification-time {
    font-size: 11px;
    color: var(--text-muted);
    margin-top: 4px;
}

.plugin-card {
    padding: 16px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    margin-bottom: 8px;
}

.plugin-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
}

.plugin-name {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
}

.plugin-desc {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
}

.plugin-meta {
    display: flex;
    gap: 12px;
    margin-top: 8px;
    font-size: 11px;
    color: var(--text-muted);
}

.tab-bar {
    display: flex;
    gap: 2px;
    background: var(--bg-tertiary);
    padding: 3px;
    border-radius: var(--radius-md);
    margin-bottom: 16px;
}

.tab {
    flex: 1;
    padding: 8px 16px;
    text-align: center;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all var(--transition-fast);
    border: none;
    background: none;
}

.tab:hover {
    color: var(--text-secondary);
}

.tab.active {
    background: var(--bg-secondary);
    color: var(--text-primary);
    box-shadow: var(--shadow-sm);
}

.toast-container {
    position: fixed;
    bottom: 24px;
    right: 24px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 1000;
}

.toast {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    background: var(--bg-elevated);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    animation: slide-in 0.3s ease;
    min-width: 280px;
}

@keyframes slide-in {
    from { transform: translateX(100%); opacity: 0; }
    to { transform: translateX(0); opacity: 1; }
}

/* ==========================================================================
   OSMOO COMPUTATIONAL CORE — SPATIAL 3D GIMBAL (SNOW BLACK AESTHETIC)
   ========================================================================== */

.osmoo-spatial-stage {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    perspective: 1200px;
    background: radial-gradient(circle at center, #0f0f15 0%, #070709 75%);
    overflow: hidden;
    cursor: pointer;
    user-select: none;
}

.core-ambient-plane {
    position: absolute;
    width: 600px;
    height: 600px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(255, 255, 255, 0.03) 0%, transparent 70%);
    pointer-events: none;
    transition: opacity 0.8s ease;
}

.core-gimbal-assembly {
    position: relative;
    width: 480px;
    height: 480px;
    display: flex;
    align-items: center;
    justify-content: center;
    transform-style: preserve-3d;
}

.core-ring {
    position: absolute;
    border-radius: 50%;
    border: 1px solid rgba(228, 228, 231, 0.22);
    box-sizing: border-box;
    transform-style: preserve-3d;
    transition: border-color 0.5s ease;
}

/* Alpha Ring: 460px */
.ring-alpha {
    width: 460px;
    height: 460px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    box-shadow: inset 0 0 15px rgba(255, 255, 255, 0.02);
    transform: rotateX(68deg) rotateY(15deg);
    animation: spin-alpha 32s cubic-bezier(0.45, 0.05, 0.55, 0.95) infinite;
}

/* Beta Ring: 360px */
.ring-beta {
    width: 360px;
    height: 360px;
    border: 1px dashed rgba(228, 228, 231, 0.22);
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.5);
    transform: rotateX(-54deg) rotateY(-20deg);
    animation: spin-beta 22s cubic-bezier(0.4, 0.0, 0.6, 1.0) infinite;
}

/* Gamma Ring: 270px */
.ring-gamma {
    width: 270px;
    height: 270px;
    border: 1px solid rgba(255, 255, 255, 0.28);
    transform: rotateX(35deg) rotateY(40deg);
    animation: spin-gamma 14s linear infinite;
}

/* Delta Ring: 190px */
.ring-delta {
    width: 190px;
    height: 190px;
    border: 1px dotted rgba(255, 255, 255, 0.35);
    transform: rotateX(-20deg) rotateY(60deg);
    animation: spin-delta 9s linear infinite;
}

/* Ring Coordinate Ticks */
.ring-tick {
    position: absolute;
    width: 6px;
    height: 2px;
    background: #ffffff;
}
.tick-0 { top: 0; left: 50%; transform: translateX(-50%); }
.tick-90 { top: 50%; right: 0; transform: translateY(-50%); }
.tick-180 { bottom: 0; left: 50%; transform: translateX(-50%); }
.tick-270 { top: 50%; left: 0; transform: translateY(-50%); }

.ring-subtick {
    position: absolute;
    width: 3px;
    height: 1px;
    background: rgba(255, 255, 255, 0.6);
}
.subtick-45 { top: 14%; right: 14%; }
.subtick-135 { bottom: 14%; right: 14%; }
.subtick-225 { bottom: 14%; left: 14%; }
.subtick-315 { top: 14%; left: 14%; }
.subtick-30 { top: 6.7%; right: 25%; }
.subtick-150 { bottom: 6.7%; right: 25%; }
.subtick-210 { bottom: 6.7%; left: 25%; }
.subtick-330 { top: 6.7%; left: 25%; }

.core-volumetric-haze {
    position: absolute;
    width: 320px;
    height: 320px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(255, 255, 255, 0.02) 0%, transparent 65%);
    pointer-events: none;
    filter: blur(12px);
}

/* Compact Overlay Presentation Mode */
.osmoo-spatial-stage.compact {
    background: transparent !important;
}

.osmoo-spatial-stage.compact .core-gimbal-assembly {
    transform: scale(0.48);
}

.osmoo-spatial-stage.compact .core-ambient-plane {
    display: none;
}

/* Monolithic Nucleus */
.core-nucleus {
    position: absolute;
    width: 110px;
    height: 110px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: radial-gradient(circle at 35% 35%, #1f202b 0%, #0d0d14 60%, #050508 100%);
    border: 1px solid rgba(255, 255, 255, 0.4);
    box-shadow: 0 0 45px rgba(0, 0, 0, 0.95), inset 0 0 25px rgba(255, 255, 255, 0.07);
    transition: transform 0.4s ease, border-color 0.4s ease;
}

.nucleus-lens-outer {
    position: absolute;
    width: 95px;
    height: 95px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.12);
    box-shadow: inset 0 0 15px rgba(0, 0, 0, 0.8);
}

.nucleus-shell {
    position: absolute;
    width: 75px;
    height: 75px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.22);
    animation: pulse-nucleus 6s ease-in-out infinite;
}

.nucleus-lattice {
    position: absolute;
    width: 54px;
    height: 54px;
    border-radius: 50%;
    border: 1px dashed rgba(255, 255, 255, 0.18);
    opacity: 0.6;
}

.nucleus-emitter {
    position: absolute;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(255, 255, 255, 0.65) 0%, transparent 80%);
    opacity: 0.25;
    transition: opacity 0.3s ease;
}

.nucleus-singularity {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 0 14px rgba(255, 255, 255, 0.85);
    transition: all 0.3s ease;
}

.nucleus-lens-reflection {
    position: absolute;
    top: 14px;
    left: 20px;
    width: 28px;
    height: 14px;
    border-radius: 50%;
    background: linear-gradient(135deg, rgba(255, 255, 255, 0.25) 0%, transparent 80%);
    transform: rotate(-25deg);
    pointer-events: none;
}

/* Minimalist Telemetry */
.core-telemetry {
    position: absolute;
    bottom: 50px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 16px;
    border-radius: 999px;
    background: rgba(13, 13, 18, 0.85);
    border: 1px solid rgba(255, 255, 255, 0.1);
    backdrop-filter: blur(8px);
}

.telemetry-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #52525b;
    transition: background 0.3s ease;
}

.telemetry-label {
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.12em;
    color: #a1a1aa;
    text-transform: uppercase;
}

/* State Transformations */
.core-state-idle .telemetry-dot { background: #71717a; }

.core-state-listening .ring-gamma {
    border-color: rgba(16, 185, 129, 0.8);
    animation-duration: 6s;
}
.core-state-listening .nucleus-emitter {
    opacity: 0.6;
    background: radial-gradient(circle, rgba(16, 185, 129, 0.6) 0%, transparent 80%);
}
.core-state-listening .telemetry-dot { background: #10b981; }

.core-state-thinking .ring-beta {
    border-color: rgba(255, 255, 255, 0.7);
    animation-duration: 8s;
}
.core-state-thinking .ring-delta {
    animation-duration: 3s;
}
.core-state-thinking .nucleus-shell {
    animation-duration: 1.5s;
}
.core-state-thinking .telemetry-dot { background: #ffffff; }

.core-state-speaking .ring-alpha {
    animation-duration: 18s;
}
.core-state-speaking .nucleus-emitter {
    opacity: 0.8;
}
.core-state-speaking .telemetry-dot { background: #e4e4e7; }

.core-state-executing .ring-gamma {
    border-color: rgba(245, 158, 11, 0.8);
    animation-duration: 4s;
}
.core-state-executing .telemetry-dot { background: #f59e0b; }

.core-state-error .core-ring {
    border-color: rgba(239, 68, 68, 0.4);
}
.core-state-error .telemetry-dot { background: #ef4444; }

/* Gimbal Rotations */
@keyframes spin-alpha {
    from { transform: rotateX(68deg) rotateY(15deg) rotateZ(0deg); }
    to { transform: rotateX(68deg) rotateY(15deg) rotateZ(360deg); }
}

@keyframes spin-beta {
    from { transform: rotateX(-54deg) rotateY(-20deg) rotateZ(0deg); }
    to { transform: rotateX(-54deg) rotateY(-20deg) rotateZ(-360deg); }
}

@keyframes spin-gamma {
    from { transform: rotateX(35deg) rotateY(40deg) rotateZ(0deg); }
    to { transform: rotateX(35deg) rotateY(40deg) rotateZ(360deg); }
}

@keyframes spin-delta {
    from { transform: rotateX(-20deg) rotateY(60deg) rotateZ(0deg); }
    to { transform: rotateX(-20deg) rotateY(60deg) rotateZ(-360deg); }
}

@keyframes pulse-nucleus {
    0%, 100% { transform: scale(1); opacity: 0.4; }
    50% { transform: scale(1.12); opacity: 0.8; }
}

/* ==========================================================================
   OSMOO CINEMATIC STARTUP SEQUENCE (NO AI SLOP — PURE RESTRAINT)
   ========================================================================== */

.cinematic-viewport {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: #040405;
    z-index: 99999;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 1;
    transition: opacity 0.6s cubic-bezier(0.16, 1, 0.3, 1);
}

.cinematic-viewport.fade-exit {
    opacity: 0;
    pointer-events: none;
}

.cinematic-brand-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 16px;
    user-select: none;
}

.osmoo-brand-mark {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    opacity: 0;
    transform: translateY(8px) scale(0.98);
    transition: opacity 1.2s cubic-bezier(0.16, 1, 0.3, 1), transform 1.2s cubic-bezier(0.16, 1, 0.3, 1);
}

.osmoo-brand-mark.visible {
    opacity: 1;
    transform: translateY(0) scale(1);
}

.brand-mark-glyph {
    position: relative;
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
}

.mark-orbit-ring {
    position: absolute;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    border: 1.5px solid rgba(255, 255, 255, 0.45);
    box-shadow: 0 0 16px rgba(255, 255, 255, 0.1);
}

.mark-core-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 0 10px rgba(255, 255, 255, 0.9);
}

.brand-wordmark {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: 0.28em;
    color: #ededed;
    margin-right: -0.28em; /* Offset letter-spacing centering */
    text-transform: uppercase;
}

.osmiora-parent-label {
    font-size: 11px;
    font-weight: 400;
    letter-spacing: 0.22em;
    color: #52525b;
    margin-right: -0.22em;
    text-transform: uppercase;
    opacity: 0;
    transform: translateY(4px);
    transition: opacity 1s cubic-bezier(0.16, 1, 0.3, 1), transform 1s cubic-bezier(0.16, 1, 0.3, 1);
}

.osmiora-parent-label.visible {
    opacity: 1;
    transform: translateY(0);
}

/* ==========================================================================
   OSMOO AUTHENTICATION & ACCESS PANEL (MONOLITHIC SNOW BLACK)
   ========================================================================== */

.osmoo-auth-viewport {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: radial-gradient(circle at center, #0d0d12 0%, #070709 80%);
    padding: 24px;
}

.osmoo-monolithic-panel {
    width: 440px;
    background: #0d0d12;
    border: 1px solid #1f1f28;
    border-radius: 12px;
    padding: 36px 32px;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.7);
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
}

.panel-header-mark {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin-bottom: 28px;
}

.glyph-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 0 10px rgba(255, 255, 255, 0.6);
    margin-bottom: 6px;
}

.glyph-title {
    font-size: 20px;
    font-weight: 600;
    letter-spacing: 0.24em;
    color: #ededed;
    margin-right: -0.24em;
}

.glyph-subtitle {
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.16em;
    color: #71717a;
    text-transform: uppercase;
}

.auth-mode-indicator {
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.12em;
    color: #a1a1aa;
    margin-bottom: 24px;
}

.field-group {
    width: 100%;
    margin-bottom: 16px;
    text-align: left;
}

.field-label {
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.12em;
    color: #71717a;
    margin-bottom: 6px;
}

.auth-status-card {
    width: 100%;
    background: #14141b;
    border: 1px solid #1f1f28;
    border-radius: 8px;
    padding: 18px 16px;
    text-align: left;
}

.session-caption {
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.14em;
    color: #71717a;
    margin-bottom: 4px;
}

.session-principal {
    font-size: 15px;
    font-weight: 500;
    color: #ededed;
    word-break: break-all;
}

.session-badge {
    display: inline-block;
    margin-top: 10px;
    padding: 3px 8px;
    background: #1f1f28;
    border-radius: 4px;
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.1em;
    color: #a1a1aa;
}

.feedback-msg {
    width: 100%;
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 12px;
    margin-bottom: 12px;
    text-align: left;
}

.feedback-msg.error {
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.25);
    color: #ef4444;
}

.feedback-msg.success {
    background: rgba(16, 185, 129, 0.1);
    border: 1px solid rgba(16, 185, 129, 0.25);
    color: #10b981;
}

.auth-toggle-row {
    margin-top: 20px;
    font-size: 12px;
    color: #71717a;
}

.auth-toggle-btn {
    color: #ededed;
    cursor: pointer;
    font-weight: 500;
    margin-left: 4px;
    text-decoration: underline;
}

.panel-parent-footnote {
    margin-top: 28px;
    font-size: 9px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.18em;
    color: #3f3f46;
}

/* ==========================================================================
   OSMOO GUIDED ONBOARDING SYSTEM (NO AI SLOP — STEPPED DISCLOSURE)
   ========================================================================== */

.onboarding-viewport {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: radial-gradient(circle at center, #0f0f15 0%, #070709 85%);
    padding: 32px;
}

.onboarding-container {
    width: 580px;
    background: #0d0d12;
    border: 1px solid #1f1f28;
    border-radius: 14px;
    padding: 36px 36px 32px 36px;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.75);
    display: flex;
    flex-direction: column;
}

.onboarding-progress-bar {
    display: flex;
    gap: 8px;
    margin-bottom: 28px;
}

.step-pip {
    flex: 1;
    height: 3px;
    border-radius: 2px;
    background: #1f1f28;
    transition: background 0.4s ease;
}

.step-pip.active {
    background: #ededed;
}

.step-pip.passed {
    background: #3f3f46;
}

.step-pane {
    display: flex;
    flex-direction: column;
}

.step-header {
    margin-bottom: 24px;
}

.step-category {
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.16em;
    color: #71717a;
    margin-bottom: 6px;
}

.step-title {
    font-size: 18px;
    font-weight: 600;
    color: #ededed;
    margin-bottom: 6px;
}

.step-desc {
    font-size: 12px;
    color: #a1a1aa;
    line-height: 1.5;
}

.step-form-block {
    margin-bottom: 24px;
}

.step-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    margin-top: 28px;
    padding-top: 20px;
    border-top: 1px solid #1f1f28;
}

.wake-word-list {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 12px;
}

.wake-word-chip {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    background: #14141b;
    border: 1px solid #27272a;
    border-radius: 6px;
    font-size: 12px;
    color: #ededed;
}

.chip-remove-btn {
    background: none;
    border: none;
    color: #71717a;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
}

.chip-remove-btn:hover {
    color: #ef4444;
}

.provider-config-stack {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 280px;
    overflow-y: auto;
    padding-right: 4px;
}

.provider-row {
    background: #14141b;
    border: 1px solid #1f1f28;
    border-radius: 8px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
}

.provider-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.provider-name {
    font-size: 12px;
    font-weight: 500;
    color: #ededed;
}

.provider-tag {
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    color: #71717a;
}

.provider-key-input {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    font-size: 12px;
    letter-spacing: 0.08em;
}

.empty-local-box {
    padding: 24px;
    background: #14141b;
    border: 1px dashed #27272a;
    border-radius: 8px;
    text-align: center;
}

.voice-grid-setup {
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.voice-panel-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
}

/* ==========================================================================
   OSMOO HUD OVERLAYS & SLIDE-IN COMPANION DRAWER
   ========================================================================== */

.hud-topbar {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 56px;
    padding: 0 24px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    z-index: 50;
    pointer-events: none;
}

.hud-brand {
    display: flex;
    align-items: center;
    gap: 12px;
    pointer-events: auto;
}

.hud-logo {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 1px solid rgba(255, 255, 255, 0.35);
    background: radial-gradient(circle, #27272a 0%, #0d0d12 100%);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #ededed;
}

.hud-title {
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.18em;
    color: #ededed;
    text-transform: uppercase;
}

.hud-parent {
    font-size: 9px;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    letter-spacing: 0.14em;
    color: #52525b;
}

.hud-controls {
    display: flex;
    align-items: center;
    gap: 10px;
    pointer-events: auto;
}

.hud-btn {
    background: rgba(13, 13, 18, 0.75);
    border: 1px solid #1f1f28;
    color: #a1a1aa;
    padding: 6px 14px;
    border-radius: 8px;
    font-size: 11px;
    font-weight: 500;
    letter-spacing: 0.06em;
    cursor: pointer;
    backdrop-filter: blur(12px);
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    gap: 8px;
}

.hud-btn:hover {
    background: #14141b;
    border-color: #3f3f46;
    color: #ededed;
}

.hud-btn.active {
    background: #1a1a24;
    border-color: #52525b;
    color: #ffffff;
}

/* Bottom Command Prompt Bar */
.hud-prompt-bar {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    width: min(640px, 90vw);
    z-index: 50;
    display: flex;
    align-items: center;
    background: rgba(13, 13, 18, 0.88);
    border: 1px solid #1f1f28;
    border-radius: 12px;
    padding: 6px 8px 6px 16px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(16px);
    transition: border-color 0.2s ease, box-shadow 0.2s ease;
}

.hud-prompt-bar:focus-within {
    border-color: #3f3f46;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.85);
}

.hud-prompt-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #ededed;
    font-size: 13px;
    font-family: inherit;
}

.hud-prompt-input::placeholder {
    color: #52525b;
}

.hud-send-btn {
    background: #1a1a24;
    border: 1px solid #27272a;
    color: #ededed;
    padding: 6px 14px;
    border-radius: 8px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s ease;
}

.hud-send-btn:hover {
    background: #27272a;
    border-color: #3f3f46;
}

/* Sidebar Backdrop and Drawer */
.sidebar-overlay-backdrop {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(4px);
    z-index: 100;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.3s cubic-bezier(0.16, 1, 0.3, 1);
}

.sidebar-overlay-backdrop.open {
    opacity: 1;
    pointer-events: auto;
}

.sidebar-drawer {
    position: fixed;
    top: 0;
    left: 0;
    width: var(--sidebar-width);
    height: 100vh;
    background: #0d0d12;
    border-right: 1px solid #1f1f28;
    z-index: 101;
    transform: translateX(-100%);
    transition: transform 0.35s cubic-bezier(0.16, 1, 0.3, 1);
    box-shadow: 20px 0 50px rgba(0, 0, 0, 0.85);
    display: flex;
    flex-direction: column;
}

.sidebar-drawer.open {
    transform: translateX(0);
}

.sidebar-close-btn {
    background: transparent;
    border: none;
    color: #71717a;
    font-size: 16px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
    transition: color 0.15s ease;
}

.sidebar-close-btn:hover {
    color: #ededed;
}

.active-view-container {
    position: fixed;
    top: 56px;
    left: 0;
    right: 0;
    bottom: 0;
    background: #070709;
    z-index: 40;
    overflow-y: auto;
    padding: 24px;
}

/* ── Approval & Governance Modal Styles ── */
.approval-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(4, 4, 6, 0.82);
    backdrop-filter: blur(8px);
    z-index: 99999;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
}

.approval-modal {
    width: 100%;
    max-width: 580px;
    background: #0d0d12;
    border: 1px solid #272733;
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.9), 0 0 1px rgba(255, 255, 255, 0.1);
    overflow: hidden;
    animation: approvalPop 0.2s cubic-bezier(0.16, 1, 0.3, 1);
}

@keyframes approvalPop {
    from { opacity: 0; transform: scale(0.96) translateY(8px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
}

.approval-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 24px;
    background: #12121a;
    border-bottom: 1px solid #1f1f2a;
}

.approval-header-left {
    display: flex;
    align-items: center;
    gap: 14px;
}

.approval-shield-icon {
    font-size: 20px;
    color: #f59e0b;
}

.approval-title {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: #ededed;
}

.approval-subtitle {
    font-size: 11px;
    color: #71717a;
    margin-top: 1px;
}

.approval-risk-tag {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 4px 10px;
    border-radius: 4px;
    text-transform: uppercase;
}

.risk-badge-destructive {
    background: rgba(239, 68, 68, 0.15);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.4);
}

.risk-badge-privileged {
    background: rgba(245, 158, 11, 0.15);
    color: #f59e0b;
    border: 1px solid rgba(245, 158, 11, 0.4);
}

.risk-badge-modify {
    background: rgba(59, 130, 246, 0.15);
    color: #60a5fa;
    border: 1px solid rgba(59, 130, 246, 0.4);
}

.risk-badge-low, .risk-badge-safe {
    background: rgba(16, 185, 129, 0.15);
    color: #10b981;
    border: 1px solid rgba(16, 185, 129, 0.4);
}

.approval-body {
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
}

.approval-meta-row {
    display: flex;
    align-items: center;
    gap: 16px;
}

.approval-meta-label {
    font-size: 12px;
    font-weight: 600;
    color: #71717a;
    width: 90px;
    flex-shrink: 0;
}

.approval-meta-value {
    font-size: 13px;
    color: #d4d4d8;
}

.approval-meta-value.tool-badge {
    font-family: monospace;
    font-size: 12px;
    background: #181822;
    padding: 3px 8px;
    border-radius: 4px;
    border: 1px solid #272736;
    color: #e4e4e7;
}

.approval-params-section {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
}

.approval-params-title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: #71717a;
    text-transform: uppercase;
}

.approval-params-code {
    background: #07070a;
    border: 1px solid #1c1c26;
    border-radius: 6px;
    padding: 12px 14px;
    font-family: Consolas, 'Courier New', monospace;
    font-size: 12px;
    color: #a1a1aa;
    max-height: 140px;
    overflow-y: auto;
    white-space: pre-wrap;
    word-break: break-all;
}

.approval-footer {
    padding: 16px 24px;
    background: #111118;
    border-top: 1px solid #1f1f2a;
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.approval-footer-actions {
    display: flex;
    align-items: center;
    gap: 10px;
}

.approval-btn-stop {
    background: transparent;
    border: 1px solid rgba(239, 68, 68, 0.4);
    color: #ef4444;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    padding: 8px 14px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
}

.approval-btn-stop:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: #ef4444;
}

.approval-btn-deny {
    background: #1a1a24;
    border: 1px solid #2d2d3d;
    color: #a1a1aa;
    font-size: 12px;
    font-weight: 600;
    padding: 8px 16px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
}

.approval-btn-deny:hover {
    background: #252533;
    color: #ededed;
}

.approval-btn-approve {
    background: #ededed;
    border: 1px solid #ffffff;
    color: #070709;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.04em;
    padding: 8px 20px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
}

.approval-btn-approve:hover {
    background: #ffffff;
    box-shadow: 0 0 16px rgba(255, 255, 255, 0.25);
}
"#;
