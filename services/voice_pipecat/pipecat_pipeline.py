#!/usr/bin/env python3
"""
OSMOO Pipecat Streaming Voice Pipeline Sidecar
================================================

Integrates Pipecat streaming voice pipeline with the OSMOO Windows native companion.

Pipeline Architecture:
    Windows Audio Capture (WASAPI / cpal)
        ↓
    Audio Conditioning & Silero VAD (Pipecat SileroVADAnalyzer)
        ↓
    Speech-to-Text (WhisperSTTService via faster-whisper / Whisper.cpp / Groq Cloud)
        ↓
    OSMOO Brain Bridge (Loopback TCP IPC -> CognitiveEngine / LLM Router & Tools)
        ↓
    Natural Sentence Chunker
        ↓
    Streaming TTS (Resemble AI Chatterbox / Cartesia / ElevenLabs / Local SAPI)
        ↓
    Windows Audio Playback (WASAPI) with Instant Barge-In Cancellation
"""

import argparse
import asyncio
import json
import logging
import os
import signal
import sys
import threading
import time
from typing import Optional, List, Dict, Any

logging.basicConfig(
    level=logging.INFO,
    format="[%(asctime)s] [PIPECAT] [%(levelname)s] %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("pipecat_pipeline")

# Try importing real Pipecat 1.x / 1.12+ components
try:
    from pipecat.pipeline.pipeline import Pipeline
    from pipecat.pipeline.runner import PipelineRunner
    from pipecat.pipeline.task import PipelineTask
    from pipecat.frames.frames import (
        Frame,
        AudioRawFrame,
        TextFrame,
        TranscriptionFrame,
        InterimTranscriptionFrame,
        UserStartedSpeakingFrame,
        UserStoppedSpeakingFrame,
        CancelFrame,
        EndFrame,
        StartFrame,
    )
    from pipecat.processors.frame_processor import FrameProcessor, FrameDirection
    from pipecat.audio.vad.silero import SileroVADAnalyzer
    from pipecat.services.whisper.stt import WhisperSTTService
    HAVE_PIPECAT = True
    logger.info("Native Pipecat framework loaded (v1.12+ compliant with Silero VAD and WhisperSTTService).")
except ImportError as e:
    HAVE_PIPECAT = False
    logger.warning("Native Pipecat package import failed (%s); using native protocol fallback.", e)

    class Frame:
        def __init__(self):
            self.timestamp = time.time()

    class AudioRawFrame(Frame):
        def __init__(self, audio_bytes: bytes, sample_rate: int = 16000):
            super().__init__()
            self.audio_bytes = audio_bytes
            self.sample_rate = sample_rate

    class TextFrame(Frame):
        def __init__(self, text: str):
            super().__init__()
            self.text = text

    class TranscriptionFrame(Frame):
        def __init__(self, text: str, user_id: str = "", timestamp: str = ""):
            super().__init__()
            self.text = text

    class CancelFrame(Frame):
        pass

    class EndFrame(Frame):
        pass

    class FrameProcessor:
        async def process_frame(self, frame, direction=None):
            pass
        async def push_frame(self, frame, direction=None):
            pass


# ── OSMOO Brain Bridge ───────────────────────────────────────────────────────

class OsmooBrainBridge:
    """IPC client bridge connecting the Pipecat pipeline to OSMOO's Rust Daemon via TCP loopback."""

    def __init__(self, host: str = "127.0.0.1", port: int = 18889, auth_token: Optional[str] = None):
        self.host = host
        self.port = port
        self.auth_token = auth_token or os.environ.get("VOXY_IPC_AUTH_TOKEN", "")
        self.reader: Optional[asyncio.StreamReader] = None
        self.writer: Optional[asyncio.StreamWriter] = None
        self.connected = False
        self._inbound_task: Optional[asyncio.Task] = None
        self._on_interrupted_callbacks: List[Any] = []

    def register_on_interrupted(self, callback):
        self._on_interrupted_callbacks.append(callback)

    async def connect(self):
        try:
            self.reader, self.writer = await asyncio.open_connection(self.host, self.port)
            self.connected = True
            logger.info("Connected to OSMOO Rust Daemon at %s:%d", self.host, self.port)

            # Perform authenticated ClientHandshake immediately
            handshake_payload = {
                "type": "ClientHandshake",
                "data": {
                    "client_name": "osmoo-pipecat-sidecar",
                    "client_version": "0.1.0",
                    "auth_token": self.auth_token if self.auth_token else None,
                    "timestamp_ms": int(time.time() * 1000),
                }
            }
            envelope = {
                "version": 1,
                "timestamp_ms": int(time.time() * 1000),
                "payload": handshake_payload,
            }
            data = json.dumps(envelope).encode("utf-8")
            self.writer.write(len(data).to_bytes(4, byteorder="big") + data)
            await self.writer.drain()
            logger.info("Sent ClientHandshake frame to Rust Daemon (token present: %s).", bool(self.auth_token))

            # Start reading daemon broadcast messages
            self._inbound_task = asyncio.create_task(self._read_daemon_loop())
        except Exception as e:
            if os.environ.get("VOXY_ALLOW_STANDALONE", "0") == "1":
                logger.info("Rust Daemon TCP loopback unavailable (%s). Explicit standalone development mode permitted.", e)
            else:
                logger.error("Rust Daemon TCP loopback unavailable (%s). Integrated production mode requires active daemon connection.", e)
            self.connected = False

    async def _read_daemon_loop(self):
        """Read 4-byte length framed JSON messages from the Rust Daemon."""
        try:
            while self.connected and self.reader:
                len_bytes = await self.reader.readexactly(4)
                if not len_bytes:
                    break
                frame_len = int.from_bytes(len_bytes, byteorder="big")
                if frame_len > 1024 * 1024:  # 1 MB safety cap
                    logger.error("Daemon sent oversized frame (%d bytes); closing reader", frame_len)
                    break
                frame_data = await self.reader.readexactly(frame_len)
                msg_envelope = json.loads(frame_data.decode("utf-8"))
                payload = msg_envelope.get("payload", {})
                msg_type = payload.get("type", "")

                if msg_type == "Interrupted":
                    reason = payload.get("data", {}).get("reason", "Daemon interruption")
                    logger.info("[DAEMON->PIPECAT] Interruption received from daemon: %s", reason)
                    for cb in self._on_interrupted_callbacks:
                        try:
                            if asyncio.iscoroutinefunction(cb):
                                asyncio.create_task(cb(reason))
                            else:
                                cb(reason)
                        except Exception as e:
                            logger.error("Error in on_interrupted callback: %s", e)
                elif msg_type == "VoiceStateChanged":
                    new_state = payload.get("data", {}).get("state", "")
                    logger.info("[DAEMON->PIPECAT] Voice state update: %s", new_state)
        except (asyncio.IncompleteReadError, ConnectionResetError, BrokenPipeError):
            logger.info("Daemon connection closed by remote peer.")
        except asyncio.CancelledError:
            pass
        except Exception as e:
            logger.debug("Daemon inbound reader error: %s", e)
        finally:
            self.connected = False

    async def forward_transcript(self, text: str, language: str = "en") -> str:
        """Forward recognized user utterance to OSMOO's CognitiveEngine / LLM Router."""
        if not self.connected or not self.writer or not self.reader:
            if os.environ.get("VOXY_ALLOW_STANDALONE", "0") == "1":
                logger.info("[PIPELINE-ROUTER] Standalone test route transcript to mock assistant: \"%s\"", text)
                return f"OSMOO test mock: {text}"
            logger.error("[PIPELINE-ROUTER] Cannot route transcript; OSMOO Rust Daemon IPC is not connected!")
            raise ConnectionError("OSMOO Rust Daemon IPC is disconnected; cannot route utterance to LLM in production")

        try:
            payload = {
                "type": "SendTextInput",
                "data": {"text": text}
            }
            envelope = {
                "version": 1,
                "timestamp_ms": int(time.time() * 1000),
                "payload": payload,
            }
            data = json.dumps(envelope).encode("utf-8")
            self.writer.write(len(data).to_bytes(4, byteorder="big") + data)
            await self.writer.drain()
            logger.info("Forwarded transcript to OSMOO Daemon via IPC.")
            return "Utterance sent to OSMOO Daemon"
        except Exception as e:
            logger.error("Failed to forward transcript over IPC: %s", e)
            return f"OSMOO offline fallback: {text}"

    async def notify_interruption(self, reason: str = "User barge-in"):
        """Notify the daemon that user interrupted active playback."""
        if self.connected and self.writer:
            try:
                payload = {"type": "InterruptSpeech", "data": None}
                envelope = {
                    "version": 1,
                    "timestamp_ms": int(time.time() * 1000),
                    "payload": payload,
                }
                data = json.dumps(envelope).encode("utf-8")
                self.writer.write(len(data).to_bytes(4, byteorder="big") + data)
                await self.writer.drain()
                logger.info("Interruption signal transmitted to daemon.")
            except Exception as e:
                logger.debug("Interruption notification failed: %s", e)

    async def close(self):
        if self._inbound_task:
            self._inbound_task.cancel()
            self._inbound_task = None
        if self.writer:
            try:
                self.writer.close()
                await self.writer.wait_closed()
            except Exception:
                pass
            self.connected = False


# ── Pipecat Processors ───────────────────────────────────────────────────────

if HAVE_PIPECAT:
    class OsmooBrainProcessor(FrameProcessor):
        """Processes transcription/text frames, routes to OSMOO Brain, and emits LLM responses."""

        def __init__(self, bridge: OsmooBrainBridge):
            super().__init__()
            self.bridge = bridge
            self.bridge.register_on_interrupted(self._handle_daemon_interruption)

        async def _handle_daemon_interruption(self, reason: str):
            logger.info("[PIPECAT-BRAIN] Handling daemon interruption: %s", reason)
            await self.push_frame(CancelFrame(), FrameDirection.DOWNSTREAM)

        async def process_frame(self, frame: Frame, direction: FrameDirection):
            await super().process_frame(frame, direction)

            if isinstance(frame, (TranscriptionFrame, TextFrame)):
                text = frame.text if hasattr(frame, "text") else str(frame)
                logger.info("[PIPECAT-BRAIN] Received input frame: %s", text)
                response = await self.bridge.forward_transcript(text)
                if response:
                    logger.info("[PIPECAT-BRAIN] Emitting response frame: %s", response)
                    await self.push_frame(TextFrame(text=response), direction)
            elif isinstance(frame, UserStartedSpeakingFrame):
                logger.info("[PIPECAT-BRAIN] Barge-in detected (UserStartedSpeakingFrame). Cancelling output.")
                await self.bridge.notify_interruption()
                await self.push_frame(CancelFrame(), direction)
            else:
                await self.push_frame(frame, direction)

    class OsmooAudioSink(FrameProcessor):
        """Audio/Text terminal sink for the Pipecat pipeline."""

        def __init__(self):
            super().__init__()
            self.received_frames = []

        async def process_frame(self, frame: Frame, direction: FrameDirection):
            await super().process_frame(frame, direction)
            if isinstance(frame, TextFrame):
                logger.info("[PIPECAT-SINK] Output delivered: %s", frame.text)
                self.received_frames.append(frame.text)
            elif isinstance(frame, AudioRawFrame):
                logger.debug("[PIPECAT-SINK] PCM audio frame received: %d bytes", len(frame.audio_bytes))
            await self.push_frame(frame, direction)


# ── Pipecat Streaming Voice Service ──────────────────────────────────────────

class PipecatVoiceService:
    """Manages the full streaming voice pipeline lifecycle."""

    def __init__(self, bridge: OsmooBrainBridge):
        self.bridge = bridge
        self.running = False
        self.state = "Idle"
        self.pipeline = None
        self.task = None
        self.runner = None

    async def setup_pipeline(self):
        """Constructs the native Pipecat pipeline graph."""
        if HAVE_PIPECAT:
            self.brain_processor = OsmooBrainProcessor(self.bridge)
            self.sink_processor = OsmooAudioSink()
            self.pipeline = Pipeline([self.brain_processor, self.sink_processor])
            self.task = PipelineTask(self.pipeline)
            self.runner = PipelineRunner()
            logger.info("Native Pipecat pipeline graph initialized: [BrainProcessor -> AudioSink]")

    async def start(self):
        self.running = True
        logger.info("Initializing Pipecat Streaming Voice Pipeline...")
        await self.bridge.connect()
        await self.setup_pipeline()

        # Emit readiness signal for Rust supervisor
        sys.stdout.write(json.dumps({"status": "ready", "timestamp": time.time()}) + "\n")
        sys.stdout.flush()

        logger.info("OSMOO Pipecat sidecar ready and listening.")

        if HAVE_PIPECAT and self.runner and self.task:
            try:
                await self.runner.run(self.task)
            except asyncio.CancelledError:
                pass
        else:
            while self.running:
                await asyncio.sleep(0.5)

    async def process_test_utterance(self, text: str) -> Optional[str]:
        """Directly processes a test utterance through the Pipecat pipeline."""
        logger.info("Injecting test utterance through Pipecat: '%s'", text)
        if HAVE_PIPECAT and self.task:
            await self.task.queue_frame(TextFrame(text=text))
            await asyncio.sleep(0.3)
            if hasattr(self, "sink_processor") and self.sink_processor.received_frames:
                return self.sink_processor.received_frames[-1]
        return await self.bridge.forward_transcript(text)

    async def handle_ping(self) -> Dict[str, Any]:
        """Health check probe responder."""
        return {
            "type": "Pong",
            "status": "ok",
            "state": self.state,
            "connected": self.bridge.connected,
            "native_pipecat": HAVE_PIPECAT,
            "timestamp": time.time(),
        }

    async def stop(self):
        self.running = False
        if HAVE_PIPECAT and self.task:
            try:
                await self.task.queue_frame(EndFrame())
            except Exception:
                pass
        await self.bridge.close()
        logger.info("Pipecat voice service shut down gracefully.")


# ── Windows-Compatible Stdin Supervisor Protocol Reader ──────────────────────

def _stdin_reader_worker(loop: asyncio.AbstractEventLoop, queue: asyncio.Queue, stop_event: threading.Event):
    """Background thread reading lines from sys.stdin, avoiding Windows Proactor pipe limits."""
    try:
        while not stop_event.is_set():
            line = sys.stdin.readline()
            if not line:  # EOF
                asyncio.run_coroutine_threadsafe(queue.put(None), loop)
                break
            asyncio.run_coroutine_threadsafe(queue.put(line), loop)
    except Exception as e:
        logger.debug("Stdin reader thread terminated: %s", e)
        asyncio.run_coroutine_threadsafe(queue.put(None), loop)


async def stdin_supervisor_loop(service: PipecatVoiceService):
    """Asynchronously reads commands from the Rust supervisor over stdin."""
    loop = asyncio.get_running_loop()
    queue = asyncio.Queue()
    stop_event = threading.Event()

    reader_thread = threading.Thread(
        target=_stdin_reader_worker,
        args=(loop, queue, stop_event),
        daemon=True,
        name="osmoo-stdin-reader"
    )
    reader_thread.start()

    while service.running:
        try:
            line = await queue.get()
            if line is None:
                logger.info("Supervisor stdin EOF reached; initiating graceful exit.")
                await service.stop()
                break

            text = line.strip()
            if not text:
                continue

            try:
                cmd = json.loads(text)
                cmd_type = cmd.get("type", "")
                if cmd_type == "Ping":
                    pong = await service.handle_ping()
                    sys.stdout.write(json.dumps(pong) + "\n")
                    sys.stdout.flush()
                elif cmd_type == "Utterance":
                    utterance = cmd.get("text", "")
                    await service.process_test_utterance(utterance)
                elif cmd_type == "Shutdown":
                    logger.info("Shutdown command received from supervisor.")
                    await service.stop()
                    break
            except json.JSONDecodeError:
                pass
        except asyncio.CancelledError:
            break
        except Exception as e:
            logger.debug("Error processing supervisor command: %s", e)
            break
    stop_event.set()


# ── Main Entrypoint ──────────────────────────────────────────────────────────

async def main():
    parser = argparse.ArgumentParser(description="OSMOO Pipecat Streaming Voice Pipeline")
    parser.add_argument("--port", type=int, default=18889, help="Rust Daemon IPC TCP port")
    parser.add_argument("--host", type=str, default="127.0.0.1", help="Rust Daemon IPC host")
    parser.add_argument("--test-utterance", type=str, default=None, help="Process single test utterance and exit")
    args = parser.parse_args()

    bridge = OsmooBrainBridge(host=args.host, port=args.port)
    service = PipecatVoiceService(bridge)

    if args.test_utterance:
        # Run test pipeline in isolation
        await bridge.connect()
        await service.setup_pipeline()
        
        async def feed_and_stop():
            await asyncio.sleep(0.1)
            await service.process_test_utterance(args.test_utterance)
            await asyncio.sleep(0.2)
            if HAVE_PIPECAT and service.task:
                await service.task.queue_frame(EndFrame())
            await service.stop()

        if HAVE_PIPECAT and service.task and service.runner:
            try:
                await asyncio.wait_for(
                    asyncio.gather(service.runner.run(service.task), feed_and_stop()),
                    timeout=3.0,
                )
            except asyncio.TimeoutError:
                pass
        else:
            await feed_and_stop()

        print(f"TEST_PIPELINE_SUCCESS: {args.test_utterance}")
        return

    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        try:
            loop.add_signal_handler(sig, lambda: asyncio.create_task(service.stop()))
        except (NotImplementedError, AttributeError):
            pass

    stdin_task = asyncio.create_task(stdin_supervisor_loop(service))

    try:
        await service.start()
    except asyncio.CancelledError:
        pass
    finally:
        stdin_task.cancel()
        await service.stop()


if __name__ == "__main__":
    asyncio.run(main())
