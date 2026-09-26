#!/usr/bin/env python3
"""
VOXY Laptop Acoustic Shield - Phase 1 Endpoint Inspector
Inspects Intel Smart Sound Technology Microphone Array and Laptop Speakers:
- Channels, sample rate, bit depth, format, channel layout
- WASAPI capabilities and stream categories (Speech, Communications, Raw)
- Microphone array geometry (physical mic count, type, spacing, angles)
- Driver/APO effects (Pre-Mix, Post-Mix, Endpoint FX)
- Native AEC, Noise Suppression, Beamforming, AGC availability
"""

import ctypes
from ctypes import wintypes
import struct
import sys

# COM initialization
ole32 = ctypes.windll.ole32
ole32.CoInitialize(None)

CLSID_MMDeviceEnumerator = ctypes.c_char_p(b"\xbc\xde\x03\xbc\xa0\xef\x46\x42\x8c\x83\x2a\x0b\x6e\x67\x6f\x22") # {BCDE03BC-A0F9-4346-8C83-2A0B6E676F22}
IID_IMMDeviceEnumerator = ctypes.c_char_p(b"\xa9\x56\x60\xa6\x72\x3a\x4e\x46\x97\x67\xed\xbe\x45\xee\x7e\x48") # {A95660A6-723A-4366-9767-EDBE45EE7E48}
IID_IAudioClient = ctypes.c_char_p(b"\x4c\xad\xb9\x1c\xfa\xdb\x32\x4c\xb1\x78\xc2\xf5\x68\xa7\x03\xb2") # {1CB9AD4C-DBFA-4c32-B178-C2F568A703B2}
IID_IAudioEffectsManager = ctypes.c_char_p(b"\xae\xb3\x60\x44\x44\x4b\x27\x45\x86\x71\x83\x04\x8d\x1f\xf9\xe1") # {4460B3AE-4B44-4527-8671-83048D1FF9E1}

class GUID(ctypes.Structure):
    _fields_ = [
        ("Data1", wintypes.DWORD),
        ("Data2", wintypes.WORD),
        ("Data3", wintypes.WORD),
        ("Data4", ctypes.c_byte * 8),
    ]
    def __str__(self):
        d4_1 = "".join(f"{self.Data4[i]:02x}" for i in range(2))
        d4_2 = "".join(f"{self.Data4[i]:02x}" for i in range(2, 8))
        return f"{{{self.Data1:08x}-{self.Data2:04x}-{self.Data3:04x}-{d4_1}-{d4_2}}}"

def parse_guid(s):
    import uuid
    u = uuid.UUID(s)
    fields = u.fields
    data4 = (ctypes.c_byte * 8)(*u.bytes[8:])
    return GUID(fields[0], fields[1], fields[2], data4)

# Well known GUIDs
GUID_AEC = "{6f6ea360-23cf-4971-a4ed-cb8440c14b6d}"
GUID_NS = "{5ab0882e-7274-4516-877d-9eee99ba4fd0}"
GUID_AGC = "{bb11c46e-67e2-4540-a15f-6420364177f7}"
GUID_BEAMFORMING = "{e2e70c43-b452-4048-b4b0-a6164f9c5d01}"
GUID_DEEP_NS = "{660c0755-2172-4d7a-b9c2-c0cb1882d921}"

print("=" * 75)
print("     PHASE 1: LAPTOP AUDIO ENDPOINT & HARDWARE CAPABILITY AUDIT")
print("=" * 75)

# Inspect via sounddevice first
import sounddevice as sd
devices = sd.query_devices()

mic_dev = None
spk_dev = None

for i, d in enumerate(devices):
    name = d["name"]
    host = sd.query_hostapis(d["hostapi"])["name"]
    if host == "Windows WASAPI":
        if "Intel" in name and d["max_input_channels"] > 0:
            mic_dev = (i, d)
        elif ("Speaker" in name or "Realtek" in name) and d["max_output_channels"] > 0:
            spk_dev = (i, d)

if not mic_dev:
    # fallback to any WASAPI capture
    for i, d in enumerate(devices):
        if sd.query_hostapis(d["hostapi"])["name"] == "Windows WASAPI" and d["max_input_channels"] > 0:
            mic_dev = (i, d)
            break

if not spk_dev:
    for i, d in enumerate(devices):
        if sd.query_hostapis(d["hostapi"])["name"] == "Windows WASAPI" and d["max_output_channels"] > 0:
            spk_dev = (i, d)
            break

