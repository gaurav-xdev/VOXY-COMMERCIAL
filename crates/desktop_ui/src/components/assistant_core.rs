use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CoreState {
    Idle,
    Listening,
    Processing,
    Speaking,
    Interrupted,
    Error,
}

impl CoreState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Listening => "Listening",
            Self::Processing => "Processing",
            Self::Speaking => "Speaking",
            Self::Interrupted => "Interrupted",
            Self::Error => "Error",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "IDLE // STANDBY",
            Self::Listening => "ACOUSTIC STREAM ACTIVE",
            Self::Processing => "EVALUATING CONTEXT & INTENT",
            Self::Speaking => "SYNTHESIZING RESPONSE",
            Self::Interrupted => "BARGE-IN DETECTED",
            Self::Error => "SYSTEM ATTENTION REQUIRED",
        }
    }

    pub fn color_hex(&self) -> &'static str {
        match self {
            Self::Idle => "#4a5568",
            Self::Listening => "#10b981",
            Self::Processing => "#8b5cf6",
            Self::Speaking => "#06b6d4",
            Self::Interrupted => "#f59e0b",
            Self::Error => "#ef4444",
        }
    }
}

pub const THREE_JS_CORE_SCRIPT: &str = r##"
<script src="https://cdnjs.cloudflare.com/ajax/libs/three.js/r128/three.min.js"></script>
<script>
(function() {
    let scene, camera, renderer;
    let coreMesh, auraMesh, outerRing, innerRing, particles;
    let currentVoiceState = 'Idle';
    let targetCoreColor = new THREE.Color(0x38bdf8);
    let currentCoreColor = new THREE.Color(0x38bdf8);
    let micEnergy = 0.05;
    let smoothedEnergy = 0.05;
    let clock = new THREE.Clock();

    window.osmooSetCoreState = function(stateName) {
        currentVoiceState = stateName;
        if (stateName === 'Listening') {
            targetCoreColor.setHex(0x10b981); // Emerald
        } else if (stateName === 'Processing') {
            targetCoreColor.setHex(0xa855f7); // Deep Purple
        } else if (stateName === 'Speaking') {
            targetCoreColor.setHex(0x06b6d4); // Cyan
        } else if (stateName === 'Interrupted') {
            targetCoreColor.setHex(0xf59e0b); // Amber
        } else if (stateName === 'Error') {
            targetCoreColor.setHex(0xef4444); // Crimson
        } else {
            targetCoreColor.setHex(0x38bdf8); // Sky blue idle
        }
    };

    window.osmooSetAudioEnergy = function(micRms, outputRms) {
        if (currentVoiceState === 'Speaking') {
            micEnergy = Math.min(1.0, (outputRms || 0.0) * 8.0 + 0.1);
        } else {
            micEnergy = Math.min(1.0, (micRms || 0.0) * 8.0 + 0.05);
        }
    };

    function initThree() {
        const container = document.getElementById("osmoo-three-container");
        if (!container) return;

        const width = container.clientWidth || 600;
        const height = container.clientHeight || 500;

        scene = new THREE.Scene();

        camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 1000);
        camera.position.z = 8.5;

        renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true, powerPreference: "high-performance" });
        renderer.setSize(width, height);
        renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
        renderer.toneMapping = THREE.ACESFilmicToneMapping;
        renderer.toneMappingExposure = 1.25;

        container.innerHTML = "";
        container.appendChild(renderer.domElement);

        // 1. Core Volumetric Orb (Icosahedron high poly)
        const coreGeo = new THREE.IcosahedronGeometry(2.0, 5);
        const coreMat = new THREE.MeshPhysicalMaterial({
            color: 0x0284c7,
            emissive: 0x0369a1,
            emissiveIntensity: 0.85,
            roughness: 0.1,
            metalness: 0.1,
            transmission: 0.88,
            thickness: 1.8,
            transparent: true,
            opacity: 0.95,
            ior: 1.45,
            reflectivity: 0.8,
            clearcoat: 1.0,
            clearcoatRoughness: 0.1
        });
        coreMesh = new THREE.Mesh(coreGeo, coreMat);
        scene.add(coreMesh);

        // 2. Inner Living Filament Core
        const innerGeo = new THREE.IcosahedronGeometry(1.2, 3);
        const innerMat = new THREE.MeshStandardMaterial({
            color: 0xffffff,
            emissive: 0x38bdf8,
            emissiveIntensity: 2.2,
            wireframe: true,
            transparent: true,
            opacity: 0.45
        });
        const innerMesh = new THREE.Mesh(innerGeo, innerMat);
        coreMesh.add(innerMesh);

        // 3. Gyroscopic Orbital Gimbal Rings
        const ringGeo1 = new THREE.TorusGeometry(3.0, 0.022, 16, 120);
        const ringMat1 = new THREE.MeshStandardMaterial({
            color: 0x7dd3fc,
            emissive: 0x0284c7,
            emissiveIntensity: 0.6,
            metalness: 0.9,
            roughness: 0.2
        });
        outerRing = new THREE.Mesh(ringGeo1, ringMat1);
        scene.add(outerRing);

        const ringGeo2 = new THREE.TorusGeometry(2.5, 0.018, 16, 100);
        const ringMat2 = new THREE.MeshStandardMaterial({
            color: 0xbae6fd,
            emissive: 0x0369a1,
            emissiveIntensity: 0.5,
            metalness: 0.8,
            roughness: 0.3
        });
        innerRing = new THREE.Mesh(ringGeo2, ringMat2);
        scene.add(innerRing);

        // 4. Volumetric Quantum Particle Field
        const particleCount = 260;
        const particleGeo = new THREE.BufferGeometry();
        const positions = new Float32Array(particleCount * 3);
        for(let i = 0; i < particleCount * 3; i += 3) {
            const rad = 2.4 + Math.random() * 2.2;
            const theta = Math.random() * Math.PI * 2;
            const phi = Math.acos(2 * Math.random() - 1);
            positions[i] = rad * Math.sin(phi) * Math.cos(theta);
            positions[i+1] = rad * Math.sin(phi) * Math.sin(theta);
            positions[i+2] = rad * Math.cos(phi);
        }
        particleGeo.setAttribute('position', new THREE.BufferAttribute(positions, 3));
        const particleMat = new THREE.PointsMaterial({
            color: 0x38bdf8,
            size: 0.045,
            transparent: true,
            opacity: 0.65,
            blending: THREE.AdditiveBlending
        });
        particles = new THREE.Points(particleGeo, particleMat);
        scene.add(particles);

        // 5. Studio Lighting
        const keyLight = new THREE.PointLight(0xffffff, 2.5, 50);
        keyLight.position.set(4, 5, 5);
        scene.add(keyLight);

        const rimLight = new THREE.PointLight(0x0ea5e9, 3.5, 50);
        rimLight.position.set(-5, -4, -4);
        scene.add(rimLight);

        const ambientLight = new THREE.AmbientLight(0x081525, 1.2);
        scene.add(ambientLight);

        window.addEventListener('resize', onResize);
        animate();
    }

    function onResize() {
        const container = document.getElementById("osmoo-three-container");
        if (!container || !renderer || !camera) return;
        const width = container.clientWidth;
        const height = container.clientHeight;
        camera.aspect = width / height;
        camera.updateProjectionMatrix();
        renderer.setSize(width, height);
    }

    function animate() {
        requestAnimationFrame(animate);

        const dt = clock.getDelta();
        const time = clock.getElapsedTime();

        // Smooth energy interpolation
        smoothedEnergy += (micEnergy - smoothedEnergy) * 0.2;

        // Color transition
        currentCoreColor.lerp(targetCoreColor, 0.08);
        if (coreMesh && coreMesh.material) {
            coreMesh.material.emissive.copy(currentCoreColor);
            coreMesh.material.color.copy(currentCoreColor);
        }
        if (particles && particles.material) {
            particles.material.color.copy(currentCoreColor);
        }

        // Pulse scale based on acoustic RMS
        const pulse = 1.0 + Math.sin(time * 2.5) * 0.03 + (smoothedEnergy * 0.35);
        if (coreMesh) {
            coreMesh.scale.set(pulse, pulse, pulse);
            coreMesh.rotation.y += (0.2 + smoothedEnergy * 0.8) * dt;
            coreMesh.rotation.x += 0.1 * dt;
        }

        // Gimbal Rotation
        if (outerRing) {
            outerRing.rotation.x = Math.sin(time * 0.4) * 0.6;
            outerRing.rotation.y += 0.35 * dt;
            outerRing.rotation.z = Math.cos(time * 0.3) * 0.4;
        }
        if (innerRing) {
            innerRing.rotation.x = Math.cos(time * 0.5) * 0.8;
            innerRing.rotation.y -= 0.45 * dt;
            innerRing.rotation.z = Math.sin(time * 0.6) * 0.5;
        }

        // Orbit particles slowly
        if (particles) {
            particles.rotation.y += 0.12 * dt;
            particles.rotation.x -= 0.08 * dt;
        }

        renderer.render(scene, camera);
    }

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initThree);
    } else {
        setTimeout(initThree, 50);
    }
})();
</script>
"##;

