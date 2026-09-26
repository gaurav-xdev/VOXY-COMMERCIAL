using System;
using System.Runtime.InteropServices;

namespace TestRaw
{
    [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceEnumerator
    {
        [PreserveSig] int EnumAudioEndpoints(int dataFlow, int stateMask, out IntPtr devices);
        [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice endpoint);
    }

    [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid id, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object interfacePointer);
    }

    [Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioClient
    {
        [PreserveSig] int Initialize(int shareMode, int streamFlags, long hnsBufferDuration, long hnsPeriodicity, IntPtr pFormat, IntPtr AudioSessionGuid);
        [PreserveSig] int GetBufferSize(out uint pNumBufferFrames);
        [PreserveSig] int GetStreamLatency(out long phnsLatency);
        [PreserveSig] int GetCurrentPadding(out uint pNumPaddingFrames);
        [PreserveSig] int IsFormatSupported(int shareMode, IntPtr pFormat, out IntPtr ppClosestMatch);
        [PreserveSig] int GetMixFormat(out IntPtr ppDeviceFormat);
    }

    class Program
    {
        [DllImport("ole32.dll")] public static extern int CoInitialize(IntPtr pv);

        static void Main()
        {
            CoInitialize(IntPtr.Zero);
            Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
            Type enumType = Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator);
            IMMDeviceEnumerator enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(enumType);

            IMMDevice dev;
            int hr = enumerator.GetDefaultAudioEndpoint(1, 1, out dev);

            Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
            object clientObj;
            hr = dev.Activate(ref IID_IAudioClient, 1, IntPtr.Zero, out clientObj);
            IAudioClient client = (IAudioClient)clientObj;
            IntPtr pFormat;
            client.GetMixFormat(out pFormat);

            // AUDCLNT_STREAMFLAGS_RAW = 0x04000000
            hr = client.Initialize(0, 0x04000000, 10000000, 0, pFormat, IntPtr.Zero);
            Console.WriteLine("Initialize(AUDCLNT_STREAMFLAGS_RAW): 0x{0:X8}", hr);

            // AUDCLNT_STREAMFLAGS_RAW | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM = 0x84000000
            hr = client.Initialize(0, unchecked((int)0x84000000), 10000000, 0, pFormat, IntPtr.Zero);
            Console.WriteLine("Initialize(RAW | AUTOCONVERTPCM):    0x{0:X8}", hr);
        }
    }
}
