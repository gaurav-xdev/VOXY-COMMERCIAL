pub const GLOBAL_STYLES: &str = r#"
<style>
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@500;700&display=swap');

:root {
    --cat-size: 1.0;
    --cat-opacity: 1.0;
    --font-sans: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    --font-mono: 'JetBrains Mono', monospace;
    --ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);
    --status-idle: #94a3b8;
    --status-listening: #10b981;
    --status-thinking: #f59e0b;
    --status-speaking: #38bdf8;
    --status-executing: #a855f7;
    --status-success: #22c55e;
    --status-error: #ef4444;
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
    pointer-events: none;
}

.voxy-app-root {
    position: relative;
    width: 100vw;
    height: 100vh;
    background: transparent !important;
    overflow: hidden;
    pointer-events: none;
}

/* Screen awareness auto-hide transition */
.voxy-app-root.suppressed {
    opacity: 0 !important;
    pointer-events: none !important;
    transition: opacity 0.4s var(--ease-out-expo);
}

/* ==========================================================================
   OSMOO 2D RIGGED ANIMATED CAT DESKTOP COMPANION
   ========================================================================== */

#osmoo-cat-companion {
    position: absolute;
    bottom: 24px;
    right: 36px;
    width: calc(220px * var(--cat-size));
    height: calc(233px * var(--cat-size));
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: flex-end;
    pointer-events: auto !important;
    cursor: grab;
    opacity: var(--cat-opacity);
    transition: opacity 0.3s ease, filter 0.3s ease;
    z-index: 9999;
    outline: none;
    transform-origin: 50% 100%;
}

#osmoo-cat-companion:active {
    cursor: grabbing;
}

/* Soft contact shadow beneath paws */
.cat-contact-shadow {
    position: absolute;
    bottom: 0px;
    left: 50%;
    transform: translateX(-50%);
    width: 68%;
    height: 18px;
    background: radial-gradient(ellipse at center, rgba(15, 23, 42, 0.52) 0%, rgba(15, 23, 42, 0.20) 55%, transparent 75%);
    border-radius: 50%;
    pointer-events: none;
    transition: transform 0.25s ease, opacity 0.25s ease;
    animation: catShadowPulse 3.8s ease-in-out infinite;
    z-index: 0;
}

/* Primary Rigged Cat Character Frame */
.cat-character-frame {
    position: relative;
    width: 100%;
    height: 100%;
    transform-origin: 50% 95%;
}

/* ==========================================================================
   RIGGED LAYERS (TAIL, BODY, HEAD, EARS, SUNGLASSES, MOUTH)
   ========================================================================== */

/* 1. Tail Layer (Behind body, swings organically) */
.cat-tail-container {
    position: absolute;
    left: 66.21%;
    top: 73.28%;
    width: 33.11%;
    height: 25.86%;
    transform-origin: 5% 55%;
    z-index: 1;
    animation: catTailSwish 2.6s ease-in-out infinite;
}

.cat-tail-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    image-rendering: pixelated;
}

/* 2. Body Layer (Torso in black 'osmoo' t-shirt and legs) */
.cat-body-container {
    position: absolute;
    left: 1.14%;
    top: 47.41%;
    width: 79.91%;
    height: 52.59%;
    transform-origin: 50% 95%;
    z-index: 2;
    animation: catBodyBreathe 3.6s ease-in-out infinite;
}

.cat-body-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    image-rendering: pixelated;
    filter: drop-shadow(0 4px 12px rgba(0, 0, 0, 0.32));
}

/* 3. Head Layer (Nodding, tilting, looking around) */
.cat-head-container {
    position: absolute;
    left: 8.68%;
    top: 0%;
    width: 64.84%;
    height: 49.14%;
    transform-origin: 50% 95%;
    z-index: 3;
    animation: catHeadIdle 4.2s ease-in-out infinite;
}

.cat-head-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    image-rendering: pixelated;
}

/* Left Ear Twitch */
.cat-ear-left-container {
    position: absolute;
    left: 2.46%;
    top: 6.58%;
    width: 31.69%;
    height: 43.86%;
    transform-origin: 80% 90%;
    z-index: 4;
}

/* Right Ear Twitch */
.cat-ear-right-container {
    position: absolute;
    left: 67.61%;
    top: 6.58%;
    width: 29.93%;
    height: 43.86%;
    transform-origin: 20% 90%;
    z-index: 4;
}

.cat-ear-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    image-rendering: pixelated;
}

.ear-twitch-l {
    animation: catEarTwitchL 0.22s ease-in-out !important;
}

.ear-twitch-r {
    animation: catEarTwitchR 0.22s ease-in-out !important;
}

/* Sunglasses Layer & Glint */
.cat-sunglasses-container {
    position: absolute;
    left: 21.83%;
    top: 46.05%;
    width: 75.70%;
    height: 48.25%;
    z-index: 5;
}

