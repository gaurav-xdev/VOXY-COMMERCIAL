#!/usr/bin/env python3
"""
VOXY Performance Benchmark
Continuously samples process CPU%, RAM (Working Set), and sustained resource usage for:
1. Dormant Mode (HUD collapsed, low power camera)
2. Active HUD Mode (HUD expanded, 60 FPS Canvas rendering)
3. Voice Interaction Mode (Audio streaming & active pipeline)
"""

import argparse
import json
import time
from typing import Dict, List

import numpy as np
import psutil

def find_target_processes():
    overlay_proc = None
    gesture_proc = None
    for p in psutil.process_iter(['pid', 'name', 'cmdline']):
        try:
            name = p.info['name'].lower()
            cmdline = " ".join(p.info['cmdline'] or []).lower()
            if 'voxy-overlay' in name:
                overlay_proc = p
            elif 'python' in name and 'gesture_service' in cmdline:
                gesture_proc = p
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            continue
    return overlay_proc, gesture_proc


def sample_metrics(duration_sec: int, mode_name: str) -> Dict:
    print(f"\n--- Sustained Sampling for {mode_name} ({duration_sec}s) ---")
    overlay_proc, gesture_proc = find_target_processes()

    overlay_cpu = []
    overlay_ram = []
    gesture_cpu = []
    gesture_ram = []

    # Prime cpu_percent
    if overlay_proc:
        overlay_proc.cpu_percent()
    if gesture_proc:
        gesture_proc.cpu_percent()

    start_t = time.time()
    samples = 0

    while time.time() - start_t < duration_sec:
        time.sleep(1.0)
        samples += 1

        if overlay_proc and overlay_proc.is_running():
            try:
                c = overlay_proc.cpu_percent()
                m = overlay_proc.memory_info().rss / (1024 * 1024)
                overlay_cpu.append(c)
                overlay_ram.append(m)
            except Exception:
                pass

        if gesture_proc and gesture_proc.is_running():
            try:
                c = gesture_proc.cpu_percent()
                m = gesture_proc.memory_info().rss / (1024 * 1024)
                gesture_cpu.append(c)
                gesture_ram.append(m)
            except Exception:
                pass

        if samples % 10 == 0:
            o_c = overlay_cpu[-1] if overlay_cpu else 0.0
            g_c = gesture_cpu[-1] if gesture_cpu else 0.0
            print(f"    [{samples}s] Overlay CPU: {o_c:.1f}%, Gesture CPU: {g_c:.1f}%")

    def calc_stats(arr: List[float]):
        if not arr:
            return {"avg": 0.0, "peak": 0.0}
        return {"avg": round(float(np.mean(arr)), 2), "peak": round(float(np.max(arr)), 2)}

    stats = {
        "mode": mode_name,
        "duration_sec": duration_sec,
        "samples": samples,
        "overlay_cpu_pct": calc_stats(overlay_cpu),
        "overlay_ram_mb": calc_stats(overlay_ram),
        "gesture_cpu_pct": calc_stats(gesture_cpu),
        "gesture_ram_mb": calc_stats(gesture_ram),
        "total_cpu_pct": {
            "avg": round(float(np.mean([o + g for o, g in zip(overlay_cpu, gesture_cpu)]) if overlay_cpu and gesture_cpu else 0.0), 2),
            "peak": round(float(np.max([o + g for o, g in zip(overlay_cpu, gesture_cpu)]) if overlay_cpu and gesture_cpu else 0.0), 2),
        }
    }

    print(f"  Summary for {mode_name}:")
    print(f"    Overlay CPU: avg = {stats['overlay_cpu_pct']['avg']}%, peak = {stats['overlay_cpu_pct']['peak']}%")
    print(f"    Overlay RAM: avg = {stats['overlay_ram_mb']['avg']} MB, peak = {stats['overlay_ram_mb']['peak']} MB")
    print(f"    Gesture CPU: avg = {stats['gesture_cpu_pct']['avg']}%, peak = {stats['gesture_cpu_pct']['peak']}%")
    print(f"    Total CPU:   avg = {stats['total_cpu_pct']['avg']}%, peak = {stats['total_cpu_pct']['peak']}%")
    return stats


def main():
    parser = argparse.ArgumentParser(description="VOXY Sustained Performance Benchmark")
    parser.add_argument("--duration", type=int, default=30, help="Duration in seconds per phase")
    args = parser.parse_args()

    print("=" * 70)
    print("  VOXY SUSTAINED HARDWARE PERFORMANCE BENCHMARK")
    print("=" * 70)

    # 1. Dormant Mode
    # Toggle to collapsed
    import socket
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.sendto(b'{"type":"gesture","action":"COLLAPSED","source":"benchmark"}', ("127.0.0.1", 18889))
    time.sleep(1)
    dormant_stats = sample_metrics(args.duration, "Dormant (HUD Collapsed)")

    # 2. Active Mode
    sock.sendto(b'{"type":"gesture","action":"EXPANDED","source":"benchmark"}', ("127.0.0.1", 18889))
    time.sleep(1)
    active_stats = sample_metrics(args.duration, "Active (HUD Expanded, 60 FPS Canvas)")

    # 3. Voice Interaction Mode
    sock.sendto(b'{"type":"voice","state":"SPEAKING","text":"Active voice playback..."}', ("127.0.0.1", 18889))
    sock.sendto(b'{"type":"audio_output","output_rms":0.045}', ("127.0.0.1", 18889))
    voice_stats = sample_metrics(args.duration, "Voice Interaction (Active Pipeline & Audio)")
    sock.sendto(b'{"type":"voice","state":"IDLE"}', ("127.0.0.1", 18889))
    sock.sendto(b'{"type":"audio_output","output_rms":0.0}', ("127.0.0.1", 18889))
    sock.close()

    full_results = {
        "dormant": dormant_stats,
        "active": active_stats,
        "voice": voice_stats,
    }

    with open("performance_benchmark_results.json", "w") as f:
        json.dump(full_results, f, indent=2)

    print("\n" + "=" * 70)
    print("  FINAL PERFORMANCE BENCHMARK MATRIX")
    print("=" * 70)
    print(f"| Mode                      | Total CPU Avg | Total CPU Peak | Overlay RAM | Gesture RAM |")
    print(f"| ------------------------- | ------------- | -------------- | ----------- | ----------- |")
    print(f"| Dormant (HUD Collapsed)   | {dormant_stats['total_cpu_pct']['avg']:>11}% | {dormant_stats['total_cpu_pct']['peak']:>12}% | {dormant_stats['overlay_ram_mb']['avg']:>9}MB | {dormant_stats['gesture_ram_mb']['avg']:>9}MB |")
    print(f"| Active (HUD Expanded)     | {active_stats['total_cpu_pct']['avg']:>11}% | {active_stats['total_cpu_pct']['peak']:>12}% | {active_stats['overlay_ram_mb']['avg']:>9}MB | {active_stats['gesture_ram_mb']['avg']:>9}MB |")
    print(f"| Voice Interaction         | {voice_stats['total_cpu_pct']['avg']:>11}% | {voice_stats['total_cpu_pct']['peak']:>12}% | {voice_stats['overlay_ram_mb']['avg']:>9}MB | {voice_stats['gesture_ram_mb']['avg']:>9}MB |")
    print("=" * 70)


if __name__ == "__main__":
    main()