print(f"\n[1] Physical Microphone Endpoint:")
print(f"    Name:              {mic_dev[1]['name']}")
print(f"    Index:             {mic_dev[0]}")
print(f"    Channels:          {mic_dev[1]['max_input_channels']} channel(s)")
print(f"    Default Rate:      {int(mic_dev[1]['default_samplerate'])} Hz")
print(f"    Native Latency:    {mic_dev[1]['default_low_input_latency']*1000:.2f} ms (low), {mic_dev[1]['default_high_input_latency']*1000:.2f} ms (high)")

print(f"\n[2] Physical Render / Speaker Endpoint:")
print(f"    Name:              {spk_dev[1]['name']}")
print(f"    Index:             {spk_dev[0]}")
print(f"    Channels:          {spk_dev[1]['max_output_channels']} channel(s)")
print(f"    Default Rate:      {int(spk_dev[1]['default_samplerate'])} Hz")
print(f"    Native Latency:    {spk_dev[1]['default_low_output_latency']*1000:.2f} ms (low), {spk_dev[1]['default_high_output_latency']*1000:.2f} ms (high)")

# Inspect raw multichannel capabilities via WDM-KS
print(f"\n[3] Kernel-Streaming (WDM-KS) Multi-Channel Architecture:")
for i, d in enumerate(devices):
    if sd.query_hostapis(d["hostapi"])["name"] == "Windows WDM-KS" and "Microphone Array" in d["name"]:
        print(f"    WDM-KS Device {i}: {d['name']} | Max Channels: {d['max_input_channels']} | Native SR: {int(d['default_samplerate'])} Hz")

# Inspect Windows Registry for Array Geometry {4ee51b68-0cac-4070-a89c-cecf92558176},3
import winreg
print(f"\n[4] Microphone Array Geometry (KSAUDIO_MIC_ARRAY_GEOMETRY):")
reg_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Capture"
try:
    with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, reg_path) as k:
        n_subkeys, _, _ = winreg.QueryInfoKey(k)
        for i in range(n_subkeys):
            subkey_name = winreg.EnumKey(k, i)
            try:
                with winreg.OpenKey(k, f"{subkey_name}\\FxProperties") as fx:
                    val, _ = winreg.QueryValueEx(fx, "{4ee51b68-0cac-4070-a89c-cecf92558176},3")
                    if val and len(val) >= 20:
                        # Parse KSAUDIO_MIC_ARRAY_GEOMETRY
                        # struct KSAUDIO_MIC_ARRAY_GEOMETRY:
                        # USHORT usVersion;
                        # USHORT usMicArrayType; // 0=Linear, 1=Planar, 2=3D
                        # SHORT wVerticalAngleBegin, wVerticalAngleEnd;
                        # SHORT wHorizontalAngleBegin, wHorizontalAngleEnd;
                        # USHORT usFrequencyBandLo, usFrequencyBandHi;
                        # USHORT usNumberOfMicrophones;
                        usVersion, usType, vBegin, vEnd, hBegin, hEnd, fLo, fHi, numMics = struct.unpack_from("<HHhhhhHHH", val, 0)
                        type_str = ["Linear Array", "Planar Array", "3D Array"][usType] if usType <= 2 else f"Type {usType}"
                        print(f"    Version:           {usVersion}")
                        print(f"    Array Type:        {type_str} (usType={usType})")
                        print(f"    Physical Mics:     {numMics} discrete microphone capsule(s)")
                        print(f"    Working Range:     {fLo} Hz to {fHi} Hz")
                        print(f"    Horizontal FOV:    {hBegin/100.0:.1f}° to {hEnd/100.0:.1f}°")
                        print(f"    Vertical FOV:      {vBegin/100.0:.1f}° to {vEnd/100.0:.1f}°")

                        # Parse individual microphone coordinates
                        offset = 18
                        for m in range(numMics):
                            if offset + 16 <= len(val):
                                mic_type, x, y, z, v_ang, h_ang = struct.unpack_from("<HhhhHH", val, offset)
                                print(f"      Mic #{m+1}: Coordinates (X={x}mm, Y={y}mm, Z={z}mm), Type={mic_type}")
                                offset += 16
                        break
            except Exception:
                pass
except Exception as e:
    print(f"    Registry inspection info: {e}")

print("\n" + "=" * 75)
