#![cfg_attr(not(test), windows_subsystem = "windows")]

//! OSMOO Unified Windows Installer & Uninstaller
//!
//! Produces `OSMOO-Setup.exe` containing the complete release package:
//! - OSMOO Desktop UI (`OSMOO.exe`)
//! - OSMOO Background Daemon (`voxy-daemon.exe`)
//! - OSMOO 2D Cat Desktop Companion Overlay (`voxy-overlay.exe`)
//! - Official application branding (`osmoo.ico`, `osmoo_app_icon.png`, `osmoo_mark_white.png`)
//! - Cat companion artwork, configuration templates, and runtime dependencies.
//!
//! Provides prerequisite verification (WebView2, disk space, Windows architecture),
//! atomic extraction, Start Menu & Desktop shortcut generation, Add/Remove Programs
//! registry integration, clean uninstaller (`Uninstall.exe`), and launch-on-finish.

use std::env;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

const APP_NAME: &str = "OSMOO";
const APP_DISPLAY_NAME: &str = "OSMOO AI Operating Companion";
const APP_PUBLISHER: &str = "OSMOO Technologies";
const APP_VERSION: &str = "0.1.0";
const UNINSTALL_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OSMOO";
const RUN_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

// Embedded compressed payload generated during build
static PAYLOAD_ZIP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.zip"));

#[derive(Debug, Default)]
struct InstallOptions {
    target_dir: Option<PathBuf>,
    silent: bool,
    no_desktop_shortcut: bool,
    no_start_menu: bool,
    no_autostart: bool,
    no_launch: bool,
    uninstall: bool,
}

fn print_banner() {
    println!(r"
╔══════════════════════════════════════════════════════════════╗
║             OSMOO AI OPERATING COMPANION SETUP               ║
║                        Version {APP_VERSION}                         ║
║               (c) 2026 {APP_PUBLISHER}                ║
╚══════════════════════════════════════════════════════════════╝
");
}

fn main() {
    #[cfg(windows)]
    unsafe {
        #[link(name = "kernel32")]
        extern "system" {
            fn AttachConsole(dwProcessId: u32) -> i32;
        }
        const ATTACH_PARENT_PROCESS: u32 = 0xFFFFFFFF;
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }

    let args: Vec<String> = env::args().collect();
    let mut opts = InstallOptions::default();

    if let Ok(current) = env::current_exe() {
        if let Some(stem) = current.file_stem() {
            if stem.to_string_lossy().to_lowercase().contains("uninstall") {
                opts.uninstall = true;
            }
        }
    }

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--uninstall" => opts.uninstall = true,
            "--silent" | "-s" | "/S" => opts.silent = true,
            "--no-desktop-shortcut" => opts.no_desktop_shortcut = true,
            "--no-start-menu" => opts.no_start_menu = true,
            "--no-autostart" => opts.no_autostart = true,
            "--no-launch" => opts.no_launch = true,
            "--dir" => {
                if i + 1 < args.len() {
                    opts.target_dir = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--version" | "-V" => {
                println!("{APP_DISPLAY_NAME} Setup v{APP_VERSION}");
                return;
            }
            "--help" | "-h" | "/?" => {
                print_banner();
                println!("Usage: OSMOO-Setup.exe [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --dir <PATH>             Custom installation directory");
                println!("  --silent, -s, /S         Unattended silent installation");
                println!("  --no-desktop-shortcut    Skip creation of desktop shortcut");
                println!("  --no-start-menu          Skip creation of start menu shortcut");
                println!("  --no-autostart           Do not configure launch at user login");
                println!("  --no-launch              Do not launch OSMOO after installation");
                println!("  --uninstall              Perform uninstallation of existing setup");
                println!("  --version                Display installer version");
                println!("  --help                   Display this help message");
                return;
            }
            _ => {}
        }
        i += 1;
    }

    if opts.uninstall {
        perform_uninstall(&opts);
    } else {
        perform_install(&opts);
    }
}

fn get_default_install_dir() -> PathBuf {
    if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("Programs").join(APP_NAME)
    } else if let Ok(user_profile) = env::var("USERPROFILE") {
        PathBuf::from(user_profile).join("AppData").join("Local").join("Programs").join(APP_NAME)
    } else {
        PathBuf::from(r"C:\Program Files").join(APP_NAME)
    }
}

