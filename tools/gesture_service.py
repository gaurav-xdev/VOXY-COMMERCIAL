#!/usr/bin/env python3
"""
VOXY Real-Time Gesture Tracking Service
Uses Google MediaPipe Tasks HandLandmarker to track dual-hand spatial gestures:
- Hand Expansion / Separation: Triggers UI Spatial Assembly
- Hand Clap: Triggers UI Spatial Assembly / Toggle
- Hands Coming Together: Triggers UI Collapse / Dormancy
Emits events over local WebSocket server on ws://127.0.0.1:18888.
"""

import argparse
import asyncio
import collections
import json
import logging
import math
import os
import sys
import time
from typing import Dict, List, Optional, Set, Tuple

import cv2
import mediapipe as mp
from mediapipe.tasks import python
from mediapipe.tasks.python import vision
import sounddevice as sd
import numpy as np
import websockets

try:
    from clap_detector import AcousticClapDetector
except ImportError:
    from tools.clap_detector import AcousticClapDetector

try:
    from acoustic_shield import LaptopAcousticShield
except ImportError:
    from tools.acoustic_shield import LaptopAcousticShield

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] (GestureService) %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("voxy_gestures")

MODEL_PATH = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "models",
    "hand_landmarker.task",
)

WS_HOST = "127.0.0.1"
WS_PORT = 18888


class GestureTracker:
    def __init__(self, model_path: str):
        if not os.path.exists(model_path):
            raise FileNotFoundError(f"Model file not found at: {model_path}")

        base_options = python.BaseOptions(model_asset_path=model_path)
        options = vision.HandLandmarkerOptions(
            base_options=base_options,
            running_mode=vision.RunningMode.VIDEO,
            num_hands=2,
            min_hand_detection_confidence=0.5,
            min_hand_presence_confidence=0.5,
            min_tracking_confidence=0.5,
        )
        self.detector = vision.HandLandmarker.create_from_options(options)

        # Tracking state
        self.distance_history: collections.deque = collections.deque(maxlen=15)
        self.last_trigger_time: float = 0.0
        self.cooldown_sec: float = 1.0
        self.ui_state: str = "COLLAPSED"  # "COLLAPSED" or "EXPANDED"
        self.no_hands_frames: int = 0

    def get_palm_center(self, landmarks) -> Tuple[float, float]:
        """Calculates normalized center of the palm (wrist + MCP bases)."""
        indices = [0, 5, 9, 13, 17]
        xs = [landmarks[i].x for i in indices]
        ys = [landmarks[i].y for i in indices]
        return (sum(xs) / len(indices), sum(ys) / len(indices))

    def process_frame(
        self, frame_bgr, timestamp_ms: int
    ) -> Tuple[Optional[Dict], Optional[Dict]]:
        """
        Processes a single BGR camera frame.
        Returns: (gesture_event, tracking_event)
        """
        frame_rgb = cv2.cvtColor(frame_bgr, cv2.COLOR_BGR2RGB)
        mp_image = mp.Image(image_format=mp.ImageFormat.SRGB, data=frame_rgb)
        result = self.detector.detect_for_video(mp_image, timestamp_ms)

        num_hands = len(result.hand_landmarks)
        now = time.time()

        tracking_data = {
            "type": "tracking",
            "hands": num_hands,
            "timestamp": now,
        }

        if num_hands < 2:
            self.distance_history.clear()
            self.no_hands_frames += 1
            if num_hands == 1:
                p1 = self.get_palm_center(result.hand_landmarks[0])
                tracking_data["primary_pos"] = [round(p1[0], 3), round(p1[1], 3)]
            return (None, tracking_data)

        self.no_hands_frames = 0
        p1 = self.get_palm_center(result.hand_landmarks[0])
        p2 = self.get_palm_center(result.hand_landmarks[1])

        # Ensure left hand is p_left, right is p_right
        p_left = p1 if p1[0] < p2[0] else p2
        p_right = p2 if p1[0] < p2[0] else p1

        dx = p_right[0] - p_left[0]
        dy = p_right[1] - p_left[1]
        dist = math.sqrt(dx * dx + dy * dy)

        self.distance_history.append((now, dist, p_left[0], p_right[0]))
        tracking_data["distance"] = round(dist, 3)
        tracking_data["left_pos"] = [round(p_left[0], 3), round(p_left[1], 3)]
        tracking_data["right_pos"] = [round(p_right[0], 3), round(p_right[1], 3)]

        # Check for gestures if outside cooldown window
        if now - self.last_trigger_time < self.cooldown_sec:
            return (None, tracking_data)

        if len(self.distance_history) < 6:
            return (None, tracking_data)

        old_t, old_dist, old_l, old_r = self.distance_history[0]
        dt = now - old_t
        if dt <= 0.05 or dt > 0.6:
            return (None, tracking_data)

        delta_dist = dist - old_dist

        # 1. CLAP DETECTION: Fast convergence to very close proximity
        if dist < 0.12 and delta_dist < -0.15:
            self.last_trigger_time = now
            self.ui_state = "EXPANDED"
            det_lat = round((now - old_t) * 1000.0, 2)
            logger.info(f"Gesture detected: VISUAL CLAP -> EXPANDED (lat={det_lat}ms)")
            return (
                {
                    "type": "clap",
                    "action": "EXPANDED",
                    "source": "visual_clap",
                    "confidence": 0.98,
                    "distance": round(dist, 3),
                    "physical_timestamp": old_t,
                    "detector_timestamp": now,
                    "detection_latency_ms": det_lat,
                },
                tracking_data,
            )

        # 2. HAND EXPANSION / SEPARATION: Both hands moving outward
        if delta_dist > 0.16 and dist > 0.28:
            if self.ui_state != "EXPANDED":
                self.last_trigger_time = now
                self.ui_state = "EXPANDED"
                logger.info("Gesture detected: HAND EXPANSION -> EXPANDED")
                return (
                    {
                        "type": "gesture",
                        "action": "EXPANDED",
                        "source": "hands_separation",
                        "confidence": 0.95,
                        "distance": round(dist, 3),
                        "timestamp": now,
                    },
                    tracking_data,
                )

        # 3. HAND CONTRACTION / COLLAPSE: Hands moving inward from wide
        if delta_dist < -0.16 and dist < 0.22:
            if self.ui_state != "COLLAPSED":
                self.last_trigger_time = now
                self.ui_state = "COLLAPSED"
                logger.info("Gesture detected: HAND CONTRACTION -> COLLAPSED")
                return (
                    {
                        "type": "gesture",
                        "action": "COLLAPSED",
                        "source": "hands_together",
                        "confidence": 0.93,
                        "distance": round(dist, 3),
                        "timestamp": now,
                    },
                    tracking_data,
                )

        return (None, tracking_data)

    def close(self):
        if hasattr(self, "detector") and self.detector:
            self.detector.close()


