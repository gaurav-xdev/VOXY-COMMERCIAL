#!/usr/bin/env python3
"""
VOXY 20 Real Physical Claps Benchmark
Listens to live events from gesture_service.py and overlay HUD on ws://127.0.0.1:18888.
Records 20 real physical claps, logging:
CLAP_DETECTED
physical_timestamp
detector_timestamp
ui_event_timestamp
ui_visible_timestamp
detection_latency_ms
total_latency_ms
"""

import asyncio
import json
import logging
import time
import numpy as np
import websockets

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(message)s",
    datefmt="%H:%M:%S",
)
logger = logging.getLogger("clap_20_test")

WS_URI = "ws://127.0.0.1:18888"


async def main():
    logger.info(f"Connecting to live VOXY event stream at {WS_URI}...")
    try:
        ws = await websockets.connect(WS_URI)
        logger.info(f"Connected to {WS_URI}. Ready for 20 real physical claps.")
    except Exception as e:
        logger.error(f"Failed to connect to {WS_URI}: {e}")
        return

    clap_records = []
    pending_claps = {}  # keyed by physical_ts or sequential

    print("\n" + "=" * 75)
    print("      VOXY 20 PHYSICAL CLAP HARDWARE BENCHMARK (LIVE)")
    print("=" * 75)
    print("Please execute 20 physical hand claps in front of the machine.")
    print("Each physical clap will be captured by the Intel SST microphone, verified by DSP,")
    print("and rendered by the Holographic UI overlay.\n")

    trial = 1
    t_start = time.time()

    try:
        async for message in ws:
            data = json.loads(message)
            msg_type = data.get("type")

            if msg_type == "clap":
                phys_ts = data.get("physical_timestamp", time.time())
                det_ts = data.get("detector_timestamp", time.time())
                det_lat = data.get("detection_latency_ms", (det_ts - phys_ts) * 1000.0)
                source = data.get("source", "clap")
                peak = data.get("peak_amplitude", 0.0)
                decay = data.get("decay_ratio", 0.0)

                pending_claps[trial] = {
                    "trial": trial,
                    "source": source,
                    "physical_timestamp": phys_ts,
                    "detector_timestamp": det_ts,
                    "detection_latency_ms": det_lat,
                    "peak_amplitude": peak,
                    "decay_ratio": decay,
                    "rx_time": time.time(),
                }

            elif msg_type == "clap_ack":
                phys_ts = data.get("physical_timestamp")
                det_ts = data.get("detector_timestamp")
                ui_event_ts = data.get("ui_event_timestamp")
                ui_vis_ts = data.get("ui_visible_timestamp")
                det_lat = data.get("detection_latency_ms")
                tot_lat = data.get("total_latency_ms")
                source = data.get("source", "clap")

                record = {
                    "trial": trial,
                    "source": source,
                    "physical_timestamp": phys_ts,
                    "detector_timestamp": det_ts,
                    "ui_event_timestamp": ui_event_ts,
                    "ui_visible_timestamp": ui_vis_ts,
                    "detection_latency_ms": round(det_lat, 2),
                    "total_latency_ms": round(tot_lat, 2),
                }

                print(f"--- [TRIAL {trial}/20] CLAP_DETECTED [{source.upper()}] ---")
                print(f"physical_timestamp:    {phys_ts:.6f}")
                print(f"detector_timestamp:    {det_ts:.6f}")
                print(f"ui_event_timestamp:    {ui_event_ts:.6f}")
                print(f"ui_visible_timestamp:  {ui_vis_ts:.6f}")
                print(f"detection_latency_ms:  {record['detection_latency_ms']:.2f}")
                print(f"total_latency_ms:      {record['total_latency_ms']:.2f}\n")

                clap_records.append(record)
                trial += 1

                if trial > 20:
                    break

            # Timeout after 60 seconds if not all claps received
            if time.time() - t_start > 60.0:
                logger.info("Session time elapsed.")
                break

    finally:
        await ws.close()

    print("\n" + "=" * 75)
    print("                 20-CLAP BENCHMARK SUMMARY REPORT")
    print("=" * 75)

    tp_count = len(clap_records)
    fn_count = 20 - tp_count

    if clap_records:
        det_latencies = [r["detection_latency_ms"] for r in clap_records]
        tot_latencies = [r["total_latency_ms"] for r in clap_records]

        p50_det = np.percentile(det_latencies, 50)
        p90_det = np.percentile(det_latencies, 90)
        max_det = np.max(det_latencies)

        p50_tot = np.percentile(tot_latencies, 50)
        p90_tot = np.percentile(tot_latencies, 90)
        max_tot = np.max(tot_latencies)

        print(f"Total Claps Target:     20")
        print(f"True Positives (TP):    {tp_count}")
        print(f"False Negatives (FN):   {fn_count}")
        print(f"Detection Latency p50:  {p50_det:.2f} ms")
        print(f"Detection Latency p90:  {p90_det:.2f} ms")
        print(f"Detection Latency Max:  {max_det:.2f} ms")
        print(f"Total UI Latency p50:   {p50_tot:.2f} ms")
        print(f"Total UI Latency p90:   {p90_tot:.2f} ms")
        print(f"Total UI Latency Max:   {max_tot:.2f} ms")
    else:
        print(f"No claps recorded.")

    print("=" * 75)


if __name__ == "__main__":
    asyncio.run(main())
