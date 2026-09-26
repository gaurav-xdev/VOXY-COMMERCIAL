import sys, os, time
import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from acoustic_shield import LaptopAcousticShield

shield = LaptopAcousticShield(sample_rate=16000, block_size=256)
times = []
for _ in range(200):
    raw = np.random.randn(256, 2).astype(np.float32) * 0.05
    spk = np.random.randn(256).astype(np.float32) * 0.05
    t0 = time.perf_counter()
    clean, m = shield.process(raw, spk)
    times.append((time.perf_counter() - t0) * 1000.0)

# Drop warmup
valid_times = times[20:]
print(f"Laptop Acoustic Shield DSP Latency (256 samples @ 16kHz = 16.0ms block):")
print(f"  Average DSP Latency: {np.mean(valid_times):.3f} ms")
print(f"  Median (p50):        {np.percentile(valid_times, 50):.3f} ms")
print(f"  90th percentile:     {np.percentile(valid_times, 90):.3f} ms")
print(f"  Maximum DSP Latency: {np.max(valid_times):.3f} ms")
print(f"  Real-time Ratio:     {(np.mean(valid_times) / 16.0) * 100.0:.2f}% of budget")
