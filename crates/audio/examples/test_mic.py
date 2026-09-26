import ctypes
from ctypes import wintypes
import comtypes
from comtypes import GUID, IUnknown, COMMETHOD, HRESULT

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
        COMMETHOD([], HRESULT, 'GetBufferSize'),
        COMMETHOD([], HRESULT, 'GetStreamLatency'),
        COMMETHOD([], HRESULT, 'GetCurrentPadding'),
        COMMETHOD([], HRESULT, 'IsFormatSupported'),
        COMMETHOD([], HRESULT, 'GetMixFormat',
                  (['out'], ctypes.POINTER(ctypes.c_void_p), 'ppDeviceFormat')),
    ]

ole32 = ctypes.oledll.ole32
ole32.CoInitialize(None)
CLSID_MMDeviceEnumerator = GUID('{BCDE0395-E52F-467C-8E3D-C4579291692E}')
pEnum = ctypes.POINTER(IMMDeviceEnumerator)()
hr = ole32.CoCreateInstance(
    ctypes.byref(CLSID_MMDeviceEnumerator),
    None,
    1,
    ctypes.byref(IMMDeviceEnumerator._iid_),
    ctypes.byref(pEnum)
)
print(f"CoCreateInstance: {hex(hr)}")

pDev = pEnum.GetDefaultAudioEndpoint(1, 1)
print(f"GetDefaultAudioEndpoint: {pDev}")

pClientPtr = pDev.Activate(ctypes.byref(IAudioClient._iid_), 1, None)
print(f"Activate IAudioClient: {pClientPtr}")

client = ctypes.cast(pClientPtr, ctypes.POINTER(IAudioClient))
pFormat = client.GetMixFormat()
print(f"GetMixFormat: {pFormat}")

try:
    hr = client.Initialize(0, 0, 10000000, 0, pFormat, None)
    print(f"Initialize: {hex(hr)}")
except Exception as e:
    print(f"Initialize failed with exception: {e}")
