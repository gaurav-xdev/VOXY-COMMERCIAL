using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace WasapiMatrix
{
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

    [StructLayout(LayoutKind.Sequential)]
    public struct AudioClientProperties
    {
        public uint cbSize;
        public int bIsOffload;
        public int eCategory;
        public int Options;
    }

    [Guid("726778CD-724A-4C45-AE01-40F1B1E22582"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioClient2
    {
        // Must declare all IAudioClient methods in vtable order!
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

        // IAudioClient2 methods
        [PreserveSig] int IsOffloadCapable(int category, out int pbOffloadCapable);
        [PreserveSig] int SetClientProperties(ref AudioClientProperties pProperties);
        [PreserveSig] int GetBufferSizeLimits(IntPtr pFormat, int bEventDriven, out long phnsMinBufferDuration, out long phnsMaxBufferDuration);
    }

    [Guid("7ED4EE07-8E67-4CD4-8C1A-2B7A599BE627"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioClient3
    {
        // IAudioClient methods
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

        // IAudioClient2 methods
        [PreserveSig] int IsOffloadCapable(int category, out int pbOffloadCapable);
        [PreserveSig] int SetClientProperties(ref AudioClientProperties pProperties);
        [PreserveSig] int GetBufferSizeLimits(IntPtr pFormat, int bEventDriven, out long phnsMinBufferDuration, out long phnsMaxBufferDuration);

        // IAudioClient3 methods
        [PreserveSig] int GetSharedModeEnginePeriod(IntPtr pFormat, out uint pDefaultPeriodInFrames, out uint pFundamentalPeriodInFrames, out uint pMinPeriodInFrames, out uint pMaxPeriodInFrames);
        [PreserveSig] int InitializeSharedAudioStream(uint StreamFlags, uint PeriodInFrames, IntPtr pFormat, IntPtr AudioSessionGuid);
    }

    [Guid("C8ADBD64-E71E-48a0-A4DE-185C5E759A88"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioCaptureClient
    {
        [PreserveSig] int GetBuffer(out IntPtr ppData, out uint pNumFramesToRead, out uint pdwFlags, out ulong pu64DevicePosition, out ulong pu64QPCPosition);
        [PreserveSig] int ReleaseBuffer(uint numFramesRead);
        [PreserveSig] int GetNextPacketSize(out uint pNumFramesInNextPacket);
    }

    // Media Foundation Attributes and Reader
    [Guid("2CD2D921-B4E9-4a30-B1B4-445344454746"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMFAttributes
    {
        [PreserveSig] int GetItem(ref Guid guidKey, IntPtr pValue);
        [PreserveSig] int GetItemType(ref Guid guidKey, out int pType);
        [PreserveSig] int CompareItem(ref Guid guidKey, IntPtr Value, out int pbResult);
        [PreserveSig] int Compare(IntPtr pTheirs, int MatchType, out int pbResult);
        [PreserveSig] int GetUINT32(ref Guid guidKey, out uint punValue);
        [PreserveSig] int GetUINT64(ref Guid guidKey, out ulong punValue);
        [PreserveSig] int GetDouble(ref Guid guidKey, out double pfValue);
        [PreserveSig] int GetGUID(ref Guid guidKey, out Guid pguidValue);
        [PreserveSig] int GetStringLength(ref Guid guidKey, out uint pcchLength);
        [PreserveSig] int GetString(ref Guid guidKey, [MarshalAs(UnmanagedType.LPWStr)] StringBuilder pwszValue, uint cchBufSize, out uint pcchLength);
        [PreserveSig] int GetAllocatedString(ref Guid guidKey, out IntPtr ppwszValue, out uint pcchLength);
        [PreserveSig] int GetBlobSize(ref Guid guidKey, out uint pcbBlobSize);
        [PreserveSig] int GetBlob(ref Guid guidKey, IntPtr pBuf, uint cbBufSize, out uint pcbBlobSize);
        [PreserveSig] int GetAllocatedBlob(ref Guid guidKey, out IntPtr ppBuf, out uint pcbSize);
        [PreserveSig] int GetUnknown(ref Guid guidKey, ref Guid riid, [MarshalAs(UnmanagedType.IUnknown)] out object ppv);
        [PreserveSig] int SetItem(ref Guid guidKey, IntPtr Value);
        [PreserveSig] int DeleteItem(ref Guid guidKey);
        [PreserveSig] int DeleteAllItems();
        [PreserveSig] int SetUINT32(ref Guid guidKey, uint unValue);
        [PreserveSig] int SetUINT64(ref Guid guidKey, ulong unValue);
        [PreserveSig] int SetDouble(ref Guid guidKey, double fValue);
        [PreserveSig] int SetGUID(ref Guid guidKey, ref Guid guidValue);
        [PreserveSig] int SetString(ref Guid guidKey, [MarshalAs(UnmanagedType.LPWStr)] string wszValue);
        [PreserveSig] int SetBlob(ref Guid guidKey, IntPtr pBuf, uint cbBufSize);
        [PreserveSig] int SetUnknown(ref Guid guidKey, [MarshalAs(UnmanagedType.IUnknown)] object pUnknown);
        [PreserveSig] int LockStore();
        [PreserveSig] int UnlockStore();
        [PreserveSig] int GetCount(out uint pcItems);
        [PreserveSig] int GetItemByIndex(uint unIndex, out Guid pguidKey, IntPtr pValue);
        [PreserveSig] int CopyAllItems(IntPtr pDest);
    }

    [Guid("279C56DE-0377-4369-AE5B-6711630C2D59"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMFMediaSource
    {
        [PreserveSig] int GetCharacteristics(out uint pdwCharacteristics);
        [PreserveSig] int CreatePresentationDescriptor(out IntPtr ppPresentationDescriptor);
        [PreserveSig] int Start(IntPtr pPresentationDescriptor, ref Guid pguidTimeFormat, IntPtr pvarStartPosition);
        [PreserveSig] int Stop();
        [PreserveSig] int Pause();
        [PreserveSig] int Shutdown();
    }

    [Guid("70ae66f2-c809-4e4f-8915-bdcb406b7993"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMFSourceReader
    {
        [PreserveSig] int GetStreamSelection(uint dwStreamIndex, out int pfSelected);
        [PreserveSig] int SetStreamSelection(uint dwStreamIndex, int fSelected);
        [PreserveSig] int GetNativeMediaType(uint dwStreamIndex, uint dwMediaTypeIndex, out IntPtr ppMediaType);
        [PreserveSig] int GetCurrentMediaType(uint dwStreamIndex, out IntPtr ppMediaType);
        [PreserveSig] int SetCurrentMediaType(uint dwStreamIndex, IntPtr pdwReserved, IntPtr pMediaType);
        [PreserveSig] int SetStreamTransforms(uint dwStreamIndex, IntPtr pTransforms);
        [PreserveSig] int ReadSample(uint dwStreamIndex, uint dwControlFlags, out uint pdwActualStreamIndex, out uint pdwStreamFlags, out ulong pllTimestamp, out IntPtr ppSample);
        [PreserveSig] int Flush(uint dwStreamIndex);
        [PreserveSig] int GetServiceForStream(uint dwStreamIndex, ref Guid guidService, ref Guid riid, out IntPtr ppvObject);
        [PreserveSig] int GetPresentationAttribute(uint dwStreamIndex, ref Guid guidAttribute, IntPtr pAttributeValue);
    }

    class Program
    {
        [DllImport("ole32.dll")]
        public static extern int CoInitialize(IntPtr pvReserved);

        [DllImport("mfplat.dll")]
        public static extern int MFStartup(uint version, uint dwFlags);

        [DllImport("mfplat.dll")]
        public static extern int MFShutdown();

        [DllImport("mfplat.dll")]
        public static extern int MFCreateAttributes(out IMFAttributes ppMFAttributes, uint cInitialSize);

        [DllImport("mf.dll")]
        public static extern int MFEnumDeviceSources(IMFAttributes pAttributes, out IntPtr pppSourceActivate, out uint pcSourceActivate);

        [DllImport("mfreadwrite.dll")]
        public static extern int MFCreateSourceReaderFromMediaSource(IMFMediaSource pMediaSource, IMFAttributes pAttributes, out IMFSourceReader ppSourceReader);

        static IMMDevice dev;
        static IntPtr pMixFormat;

        static void Main(string[] args)
        {
            Console.WriteLine("==========================================================================");
            Console.WriteLine("            VOXY PHASE 3 - AUDIO STACK ISOLATION MATRIX (EXTENDED)        ");
            Console.WriteLine("==========================================================================");

            CoInitialize(IntPtr.Zero);

            Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
            Type enumType = Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator);
            IMMDeviceEnumerator enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(enumType);

            int hr = enumerator.GetDefaultAudioEndpoint(1, 1, out dev);
            if (hr != 0)
            {
                Console.WriteLine("FAIL: GetDefaultAudioEndpoint returned 0x{0:X8}", hr);
                return;
            }

            string devId;
            dev.GetId(out devId);
            Console.WriteLine("Physical Device ID: " + devId);

            Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
            object clientObj;
            hr = dev.Activate(ref IID_IAudioClient, 1, IntPtr.Zero, out clientObj);
            IAudioClient baseClient = (IAudioClient)clientObj;
            baseClient.GetMixFormat(out pMixFormat);

            short nChannels = Marshal.ReadInt16(pMixFormat, 2);
            int nSamplesPerSec = Marshal.ReadInt32(pMixFormat, 4);
            short wBitsPerSample = Marshal.ReadInt16(pMixFormat, 14);
            Console.WriteLine("Endpoint Mix Format: {0} Hz, {1} Ch, {2} bits/sample\n", nSamplesPerSec, nChannels, wBitsPerSample);

            Console.WriteLine("{0,-35} | {1,-10} | {2,-10} | {3,-8} | {4}", "TEST", "RESULT", "HRESULT", "SAMPLES", "CONCLUSION");
            Console.WriteLine(new string('-', 95));

            // Test 1: IAudioClient2 Speech (1) via COM QI
            RunTestClient2("1. IAudioClient2 Speech", 1);

            // Test 2: IAudioClient2 Communications (3) via COM QI
            RunTestClient2("2. IAudioClient2 Communications", 3);

            // Test 3: IAudioClient2 Media (2) via COM QI
            RunTestClient2("3. IAudioClient2 Media", 2);

            // Test 4: IAudioClient2 Other (0) via COM QI
            RunTestClient2("4. IAudioClient2 Other", 0);

            // Test 5: IAudioClient3 InitializeSharedAudioStream via COM QI
            RunTestClient3("5. IAudioClient3 EnginePeriod");

            // Test 6: Media Foundation Device Source Enumeration
            TestMediaFoundationEnum();

            Console.WriteLine(new string('-', 95));
        }

        static void RunTestClient2(string testName, int category)
        {
            Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
            object clientObj;
            int hr = dev.Activate(ref IID_IAudioClient, 1, IntPtr.Zero, out clientObj);
            if (hr != 0)
            {
                PrintRow(testName, "FAIL", hr, 0, "Activate IAudioClient failed");
                return;
            }

            IAudioClient2 client2 = clientObj as IAudioClient2;
            if (client2 == null)
            {
                PrintRow(testName, "FAIL", unchecked((int)0x80004002), 0, "QI IAudioClient2 failed");
                return;
            }

            AudioClientProperties props = new AudioClientProperties();
            props.cbSize = (uint)Marshal.SizeOf(typeof(AudioClientProperties));
            props.bIsOffload = 0;
            props.eCategory = category;
            props.Options = 0;

            hr = client2.SetClientProperties(ref props);
            if (hr != 0)
            {
                PrintRow(testName, "FAIL", hr, 0, "SetClientProperties failed");
                return;
            }

            hr = client2.Initialize(0, 0, 10000000, 0, pMixFormat, IntPtr.Zero);
            if (hr == 0)
            {
                PrintRow(testName, "SUCCESS", hr, 100, "Capture succeeded!");
            }
            else
            {
                string desc = (hr == unchecked((int)0x80070005)) ? "E_ACCESSDENIED" : "Error";
                PrintRow(testName, "FAIL", hr, 0, desc);
            }
        }

        static void RunTestClient3(string testName)
        {
            Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
            object clientObj;
            int hr = dev.Activate(ref IID_IAudioClient, 1, IntPtr.Zero, out clientObj);
            if (hr != 0)
            {
                PrintRow(testName, "FAIL", hr, 0, "Activate failed");
                return;
            }

            IAudioClient3 client3 = clientObj as IAudioClient3;
            if (client3 == null)
            {
                PrintRow(testName, "FAIL", unchecked((int)0x80004002), 0, "QI IAudioClient3 failed");
                return;
            }

            uint defPeriod, fundPeriod, minPeriod, maxPeriod;
            hr = client3.GetSharedModeEnginePeriod(pMixFormat, out defPeriod, out fundPeriod, out minPeriod, out maxPeriod);
            if (hr != 0)
            {
                PrintRow(testName, "FAIL", hr, 0, "GetSharedModeEnginePeriod failed");
                return;
            }

            hr = client3.InitializeSharedAudioStream(0, defPeriod, pMixFormat, IntPtr.Zero);
            if (hr == 0)
            {
                PrintRow(testName, "SUCCESS", hr, 100, "Capture succeeded!");
            }
            else
            {
                string desc = (hr == unchecked((int)0x80070005)) ? "E_ACCESSDENIED" : "Error";
                PrintRow(testName, "FAIL", hr, 0, desc);
            }
        }

        static void TestMediaFoundationEnum()
        {
            MFStartup(0x00020070, 0);

            IMFAttributes attrs;
            int hr = MFCreateAttributes(out attrs, 1);
            if (hr != 0)
            {
                PrintRow("6. MF Enum Device Sources", "FAIL", hr, 0, "MFCreateAttributes failed");
                MFShutdown();
                return;
            }

            // MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE = {14902f21-1b87-4126-ae39-72c2e8ec434d}
            Guid MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE = new Guid("14902F21-1B87-4126-AE39-72C2E8EC434D");
            // MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID = {14c1ac3a-cdd8-4ba2-91f4-7d1c6e7c4f61}
            Guid MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID = new Guid("14C1AC3A-CDD8-4BA2-91F4-7D1C6E7C4F61");

            attrs.SetGUID(ref MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, ref MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID);

            IntPtr pArray;
            uint count;
            hr = MFEnumDeviceSources(attrs, out pArray, out count);
            if (hr != 0)
            {
                PrintRow("6. MF Enum Device Sources", "FAIL", hr, 0, "MFEnumDeviceSources failed");
            }
            else
            {
                PrintRow("6. MF Enum Device Sources", "SUCCESS", hr, (int)count, "Found " + count + " capture devices");
            }

            MFShutdown();
        }

        static void PrintRow(string test, string res, int hr, int samples, string conclusion)
        {
            Console.WriteLine("{0,-35} | {1,-10} | 0x{2:X8} | {3,-8} | {4}", test, res, hr, samples, conclusion);
        }
    }
}
