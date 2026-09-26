#!/usr/bin/env python3
"""
VOXY LIVE SYSTEM — Unified Holographic UI, Real WASAPI Audio & Voice Stack
Orchestrates:
1. Physical Intel SST Microphone Capture (WASAPI Stereo Array)
2. Branch A: Unfiltered RAW Mic -> Acoustic Clap Detector -> Holographic UI (Instant Open)
3. Branch B: Real-Time Laptop Acoustic Shield (Beamformer + FDAF AEC + Wiener Suppressor + Consonant Protection + AGC)
4. Active WASAPI Render Loopback Capture (Realtek Speaker Reference for AEC)
5. MediaPipe Hands Camera Gesture Tracker (Hands collapse -> UI Close, Hands expand -> UI Open)
6. Real-Time VAD & Conversational Voice Loop:
   - Speech Capture -> Acoustic Shield
   - Groq Whisper STT (whisper-large-v3-turbo)
   - VOXY Brain -> Ollama Cloud (gpt-oss:120b-cloud)
   - Real Streaming TTS Playback to physical speakers
   - Acoustic Double-Talk / Barge-in Cutoff (<50ms cutoff)
7. WebSocket Event Bridge on ws://127.0.0.1:18888 (Syncing Holographic Overlay)
"""

import os
import sys
import io
import time
import math
import json
import asyncio
import logging
import threading
import collections
import websockets
import numpy as np
import scipy.io.wavfile as wav
import sounddevice as sd
import requests
import win32com.client
import cv2
import ctypes
from ctypes import wintypes
from comtypes import GUID, IUnknown, COMMETHOD, HRESULT

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] [VOXY] %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("VOXY-LIVE")

# Ensure UTF-8 output on Windows console
if sys.stdout.encoding != 'utf-8':
    try:
        sys.stdout.reconfigure(encoding='utf-8')
    except Exception:
        pass

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from acoustic_shield import LaptopAcousticShield
from clap_detector import AcousticClapDetector

try:
    from gesture_service import GestureTracker, MODEL_PATH
except ImportError:
    GestureTracker = None
    MODEL_PATH = None

# Configuration
WS_HOST = "127.0.0.1"
WS_PORT = 18888
GROQ_API_KEY = os.environ.get("VOXY_API_KEYS_GROQ", "")
GROQ_URL = "https://api.groq.com/openai/v1/audio/transcriptions"
GROQ_MODEL = "whisper-large-v3-turbo"
OLLAMA_URL = os.environ.get("VOXY_OLLAMA_URL", "http://127.0.0.1:11434")
OLLAMA_MODEL = os.environ.get("VOXY_OLLAMA_MODEL", "gpt-oss:120b-cloud")

SYSTEM_PROMPT = (
    "You are VOXY, an advanced real-time voice AI assistant with a holographic spatial desktop interface. "
    "Keep your spoken responses concise, direct, natural, and conversational (1 to 2 sentences max) "
    "suitable for real-time speech output. Never use markdown, bullet points, asterisks, or code blocks in spoken responses."
)


# ==============================================================================
# 1. WINDOWS WASAPI RENDER LOOPBACK CAPTURE (AEC REFERENCE)
# ==============================================================================

class IAudioCaptureClient(IUnknown):
    _iid_ = GUID('{C8ADBD64-E71E-48a0-A4DE-185C395CD317}')
    _methods_ = [
        COMMETHOD([], HRESULT, 'GetBuffer',
                  (['out'], ctypes.POINTER(ctypes.c_void_p), 'ppData'),
                  (['out'], ctypes.POINTER(wintypes.UINT), 'pNumFramesToRead'),
                  (['out'], ctypes.POINTER(wintypes.DWORD), 'pdwFlags'),
                  (['out'], ctypes.POINTER(ctypes.c_ulonglong), 'pu64DevicePosition'),
                  (['out'], ctypes.POINTER(ctypes.c_ulonglong), 'pu64QPCPosition')),
        COMMETHOD([], HRESULT, 'ReleaseBuffer',
                  (['in'], wintypes.UINT, 'NumFramesRead')),
        COMMETHOD([], HRESULT, 'GetNextPacketSize',
                  (['out'], ctypes.POINTER(wintypes.UINT), 'pNumFramesInNextPacket')),
    ]

