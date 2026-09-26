using System;
using System.Runtime.InteropServices;

namespace WasapiDiag {
    [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceEnumerator {
        int EnumAudioEndpoints(int dataFlow, int stateMask, out IntPtr devices);
        int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice endpoint);
    }

    [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDevice {
        int Activate(ref Guid id, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object interfacePointer);
    }

    [Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioClient {
        int Initialize(int shareMode, int streamFlags, long hnsBufferDuration, long hnsPeriodicity, IntPtr pFormat, IntPtr AudioSessionGuid);
        int GetBufferSize(out uint pNumBufferFrames);
        int GetStreamLatency(out long phnsLatency);
        int GetCurrentPadding(out uint pNumPaddingFrames);
        int IsFormatSupported(int shareMode, IntPtr pFormat, out IntPtr ppClosestMatch);
        int GetMixFormat(out IntPtr ppDeviceFormat);
    }

    class Program {
        [DllImport("ole32.dll")]
        public static extern int CoInitialize(IntPtr pvReserved);

        static void Main() {
            CoInitialize(IntPtr.Zero);
            Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
            Type enumType = Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator);
            IMMDeviceEnumerator enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(enumType);

            IMMDevice dev;
            int hr = enumerator.GetDefaultAudioEndpoint(1, 1, out dev); // 1 = eCapture, 1 = eMultimedia
            Console.WriteLine("GetDefaultAudioEndpoint(Capture, Multimedia): 0x{0:X}", hr);
            if (hr != 0) return;

            Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
            object clientObj;
            hr = dev.Activate(ref IID_IAudioClient, 1, IntPtr.Zero, out clientObj);
            Console.WriteLine("Activate(IAudioClient): 0x{0:X}", hr);
            if (hr != 0) return;

            IAudioClient client = (IAudioClient)clientObj;
            IntPtr pMixFormat;
            hr = client.GetMixFormat(out pMixFormat);
            Console.WriteLine("GetMixFormat: 0x{0:X}", hr);
            if (hr != 0) return;

            // Try 1: Shared mode, 0 flags
            hr = client.Initialize(0, 0, 10000000, 0, pMixFormat, IntPtr.Zero);
            Console.WriteLine("Initialize(shared, flags=0): 0x{0:X}", hr);

            // Try 2: Shared mode with AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM (0x80000000)
            hr = client.Initialize(0, unchecked((int)0x80000000), 10000000, 0, pMixFormat, IntPtr.Zero);
            Console.WriteLine("Initialize(shared, AUTOCONVERTPCM): 0x{0:X}", hr);
        }
    }
}
