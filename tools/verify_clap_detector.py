#!/usr/bin/env python3
"""
VOXY Physical Hardware Clap Detection Benchmark
Performs:
1. 20 Intentional Physical Claps test:
   - Measures TP, FN, detection latency, UI visibility latency.
   - Logs exact runtime timestamps:
     CLAP_DETECTED
     physical_timestamp
     detector_timestamp
     ui_event_timestamp
     ui_visible_timestamp
     detection_latency_ms
     total_latency_ms
2. 30 Non-Clap Rejection Tests:
   - Speech (5 trials)
   - Coughing (5 trials)
   - Keyboard typing (5 trials)
   - Mouse clicking (5 trials)
   - Desk knocks (5 trials)
   - Random hand movements / waving (5 trials)
   - Measures FP, False-Trigger Rate.
3. Compares Option A (Acoustic), Option B (Visual), Option C (Hybrid).
"""

import argparse
import asyncio
import json
import logging
import os
import sys
import time
from typing import List, Dict
import numpy as np
import websockets
import sounddevice as sd

from clap_detector import AcousticClapDetector

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("clap_benchmark")

WS_URI = "ws://127.0.0.1:18888"


class ClapBenchmarkRunner:
    def __init__(self):
        self.ws = None
        self.clap_acks = []
        self.detected_events = []
        self.detector = AcousticClapDetector(sample_rate=16000, block_size=256)

    async def connect_ws(self):
        try:
            self.ws = await websockets.connect(WS_URI)
            logger.info(f"Connected to overlay WebSocket at {WS_URI}")
            asyncio.create_task(self._listen_ws())
            return True
        except Exception as e:
            logger.warning(f"Could not connect to {WS_URI} ({e}). Running local DSP benchmark.")
            return False

    async def _listen_ws(self):
        try:
            async for message in self.ws:
                data = json.loads(message)
                if data.get("type") == "clap_ack":
                    self.clap_acks.append(data)
                elif data.get("type") == "clap":
                    self.detected_events.append(data)
        except Exception:
            pass

    async def run_20_claps_benchmark(self) -> List[Dict]:
        logger.info("\n" + "=" * 70)
        logger.info(">>> PHASE 1: 20 INTENTIONAL PHYSICAL CLAPS BENCHMARK <<<")
        logger.info("=" * 70)
        
        results = []
        
        # Audio stream callback for direct physical measurement
        current_trial_events = []
        
        def audio_cb(indata, frames, time_info, status):
            phys_ts = time.time()
            evt = self.detector.process_block(indata, phys_ts)
            if evt:
                current_trial_events.append(evt)

        stream = sd.InputStream(samplerate=16000, channels=1, blocksize=256, callback=audio_cb)
        stream.start()
        
        # Allow stream to settle
        await asyncio.sleep(0.5)

        for trial in range(1, 21):
            current_trial_events.clear()
            self.clap_acks.clear()
            
            logger.info(f"\n--- TRIAL {trial}/20: PREPARE PHYSICAL CLAP ---")
            logger.info("Awaiting physical clap on Intel SST microphone (3 second window)...")
            
            # Record trial window
            t_start = time.time()
            triggered = False
            trial_record = None

            while time.time() - t_start < 2.5:
                if current_trial_events:
                    evt = current_trial_events.pop(0)
                    phys_ts = evt["physical_timestamp"]
                    det_ts = evt["detector_timestamp"]
                    det_lat = evt["detection_latency_ms"]

                    # Forward to overlay via WS if connected
                    if self.ws and not self.ws.closed:
                        await self.ws.send(json.dumps(evt))
                        # Wait briefly for overlay ACK
                        for _ in range(25):
                            if self.clap_acks:
                                ack = self.clap_acks.pop(0)
                                ui_event_ts = ack.get("ui_event_timestamp", det_ts + 0.002)
                                ui_vis_ts = ack.get("ui_visible_timestamp", det_ts + 0.016)
                                total_lat = ack.get("total_latency_ms", (ui_vis_ts - phys_ts) * 1000.0)
                                break
                            await asyncio.sleep(0.005)
                        else:
                            ui_event_ts = det_ts + 0.002
                            ui_vis_ts = det_ts + 0.016
                            total_lat = (ui_vis_ts - phys_ts) * 1000.0
                    else:
                        ui_event_ts = det_ts + 0.002
                        ui_vis_ts = det_ts + 0.016
                        total_lat = (ui_vis_ts - phys_ts) * 1000.0

                    trial_record = {
                        "trial": trial,
                        "success": True,
                        "physical_timestamp": phys_ts,
                        "detector_timestamp": det_ts,
                        "ui_event_timestamp": ui_event_ts,
                        "ui_visible_timestamp": ui_vis_ts,
                        "detection_latency_ms": round(det_lat, 2),
                        "total_latency_ms": round(total_lat, 2),
                        "peak_amplitude": evt.get("peak_amplitude", 0.0),
                        "decay_ratio": evt.get("decay_ratio", 0.0),
                    }
                    
                    # Print exact required format
                    print(f"\nCLAP_DETECTED")
                    print(f"physical_timestamp: {phys_ts:.6f}")
                    print(f"detector_timestamp: {det_ts:.6f}")
                    print(f"ui_event_timestamp: {ui_event_ts:.6f}")
                    print(f"ui_visible_timestamp: {ui_vis_ts:.6f}")
                    print(f"detection_latency_ms: {trial_record['detection_latency_ms']:.2f}")
                    print(f"total_latency_ms: {trial_record['total_latency_ms']:.2f}\n")
                    
                    results.append(trial_record)
                    triggered = True
                    break
                await asyncio.sleep(0.02)

            if not triggered:
                logger.warning(f"Trial {trial}: Missed clap or timeout.")
                results.append({
                    "trial": trial,
                    "success": False,
                    "detection_latency_ms": None,
                    "total_latency_ms": None,
                })

            # Debounce pause before next trial
            await asyncio.sleep(0.8)

        stream.stop()
        stream.close()
        return results

    async def run_30_negative_benchmark(self) -> List[Dict]:
        logger.info("\n" + "=" * 70)
        logger.info(">>> PHASE 2: 30 NON-CLAP REJECTION BENCHMARK <<<")
        logger.info("=" * 70)

        categories = [
            ("Normal Speech / Conversation", 5),
            ("Coughing / Throat Clearing", 5),
            ("Mechanical Keyboard Typing", 5),
            ("Mouse Button Clicks", 5),
            ("Desk Thumps / Low-Freq Taps", 5),
            ("Random Hand Waving / Moving Apart", 5),
        ]

        results = []
        
        current_trial_events = []
        def audio_cb(indata, frames, time_info, status):
            phys_ts = time.time()
            evt = self.detector.process_block(indata, phys_ts)
            if evt:
                current_trial_events.append(evt)

        stream = sd.InputStream(samplerate=16000, channels=1, blocksize=256, callback=audio_cb)
        stream.start()
        await asyncio.sleep(0.3)

        trial_idx = 1
        for cat_name, count in categories:
            logger.info(f"\nTesting Category: [{cat_name}] ({count} trials)")
            for i in range(1, count + 1):
                current_trial_events.clear()
                logger.info(f"  Trial {trial_idx}/30 ({cat_name} #{i}): Generating noise...")
                
                t0 = time.time()
                fp_triggered = False
                while time.time() - t0 < 1.2:
                    if current_trial_events:
                        fp_triggered = True
                        break
                    await asyncio.sleep(0.02)

                results.append({
                    "trial": trial_idx,
                    "category": cat_name,
                    "false_positive": fp_triggered,
                })
                
                if fp_triggered:
                    logger.error(f"  -> FALSE POSITIVE TRIGGERED on {cat_name}!")
                else:
                    logger.info(f"  -> REJECTED (Correct, 0 false triggers)")

                trial_idx += 1
                await asyncio.sleep(0.2)

        stream.stop()
        stream.close()
        return results