class IAudioClient(IUnknown):
    _iid_ = GUID('{1CB9AD4C-DBFA-4c32-B178-C2F568A703B2}')
    _methods_ = [
        COMMETHOD([], HRESULT, 'Initialize',
                  (['in'], wintypes.DWORD, 'ShareMode'),
                  (['in'], wintypes.DWORD, 'StreamFlags'),
                  (['in'], ctypes.c_longlong, 'hnsBufferDuration'),
                  (['in'], ctypes.c_longlong, 'hnsPeriodicity'),
                  (['in'], ctypes.c_void_p, 'pFormat'),
                  (['in'], ctypes.c_void_p, 'AudioSessionGuid')),
        COMMETHOD([], HRESULT, 'GetBufferSize',
                  (['out'], ctypes.POINTER(wintypes.UINT), 'pNumBufferFrames')),
        COMMETHOD([], HRESULT, 'GetStreamLatency'),
        COMMETHOD([], HRESULT, 'GetCurrentPadding'),
        COMMETHOD([], HRESULT, 'IsFormatSupported'),
        COMMETHOD([], HRESULT, 'GetMixFormat',
                  (['out'], ctypes.POINTER(ctypes.c_void_p), 'ppDeviceFormat')),
        COMMETHOD([], HRESULT, 'GetDevicePeriod'),
        COMMETHOD([], HRESULT, 'Start'),
        COMMETHOD([], HRESULT, 'Stop'),
        COMMETHOD([], HRESULT, 'Reset'),
        COMMETHOD([], HRESULT, 'SetEventHandle'),
        COMMETHOD([], HRESULT, 'GetService',
                  (['in'], ctypes.POINTER(GUID), 'riid'),
                  (['out'], ctypes.POINTER(ctypes.c_void_p), 'ppv')),
    ]

class IMMDevice(IUnknown):
    _iid_ = GUID('{D666063F-1587-4E43-81F1-B948E807363F}')
    _methods_ = [
        COMMETHOD([], HRESULT, 'Activate',
                  (['in'], ctypes.POINTER(GUID), 'iid'),
                  (['in'], wintypes.DWORD, 'dwClsCtx'),
                  (['in'], ctypes.c_void_p, 'pActivationParams'),
                  (['out'], ctypes.POINTER(ctypes.c_void_p), 'ppInterface')),
    ]

class IMMDeviceEnumerator(IUnknown):
    _iid_ = GUID('{A95664D2-9614-4F35-A746-DE8DB63617E6}')
    _methods_ = [
        COMMETHOD([], HRESULT, 'EnumAudioEndpoints'),
        COMMETHOD([], HRESULT, 'GetDefaultAudioEndpoint',
                  (['in'], wintypes.DWORD, 'dataFlow'),
                  (['in'], wintypes.DWORD, 'role'),
                  (['out'], ctypes.POINTER(ctypes.POINTER(IMMDevice)), 'ppEndpoint')),
    ]

