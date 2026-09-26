using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Principal;
using System.Threading;
using System.Windows.Forms;

namespace VoxyGuiMicTest
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

    [Guid("C8ADBD64-E71E-48a0-A4DE-185C5E759A88"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IAudioCaptureClient
    {
        [PreserveSig] int GetBuffer(out IntPtr ppData, out uint pNumFramesToRead, out uint pdwFlags, out ulong pu64DevicePosition, out ulong pu64QPCPosition);
        [PreserveSig] int ReleaseBuffer(uint numFramesRead);
        [PreserveSig] int GetNextPacketSize(out uint pNumFramesInNextPacket);
    }

    public class MainForm : Form
    {
        private TextBox txtLog;
        private Button btnTest;
        private string logFilePath;

        [DllImport("ole32.dll")]
        public static extern int CoInitialize(IntPtr pvReserved);

        public MainForm()
        {
            this.Text = "VOXY - Native Win32 GUI Mic Isolation Test";
            this.Width = 720;
            this.Height = 520;
            this.StartPosition = FormStartPosition.CenterScreen;

            logFilePath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "gui_test_result.log");

            txtLog = new TextBox();
            txtLog.Multiline = true;
            txtLog.ScrollBars = ScrollBars.Vertical;
            txtLog.Dock = DockStyle.Fill;
            txtLog.Font = new System.Drawing.Font("Consolas", 9.75f);
            txtLog.ReadOnly = true;

            btnTest = new Button();
            btnTest.Text = "Run WASAPI Microphone Test";
            btnTest.Dock = DockStyle.Top;
            btnTest.Height = 35;
            btnTest.Click += (s, e) => RunTest();

            this.Controls.Add(txtLog);
            this.Controls.Add(btnTest);
        }

        public void Log(string msg)
        {
            try
            {
                if (txtLog.InvokeRequired)
                {
                    txtLog.Invoke((MethodInvoker)delegate { Log(msg); });
                    return;
                }
                txtLog.AppendText(msg + "\r\n");
                txtLog.SelectionStart = txtLog.Text.Length;
                txtLog.ScrollToCaret();
            }
            catch {}
            Console.WriteLine(msg);
            try
            {
                File.AppendAllText(logFilePath, msg + "\r\n");
            }
            catch {}
        }

        public void RunTest()
        {
            try { File.WriteAllText(logFilePath, ""); } catch {}
            Log("============================================================");
            Log("      NATIVE WINDOWS GUI MICROPHONE TEST APPLICATION        ");
            Log("============================================================");

            // Log Process Integrity & Identity
            WindowsIdentity id = WindowsIdentity.GetCurrent();
            WindowsPrincipal principal = new WindowsPrincipal(id);
            bool isElevated = principal.IsInRole(WindowsBuiltInRole.Administrator);
            Log("User:            " + id.Name);
            Log("Is Elevated:     " + isElevated);
            Log("Process ID:      " + System.Diagnostics.Process.GetCurrentProcess().Id);
            Log("Process Path:    " + Application.ExecutablePath);
            Log("Window Handle:   0x" + this.Handle.ToString("X"));

            try
            {
                CoInitialize(IntPtr.Zero);
                Guid CLSID_MMDeviceEnumerator = new Guid("BCDE0395-E52F-467C-8E3D-C4579291692E");
                Type enumType = Type.GetTypeFromCLSID(CLSID_MMDeviceEnumerator);
                IMMDeviceEnumerator enumerator = (IMMDeviceEnumerator)Activator.CreateInstance(enumType);

                IMMDevice dev;
                // dataFlow = 1 (eCapture), role = 1 (eMultimedia)
                int hr = enumerator.GetDefaultAudioEndpoint(1, 1, out dev);
                Log(string.Format("GetDefaultAudioEndpoint(eCapture, eMultimedia): 0x{0:X8}", hr));
                if (hr != 0)
                {
                    Log("FAILURE: Could not get default capture endpoint.");
                    return;
                }

                string devId;
                dev.GetId(out devId);
                Log("Endpoint ID:     " + devId);

                int state;
                dev.GetState(out state);
                Log("Endpoint State:  0x" + state.ToString("X") + (state == 1 ? " (DEVICE_STATE_ACTIVE)" : " (INACTIVE)"));

                Guid IID_IAudioClient = new Guid("1CB9AD4C-DBFA-4c32-B178-C2F568A703B2");
                object clientObj;
                hr = dev.Activate(ref IID_IAudioClient, 1, IntPtr.Zero, out clientObj);
                Log(string.Format("dev.Activate(IAudioClient): 0x{0:X8}", hr));
                if (hr != 0)
                {
                    Log("FAILURE: Could not activate IAudioClient.");
                    return;
                }

                IAudioClient client = (IAudioClient)clientObj;
                IntPtr pMixFormat;
                hr = client.GetMixFormat(out pMixFormat);
                Log(string.Format("client.GetMixFormat: 0x{0:X8}", hr));
                if (hr != 0)
                {
                    Log("FAILURE: Could not get mix format.");
                    return;
                }

                // Read WAVEFORMATEX
                short wFormatTag = Marshal.ReadInt16(pMixFormat, 0);
                short nChannels = Marshal.ReadInt16(pMixFormat, 2);
                int nSamplesPerSec = Marshal.ReadInt32(pMixFormat, 4);
                short wBitsPerSample = Marshal.ReadInt16(pMixFormat, 14);
                Log(string.Format("Mix Format:      {0} Hz, {1} Channels, {2} bits/sample, Tag: 0x{3:X}", nSamplesPerSec, nChannels, wBitsPerSample, wFormatTag));

                // Initialize AudioClient in SHARED mode
                // AUDCLNT_SHAREMODE_SHARED = 0
                // 10,000,000 hns = 1 second buffer
                Log("\n[TEST] Calling IAudioClient::Initialize (Shared Mode, 0 flags)...");
                hr = client.Initialize(0, 0, 10000000, 0, pMixFormat, IntPtr.Zero);
                Log(string.Format("IAudioClient::Initialize Result: 0x{0:X8}", hr));

                if (hr != 0)
                {
                    if (hr == unchecked((int)0x80070005))
                    {
                        Log("\n>>> CRITICAL RESULT: 0x80070005 (E_ACCESSDENIED) in interactive GUI Win32 process!");
                        Log(">>> Conclusion: Native GUI process context ALSO fails with 0x80070005.");
                    }
                    else
                    {
                        Log(string.Format("\n>>> WASAPI Initialization Failed with error: 0x{0:X8}", hr));
                    }
                    return;
                }

                Log(">>> SUCCESS: IAudioClient::Initialize SUCCEEDED!");

                uint bufferFrameCount;
                client.GetBufferSize(out bufferFrameCount);
                Log("Buffer Size:     " + bufferFrameCount + " frames");

                Guid IID_IAudioCaptureClient = new Guid("C8ADBD64-E71E-48a0-A4DE-185C5E759A88");
                object captureObj;
                hr = client.GetService(ref IID_IAudioCaptureClient, out captureObj);
                Log(string.Format("client.GetService(IAudioCaptureClient): 0x{0:X8}", hr));
                if (hr != 0) return;

                IAudioCaptureClient capture = (IAudioCaptureClient)captureObj;

                hr = client.Start();
                Log(string.Format("client.Start(): 0x{0:X8}", hr));
                if (hr != 0) return;

                Log("\n[CAPTURE] Recording for 3 seconds to test actual microphone audio...");
                MemoryStream pcmStream = new MemoryStream();
                DateTime start = DateTime.Now;
                long totalPackets = 0;
                long totalFrames = 0;
                float peakSample = 0f;
                bool hasNonZero = false;

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

                            if ((flags & 0x01) == 0) // AUDCLNT_BUFFERFLAGS_SILENT = 0x01
                            {
                                pcmStream.Write(buffer, 0, bytesToRead);
                                if (wBitsPerSample == 32)
                                {
                                    for (int b = 0; b < bytesToRead; b += 4)
                                    {
                                        float val = BitConverter.ToSingle(buffer, b);
                                        float abs = Math.Abs(val);
                                        if (abs > peakSample) peakSample = abs;
                                        if (abs > 0.0001f) hasNonZero = true;
                                    }
                                }
                            }

                            capture.ReleaseBuffer(numFramesToRead);
                        }
                        capture.GetNextPacketSize(out packetSize);
                    }
                    Thread.Sleep(10);
                }

                client.Stop();
                Log("\n[SUMMARY]");
                Log("Packets Received: " + totalPackets);
                Log("Frames Captured:  " + totalFrames);
                Log("Peak Amplitude:   " + peakSample);
                Log("Non-Zero Audio:   " + hasNonZero);

                // Save to WAV
                if (pcmStream.Length > 0)
                {
                    string wavPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "gui_mic_capture.wav");
                    using (FileStream fs = new FileStream(wavPath, FileMode.Create))
                    using (BinaryWriter bw = new BinaryWriter(fs))
                    {
                        bw.Write(new char[] { 'R', 'I', 'F', 'F' });
                        bw.Write((int)(36 + pcmStream.Length));
                        bw.Write(new char[] { 'W', 'A', 'V', 'E' });
                        bw.Write(new char[] { 'f', 'm', 't', ' ' });
                        bw.Write((int)16);
                        bw.Write((short)(wBitsPerSample == 32 ? 3 : 1));
                        bw.Write(nChannels);
                        bw.Write(nSamplesPerSec);
                        bw.Write((int)(nSamplesPerSec * nChannels * (wBitsPerSample / 8)));
                        bw.Write((short)(nChannels * (wBitsPerSample / 8)));
                        bw.Write(wBitsPerSample);
                        bw.Write(new char[] { 'd', 'a', 't', 'a' });
                        bw.Write((int)pcmStream.Length);
                        bw.Write(pcmStream.ToArray());
                    }
                    Log("Saved captured audio to: " + wavPath + " (" + pcmStream.Length + " bytes)");
                }
            }
            catch (Exception ex)
            {
                Log("EXCEPTION: " + ex.ToString());
            }
        }

        [STAThread]
        static void Main(string[] args)
        {
            Application.EnableVisualStyles();
            Application.SetCompatibleTextRenderingDefault(false);
            MainForm form = new MainForm();

            form.HandleCreated += (s, e) => {
                new Thread(() => {
                    Thread.Sleep(500);
                    form.RunTest();
                    if (args.Length > 0 && args[0] == "--auto-close")
                    {
                        Thread.Sleep(1000);
                        form.Invoke((MethodInvoker)delegate { form.Close(); });
                    }
                }).Start();
            };

            Application.Run(form);
        }
    }
}
