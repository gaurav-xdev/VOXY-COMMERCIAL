pub const CLIENT_JS: &str = r##"
<script>
(function() {
    let liveMicRms = 0.0;
    let liveOutputRms = 0.0;
    let smoothedOutput = 0.0;
    let visualState = 'Idle';
    let isDragging = false;
    let dragStartX = 0;
    let dragStartY = 0;
    let initialLeft = 0;
    let initialTop = 0;
    let mouthSvgEl = null;
    let eyelidsEl = null;
    let earLeftEl = null;
    let earRightEl = null;

    // Autonomous Walking Engine State
    let autoWalkEnabled = true;
    let isWalking = false;
    let walkTargetX = null;
    let walkSpeed = 65; // pixels per second
    let lastWalkFrameTime = null;
    let nextWanderTimeout = null;

    // SVG Viseme Path Definitions
    // Coordinates relative to mouth viewBox "0 0 28 18", centered around (14, 9)
    const MOUTH_PATHS = {
        // 1. Natural closed cat 'w' smile line
        closed: '<path d="M 5,8 Q 10,12 14,9 Q 18,12 23,8" fill="none" stroke="#1e293b" stroke-width="2.2" stroke-linecap="round"/>',
        // 2. Slight open (small vowels / quiet speech)
        slight: '<ellipse cx="14" cy="9" rx="6.5" ry="3.5" fill="#0f172a" stroke="#1e293b" stroke-width="1.8"/>' +
                '<path d="M 11,10.5 Q 14,12.5 17,10.5" fill="#f472b6"/>',
        // 3. Medium open (standard conversational vowels)
        medium: '<path d="M 6,7 C 6,14 22,14 22,7 Z" fill="#0f172a" stroke="#1e293b" stroke-width="2"/>' +
                '<path d="M 9,12 Q 14,14.5 19,12 Q 14,10 9,12" fill="#f472b6"/>',
        // 4. Wide open (loud syllables / expressive emphasis)
        wide: '<path d="M 5,6 C 4,16 24,16 23,6 Z" fill="#0f172a" stroke="#1e293b" stroke-width="2.2"/>' +
              '<path d="M 8,12 Q 14,16 20,12 Q 14,10 8,12" fill="#f472b6"/>' +
              '<path d="M 8,7 Q 14,9 20,7" fill="#ffffff"/>',
        // 5. Cheerful celebration smile
        smile: '<path d="M 4,7 Q 14,15 24,7" fill="none" stroke="#1e293b" stroke-width="2.4" stroke-linecap="round"/>',
        // 6. Concerned error pout
        pout: '<path d="M 5,11 Q 14,6 23,11" fill="none" stroke="#1e293b" stroke-width="2.2" stroke-linecap="round"/>'
    };

    // ── Exported Bridge Functions for Native IPC Driver ───────────────────
    window.voxyUpdateAudioEnergy = function(micRms, outputRms) {
        liveMicRms = typeof micRms === 'number' ? micRms : 0.0;
        liveOutputRms = typeof outputRms === 'number' ? outputRms : 0.0;
        if (visualState === 'Speaking' && liveOutputRms <= 0.001 && liveMicRms > 0.01) {
            liveOutputRms = liveMicRms;
        }
    };

    window.voxySetVisualState = function(stateName) {
        setVisualState(stateName);
    };

    window.voxySetTranscript = function(text) {
        const pill = document.getElementById("cat-state-text");
        if (pill && visualState === 'Speaking' && text) {
            const truncated = text.length > 24 ? text.substring(0, 24) + '...' : text;
            pill.innerText = truncated.toUpperCase();
        }
    };

    window.voxySetConnectionStatus = function(connected) {
        const dot = document.getElementById("cat-state-dot");
        if (dot) {
            dot.style.backgroundColor = connected ? "var(--status-listening)" : "var(--status-idle)";
        }
    };

    function setVisualState(stateName) {
        visualState = stateName;
        const normalized = stateName.toLowerCase();
        const root = document.getElementById("app-root");
        if (root) {
            root.className = root.className.replace(/\bstate-[a-z]+\b/g, '') + ' state-' + normalized;
        }
        const text = document.getElementById("cat-state-text");
        if (text) {
            text.innerText = stateName.toUpperCase();
        }

        // If not idle, cancel autonomous wander immediately
        if (visualState !== 'Idle') {
            stopAutonomousWalk();
        }

        // State-driven immediate mouth shape updates
        if (visualState !== 'Speaking') {
            liveOutputRms = 0.0;
            smoothedOutput = 0.0;
            if (visualState === 'Success') {
                updateMouthShape('smile');
            } else if (visualState === 'Error' || visualState === 'Interrupted') {
                updateMouthShape('pout');
            } else {
                updateMouthShape('closed');
            }
        }
    }

    function updateMouthShape(shape) {
        if (!mouthSvgEl) {
            mouthSvgEl = document.getElementById("cat-mouth-svg");
        }
        if (mouthSvgEl && MOUTH_PATHS[shape]) {
            mouthSvgEl.innerHTML = MOUTH_PATHS[shape];
        }
    }

    // ── Continuous Audio-Driven Lip Sync Animation Loop ───────────────────
    function runLipSyncLoop() {
        if (visualState === 'Speaking') {
            smoothedOutput += (liveOutputRms - smoothedOutput) * 0.42;

            if (smoothedOutput < 0.02) {
                updateMouthShape('closed');
            } else if (smoothedOutput < 0.09) {
                updateMouthShape('slight');
            } else if (smoothedOutput < 0.22) {
                updateMouthShape('medium');
            } else {
                updateMouthShape('wide');
            }
        }
        requestAnimationFrame(runLipSyncLoop);
    }

    // ── Periodic Organic Idles: Blinking & Ear Twitches ───────────────────
    function scheduleBlink() {
        const delay = 3200 + Math.random() * 2400; // 3.2s to 5.6s
        setTimeout(function() {
            if (!eyelidsEl) {
                eyelidsEl = document.getElementById("cat-eyelids");
            }
            if (eyelidsEl && visualState !== 'Speaking') {
                eyelidsEl.classList.add("blink");
                setTimeout(function() {
                    if (eyelidsEl) eyelidsEl.classList.remove("blink");
                }, 130);
            }
            scheduleBlink();
        }, delay);
    }

    function scheduleEarTwitch() {
        const delay = 4000 + Math.random() * 4500; // 4s to 8.5s
        setTimeout(function() {
            if (!earLeftEl) earLeftEl = document.getElementById("cat-ear-left");
            if (!earRightEl) earRightEl = document.getElementById("cat-ear-right");

            const twitchLeft = Math.random() > 0.5;
            if (twitchLeft && earLeftEl) {
                earLeftEl.classList.add("ear-twitch-l");
                setTimeout(function() {
                    if (earLeftEl) earLeftEl.classList.remove("ear-twitch-l");
                }, 220);
            } else if (!twitchLeft && earRightEl) {
                earRightEl.classList.add("ear-twitch-r");
                setTimeout(function() {
                    if (earRightEl) earRightEl.classList.remove("ear-twitch-r");
                }, 220);
            }
            scheduleEarTwitch();
        }, delay);
    }

    // ── Autonomous Desktop Walking Engine ─────────────────────────────────
    function scheduleNextWander() {
        if (nextWanderTimeout) clearTimeout(nextWanderTimeout);
        if (!autoWalkEnabled) return;

        const delay = 14000 + Math.random() * 16000; // 14s to 30s
        nextWanderTimeout = setTimeout(function() {
            if (visualState === 'Idle' && !isDragging && autoWalkEnabled) {
                startAutonomousWalk();
            } else {
                scheduleNextWander();
            }
        }, delay);
    }

    function startAutonomousWalk() {
        const cat = document.getElementById("osmoo-cat-companion");
        if (!cat) return;

        const catWidth = cat.offsetWidth;
        const screenW = window.innerWidth;
        const minX = 30;
        const maxX = Math.max(minX, screenW - catWidth - 30);

        const currentLeft = cat.getBoundingClientRect().left;
        // Choose target X at least 120px away from current position
        let targetX = minX + Math.random() * (maxX - minX);
        if (Math.abs(targetX - currentLeft) < 100) {
            targetX = currentLeft > screenW / 2 ? currentLeft - 180 : currentLeft + 180;
            targetX = Math.min(Math.max(minX, targetX), maxX);
        }

        walkTargetX = targetX;
        isWalking = true;
        lastWalkFrameTime = performance.now();

        // Face walking direction
        const charFrame = cat.querySelector(".cat-character-frame");
        if (charFrame) {
            charFrame.style.transform = targetX < currentLeft ? "scaleX(-1)" : "scaleX(1)";
        }
        cat.classList.add("walking");

        requestAnimationFrame(walkStep);
    }

    function walkStep(timestamp) {
        if (!isWalking || walkTargetX === null || isDragging || visualState !== 'Idle') {
            stopAutonomousWalk();
            return;
        }

        const cat = document.getElementById("osmoo-cat-companion");
        if (!cat) return;

        const dt = (timestamp - (lastWalkFrameTime || timestamp)) / 1000.0;
        lastWalkFrameTime = timestamp;

        const currentLeft = cat.getBoundingClientRect().left;
        const diff = walkTargetX - currentLeft;

        if (Math.abs(diff) < 4) {
            // Reached target
            cat.style.left = walkTargetX + "px";
            cat.style.right = "auto";
            stopAutonomousWalk();
            // Settle and face front
            const charFrame = cat.querySelector(".cat-character-frame");
            if (charFrame) charFrame.style.transform = "scaleX(1)";
            scheduleNextWander();
            return;
        }

        const dir = diff > 0 ? 1 : -1;
        const moveDist = Math.min(Math.abs(diff), walkSpeed * dt) * dir;
        const nextLeft = currentLeft + moveDist;

        cat.style.left = nextLeft + "px";
        cat.style.right = "auto";

        requestAnimationFrame(walkStep);
    }

    function stopAutonomousWalk() {
        isWalking = false;
        walkTargetX = null;
        lastWalkFrameTime = null;
        const cat = document.getElementById("osmoo-cat-companion");
        if (cat) {
            cat.classList.remove("walking");
        }
    }

    // ── Desktop Drag-and-Drop & Position Persistence ──────────────────────
    function initCatDragging() {
        const cat = document.getElementById("osmoo-cat-companion");
        if (!cat) return;

        // Restore saved position
        try {
            const saved = localStorage.getItem("osmoo_cat_position");
            if (saved) {
                const pos = JSON.parse(saved);
                if (typeof pos.left === 'number' && typeof pos.top === 'number') {
                    const maxLeft = Math.max(0, window.innerWidth - cat.offsetWidth);
                    const maxTop = Math.max(0, window.innerHeight - cat.offsetHeight);
                    cat.style.left = Math.min(Math.max(0, pos.left), maxLeft) + "px";
                    cat.style.top = Math.min(Math.max(0, pos.top), maxTop) + "px";
                    cat.style.right = "auto";
                    cat.style.bottom = "auto";
                }
            }
        } catch(e) {
            console.warn("Failed to restore cat position", e);
        }

        cat.addEventListener("mousedown", function(e) {
            if (e.button !== 0) return; // Only primary mouse button
            if (e.target.closest && e.target.closest("#cat-context-menu")) return; // Allow menu clicks
            stopAutonomousWalk();
            isDragging = true;
            dragStartX = e.clientX;
            dragStartY = e.clientY;

            const rect = cat.getBoundingClientRect();
            initialLeft = rect.left;
            initialTop = rect.top;

            cat.classList.add("walking");
            closeContextMenu();
            e.preventDefault();
        });

        window.addEventListener("mousemove", function(e) {
            if (!isDragging) return;

            const deltaX = e.clientX - dragStartX;
            const deltaY = e.clientY - dragStartY;

            const newLeft = initialLeft + deltaX;
            const newTop = initialTop + deltaY;

            // Clamping to visible desktop bounds
            const maxLeft = Math.max(0, window.innerWidth - cat.offsetWidth);
            const maxTop = Math.max(0, window.innerHeight - cat.offsetHeight);

            const clampedLeft = Math.min(Math.max(0, newLeft), maxLeft);
            const clampedTop = Math.min(Math.max(0, newTop), maxTop);

            cat.style.left = clampedLeft + "px";
            cat.style.top = clampedTop + "px";
            cat.style.right = "auto";
            cat.style.bottom = "auto";
        });

        window.addEventListener("mouseup", function() {
            if (isDragging) {
                isDragging = false;
                cat.classList.remove("walking");

                // Drop settle animation
                cat.classList.add("cat-drop-settle");
                setTimeout(function() {
                    cat.classList.remove("cat-drop-settle");
                }, 400);

                // Persist new position
                try {
                    const rect = cat.getBoundingClientRect();
                    localStorage.setItem("osmoo_cat_position", JSON.stringify({
                        left: Math.round(rect.left),
                        top: Math.round(rect.top)
                    }));
                } catch(e) {}

                scheduleNextWander();
            }
        });

        // Double-click to trigger quick dashboard reveal
        cat.addEventListener("dblclick", function() {
            window.voxyOpenDashboard();
        });

        // Context menu trigger (Right click)
        cat.addEventListener("contextmenu", function(e) {
            e.preventDefault();
            e.stopPropagation();
            toggleContextMenu();
        });
    }

    function toggleContextMenu() {
        const menu = document.getElementById("cat-context-menu");
        if (menu) {
            menu.classList.toggle("open");
        }
    }

    function closeContextMenu() {
        const menu = document.getElementById("cat-context-menu");
        if (menu) {
            menu.classList.remove("open");
        }
    }

    window.addEventListener("click", function(e) {
        const cat = document.getElementById("osmoo-cat-companion");
        if (cat && !cat.contains(e.target)) {
            closeContextMenu();
        }
    });

    // ── Desktop Companion Interactive Actions ─────────────────────────────
    window.voxyOpenDashboard = function() {
        closeContextMenu();
        if (window.ipc) {
            window.ipc.postMessage(JSON.stringify({ type: "open_dashboard" }));
        }
        console.info("[OSMOO] Open Dashboard requested from Cat Companion");
    };

    window.voxySetCatScale = function(scale) {
        document.documentElement.style.setProperty("--cat-size", scale);
        localStorage.setItem("osmoo_cat_scale", scale);
        closeContextMenu();
    };

    window.voxyToggleAutoWalk = function() {
        autoWalkEnabled = !autoWalkEnabled;
        localStorage.setItem("osmoo_cat_autowalk", autoWalkEnabled ? "true" : "false");
        closeContextMenu();
        console.info("[OSMOO] Autonomous walking is now:", autoWalkEnabled ? "ENABLED" : "DISABLED");
        if (autoWalkEnabled) {
            scheduleNextWander();
        } else {
            stopAutonomousWalk();
        }
    };

    window.voxyToggleAlwaysOnTop = function() {
        closeContextMenu();
        if (window.ipc) {
            window.ipc.postMessage(JSON.stringify({ type: "toggle_always_on_top" }));
        }
    };

    window.voxyEmergencyStop = function() {
        closeContextMenu();
        console.warn("[OSMOO] EMERGENCY STOP TRIGGERED BY USER");
        setVisualState('Error');
        if (window.ipc) {
            window.ipc.postMessage(JSON.stringify({ type: "emergency_stop" }));
        }
    };

    window.voxyExitApp = function() {
        closeContextMenu();
        console.info("[OSMOO] Explicit Application Exit requested from Cat Companion");
        if (window.ipc) {
            window.ipc.postMessage(JSON.stringify({ type: "exit_app" }));
        }
    };

    window.voxyToggleMic = function() {
        closeContextMenu();
        if (window.ipc) {
            window.ipc.postMessage(JSON.stringify({ type: "toggle_mic" }));
        }
    };

    // Restore saved settings on boot
    try {
        const savedScale = localStorage.getItem("osmoo_cat_scale");
        if (savedScale) {
            document.documentElement.style.setProperty("--cat-size", savedScale);
        }
        const savedWalk = localStorage.getItem("osmoo_cat_autowalk");
        if (savedWalk !== null) {
            autoWalkEnabled = (savedWalk === "true");
        }
    } catch(e) {}

    window.addEventListener("DOMContentLoaded", function() {
        mouthSvgEl = document.getElementById("cat-mouth-svg");
        eyelidsEl = document.getElementById("cat-eyelids");
        earLeftEl = document.getElementById("cat-ear-left");
        earRightEl = document.getElementById("cat-ear-right");

        initCatDragging();
        runLipSyncLoop();
        scheduleBlink();
        scheduleEarTwitch();
        scheduleNextWander();
    });
})();
</script>
"##;
