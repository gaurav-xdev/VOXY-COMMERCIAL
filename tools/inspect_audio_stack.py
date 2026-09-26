import json
import sounddevice as sd

devices = sd.query_devices()
print("=== AUDIO ENDPOINTS ===")
for i, d in enumerate(devices):
    name = d["name"]
    hostapi = sd.query_hostapis(d["hostapi"])["name"]
    if any(k in name.lower() for k in ["intel", "micro", "realtek", "speaker", "array"]):
        print(f"Index {i:2d} | HostAPI: {hostapi:15s} | In: {d['max_input_channels']} | Out: {d['max_output_channels']} | SR: {d['default_samplerate']} | Name: {name}")

print("\n=== WASAPI DEVICES DETAIL ===")
for i, d in enumerate(devices):
    hostapi = sd.query_hostapis(d["hostapi"])["name"]
    if hostapi == "Windows WASAPI":
        print(f"\nDevice {i}: {d['name']}")
        print(f"  Max In: {d['max_input_channels']}, Max Out: {d['max_output_channels']}")
        print(f"  Default SR: {d['default_samplerate']}")
        print(f"  Low In Latency: {d['default_low_input_latency']*1000:.1f}ms, High In Latency: {d['default_high_input_latency']*1000:.1f}ms")
        print(f"  Low Out Latency: {d['default_low_output_latency']*1000:.1f}ms, High Out Latency: {d['default_high_output_latency']*1000:.1f}ms")
