using System;
using System.Runtime.InteropServices;

namespace AllDevices
{
    [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceEnumerator
    {
        [PreserveSig] int EnumAudioEndpoints(int dataFlow, int stateMask, out IMMDeviceCollection devices);
    }

    [Guid("0BD7A1BE-7A1A-44DB-8397-CC5392387B5E"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDeviceCollection
    {
        [PreserveSig] int GetCount(out uint pcDevices);
        [PreserveSig] int Item(uint nDevice, out IMMDevice ppDevice);
    }

    [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IMMDevice
    {
        [PreserveSig] int Activate(ref Guid id, int clsCtx, IntPtr activationParams, [MarshalAs(UnmanagedType.IUnknown)] out object interfacePointer);
        [PreserveSig] int OpenPropertyStore(int stgmAccess, out IPropertyStore ppProperties);
        [PreserveSig] int GetId([MarshalAs(UnmanagedType.LPWStr)] out string ppstrId);
        [PreserveSig] int GetState(out int pdwState);
    }

    [Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IPropertyStore
    {
        [PreserveSig] int GetCount(out uint cProps);
        [PreserveSig] int GetAt(uint iProp, out PropertyKey pkey);
        [PreserveSig] int GetValue(ref PropertyKey key, out PropVariant pv);
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct PropertyKey
    {
        public Guid fmtid;
        public uint pid;
    }

    [StructLayout(LayoutKind.Explicit)]
    public struct PropVariant
    {
        [FieldOffset(0)] public ushort vt;
        [FieldOffset(8)] public IntPtr pwszVal;
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

            // DEVICE_STATEMASK_ALL = 0x0000000F
            IMMDeviceCollection coll;
            enumerator.EnumAudioEndpoints(1, 0x0F, out coll);
            uint count;
            coll.GetCount(out count);
            Console.WriteLine("Total Capture Endpoints (All States): {0}", count);

            PropertyKey PKEY_Device_FriendlyName = new PropertyKey { fmtid = new Guid("A45C254E-DF1C-4EFD-8020-67D146A850E0"), pid = 14 };

            for (uint i = 0; i < count; i++)
            {
                IMMDevice dev;
                coll.Item(i, out dev);
                string id;
                dev.GetId(out id);
                int state;
                dev.GetState(out state);

                string stateStr = (state == 1) ? "ACTIVE" : (state == 2) ? "DISABLED" : (state == 4) ? "NOTPRESENT" : (state == 8) ? "UNPLUGGED" : state.ToString();

                IPropertyStore store;
                dev.OpenPropertyStore(0, out store);
                PropVariant pv;
                store.GetValue(ref PKEY_Device_FriendlyName, out pv);
                string name = (pv.vt == 31 && pv.pwszVal != IntPtr.Zero) ? Marshal.PtrToStringUni(pv.pwszVal) : "Unknown";

                Console.WriteLine("[{0}] State: {1,-10} | Name: {2} | ID: {3}", i, stateStr, name, id);
            }
        }
    }
}