class GestureServiceServer:
    def __init__(self, camera_idx: int = 0, clap_mode: str = "hybrid"):
        self.camera_idx = camera_idx
        self.clap_mode = clap_mode  # "acoustic", "visual", or "hybrid"
        self.clients: Set[websockets.WebSocketServerProtocol] = set()
        self.tracker: Optional[GestureTracker] = None
        self.running = False
        self.live_mic_rms: float = 0.0
        self.audio_stream = None
        self.clap_detector = AcousticClapDetector(sample_rate=16000, block_size=256)
        self.acoustic_shield = LaptopAcousticShield(sample_rate=16000, block_size=256)
        self.speaker_ref_buffer = np.zeros(256, dtype=np.float32)
        self.clean_mic_rms: float = 0.0
        self.snr_gain_db: float = 0.0
        self.erle_db: float = 0.0
        self.shield_metrics: dict = {}
        self.loop = None

    def _audio_callback(self, indata, frames, time_info, status):
        try:
            raw_mono = np.mean(indata, axis=1) if indata.ndim > 1 and indata.shape[1] > 1 else indata.flatten()
            self.live_mic_rms = float(np.sqrt(np.mean(raw_mono**2)))

            # ==============================================================
            # BRANCH A: UNFILTERED RAW MIC -> CLAP DETECTOR -> UI
            # Preserves original physical impulse sharpness; ZERO filtering!
            # ==============================================================
            if self.clap_mode in ("acoustic", "hybrid"):
                phys_ts = time.time()
                clap_evt = self.clap_detector.process_block(raw_mono, phys_ts)
                if clap_evt:
                    # In hybrid mode: check visual correlation if hands recently tracked
                    if self.clap_mode == "hybrid" and self.tracker and len(self.tracker.distance_history) > 0:
                        last_t, last_d, _, _ = self.tracker.distance_history[-1]
                        if (time.time() - last_t < 0.35) and last_d < 0.35:
                            clap_evt["source"] = "hybrid_clap"
                            clap_evt["confidence"] = 0.99

                    if self.tracker:
                        self.tracker.ui_state = "EXPANDED"

                    logger.info(
                        f"Local Clap Detected [{clap_evt['source']}]: "
                        f"latency={clap_evt['detection_latency_ms']}ms, "
                        f"peak={clap_evt['peak_amplitude']}"
                    )
                    if self.loop and self.loop.is_running():
                        asyncio.run_coroutine_threadsafe(self.broadcast(clap_evt), self.loop)

            # ==============================================================
            # BRANCH B: RAW STEREO MIC -> LAPTOP ACOUSTIC SHIELD -> STT
            # Multi-stage beamformer + AEC + spectral suppression + consonant protection + AGC
            # ==============================================================
            clean_audio, metrics = self.acoustic_shield.process(indata, self.speaker_ref_buffer)
            self.clean_mic_rms = metrics["output_rms"]
            self.snr_gain_db = metrics["snr_improvement_db"]
            self.erle_db = metrics["erle_db"]
            self.shield_metrics = metrics

            # Smoothly decay reference buffer
            self.speaker_ref_buffer *= 0.5
        except Exception as e:
            pass

    async def register(self, websocket):
        self.clients.add(websocket)
        logger.info(f"Client connected from {websocket.remote_address}. Total: {len(self.clients)}")
        state_msg = {
            "type": "status",
            "ui_state": self.tracker.ui_state if self.tracker else "COLLAPSED",
            "server": "VOXY Gesture Service v1.0",
        }
        await websocket.send(json.dumps(state_msg))
        try:
            async for message in websocket:
                try:
                    data = json.loads(message)
                    if data.get("cmd") == "toggle":
                        if self.tracker:
                            self.tracker.ui_state = (
                                "COLLAPSED" if self.tracker.ui_state == "EXPANDED" else "EXPANDED"
                            )
                            await self.broadcast({
                                "type": "gesture",
                                "action": self.tracker.ui_state,
                                "source": "command",
                                "confidence": 1.0,
                                "timestamp": time.time(),
                            })
                    elif data.get("type") in ("voice", "voice_state", "audio_output", "barge_in"):
                        # Broadcast voice pipeline states to all UI clients
                        await self.broadcast(data)
                    elif data.get("type") == "clap_ack":
                        logger.info(
                            f"[HUD ACK] CLAP_DETECTED: total_latency={data.get('total_latency_ms')}ms, "
                            f"det_latency={data.get('detection_latency_ms')}ms, "
                            f"source={data.get('source')}"
                        )
                except Exception:
                    pass
        except websockets.ConnectionClosed:
            pass
        finally:
            self.clients.remove(websocket)
            logger.info(f"Client disconnected. Total: {len(self.clients)}")

    async def broadcast(self, message_dict: dict):
        if not self.clients:
            return
        payload = json.dumps(message_dict)
        coros = [client.send(payload) for client in self.clients]
        await asyncio.gather(*coros, return_exceptions=True)

    async def run_loop(self):
        self.loop = asyncio.get_running_loop()
        logger.info(f"Initializing MediaPipe HandLandmarker with {MODEL_PATH}...")
        self.tracker = GestureTracker(MODEL_PATH)

        # Initialize real microphone hardware energy monitor and clap detector
        try:
            self.audio_stream = sd.InputStream(
                samplerate=16000, channels=1, blocksize=256, callback=self._audio_callback
            )
            self.audio_stream.start()
            logger.info(f"Real microphone active (16kHz WASAPI). Clap detection mode: {self.clap_mode.upper()}.")
        except Exception as e:
            logger.warning(f"Microphone audio stream init non-fatal warning: {e}")

        logger.info(f"Opening camera {self.camera_idx}...")
        cap = cv2.VideoCapture(self.camera_idx)
        if not cap.isOpened():
            logger.warning(f"Cannot open camera index {self.camera_idx}. Proceeding in acoustic-only mode without camera.")
            self.running = True
            try:
                while self.running:
                    # Broadcast audio RMS even when camera is off
                    await self.broadcast({
                        "type": "audio",
                        "mic_rms": round(self.live_mic_rms, 5),
                        "timestamp": time.time(),
                    })
                    await asyncio.sleep(0.04)
            finally:
                if self.audio_stream:
                    try:
                        self.audio_stream.stop()
                        self.audio_stream.close()
                    except Exception:
                        pass
            return

        cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
        cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
        self.running = True
        logger.info("Gesture detection active. Ready for hand expansion, clap, and collapse.")

        t_start = time.time()
        last_tracking_emit = 0.0
        last_audio_emit = 0.0

        try:
            while self.running:
                loop_start = time.time()
                ret, frame = cap.read()
                if not ret:
                    await asyncio.sleep(0.02)
                    continue

                ts_ms = int((time.time() - t_start) * 1000)
                gesture_evt, tracking_evt = self.tracker.process_frame(frame, ts_ms)

                if gesture_evt:
                    await self.broadcast(gesture_evt)

                if tracking_evt and (time.time() - last_tracking_emit >= 0.05):
                    last_tracking_emit = time.time()
                    await self.broadcast(tracking_evt)

                # Broadcast real live microphone audio energy
                if time.time() - last_audio_emit >= 0.04:
                    last_audio_emit = time.time()
                    await self.broadcast({
                        "type": "audio",
                        "mic_rms": round(self.live_mic_rms, 5),
                        "timestamp": time.time(),
                    })

                if self.tracker.no_hands_frames > 90:
                    await asyncio.sleep(0.08)
                else:
                    elapsed = time.time() - loop_start
                    sleep_time = max(0.001, (1.0 / 30.0) - elapsed)
                    await asyncio.sleep(sleep_time)

        finally:
            cap.release()
            if self.audio_stream:
                try:
                    self.audio_stream.stop()
                    self.audio_stream.close()
                except Exception:
                    pass
            if self.tracker:
                self.tracker.close()
            logger.info("Camera, audio monitor, and HandLandmarker released.")