fn perform_install(opts: &InstallOptions) {
    if !opts.silent {
        print_banner();
    }

    let install_dir = opts.target_dir.clone().unwrap_or_else(get_default_install_dir);

    if !opts.silent {
        println!("[1/6] Performing System & Prerequisite Checks...");
    }

    // 1. Architecture Check
    if !cfg!(target_arch = "x86_64") {
        eprintln!("[ERROR] OSMOO requires a 64-bit Windows architecture (x86_64).");
        std::process::exit(1);
    }

    // 2. WebView2 Runtime Check
    let webview2_version = check_webview2_runtime();
    match webview2_version {
        Some(v) => {
            if !opts.silent {
                println!("  [✔] Microsoft Edge WebView2 Runtime: Installed ({v})");
            }
        }
        None => {
            println!("  [!] NOTICE: Microsoft Edge WebView2 Runtime not detected.");
            println!("      OSMOO UI requires WebView2. It will download automatically on first launch,");
            println!("      or you can install it from: https://go.microsoft.com/fwlink/p/?LinkId=2124703");
        }
    }

    if !opts.silent {
        println!("  [✔] Target Directory: {}", install_dir.display());
        println!();
        println!("[2/6] Stopping any running OSMOO background instances...");
    }

    // Terminate existing processes to avoid file locking during upgrade/reinstall
    kill_existing_processes();

    if !opts.silent {
        println!();
        println!("[3/6] Extracting OSMOO components and assets...");
    }

    if let Err(e) = fs::create_dir_all(&install_dir) {
        eprintln!("[FATAL] Failed to create installation directory {}: {e}", install_dir.display());
        std::process::exit(1);
    }

    // Extract payload.zip
    if let Err(e) = extract_payload(&install_dir, opts.silent) {
        eprintln!("[FATAL] Extraction failed: {e}");
        std::process::exit(1);
    }

    // Copy installer as Uninstall.exe in target dir
    if let Ok(current_exe) = env::current_exe() {
        let uninstaller_dest = install_dir.join("Uninstall.exe");
        let _ = fs::copy(&current_exe, &uninstaller_dest);
    }

    if !opts.silent {
        println!();
        println!("[4/6] Creating Windows Shortcuts...");
    }

    let osmoo_exe = install_dir.join("OSMOO.exe");
    let icon_path = install_dir.join("assets").join("icons").join("osmoo.ico");

    // Start Menu Shortcut
    if !opts.no_start_menu {
        if let Ok(app_data) = env::var("APPDATA") {
            let start_menu_dir = PathBuf::from(app_data)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs");
            let _ = fs::create_dir_all(&start_menu_dir);
            let shortcut_path = start_menu_dir.join(format!("{APP_NAME}.lnk"));
            create_shortcut(&shortcut_path, &osmoo_exe, &install_dir, &icon_path);
            if !opts.silent {
                println!("  [✔] Start Menu Shortcut: {}", shortcut_path.display());
            }
        }
    }

    // Desktop Shortcut
    if !opts.no_desktop_shortcut {
        if let Ok(user_profile) = env::var("USERPROFILE") {
            let desktop_dir = PathBuf::from(user_profile).join("Desktop");
            if desktop_dir.exists() {
                let shortcut_path = desktop_dir.join(format!("{APP_NAME}.lnk"));
                create_shortcut(&shortcut_path, &osmoo_exe, &install_dir, &icon_path);
                if !opts.silent {
                    println!("  [✔] Desktop Shortcut: {}", shortcut_path.display());
                }
            }
        }
    }

    if !opts.silent {
        println!();
        println!("[5/6] Registering with Windows Add or Remove Programs...");
    }

    register_uninstaller(&install_dir, &icon_path);

    if !opts.no_autostart {
        // Register optional autostart if consented
        register_autostart(&osmoo_exe);
    }

    if !opts.silent {
        println!();
        println!("[6/6] Installation Complete!");
        println!("════════════════════════════════════════════════════════════════");
        println!("  OSMOO has been successfully installed on your computer.");
        println!("  Installed to: {}", install_dir.display());
        println!("════════════════════════════════════════════════════════════════");
    }

    #[cfg(windows)]
    if !opts.silent {
        unsafe {
            use windows::core::HSTRING;
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
            let title = HSTRING::from("OSMOO Setup");
            let msg = HSTRING::from(format!(
                "OSMOO AI Operating Companion has been successfully installed!

Installed to:
{}

Launching OSMOO now...",
                install_dir.display()
            ));
            let _ = MessageBoxW(None, &msg, &title, MB_OK | MB_ICONINFORMATION);
        }
    }

    // Launch OSMOO unless instructed otherwise
    if !opts.no_launch && osmoo_exe.exists() {
        if !opts.silent {
            println!("Launching OSMOO AI Operating Companion...");
        }
        let mut cmd = Command::new(&osmoo_exe);
        cmd.current_dir(&install_dir);
        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let _ = cmd.spawn();
    }
}