.cat-sunglasses-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
    image-rendering: pixelated;
}

.cat-glasses-glint {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    border-radius: 4px;
    background: linear-gradient(135deg, transparent 20%, rgba(255, 255, 255, 0.35) 45%, rgba(255, 255, 255, 0.8) 50%, rgba(255, 255, 255, 0.35) 55%, transparent 80%);
    background-size: 200% 200%;
    background-position: -100% -100%;
    opacity: 0.55;
    mix-blend-mode: screen;
}

/* Eyelids Blink Layer */
.cat-eyelids {
    position: absolute;
    top: 18%;
    left: 10%;
    width: 80%;
    height: 58%;
    background: #ffffff;
    border-radius: 4px;
    opacity: 0;
    transition: opacity 0.08s ease;
    pointer-events: none;
    z-index: 6;
}

.cat-eyelids.blink {
    opacity: 0.96;
}

/* Live Dynamic SVG Lip-Sync Mouth */
.cat-mouth-container {
    position: absolute;
    left: 60.56%;
    top: 79.82%;
    transform: translate(-50%, -50%);
    width: 28px;
    height: 18px;
    pointer-events: none;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 6;
}

.cat-mouth-svg {
    width: 100%;
    height: 100%;
    overflow: visible;
}

/* ==========================================================================
   STATE INDICATOR PILL
   ========================================================================== */

.cat-state-pill {
    position: absolute;
    top: -30px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    background: rgba(15, 23, 42, 0.82);
    backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 12px;
    font-family: var(--font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.8px;
    color: var(--status-idle);
    pointer-events: none;
    white-space: nowrap;
    opacity: 0.7;
    transition: all 0.3s var(--ease-out-expo);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
    z-index: 10;
}

.cat-state-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: var(--status-idle);
    transition: background-color 0.3s ease, box-shadow 0.3s ease;
}

#osmoo-cat-companion:hover .cat-state-pill {
    opacity: 1.0;
    transform: translateX(-50%) translateY(-2px);
}

/* ==========================================================================
   INTERACTIVE DESKTOP CONTEXT MENU
   ========================================================================== */

.cat-context-menu {
    position: absolute;
    bottom: calc(100% + 12px);
    right: 0;
    background: rgba(15, 23, 42, 0.94);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 12px;
    padding: 6px;
    min-width: 220px;
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.55);
    display: none;
    flex-direction: column;
    gap: 2px;
    z-index: 100;
    pointer-events: auto !important;
}

.cat-context-menu.open {
    display: flex;
}

.cat-menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-radius: 8px;
    font-size: 12px;
    font-weight: 500;
    color: #e2e8f0;
    cursor: pointer;
    background: transparent;
    border: none;
    text-align: left;
    width: 100%;
    transition: background 0.15s ease, color 0.15s ease;
}

.cat-menu-item:hover {
    background: rgba(56, 189, 248, 0.15);
    color: #38bdf8;
}

.cat-menu-item.danger:hover {
    background: rgba(239, 68, 68, 0.18);
    color: #ef4444;
}

.cat-menu-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 4px 0;
}

/* ==========================================================================
   ORGANIC KEYFRAME ANIMATIONS
   ========================================================================== */

/* 1. Tail Swish */
@keyframes catTailSwish {
    0%, 100% {
        transform: rotate(0deg);
    }
    30% {
        transform: rotate(14deg);
    }
    70% {
        transform: rotate(-9deg);
    }
}

/* 2. Torso Breathing */
@keyframes catBodyBreathe {
    0%, 100% {
        transform: scale(1.0) translateY(0);
    }
    50% {
        transform: scaleY(1.022) scaleX(0.99) translateY(-1.2px);
    }
}

/* 3. Head Idle Motion */
@keyframes catHeadIdle {
    0%, 100% {
        transform: rotate(0deg) translateY(0);
    }
    35% {
        transform: rotate(2.4deg) translateY(-0.8px);
    }
    70% {
        transform: rotate(-2.4deg) translateY(-0.4px);
    }
}

/* 4. Contact Shadow Pulse */
@keyframes catShadowPulse {
    0%, 100% {
        transform: translateX(-50%) scale(1.0);
        opacity: 0.52;
    }
    50% {
        transform: translateX(-50%) scale(0.94);
        opacity: 0.38;
    }
}

/* 5. Ear Twitches */
@keyframes catEarTwitchL {
    0%, 100% { transform: rotate(0deg); }
    30% { transform: rotate(-12deg); }
    70% { transform: rotate(6deg); }
}

@keyframes catEarTwitchR {
    0%, 100% { transform: rotate(0deg); }
    30% { transform: rotate(12deg); }
    70% { transform: rotate(-6deg); }
}