class VoiceUdpProtocol(asyncio.DatagramProtocol):
    def __init__(self, broadcast_fn):
        self.broadcast_fn = broadcast_fn

    def datagram_received(self, data, addr):
        try:
            msg = json.loads(data.decode("utf-8"))
            asyncio.create_task(self.broadcast_fn(msg))
        except Exception:
            pass


async def main():
    parser = argparse.ArgumentParser(description="VOXY Gesture Service")
    parser.add_argument("--camera", type=int, default=0, help="Webcam device index")
    parser.add_argument("--port", type=int, default=WS_PORT, help="WebSocket port")
    parser.add_argument("--clap-mode", choices=["acoustic", "visual", "hybrid"], default="hybrid", help="Clap detection mode (acoustic, visual, hybrid)")
    parser.add_argument("--test-mode", action="store_true", help="Run self-test verification and exit")
    args = parser.parse_args()

    if args.test_mode:
        logger.info("Running Gesture Service Self-Test...")
        tracker = GestureTracker(MODEL_PATH)
        import numpy as np
        frame = np.zeros((480, 640, 3), dtype=np.uint8)
        gesture_evt, track_evt = tracker.process_frame(frame, 1000)
        tracker.close()
        logger.info("Self-test complete. Tracker initialized successfully. No errors.")
        return

    server = GestureServiceServer(camera_idx=args.camera, clap_mode=args.clap_mode)
    ws_server = await websockets.serve(server.register, WS_HOST, args.port)
    logger.info(f"WebSocket server listening on ws://{WS_HOST}:{args.port}")

    loop = asyncio.get_running_loop()
    udp_transport, _ = await loop.create_datagram_endpoint(
        lambda: VoiceUdpProtocol(server.broadcast),
        local_addr=(WS_HOST, 18889),
    )
    logger.info(f"UDP voice telemetry listener listening on {WS_HOST}:18889")

    try:
        await asyncio.gather(
            ws_server.wait_closed(),
            server.run_loop(),
        )
    finally:
        udp_transport.close()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        logger.info("Gesture Service stopped by user.")