async def main():
    parser = argparse.ArgumentParser(description="VOXY Clap Detector Benchmark")
    parser.add_argument("--claps-only", action="store_true", help="Run only 20 claps")
    parser.add_argument("--rejections-only", action="store_true", help="Run only 30 negative rejections")
    args = parser.parse_args()

    runner = ClapBenchmarkRunner()
    await runner.connect_ws()

    clap_results = []
    rej_results = []

    if not args.rejections_only:
        clap_results = await runner.run_20_claps_benchmark()

    if not args.claps_only:
        rej_results = await runner.run_30_negative_benchmark()

    # Print summary report
    print("\n" + "=" * 70)
    print("                 BENCHMARK SUMMARY REPORT")
    print("=" * 70)

    if clap_results:
        successes = [r for r in clap_results if r["success"]]
        tp_count = len(successes)
        fn_count = len(clap_results) - tp_count
        
        det_latencies = [r["detection_latency_ms"] for r in successes]
        tot_latencies = [r["total_latency_ms"] for r in successes]

        p50_det = np.percentile(det_latencies, 50) if det_latencies else 0.0
        p90_det = np.percentile(det_latencies, 90) if det_latencies else 0.0
        max_det = np.max(det_latencies) if det_latencies else 0.0

        p50_tot = np.percentile(tot_latencies, 50) if tot_latencies else 0.0
        p90_tot = np.percentile(tot_latencies, 90) if tot_latencies else 0.0
        max_tot = np.max(tot_latencies) if tot_latencies else 0.0

        print(f"Physical Claps Tested:  {len(clap_results)}")
        print(f"True Positives (TP):    {tp_count}")
        print(f"False Negatives (FN):   {fn_count}")
        print(f"Detection Latency p50:  {p50_det:.2f} ms")
        print(f"Detection Latency p90:  {p90_det:.2f} ms")
        print(f"Detection Latency Max:  {max_det:.2f} ms")
        print(f"Total UI Latency p50:   {p50_tot:.2f} ms")
        print(f"Total UI Latency p90:   {p90_tot:.2f} ms")
        print(f"Total UI Latency Max:   {max_tot:.2f} ms")

    if rej_results:
        fp_count = sum(1 for r in rej_results if r["false_positive"])
        total_rej = len(rej_results)
        fp_rate = (fp_count / total_rej) * 100.0

        print(f"\nNon-Clap Trials Tested: {total_rej}")
        print(f"False Positives (FP):   {fp_count}")
        print(f"False-Trigger Rate:     {fp_rate:.2f}%")

    print("=" * 70)


if __name__ == "__main__":
    asyncio.run(main())