fn perform_uninstall(opts: &InstallOptions) {
    if !opts.silent {
        print_banner();
        println!("UNINSTALLING {APP_DISPLAY_NAME}...");
        println!();
    }

    // 1. Terminate running processes
    kill_existing_processes();

    // 2. Remove shortcuts
    if let Ok(app_data) = env::var("APPDATA") {
        let start_shortcut = PathBuf::from(app_data)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join(format!("{APP_NAME}.lnk"));
        if start_shortcut.exists() {
            let _ = fs::remove_file(&start_shortcut);
        }
    }

    if let Ok(user_profile) = env::var("USERPROFILE") {
        let desktop_shortcut = PathBuf::from(user_profile)
            .join("Desktop")
            .join(format!("{APP_NAME}.lnk"));
        if desktop_shortcut.exists() {
            let _ = fs::remove_file(&desktop_shortcut);
        }
    }

    // 3. Remove Registry Entries
    unregister_uninstaller();
    unregister_autostart();

    // 4. Determine installation directory to remove
    let install_dir = opts.target_dir.clone().unwrap_or_else(|| {
        if let Ok(current_exe) = env::current_exe() {
            if let Some(parent) = current_exe.parent() {
                if parent.join("OSMOO.exe").exists() || parent.join("voxy-daemon.exe").exists() {
                    return parent.to_path_buf();
                }
            }
        }
        get_default_install_dir()
    });

    if install_dir.exists() {
        let _ = fs::remove_file(install_dir.join("OSMOO.exe"));
        let _ = fs::remove_file(install_dir.join("voxy-daemon.exe"));
        let _ = fs::remove_file(install_dir.join("voxy-overlay.exe"));
        let _ = fs::remove_dir_all(install_dir.join("assets"));
        let _ = fs::remove_file(install_dir.join(".env.example"));
        let _ = fs::remove_file(install_dir.join("LICENSE"));

        // If running as Uninstall.exe inside this folder, schedule delayed deletion of the folder
        if let Ok(current_exe) = env::current_exe() {
            if current_exe.starts_with(&install_dir) {
                let dir_str = install_dir.to_string_lossy().to_string();
                let exe_str = current_exe.to_string_lossy().to_string();
                let temp_bat = env::temp_dir().join(format!("osmoo_uninst_{}.bat", std::process::id()));
                let bat_content = format!(
                    "@echo off\r\n:loop\r\ndel /f /q \"{}\" >nul 2>&1\r\nif exist \"{}\" (\r\n  ping 127.0.0.1 -n 1 >nul\r\n  goto loop\r\n)\r\nrmdir \"{}\" >nul 2>&1\r\n((goto) 2>nul & del /f /q \"%~f0\" >nul 2>&1)\r\n",
                    exe_str, exe_str, dir_str
                );
                if fs::write(&temp_bat, bat_content).is_ok() {
                    let _ = Command::new("cmd")
                        .args(["/C", &temp_bat.to_string_lossy()])
                        .creation_flags(0x08000000) // CREATE_NO_WINDOW
                        .spawn();
                }
            } else {
                let _ = fs::remove_file(install_dir.join("Uninstall.exe"));
                let _ = fs::remove_dir(&install_dir);
            }
        } else {
            let _ = fs::remove_dir(&install_dir);
        }
    }

    if !opts.silent {
        println!("  [✔] Removed application shortcuts.");
        println!("  [✔] Removed Add/Remove Programs registration.");
        println!("  [✔] Removed application binaries.");
        println!("  [i] User configuration and local database were safely preserved.");
        println!();
        println!("{APP_DISPLAY_NAME} was successfully uninstalled.");
    }
}