#[component]
pub fn AssistantCore(
    state: CoreState,
    on_toggle_voice: EventHandler<()>,
    mic_active: bool,
) -> Element {
    let state_str = state.as_str();
    let state_label = state.label();
    let color = state.color_hex();

    rsx! {
        div { class: "core-experience-wrapper",
            script {
                dangerous_inner_html: "if (window.osmooSetCoreState) {{ window.osmooSetCoreState('{state_str}'); }}"
            }
            // 3D Three.js WebGL Core Canvas Container
            div {
                id: "osmoo-three-container",
                class: "osmoo-three-canvas-holder",
                onclick: move |_| on_toggle_voice.call(()),
                title: "Click core to activate continuous listening",
            }

            // Central Telemetry HUD Overlay
            div { class: "core-telemetry-hud",
                div {
                    class: "core-state-badge",
                    style: "border-color: {color}; color: {color}; box-shadow: 0 0 20px {color}33;",
                    span {
                        class: "core-state-pulsar",
                        style: "background-color: {color};",
                    }
                    span { "{state_label}" }
                }

                div { class: "core-subtext-stream",
                    if mic_active {
                        "CONTINUOUS WASAPI 48kHz REAL-TIME AUDIO // WAKE-WORD ACTIVE"
                    } else {
                        "AUDIO STANDBY // CLICK CORE OR SPACE TO TALK"
                    }
                }
            }
        }
    }
}
