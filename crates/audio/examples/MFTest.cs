using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace WasapiMatrix
{
    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceEnumerator
    {
        [PreserveSig] int EnumAudioEndpoints(int dataFlow, int stateMask, out IntPtr devices);
        [PreserveSig] int GetDefaultAudioEndpoint(int dataFlow, int role, out IMMDevice endpoint);
        [PreserveSig] int GetDevice(string pwstrId, out IMMDevice endpoint);
    }

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid id, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object interfacePointer);
        [PreserveSig] int OpenPropertyStore(int stgmAccess, out IntPtr ppProperties);
        [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string ppstrId);
        [PreserveSig] int GetState(out int pdwState);
    }

    [ComImport, Guid("2CD2D921-B4E9-4a30-B1B4-445344454746"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
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

    [ComImport, Guid("33BC059E-6147-4973-A15A-073BC525141E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMFActivate
    {
        // Inherits IMFAttributes methods
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

        // IMFActivate methods
        [PreserveSig] int ActivateObject(ref Guid riid, [MarshalAs(UnmanagedType.IUnknown)] out object ppv);
        [PreserveSig] int ShutdownObject();
        [PreserveSig] int DetachObject();
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
        public static extern int MFCreateAttributes([MarshalAs(UnmanagedType.Interface)] out IMFAttributes ppMFAttributes, uint cInitialSize);

        [DllImport("mf.dll")]
        public static extern int MFEnumDeviceSources(IMFAttributes pAttributes, out IntPtr pppSourceActivate, out uint pcSourceActivate);

        static void Main(string[] args)
        {
            Console.WriteLine("==========================================================================");
            Console.WriteLine("            TESTING MEDIA FOUNDATION CAPTURE ON INTEL SST                 ");
            Console.WriteLine("==========================================================================");

            CoInitialize(IntPtr.Zero);
            int hr = MFStartup(0x00020070, 0); // MF_VERSION = 0x00020070 (MF_API_VERSION 0x0070)
            Console.WriteLine("MFStartup: 0x{0:X8}", hr);

            IMFAttributes attrs;
            hr = MFCreateAttributes(out attrs, 1);
            Console.WriteLine("MFCreateAttributes: 0x{0:X8}", hr);

            // MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE = {14902f21-1b87-4126-ae39-72c2e8ec434d}
            Guid MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE = new Guid("14902F21-1B87-4126-AE39-72C2E8EC434D");
            // MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID = {14c1ac3a-cdd8-4ba2-91f4-7d1c6e7c4f61}
            Guid MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID = new Guid("14C1AC3A-CDD8-4BA2-91F4-7D1C6E7C4F61");

            attrs.SetGUID(ref MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, ref MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_AUDCAP_GUID);

            IntPtr pppActivate;
            uint count;
            hr = MFEnumDeviceSources(attrs, out pppActivate, out count);
            Console.WriteLine("MFEnumDeviceSources: 0x{0:X8}, count = {1}", hr, count);

            if (hr == 0 && count > 0)
            {
                IntPtr[] activates = new IntPtr[count];
                Marshal.Copy(pppActivate, activates, 0, (int)count);

                for (int i = 0; i < count; i++)
                {
                    IMFActivate act = (IMFActivate)Marshal.GetObjectForIUnknown(activates[i]);
                    uint nameLen;
                    // MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME = {3137f12e-6e23-45bb-856b-f952f4770026}
                    Guid MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME = new Guid("3137F12E-6E23-45BB-856B-F952F4770026");
                    StringBuilder sb = new StringBuilder(256);
                    act.GetString(ref MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME, sb, (uint)sb.Capacity, out nameLen);
                    Console.WriteLine("Device [{0}]: {1}", i, sb.ToString());

                    // Try Activating the Media Source!
                    Guid IID_IMFMediaSource = new Guid("279C56DE-0377-4369-AE5B-6711630C2D59");
                    object mediaSourceObj;
                    int actHr = act.ActivateObject(ref IID_IMFMediaSource, out mediaSourceObj);
                    Console.WriteLine("  ActivateObject(IMFMediaSource): 0x{0:X8}", actHr);
                    if (actHr == unchecked((int)0x80070005))
                    {
                        Console.WriteLine("  >>> RESULT: E_ACCESSDENIED (0x80070005) inside Media Foundation!");
                    }
                    else if (actHr == 0)
                    {
                        Console.WriteLine("  >>> SUCCESS: Media Source Activated via Media Foundation!");
                    }
                    Marshal.Release(activates[i]);
                }
                Marshal.FreeCoTaskMem(pppActivate);
            }

            MFShutdown();
        }
    }
}