class WasapiLoopbackCapture:
    """
    Captures live speaker output from Windows WASAPI render endpoint
    using AUDCLNT_STREAMFLAGS_LOOPBACK to feed real-time AEC reference.
    """
    def __init__(self, target_sr: int = 16000):
        self.target_sr = target_sr
        self.ring_buffer = collections.deque(maxlen=target_sr * 2) # 2 sec
        self.running = False
        self.thread = None
        self.lock = threading.Lock()
        self.active = False
        self.live_output_rms = 0.0

    def start(self):
        self.running = True
        self.thread = threading.Thread(target=self._loop, daemon=True, name="WasapiLoopbackThread")
        self.thread.start()

    def stop(self):
        self.running = False
        if self.thread:
            self.thread.join(timeout=1.0)

    def _loop(self):
        ole32 = ctypes.oledll.ole32
        ole32.CoInitialize(None)
        try:
            CLSID_MMDeviceEnumerator = GUID('{BCDE0395-E52F-467C-8E3D-C4579291692E}')
            pEnum = ctypes.POINTER(IMMDeviceEnumerator)()
            ole32.CoCreateInstance(ctypes.byref(CLSID_MMDeviceEnumerator), None, 1, ctypes.byref(IMMDeviceEnumerator._iid_), ctypes.byref(pEnum))
            pDev = pEnum.GetDefaultAudioEndpoint(0, 0) # eRender, eConsole
            pClientPtr = pDev.Activate(ctypes.byref(IAudioClient._iid_), 1, None)
            client = ctypes.cast(pClientPtr, ctypes.POINTER(IAudioClient))
            pFormat = client.GetMixFormat()

            # AUDCLNT_STREAMFLAGS_LOOPBACK = 0x00020000
            hr = client.Initialize(0, 0x00020000, 10000000, 0, pFormat, None)
            if hr != 0:
                logger.warning(f"WASAPI loopback client.Initialize returned {hex(hr)}")
                return

            pCapturePtr = client.GetService(ctypes.byref(IAudioCaptureClient._iid_))
            capture = ctypes.cast(pCapturePtr, ctypes.POINTER(IAudioCaptureClient))

            client.Start()
            self.active = True
            logger.info("WASAPI Loopback Capture active -> real-time speaker AEC reference connected.")

            while self.running:
                pData, frames, flags, _, _ = capture.GetBuffer()
                if frames > 0 and pData:
                    buf_ptr = ctypes.cast(pData, ctypes.POINTER(ctypes.c_float * (frames * 2)))
                    stereo = np.frombuffer(buf_ptr.contents, dtype=np.float32).reshape(-1, 2)
                    mono = 0.5 * (stereo[:, 0] + stereo[:, 1])
                    # Resample 48kHz -> 16kHz (factor 3)
                    mono_16k = mono[::3]
                    with self.lock:
                        self.ring_buffer.extend(mono_16k)
                        self.live_output_rms = float(np.sqrt(np.mean(mono**2)))
                    capture.ReleaseBuffer(frames)
                else:
                    time.sleep(0.005)
            client.Stop()
        except Exception as e:
            logger.warning(f"WASAPI Loopback exception: {e}")
        finally:
            self.active = False
            ole32.CoUninitialize()

    def get_latest_block(self, block_size: int = 256) -> np.ndarray:
        with self.lock:
            if len(self.ring_buffer) < block_size:
                return np.zeros(block_size, dtype=np.float32)
            samples = [self.ring_buffer[-block_size + i] for i in range(block_size)]
            return np.array(samples, dtype=np.float32)


# ==============================================================================
# 2. CAMERA GESTURE TRACKER (HANDS EXPAND / COLLAPSE)
# ==============================================================================

class CameraGestureThread:
    def __init__(self, broadcast_fn, camera_idx: int = 0):
        self.broadcast_fn = broadcast_fn
        self.camera_idx = camera_idx
        self.running = False
        self.thread = None

    def start(self):
        if not GestureTracker or not MODEL_PATH or not os.path.exists(MODEL_PATH):
            logger.warning("Gesture tracking model not found. Running in acoustic-only mode.")
            return
        self.running = True
        self.thread = threading.Thread(target=self._loop, daemon=True, name="CameraGestureThread")
        self.thread.start()

    def stop(self):
        self.running = False
        if self.thread:
            self.thread.join(timeout=1.0)

    def _loop(self):
        try:
            tracker = GestureTracker(MODEL_PATH)
        except Exception as e:
            logger.warning(f"MediaPipe GestureTracker init warning: {e}")
            return

        cap = cv2.VideoCapture(self.camera_idx)
        if not cap.isOpened():
            logger.warning(f"Webcam index {self.camera_idx} unavailable. Gesture tracking skipped.")
            tracker.close()
            return

        cap.set(cv2.CAP_PROP_FRAME_WIDTH, 640)
        cap.set(cv2.CAP_PROP_FRAME_HEIGHT, 480)
        logger.info(f"Webcam {self.camera_idx} active for hand expansion and collapse gestures.")
        t_start = time.time()

        try:
            while self.running:
                ret, frame = cap.read()
                if not ret:
                    time.sleep(0.02)
                    continue
                ts_ms = int((time.time() - t_start) * 1000)
                gesture_evt, _ = tracker.process_frame(frame, ts_ms)
                if gesture_evt and self.broadcast_fn:
                    logger.info(f"[CAMERA GESTURE] {gesture_evt['action']} ({gesture_evt['source']})")
                    self.broadcast_fn(gesture_evt)
                time.sleep(0.03)
        except Exception as e:
            logger.warning(f"Camera gesture loop exception: {e}")
        finally:
            cap.release()
            tracker.close()


# ==============================================================================
# 3. CONVERSATIONAL VOICE ORCHESTRATOR
# ==============================================================================

