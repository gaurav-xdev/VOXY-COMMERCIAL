import sounddevice as sd
import sys

print("Default devices (in, out):", sd.default.device)
for i, d in enumerate(sd.query_devices()):
    if d['max_input_channels'] > 0:
        print(f"{i}: {d['name']} | HostAPI: {d['hostapi']} | InCh: {d['max_input_channels']} | Rate: {d['default_samplerate']}")

# Try reading from each input device
print("\n--- ATTEMPTING 0.5s RECORDING ON EACH INPUT ---")
for i, d in enumerate(sd.query_devices()):
    if d['max_input_channels'] > 0:
        rate = int(d['default_samplerate']) if d['default_samplerate'] > 0 else 44100
        ch = min(d['max_input_channels'], 2)
        try:
            frames = int(rate * 0.5)
            data = sd.rec(frames=frames, samplerate=rate, channels=ch, device=i, blocking=True)
            print(f"SUCCESS on device {i} ({d['name']}): max_amp={abs(data).max():.4f}")
        except Exception as e:
            print(f"FAILED on device {i} ({d['name']}): {e}")