/* 6. Autonomous Desktop Walking */
.walking .cat-character-frame {
    animation: catWalkBob 0.32s ease-in-out infinite alternate !important;
}

.walking .cat-tail-container {
    animation: catTailWalk 0.32s ease-in-out infinite alternate !important;
}

@keyframes catWalkBob {
    0% {
        transform: translateY(0) rotate(-2deg);
    }
    100% {
        transform: translateY(-6px) rotate(2deg);
    }
}

@keyframes catTailWalk {
    0% {
        transform: rotate(20deg);
    }
    100% {
        transform: rotate(-14deg);
    }
}

/* Settle hop after drag drop */
.cat-drop-settle {
    animation: catDropHop 0.4s cubic-bezier(0.34, 1.56, 0.64, 1) !important;
}

@keyframes catDropHop {
    0% { transform: translateY(-12px) scale(1.02); }
    60% { transform: translateY(2px) scaleY(0.96) scaleX(1.03); }
    100% { transform: translateY(0) scale(1.0); }
}

/* 7. Attentive Listening State */
.state-listening .cat-head-container {
    transform: rotate(-3.5deg) translateY(-2px);
    transition: transform 0.3s ease;
}

.state-listening .cat-state-pill {
    color: var(--status-listening);
    border-color: rgba(16, 185, 129, 0.4);
    opacity: 1.0;
}

.state-listening .cat-state-dot {
    background-color: var(--status-listening);
    box-shadow: 0 0 8px var(--status-listening);
}

/* 8. Cognitive Thinking State */
.state-thinking .cat-head-container {
    animation: catThink 2.4s ease-in-out infinite;
}

.state-thinking .cat-glasses-glint {
    opacity: 0.95;
    animation: glintSweep 1.8s ease-in-out infinite;
}

.state-thinking .cat-state-pill {
    color: var(--status-thinking);
    border-color: rgba(245, 158, 11, 0.4);
    opacity: 1.0;
}

.state-thinking .cat-state-dot {
    background-color: var(--status-thinking);
    box-shadow: 0 0 8px var(--status-thinking);
}

@keyframes catThink {
    0%, 100% { transform: rotate(2.8deg) translateY(-2px); }
    50% { transform: rotate(4.6deg) translateY(-3.5px); }
}

@keyframes glintSweep {
    0% { background-position: -100% -100%; }
    50% { background-position: 100% 100%; }
    100% { background-position: 200% 200%; }
}

/* 9. Active Speaking State */
.state-speaking .cat-character-frame {
    animation: catSpeakCadence 0.38s ease-in-out infinite alternate;
}

.state-speaking .cat-state-pill {
    color: var(--status-speaking);
    border-color: rgba(56, 189, 248, 0.4);
    opacity: 1.0;
}

.state-speaking .cat-state-dot {
    background-color: var(--status-speaking);
    box-shadow: 0 0 10px var(--status-speaking);
}

@keyframes catSpeakCadence {
    0% { transform: scaleY(1.0) translateY(0); }
    100% { transform: scaleY(1.022) scaleX(0.99) translateY(-2.8px); }
}

/* 10. Success State */
.state-success .cat-character-frame {
    animation: catSuccessHop 0.75s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.state-success .cat-state-pill {
    color: var(--status-success);
    border-color: rgba(34, 197, 94, 0.4);
    opacity: 1.0;
}

.state-success .cat-state-dot {
    background-color: var(--status-success);
    box-shadow: 0 0 10px var(--status-success);
}

@keyframes catSuccessHop {
    0% { transform: translateY(0) scale(1.0); }
    40% { transform: translateY(-16px) scale(1.04) rotate(-2deg); }
    70% { transform: translateY(-5px) scale(1.02) rotate(1deg); }
    100% { transform: translateY(0) scale(1.0); }
}

/* 11. Error State */
.state-error .cat-character-frame {
    animation: catErrorJolt 0.6s ease-in-out;
}

.state-error .cat-state-pill {
    color: var(--status-error);
    border-color: rgba(239, 68, 68, 0.4);
    opacity: 1.0;
}

.state-error .cat-state-dot {
    background-color: var(--status-error);
    box-shadow: 0 0 10px var(--status-error);
}

@keyframes catErrorJolt {
    0%, 100% { transform: translateX(0); }
    20%, 60% { transform: translateX(-4px) rotate(-1.5deg); }
    40%, 80% { transform: translateX(4px) rotate(1.5deg); }
}

/* Reduced Motion Support */
@media (prefers-reduced-motion: reduce) {
    .cat-character-frame,
    .cat-contact-shadow,
    .cat-glasses-glint,
    .cat-tail-container,
    .cat-body-container,
    .cat-head-container {
        animation: none !important;
    }
}
</style>
"#;