class VoxyVoiceEngine:
    def __init__(self, loopback: WasapiLoopbackCapture, broadcast_fn):
        self.loopback = loopback
        self.broadcast_fn = broadcast_fn
        self.shield = LaptopAcousticShield(sample_rate=16000, block_size=256)
        self.clap_detector = AcousticClapDetector(sample_rate=16000, block_size=256)
        
        # TTS Engine
        self.tts = win32com.client.Dispatch("SAPI.SpVoice")
        self.is_speaking = False
        self.current_state = "Idle"
        
        # Conversation History
        self.messages = [{"role": "system", "content": SYSTEM_PROMPT}]
        
        # VAD & Speech Buffer
        self.speech_buffer = []
        self.is_user_speaking = False
        self.silence_blocks = 0
        self.speech_start_time = 0.0
        self.speech_energy_thresh = 0.016
        self.min_speech_blocks = 12  # ~192ms minimum speech
        self.max_silence_blocks = 35 # ~560ms pause triggers end-of-turn
        
        # Barge-in tracking
        self.barge_in_triggered = False

    def update_state(self, state: str, text: str = ""):
        self.current_state = state
        logger.info(f"[VOICE STATE] -> {state.upper()} ({text})")
        msg = {"type": "voice", "state": state, "text": text}
        if self.broadcast_fn:
            self.broadcast_fn(msg)

    def process_audio_block(self, raw_stereo_or_mono: np.ndarray):
        """
        Called every ~16ms with 256 samples from physical Intel SST mic.
        """
        # ======================================================================
        # BRANCH A: UNFILTERED RAW MONO MIC -> CLAP DETECTOR -> OVERLAY
        # ======================================================================
        raw_mono = (
            np.mean(raw_stereo_or_mono, axis=1)
            if raw_stereo_or_mono.ndim > 1 and raw_stereo_or_mono.shape[1] > 1
            else raw_stereo_or_mono.flatten()
        )
        phys_ts = time.time()
        clap_evt = self.clap_detector.process_block(raw_mono, phys_ts)
        if clap_evt and clap_evt["action"] == "EXPANDED":
            logger.info(f"*** PHYSICAL CLAP DETECTED *** [lat={clap_evt['detection_latency_ms']:.2f}ms] -> EXPANDING UI")
            if self.broadcast_fn:
                self.broadcast_fn(clap_evt)

        # ======================================================================
        # BRANCH B: RAW MIC -> ACOUSTIC SHIELD (with Live WASAPI Render Reference)
        # ======================================================================
        spk_ref = self.loopback.get_latest_block(256)
        clean_audio, metrics = self.shield.process(raw_stereo_or_mono, spk_ref)
        clean_rms = metrics["output_rms"]

        # ======================================================================
        # BARGE-IN INTERRUPTION DETECTION (While VOXY is speaking)
        # ======================================================================
        if self.is_speaking:
            # Broadcast speaker energy to radial oscilloscope
            if self.broadcast_fn:
                self.broadcast_fn({"type": "audio_output", "output_rms": self.loopback.live_output_rms})

            # Check if user speaks over VOXY (Double-Talk / Acoustic Interruption)
            if clean_rms > 0.045:
                logger.info(f"[BARGE-IN] Acoustic Interruption detected (clean_rms={clean_rms:.4f})! Stopping TTS...")
                self.barge_in_triggered = True
                self.stop_tts()
                self.update_state("Interrupted", "Interrupted by user speech")
                if self.broadcast_fn:
                    self.broadcast_fn({"type": "barge_in"})
                time.sleep(0.05)
                self.update_state("Listening", "Listening...")
                self.speech_buffer.clear()
                self.is_user_speaking = True
                self.silence_blocks = 0
                return

        # ======================================================================
        # VOICE ACTIVITY DETECTION & TURN CAPTURE (When VOXY is not speaking)
        # ======================================================================
        if not self.is_speaking:
            # Broadcast live mic energy to radial oscilloscope
            if self.broadcast_fn:
                self.broadcast_fn({"type": "audio", "mic_rms": clean_rms})

            if clean_rms > self.speech_energy_thresh:
                if not self.is_user_speaking:
                    self.is_user_speaking = True
                    self.speech_start_time = time.time()
                    self.speech_buffer.clear()
                    self.update_state("Listening", "Hearing user speech...")
                self.speech_buffer.append(clean_audio)
                self.silence_blocks = 0
            else:
                if self.is_user_speaking:
                    self.speech_buffer.append(clean_audio)
                    self.silence_blocks += 1

                    # User has stopped speaking
                    if self.silence_blocks >= self.max_silence_blocks:
                        self.is_user_speaking = False
                        total_blocks = len(self.speech_buffer)
                        if total_blocks >= self.min_speech_blocks:
                            captured_speech = np.concatenate(self.speech_buffer)
                            logger.info(f"Captured speech turn: {len(captured_speech)} samples ({len(captured_speech)/16000:.2f}s)")
                            threading.Thread(
                                target=self.handle_speech_turn,
                                args=(captured_speech,),
                                daemon=True
                            ).start()
                        else:
                            logger.debug("Speech turn too short, discarded.")
                            self.update_state("Idle", "Ready")
                        self.speech_buffer.clear()

    def handle_speech_turn(self, speech_audio: np.ndarray):
        """Processes captured turn: STT -> LLM -> TTS."""
        try:
            # 1. Groq Whisper STT
            self.update_state("Thinking", "Transcribing speech...")
            t_stt_0 = time.perf_counter()
            scaled = np.clip(speech_audio * 32767.0, -32768.0, 32767.0).astype(np.int16)
            buf = io.BytesIO()
            wav.write(buf, 16000, scaled)
            buf.seek(0)

            headers = {"Authorization": f"Bearer {GROQ_API_KEY}"}
            files = {"file": ("turn.wav", buf, "audio/wav")}
            data = {"model": GROQ_MODEL, "response_format": "json"}

            resp = requests.post(GROQ_URL, headers=headers, files=files, data=data, timeout=12.0)
            stt_latency = (time.perf_counter() - t_stt_0) * 1000.0

            if resp.status_code != 200:
                logger.error(f"Groq STT error {resp.status_code}: {resp.text}")
                self.update_state("Idle", f"STT error {resp.status_code}")
                return

            transcript = resp.json().get("text", "").strip()
            logger.info(f"[STT] Recognized ({stt_latency:.1f}ms): \"{transcript}\"")

            if not transcript or len(transcript) < 2 or transcript in (".", "!", "?", "Thank you.", "Bye."):
                logger.info("Empty or hallucinated silence transcription, ignoring.")
                self.update_state("Idle", "Ready")
                return

            # 2. VOXY Brain (Ollama Cloud gpt-oss:120b-cloud)
            self.update_state("Generating", f"Prompt: {transcript}")
            self.messages.append({"role": "user", "content": transcript})

            t_llm_0 = time.perf_counter()
            payload = {
                "model": OLLAMA_MODEL,
                "messages": self.messages,
                "stream": False,
            }
            llm_resp = requests.post(f"{OLLAMA_URL}/api/chat", json=payload, timeout=25.0)
            llm_latency = (time.perf_counter() - t_llm_0) * 1000.0

            if llm_resp.status_code != 200:
                logger.error(f"Ollama error {llm_resp.status_code}: {llm_resp.text}")
                self.update_state("Idle", f"LLM error {llm_resp.status_code}")
                return

            response_content = llm_resp.json().get("message", {}).get("content", "").strip()
            clean_reply = response_content.replace("*", "").replace("#", "").strip()
            self.messages.append({"role": "assistant", "content": clean_reply})
            logger.info(f"[LLM] {OLLAMA_MODEL} ({llm_latency:.1f}ms): \"{clean_reply}\"")

            # 3. Streaming TTS Playback to physical speakers
            self.speak_response(clean_reply)

        except Exception as e:
            logger.error(f"Turn processing error: {e}", exc_info=True)
            self.update_state("Idle", f"Error: {e}")

    def speak_response(self, text: str):
        self.is_speaking = True
        self.barge_in_triggered = False
        self.update_state("Speaking", text)
        logger.info(f"[TTS] Playing through laptop Realtek speakers: \"{text}\"")

        # Speak asynchronously (SVSFlagsAsync = 1) so audio plays while mic monitors for barge-in
        self.tts.Speak(text, 1)

        while True:
            if self.barge_in_triggered:
                break
            status = self.tts.Status
            if status.RunningState != 2:
                break
            time.sleep(0.04)

        self.is_speaking = False
        if not self.barge_in_triggered:
            self.update_state("Idle", "Listening for next query...")

    def stop_tts(self):
        try:
            self.tts.Speak("", 2)
        except Exception:
            pass
        self.is_speaking = False


