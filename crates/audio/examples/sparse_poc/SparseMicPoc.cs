using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace SparseMicPoc
{
    // Native Package Identity APIs
    internal static class NativeMethods
    {
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern int GetCurrentPackageFullName(ref int packageFullNameLength, StringBuilder packageFullName);

        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        public static extern int GetCurrentPackageFamilyName(ref int packageFamilyNameLength, StringBuilder packageFamilyName);
    }

    [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceEnumerator
    {
        [PreserveSig] int EnumAudioEndpoints(int dataFlow, int stateMask, out IntPtr devices);
        [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice endpoint);
        [PreserveSig] int GetDevice(string pwstrId, out IMMDevice endpoint);
    }

    [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid id, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object interfacePointer);
        [PreserveSig] int OpenPropertyStore(int stgmAccess, out IntPtr ppProperties);
        [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string ppstrId);
        [PreserveSig] int GetState(out int pdwState);
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
        [PreserveSig] int GetDevicePeriod(out long phnsDefaultDevicePeriod, out long phnsMinimumDevicePeriod);
        [PreserveSig] int Start();
        [PreserveSig] int Stop();
        [PreserveSig] int Reset();
        [PreserveSig] int SetEventHandle(IntPtr eventHandle);
        [PreserveSig] int GetService(ref Guid riid, [MarshalAs(UnmanagedType.IUnknown)] out object ppv);
    }

    [Guid("C8ADBD64-E71E-48a0-A4DE-185C5E759A88"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioCaptureClient
    {
        [PreserveSig] int GetBuffer(out IntPtr ppData, out uint pNumFramesToRead, out uint pdwFlags, out ulong pu64DevicePosition, out ulong pu64QPCPosition);
        [PreserveSig] int ReleaseBuffer(uint numFramesRead);
        [PreserveSig] int GetNextPacketSize(out uint pNumFramesInNextPacket);
    }

    class Program
    {
        [DllImport("ole32.dll")]
        public static extern int CoInitialize(IntPtr pvReserved);

        static void Main(string[] args)
        {
            Console.WriteLine("============================================================");
            Console.WriteLine("          VOXY SPARSE PACKAGE IDENTITY MIC POC              ");
            Console.WriteLine("============================================================");

            // 1. Inspect Process Package Identity
            int len = 0;
            StringBuilder sbFullName = new StringBuilder(0);
            int res = NativeMethods.GetCurrentPackageFullName(ref len, sbFullName);
            string packageFullName = "NONE (Non-Packaged Win32)";
            string packageFamilyName = "NONE";
            bool isPackaged = false;

            if (res == 122) // ERROR_INSUFFICIENT_BUFFER
            {
                sbFullName = new StringBuilder(len);
                if (NativeMethods.GetCurrentPackageFullName(ref len, sbFullName) == 0)
                {
                    packageFullName = sbFullName.ToString();
                    isPackaged = true;

                    int famLen = 0;
                    StringBuilder sbFamilyName = new StringBuilder(0);
                    NativeMethods.GetCurrentPackageFamilyName(ref famLen, sbFamilyName);
                    if (famLen > 0)
                    {
                        sbFamilyName = new StringBuilder(famLen);
                        if (NativeMethods.GetCurrentPackageFamilyName(ref famLen, sbFamilyName) == 0)
                        {
                            packageFamilyName = sbFamilyName.ToString();
                        }
                    }
                }
            }

            Console.WriteLine("Is Packaged:         " + isPackaged);
            Console.WriteLine("Package Full Name:   " + packageFullName);
            Console.WriteLine("Package Family Name: " + packageFamilyName);
            Console.WriteLine("Executable Path:     " + System.Reflection.Assembly.GetExecutingAssembly().Location);
            Console.WriteLine("Process ID:          " + System.Diagnostics.Process.GetCurrentProcess().Id);

            // 2. Perform WASAPI Capture
            Console.WriteLine("\n[1] Activating Default Audio Capture Endpoint...");
            CoInitialize(IntPtr.Zero);
            Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
            Type enumType = Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator);
            IMMDeviceEnumerator enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(enumType);

            IMMDevice dev;
            int hr = enumerator.GetDefaultAudioEndpoint(1, 1, out dev);
            Console.WriteLine("GetDefaultAudioEndpoint: 0x{0:X8}", hr);
            if (hr != 0) return;

            string devId;
            dev.GetId(out devId);
            Console.WriteLine("Endpoint ID:             " + devId);

            Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
            object clientObj;
            hr = dev.Activate(ref IID_IAudioClient, 1, IntPtr.Zero, out clientObj);
            Console.WriteLine("Activate(IAudioClient):  0x{0:X8}", hr);
            if (hr != 0) return;

            IAudioClient client = (IAudioClient)clientObj;
            IntPtr pMixFormat;
            hr = client.GetMixFormat(out pMixFormat);
            Console.WriteLine("GetMixFormat:            0x{0:X8}", hr);
            if (hr != 0) return;

            short nChannels = Marshal.ReadInt16(pMixFormat, 2);
            int nSamplesPerSec = Marshal.ReadInt32(pMixFormat, 4);
            short wBitsPerSample = Marshal.ReadInt16(pMixFormat, 14);
            Console.WriteLine("Mix Format:              {0} Hz, {1} Ch, {2} bits", nSamplesPerSec, nChannels, wBitsPerSample);

            // Call IAudioClient::Initialize
            Console.WriteLine("\n[2] Initializing WASAPI AudioClient (Shared Mode, 0 flags)...");
            hr = client.Initialize(0, 0, 10000000, 0, pMixFormat, IntPtr.Zero);
            Console.WriteLine("Initialize:              0x{0:X8}", hr);

            if (hr != 0)
            {
                Console.WriteLine("\n>>> INITIALIZE FAILED with HRESULT 0x{0:X8}", hr);
                if (hr == unchecked((int)0x80070005))
                {
                    Console.WriteLine(">>> RESULT: E_ACCESSDENIED (0x80070005)");
                }
                return;
            }

            Console.WriteLine(">>> SUCCESS: Initialize SUCCEEDED (S_OK)!");

            Guid IID_IAudioCaptureClient = new Guid("C8ADBD64-E71E-48a0-A4DE-185C5E759A88");
            object captureObj;
            hr = client.GetService(ref IID_IAudioCaptureClient, out captureObj);
            Console.WriteLine("GetService(CaptureClient): 0x{0:X8}", hr);
            if (hr != 0) return;

            IAudioCaptureClient capture = (IAudioCaptureClient)captureObj;

            hr = client.Start();
            Console.WriteLine("Start():                 0x{0:X8}", hr);
            if (hr != 0) return;

            Console.WriteLine("\n[3] Capturing Real Microphone PCM for 3 seconds...");
            DateTime start = DateTime.Now;
            long totalPackets = 0;
            long totalFrames = 0;
            float maxAmp = 0f;
            bool nonZero = false;

            while ((DateTime.Now - start).TotalSeconds < 3.0)
            {
                uint packetSize;
                hr = capture.GetNextPacketSize(out packetSize);
                while (packetSize != 0)
                {
                    IntPtr pData;
                    uint numFramesToRead;
                    uint flags;
                    ulong devPos, qpcPos;
                    hr = capture.GetBuffer(out pData, out numFramesToRead, out flags, out devPos, out qpcPos);
                    if (hr == 0 && numFramesToRead > 0)
                    {
                        totalPackets++;
                        totalFrames += numFramesToRead;

                        int bytesToRead = (int)(numFramesToRead * nChannels * (wBitsPerSample / 8));
                        byte[] buffer = new byte[bytesToRead];
                        Marshal.Copy(pData, buffer, 0, bytesToRead);

                        if ((flags & 0x01) == 0 && wBitsPerSample == 32)
                        {
                            for (int b = 0; b < bytesToRead; b += 4)
                            {
                                float s = Math.Abs(BitConverter.ToSingle(buffer, b));
                                if (s > maxAmp) maxAmp = s;
                                if (s > 0.0001f) nonZero = true;
                            }
                        }

                        capture.ReleaseBuffer(numFramesToRead);
                    }
                    capture.GetNextPacketSize(out packetSize);
                }
                Thread.Sleep(10);
            }

            client.Stop();
            Console.WriteLine("\n[SUMMARY]");
            Console.WriteLine("First packet:            " + (totalPackets > 0 ? "RECEIVED" : "NONE"));
            Console.WriteLine("Packet count:            " + totalPackets);
            Console.WriteLine("Captured sample count:   " + (totalFrames * nChannels));
            Console.WriteLine("Peak Amplitude:          " + maxAmp.ToString("F6"));
            Console.WriteLine("Non-zero samples:        " + nonZero);
            if (nonZero)
            {
                Console.WriteLine(">>> VERIFICATION: REAL MICROPHONE SAMPLES RECEIVED!");
            }
        }
    }
}
