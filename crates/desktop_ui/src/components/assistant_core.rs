use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum CompanionMode {
    Robot,
    ParticleOrb,
}

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum CoreState {
    Idle,
    Listening,
    Thinking,
    Speaking,
    Error,
    Offline,
}


impl CoreState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Listening => "Listening",
            Self::Thinking => "Thinking",
            Self::Speaking => "Speaking",
            Self::Error => "Error",
            Self::Offline => "Offline",
        }
    }

    #[allow(dead_code)]
    pub fn label(&self) -> &'static str {

        match self {
            Self::Idle => "IDLE // READY",
            Self::Listening => "LISTENING // AUDIO STREAM",
            Self::Thinking => "COGNITIVE REASONING ACTIVE",
            Self::Speaking => "DISPATCHING SYNTHESIZED RESPONSE",
            Self::Error => "SUBSYSTEM ATTENTION REQUIRED",
            Self::Offline => "CORE DISCONNECTED",
        }
    }
}

pub const THREE_JS_CORE_SCRIPT: &str = r##"
<script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
<script>
(function() {
    let scene, camera, renderer;
    let robotGroup, headMesh, eyeLMesh, eyeRMesh, bodyMesh, handLMesh, handRMesh;
    let particleOrbMesh, particleGeometry;
    let currentMode = 'Robot'; // 'Robot' or 'ParticleOrb'
    let currentCompanionState = 'Idle';
    let clock = new THREE.Clock();
    let mouse = { x: 0, y: 0, targetX: 0, targetY: 0 };

    window.osmooSetCompanionMode = function(mode) {
        currentMode = mode;
        if (robotGroup && particleOrbMesh) {
            robotGroup.visible = (mode === 'Robot');
            particleOrbMesh.visible = (mode === 'ParticleOrb');
        }
    };

    window.osmooSetCoreState = function(stateName) {
        currentCompanionState = stateName;
    };

    function initThree() {
        const container = document.getElementById("osmoo-viewport-container");
        if (!container) return;

        const width = container.clientWidth || window.innerWidth || 800;
        const height = container.clientHeight || window.innerHeight || 600;

        scene = new THREE.Scene();
        camera = new THREE.PerspectiveCamera(40, width / height, 0.1, 1000);
        camera.position.z = 9.5;

        try {
            renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true, powerPreference: "high-performance" });
        } catch(e) {
            console.warn("WebGL unsupported, fallback active", e);
            const fb = document.getElementById("osmoo-webgl-fallback");
            if (fb) fb.style.display = "flex";
            return;
        }

        renderer.setSize(width, height);
        renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        renderer.toneMapping = THREE.ACESFilmicToneMapping;
        renderer.toneMappingExposure = 1.35;

        container.innerHTML = "";
        container.appendChild(renderer.domElement);

        // Lighting
        const ambientLight = new THREE.AmbientLight(0xffffff, 0.6);
        scene.add(ambientLight);

        const keyLight = new THREE.DirectionalLight(0xf4f0e8, 1.2);
        keyLight.position.set(4, 6, 8);
        scene.add(keyLight);

        const rimLight = new THREE.DirectionalLight(0xffffff, 2.0);
        rimLight.position.set(-6, -4, -6);
        scene.add(rimLight);

        // ── 1. OSMOO Robot Mesh Construction (Reference 1) ──────────
        robotGroup = new THREE.Group();

        // Obsidian Metallic Physical Material
        const obsidianMat = new THREE.MeshPhysicalMaterial({
            color: 0x181a20,
            roughness: 0.18,
            metalness: 0.85,
            clearcoat: 0.9,
            clearcoatRoughness: 0.1,
            reflectivity: 0.9,
        });

        // Glowing White Pill Eye Material
        const eyeMat = new THREE.MeshBasicMaterial({
            color: 0xffffff,
        });

        // Head Sphere
        const headGeo = new THREE.SphereGeometry(1.2, 48, 48);
        headMesh = new THREE.Mesh(headGeo, obsidianMat);
        headMesh.position.y = 1.45;
        robotGroup.add(headMesh);

        // Pill Eyes (Capsules / Cylinders with Sphere ends)
        const eyeGeo = new THREE.CylinderGeometry(0.12, 0.12, 0.38, 16);
        eyeLMesh = new THREE.Mesh(eyeGeo, eyeMat);
        eyeLMesh.position.set(-0.35, 1.52, 1.12);
        robotGroup.add(eyeLMesh);

        eyeRMesh = new THREE.Mesh(eyeGeo, eyeMat);
        eyeRMesh.position.set(0.35, 1.52, 1.12);
        robotGroup.add(eyeRMesh);

        // Teardrop Torso Pod
        const bodyGeo = new THREE.SphereGeometry(1.4, 48, 48);
        bodyGeo.scale(1.0, 1.35, 0.95);
        bodyMesh = new THREE.Mesh(bodyGeo, obsidianMat);
        bodyMesh.position.y = -1.15;
        robotGroup.add(bodyMesh);

        // Floating Symmetrical Spherical Hands
        const handGeo = new THREE.SphereGeometry(0.38, 32, 32);
        handLMesh = new THREE.Mesh(handGeo, eyeMat);
        handLMesh.position.set(-1.95, -1.2, 0.2);
        robotGroup.add(handLMesh);

        handRMesh = new THREE.Mesh(handGeo, eyeMat);
        handRMesh.position.set(1.95, -1.2, 0.2);
        robotGroup.add(handRMesh);

        scene.add(robotGroup);

        // ── 2. OSMOO Luminous Particle Orb (Reference 2) ───────────
        const particleCount = 14000;
        particleGeometry = new THREE.BufferGeometry();
        const positions = new Float32Array(particleCount * 3);
        const originalPositions = new Float32Array(particleCount * 3);

        const radius = 2.4;
        for (let i = 0; i < particleCount; i++) {
            const u = Math.random();
            const v = Math.random();
            const theta = u * 2.0 * Math.PI;
            const phi = Math.acos(2.0 * v - 1.0);
            const r = radius * (0.92 + Math.random() * 0.16);

            const x = r * Math.sin(phi) * Math.cos(theta);
            const y = r * Math.sin(phi) * Math.sin(theta);
            const z = r * Math.cos(phi);

            positions[i * 3] = x;
            positions[i * 3 + 1] = y;
            positions[i * 3 + 2] = z;

            originalPositions[i * 3] = x;
            originalPositions[i * 3 + 1] = y;
            originalPositions[i * 3 + 2] = z;
        }

        particleGeometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));

        const particleMaterial = new THREE.PointsMaterial({
            color: 0xf4f0e8,
            size: 0.035,
            transparent: true,
            opacity: 0.85,
            blending: THREE.AdditiveBlending,
        });

        particleOrbMesh = new THREE.Points(particleGeometry, particleMaterial);
        particleOrbMesh.visible = false;
        scene.add(particleOrbMesh);

        // Mouse Tracking
        window.addEventListener('mousemove', (e) => {
            mouse.targetX = (e.clientX / window.innerWidth) * 2 - 1;
            mouse.targetY = -(e.clientY / window.innerHeight) * 2 + 1;
        });

        // Resize handler
        window.addEventListener('resize', () => {
            if (!container || !renderer || !camera) return;
            const w = container.clientWidth || window.innerWidth;
            const h = container.clientHeight || window.innerHeight;
            camera.aspect = w / h;
            camera.updateProjectionMatrix();
            renderer.setSize(w, h);
        });

        animate();
    }

    function animate() {
        requestAnimationFrame(animate);
        const t = clock.getElapsedTime();

        // Smooth mouse dampening
        mouse.x += (mouse.targetX - mouse.x) * 0.05;
        mouse.y += (mouse.targetY - mouse.y) * 0.05;

        // Robot Animation
        if (robotGroup && robotGroup.visible) {
            // Idle breathing floating
            robotGroup.position.y = Math.sin(t * 1.5) * 0.12;

            // Head tilting towards pointer
            if (headMesh) {
                headMesh.rotation.y = mouse.x * 0.45;
                headMesh.rotation.x = -mouse.y * 0.35;
            }
            if (eyeLMesh && eyeRMesh) {
                eyeLMesh.position.x = -0.35 + mouse.x * 0.15;
                eyeRMesh.position.x = 0.35 + mouse.x * 0.15;
            }

            // Floating hands gentle harmonic oscillation
            if (handLMesh && handRMesh) {
                handLMesh.position.y = -1.15 + Math.sin(t * 2.0) * 0.08;
                handRMesh.position.y = -1.15 + Math.sin(t * 2.0 + 0.5) * 0.08;
            }

            // State-driven lighting response
            if (currentCompanionState === 'Listening') {
                robotGroup.rotation.y = Math.sin(t * 3.0) * 0.05;
            } else if (currentCompanionState === 'Thinking') {
                robotGroup.rotation.y = t * 0.6;
            } else {
                robotGroup.rotation.y = mouse.x * 0.2;
            }
        }

        // Particle Orb Animation
        if (particleOrbMesh && particleOrbMesh.visible) {
            particleOrbMesh.rotation.y = t * 0.15;
            particleOrbMesh.rotation.x = Math.sin(t * 0.2) * 0.1;

            const posAttr = particleGeometry.attributes.position;
            const arr = posAttr.array;
            for (let i = 0; i < arr.length; i += 3) {
                const u = i / 3;
                const wave = Math.sin(t * 2.5 + u * 0.05) * 0.02;
                arr[i] += wave * (arr[i] > 0 ? 0.01 : -0.01);
            }
            posAttr.needsUpdate = true;
        }

        if (renderer && scene && camera) {
            renderer.render(scene, camera);
        }
    }

    if (document.readyState === "complete" || document.readyState === "interactive") {
        setTimeout(initThree, 50);
    } else {
        window.addEventListener("DOMContentLoaded", initThree);
    }
})();
</script>
"##;

#[component]
pub fn AssistantCore(state: CoreState, mode: CompanionMode) -> Element {
    let mode_str = match mode {
        CompanionMode::Robot => "Robot",
        CompanionMode::ParticleOrb => "ParticleOrb",
    };
    let state_str = state.as_str();

    // Trigger state changes in JavaScript
    use_effect(use_reactive!(|(mode_str, state_str)| {
        tracing::trace!("AssistantCore state updated: mode={}, state={}", mode_str, state_str);
    }));



    rsx! {
        div {
            id: "osmoo-viewport-container",
            // Fallback for non-WebGL environments
            div {
                id: "osmoo-webgl-fallback",
                class: "webgl-fallback-container",
                style: "display: none;",
                div { class: "fallback-companion-silhouette",
                    div { class: "fallback-robot-head",
                        div { class: "fallback-pill-eye" }
                        div { class: "fallback-pill-eye" }
                    }
                    div { class: "fallback-robot-body" }
                }
                span { "OSMOO CORE ACCELERATION ACTIVE" }
            }
        }
    }
}
