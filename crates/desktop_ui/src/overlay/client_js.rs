pub const CLIENT_JS: &str = r##"
<script>
(function() {
    let liveMicRms = 0.0;
    let liveOutputRms = 0.0;
    let smoothedMic = 0.0;
    let smoothedOutput = 0.0;
    let visualState = 'Idle';
    let lastRenderTime = 0;
    let animFrameId = null;

    // Exported bridge functions for native IPC driver
    window.voxyUpdateAudioEnergy = function(micRms, outputRms) {
        liveMicRms = typeof micRms === 'number' ? micRms : 0.0;
        liveOutputRms = typeof outputRms === 'number' ? outputRms : 0.0;
    };

    window.voxySetVisualState = function(stateName) {
        setVisualState(stateName);
    };

    window.voxySetTranscript = function(text) {
        setTranscript(text);
    };

    window.voxySetConnectionStatus = function(connected) {
        const statusDot = document.getElementById("connection-status-dot");
        if (statusDot) {
            statusDot.style.background = connected ? "#10b981" : "#64748b";
            statusDot.title = connected ? "Connected to VOXY Daemon (Named Pipe IPC)" : "Disconnected (Reconnecting...)";
        }
    };

    function handleIncomingPacket(data) {
        if (!data || !data.type) return;

        if (data.type === 'clap') {
            const ui_event_ts = Date.now() / 1000.0;
            const root = document.getElementById("app-root");
            if (root) {
                root.classList.remove("collapsed");
            }
            requestAnimationFrame(() => {
                const ui_visible_ts = Date.now() / 1000.0;
                const phys_ts = data.physical_timestamp || ui_event_ts;
                const total_lat = (ui_visible_ts - phys_ts) * 1000.0;
                const det_lat = data.detection_latency_ms || 0.0;
                if (ws && ws.readyState === WebSocket.OPEN) {
                    ws.send(JSON.stringify({
                        type: "clap_ack",
                        source: data.source || "clap",
                        physical_timestamp: phys_ts,
                        detector_timestamp: data.detector_timestamp || ui_event_ts,
                        ui_event_timestamp: ui_event_ts,
                        ui_visible_timestamp: ui_visible_ts,
                        detection_latency_ms: det_lat,
                        total_latency_ms: parseFloat(total_lat.toFixed(2))
                    }));
                }
            });
        } else if (data.type === 'gesture') {
            const root = document.getElementById("app-root");
            if (root) {
                if (data.action === 'EXPANDED') {
                    root.classList.remove("collapsed");
                } else if (data.action === 'COLLAPSED') {
                    root.classList.add("collapsed");
                }
            }
        } else if (data.type === 'audio') {
            liveMicRms = typeof data.mic_rms === 'number' ? data.mic_rms : 0.0;
        } else if (data.type === 'audio_output') {
            liveOutputRms = typeof data.output_rms === 'number' ? data.output_rms : 0.0;
        } else if (data.type === 'voice' || data.type === 'voice_state') {
            if (data.state) setVisualState(data.state);
            if (data.text) setTranscript(data.text);
        } else if (data.type === 'barge_in') {
            setVisualState('Interrupted');
            setTimeout(() => setVisualState('Listening'), 350);
        } else if (data.type === 'tool_execution') {
            handleToolExecution(data);
        }
    }

    function handleToolExecution(data) {
        setVisualState('Executing');
        const banner = document.getElementById("computer-control-banner");
        if (banner) {
            banner.style.display = "flex";
        }
        const actionEl = document.getElementById("control-action-text");
        if (actionEl && data.step) {
            actionEl.innerText = data.step;
        }
        if (data.status === 'completed' || data.status === 'done') {
            setTimeout(() => {
                if (banner) banner.style.display = "none";
                setVisualState('Success');
                setTimeout(() => setVisualState('Idle'), 1200);
            }, 800);
        } else if (data.status === 'failed' || data.status === 'error') {
            setVisualState('Error');
        }
    }

    function setVisualState(stateName) {
        visualState = stateName;
        const normalized = stateName.toLowerCase();
        const root = document.getElementById("app-root");
        if (root) {
            root.className = root.className.replace(/\bstate-[a-z]+\b/g, '') + ' state-' + normalized;
        }
        const badge = document.getElementById("core-badge");
        if (badge) {
            badge.innerText = stateName.toUpperCase();
        }
    }

    function setTranscript(text) {
        const t = document.getElementById("live-transcript-text");
        if (t) {
            t.innerText = text;
        }
    }

    // Emergency Stop Trigger
    window.voxyEmergencyStop = function() {
        console.warn("[VOXY] EMERGENCY STOP TRIGGERED BY USER");
        setVisualState('Error');
        setTranscript("EMERGENCY STOP ACTIVATED: All computer control actions halted.");
    };

    // Toggle Privacy Mute
    window.voxyToggleMic = function() {
        const btn = document.getElementById("mic-privacy-toggle");
        if (btn) {
            btn.classList.toggle("active");
            const isMuted = !btn.classList.contains("active");
            if (isMuted) {
                setVisualState('Disabled');
                setTranscript("Microphone hardware muted.");
            } else {
                setVisualState('Idle');
                setTranscript("Microphone active.");
            }
        }
    };

    // Volumetric Living Core Canvas Renderer
    function startCoreCanvas() {
        const canvas = document.getElementById("presence-core-canvas");
        if (!canvas) return;
        const ctx = canvas.getContext("2d");

        let angleOffset = 0;

        function render(now) {
            // Adaptive frame-rate throttling:
            // When Idle, throttle to ~15 FPS to conserve CPU and battery.
            // When active (Listening, Thinking, Speaking, Executing), run at full 60 FPS.
            const isIdle = (visualState === 'Idle' || visualState === 'Disabled' || visualState === 'Offline');
            const targetDelta = isIdle ? 66.6 : 16.6; // 15 FPS vs 60 FPS

            const delta = now - lastRenderTime;
            if (delta < targetDelta) {
                animFrameId = requestAnimationFrame(render);
                return;
            }
            lastRenderTime = now;

            // Exponential smoothing for real RMS levels (eliminates digital flicker)
            smoothedMic += (liveMicRms - smoothedMic) * 0.28;
            smoothedOutput += (liveOutputRms - smoothedOutput) * 0.28;

            const w = canvas.width;
            const h = canvas.height;
            const cx = w / 2;
            const cy = h / 2;

            ctx.clearRect(0, 0, w, h);

            // Determine active color & energy
            let primaryColor = "rgba(148, 163, 184, 0.4)";
            let energy = 0.05;

            if (visualState === 'Listening') {
                primaryColor = "rgba(16, 185, 129, 0.9)";
                energy = Math.min(1.0, smoothedMic * 18.0);
            } else if (visualState === 'Speaking') {
                primaryColor = "rgba(14, 165, 233, 0.95)";
                energy = Math.min(1.0, smoothedOutput * 18.0);
            } else if (visualState === 'Thinking' || visualState === 'Processing') {
                primaryColor = "rgba(245, 158, 11, 0.85)";
                energy = 0.35;
            } else if (visualState === 'Executing') {
                primaryColor = "rgba(249, 115, 22, 0.9)";
                energy = 0.4;
            } else if (visualState === 'Error' || visualState === 'Interrupted') {
                primaryColor = "rgba(239, 68, 68, 0.95)";
                energy = 0.6;
            } else if (visualState === 'Success') {
                primaryColor = "rgba(34, 197, 94, 0.9)";
                energy = 0.25;
            }

            // Draw Living Volumetric Iris Filaments
            const baseRadius = 78;
            const filamentCount = 64;
            angleOffset += isIdle ? 0.003 : 0.015;

            // Concentric ambient acoustic ripples
            ctx.save();
            ctx.translate(cx, cy);

            // Outer atmospheric boundary
            ctx.beginPath();
            ctx.arc(0, 0, baseRadius + 28 + energy * 20, 0, Math.PI * 2);
            ctx.strokeStyle = primaryColor;
            ctx.lineWidth = 1.0;
            ctx.globalAlpha = 0.25;
            ctx.stroke();

            // Inner harmonic filament ring
            for (let i = 0; i < filamentCount; i++) {
                const theta = (i / filamentCount) * Math.PI * 2 + angleOffset;
                const waveMod = Math.sin(theta * 6 + angleOffset * 3) * (energy * 14);
                const rInner = baseRadius;
                const rOuter = baseRadius + 6 + (energy * 38) + waveMod;

                const x1 = Math.cos(theta) * rInner;
                const y1 = Math.sin(theta) * rInner;
                const x2 = Math.cos(theta) * rOuter;
                const y2 = Math.sin(theta) * rOuter;

                ctx.beginPath();
                ctx.moveTo(x1, y1);
                ctx.lineTo(x2, y2);
                ctx.strokeStyle = primaryColor;
                ctx.lineWidth = 1.5;
                ctx.globalAlpha = 0.4 + (energy * 0.5);
                ctx.stroke();
            }

            // Central core lens ring
            ctx.beginPath();
            ctx.arc(0, 0, baseRadius - 2, 0, Math.PI * 2);
            ctx.strokeStyle = primaryColor;
            ctx.lineWidth = 2.0;
            ctx.globalAlpha = 0.7;
            ctx.stroke();

            ctx.restore();

            animFrameId = requestAnimationFrame(render);
        }

        animFrameId = requestAnimationFrame(render);
    }

    window.addEventListener('DOMContentLoaded', function() {
        startCoreCanvas();
    });
})();
</script>
"##;