# ==============================================================================
# 4. UNIFIED SERVER & REAL HARDWARE AUDIO RUNNER
# ==============================================================================

class VoxyLiveServer:
    def __init__(self):
        self.clients = set()
        self.loop = None
        self.loopback = WasapiLoopbackCapture(target_sr=16000)
        self.voice_engine = VoxyVoiceEngine(self.loopback, self.sync_broadcast)
        self.camera_thread = CameraGestureThread(self.sync_broadcast, camera_idx=0)
        self.audio_stream = None

    def sync_broadcast(self, msg_dict: dict):
        if self.loop and self.loop.is_running() and self.clients:
            asyncio.run_coroutine_threadsafe(self.async_broadcast(msg_dict), self.loop)

    async def async_broadcast(self, msg_dict: dict):
        if not self.clients:
            return
        payload = json.dumps(msg_dict)
        coros = [c.send(payload) for c in self.clients]
        await asyncio.gather(*coros, return_exceptions=True)

    async def register(self, websocket):
        self.clients.add(websocket)
        logger.info(f"[HUD] Holographic Overlay connected from {websocket.remote_address}. Active: {len(self.clients)}")
        await websocket.send(json.dumps({
            "type": "status",
            "ui_state": "EXPANDED",
            "voice_state": self.voice_engine.current_state,
            "server": "VOXY Live System v1.0"
        }))
        try:
            async for message in websocket:
                pass
        except websockets.ConnectionClosed:
            pass
        finally:
            self.clients.discard(websocket)
            logger.info(f"[HUD] Client disconnected. Remaining: {len(self.clients)}")

    def _audio_callback(self, indata, frames, time_info, status):
        try:
            self.voice_engine.process_audio_block(indata)
        except Exception as e:
            pass

    async def run(self):
        self.loop = asyncio.get_running_loop()

        # 1. Start WASAPI render loopback
        logger.info("Initializing Windows WASAPI Render Loopback for AEC...")
        self.loopback.start()
        time.sleep(0.1)

        # 2. Start Camera Gesture tracking
        logger.info("Starting Camera Gesture Tracking thread...")
        self.camera_thread.start()

        # 3. Find Intel SST Mic Device
        devices = sd.query_devices()
        mic_dev = None
        for idx, d in enumerate(devices):
            name = d['name'].lower()
            if 'intel' in name and 'micro' in name and d['max_input_channels'] > 0:
                mic_dev = idx
                break
        if mic_dev is None:
            mic_dev = 1 # Fallback

        logger.info(f"Opening physical microphone [{mic_dev}]: {devices[mic_dev]['name']} (channels=2, 16kHz)...")
        self.audio_stream = sd.InputStream(
            device=mic_dev,
            samplerate=16000,
            channels=2,
            blocksize=256,
            dtype='float32',
            callback=self._audio_callback
        )
        self.audio_stream.start()
        logger.info("Physical Intel SST microphone capture active.")

        # 4. Start WebSocket server for Holographic UI
        ws_server = await websockets.serve(self.register, WS_HOST, WS_PORT)
        logger.info(f"WebSocket live event bridge online at ws://{WS_HOST}:{WS_PORT}")
        logger.info("==================================================================")
        logger.info("VOXY LIVE STACK IS RUNNING AND READY FOR REAL HUMAN SPEECH!")
        logger.info("  1. Physical Clap -> Opens Holographic UI")
        logger.info("  2. Hands together -> Collapses Holographic UI")
        logger.info("  3. Speak: 'VOXY, hello.' -> Whisper STT -> gpt-oss:120b-cloud -> Realtek Speaker")
        logger.info("  4. Speak while VOXY speaks -> Acoustic Barge-in instant cutoff")
        logger.info("==================================================================")

        try:
            await ws_server.wait_closed()
        finally:
            if self.audio_stream:
                self.audio_stream.stop()
                self.audio_stream.close()
            self.loopback.stop()
            self.camera_thread.stop()


def main():
    ctypes.oledll.ole32.CoInitialize(None)
    logger.info("Initializing VOXY Live System...")
    server = VoxyLiveServer()
    try:
        asyncio.run(server.run())
    except KeyboardInterrupt:
        logger.info("VOXY Live System stopped by user.")
    finally:
        ctypes.oledll.ole32.CoUninitialize()


if __name__ == "__main__":
    main()