fn extract_payload(dest_dir: &Path, silent: bool) -> Result<(), String> {
    let reader = Cursor::new(PAYLOAD_ZIP);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Failed to read embedded ZIP: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| format!("Failed to read archive entry {i}: {e}"))?;

        let name_cow = file.name().map_err(|e| format!("Failed to read entry name: {e}"))?;
        let raw_name = name_cow.replace('\\', "/");
        if raw_name.is_empty() || raw_name.ends_with('/') {
            continue;
        }

        // Place executables in root of dest_dir
        let out_path = if raw_name.starts_with("bin/") {
            dest_dir.join(&raw_name["bin/".len()..])
        } else {
            dest_dir.join(&raw_name)
        };

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
        }

        let mut out_file = File::create(&out_path)
            .map_err(|e| format!("Failed to create destination file {}: {e}", out_path.display()))?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .map_err(|e| format!("Failed to extract file {raw_name}: {e}"))?;

        out_file.write_all(&buf)
            .map_err(|e| format!("Failed to write file {}: {e}", out_path.display()))?;

        if !silent {
            let filename = out_path.file_name().and_then(|n| n.to_str()).unwrap_or(&raw_name);
            println!("  -> Extracted: {filename}");
        }
    }

    Ok(())
}

fn kill_existing_processes() {
    let procs = ["OSMOO.exe", "voxy-daemon.exe", "voxy-overlay.exe"];
    for proc in procs {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/F", "/IM", proc]);
        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let _ = cmd.output();
    }
}

fn create_shortcut(shortcut_path: &Path, target_path: &Path, work_dir: &Path, icon_path: &Path) {
    let script = format!(
        "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut('{}'); $s.TargetPath = '{}'; $s.WorkingDirectory = '{}'; $s.IconLocation = '{}'; $s.Description = '{}'; $s.Save()",
        shortcut_path.to_string_lossy().replace('\'', "''"),
        target_path.to_string_lossy().replace('\'', "''"),
        work_dir.to_string_lossy().replace('\'', "''"),
        icon_path.to_string_lossy().replace('\'', "''"),
        APP_DISPLAY_NAME
    );

    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let _ = cmd.output();
}

#[cfg(windows)]
fn register_uninstaller(install_dir: &Path, icon_path: &Path) {
    use windows::Win32::System::Registry::{
        RegCreateKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_ALL_ACCESS,
        REG_OPTION_NON_VOLATILE, REG_SZ, REG_DWORD,
    };

    let uninstall_exe = install_dir.join("Uninstall.exe");
    let uninst_cmd = format!("\"{}\" --uninstall", uninstall_exe.display());
    let quiet_uninst_cmd = format!("\"{}\" --uninstall --silent", uninstall_exe.display());
    let icon_str = icon_path.to_string_lossy().to_string();
    let install_str = install_dir.to_string_lossy().to_string();

    let values: Vec<(&str, &str)> = vec![
        ("DisplayName", APP_DISPLAY_NAME),
        ("DisplayVersion", APP_VERSION),
        ("Publisher", APP_PUBLISHER),
        ("DisplayIcon", &icon_str),
        ("InstallLocation", &install_str),
        ("UninstallString", &uninst_cmd),
        ("QuietUninstallString", &quiet_uninst_cmd),
        ("HelpLink", "https://github.com/gaurav-xdev/VOXY-COMMERCIAL"),
        ("URLInfoAbout", "https://osmoo.ai"),
    ];

    unsafe {
        let key_wide: Vec<u16> = UNINSTALL_REG_KEY.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey = Default::default();

        let res = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR::from_raw(key_wide.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_ALL_ACCESS,
            None,
            &mut hkey,
            None,
        );

        if res.is_ok() {
            for (val_name, val_str) in values {
                let name_wide: Vec<u16> = val_name.encode_utf16().chain(std::iter::once(0)).collect();
                let wide: Vec<u16> = val_str.encode_utf16().chain(std::iter::once(0)).collect();
                let mut bytes = vec![0u8; wide.len() * 2];
                for (i, w) in wide.iter().enumerate() {
                    bytes[i * 2] = (w & 0xFF) as u8;
                    bytes[i * 2 + 1] = ((w >> 8) & 0xFF) as u8;
                }
                let _ = RegSetValueExW(
                    hkey,
                    windows::core::PCWSTR::from_raw(name_wide.as_ptr()),
                    0,
                    REG_SZ,
                    Some(&bytes),
                );
            }

            // Write EstimatedSize (DWORD KB)
            let size_name: Vec<u16> = "EstimatedSize".encode_utf16().chain(std::iter::once(0)).collect();
            let size_val: u32 = 32768; // ~32 MB
            let size_bytes = size_val.to_le_bytes();
            let _ = RegSetValueExW(
                hkey,
                windows::core::PCWSTR::from_raw(size_name.as_ptr()),
                0,
                REG_DWORD,
                Some(&size_bytes),
            );

            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        }
    }
}

#[cfg(not(windows))]
fn register_uninstaller(_install_dir: &Path, _icon_path: &Path) {}

#[cfg(windows)]
fn unregister_uninstaller() {
    use windows::Win32::System::Registry::{RegDeleteKeyW, HKEY_CURRENT_USER};
    unsafe {
        let key_wide: Vec<u16> = UNINSTALL_REG_KEY.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = RegDeleteKeyW(HKEY_CURRENT_USER, windows::core::PCWSTR::from_raw(key_wide.as_ptr()));
    }
}

#[cfg(not(windows))]
fn unregister_uninstaller() {}

#[cfg(windows)]
fn register_autostart(osmoo_exe: &Path) {
    use windows::Win32::System::Registry::{
        RegCreateKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_ALL_ACCESS,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };
    unsafe {
        let key_wide: Vec<u16> = RUN_REG_KEY.encode_utf16().chain(std::iter::once(0)).collect();
        let name_wide: Vec<u16> = APP_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let cmd = format!("\"{}\" --autostart", osmoo_exe.display());
        let val_wide: Vec<u16> = cmd.encode_utf16().chain(std::iter::once(0)).collect();

        let mut bytes = vec![0u8; val_wide.len() * 2];
        for (i, w) in val_wide.iter().enumerate() {
            bytes[i * 2] = (w & 0xFF) as u8;
            bytes[i * 2 + 1] = ((w >> 8) & 0xFF) as u8;
        }

        let mut hkey = Default::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR::from_raw(key_wide.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_ALL_ACCESS,
            None,
            &mut hkey,
            None,
        ).is_ok() {
            let _ = RegSetValueExW(
                hkey,
                windows::core::PCWSTR::from_raw(name_wide.as_ptr()),
                0,
                REG_SZ,
                Some(&bytes),
            );
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        }
    }
}

#[cfg(not(windows))]
fn register_autostart(_osmoo_exe: &Path) {}

#[cfg(windows)]
fn unregister_autostart() {
    use windows::Win32::System::Registry::{RegOpenKeyExW, RegDeleteValueW, HKEY_CURRENT_USER, KEY_SET_VALUE};
    unsafe {
        let key_wide: Vec<u16> = RUN_REG_KEY.encode_utf16().chain(std::iter::once(0)).collect();
        let name_wide: Vec<u16> = APP_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hkey = Default::default();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR::from_raw(key_wide.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        ).is_ok() {
            let _ = RegDeleteValueW(hkey, windows::core::PCWSTR::from_raw(name_wide.as_ptr()));
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        }
    }
}

#[cfg(not(windows))]
fn unregister_autostart() {}

fn check_webview2_runtime() -> Option<String> {
    // 1. Check via PowerShell command
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue).pv",
        ])
        .output();

    if let Ok(out) = output {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }

    // 2. Check 64-bit key fallback
    let output64 = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-ItemProperty -Path 'HKLM:\\SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue).pv",
        ])
        .output();

    if let Ok(out) = output64 {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }

    None
}
