use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icons/osmoo.ico");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root_dir = manifest_dir.parent().unwrap().parent().unwrap();

    // 1. Compile Windows Resource (.rc) with official OSMOO icon and VersionInfo
    #[cfg(windows)]
    {
        let ico_path = root_dir.join("assets").join("icons").join("osmoo.ico");
        if ico_path.exists() {
            let ico_escaped = ico_path.to_string_lossy().replace('\\', "\\\\");
            let rc_content = format!(
                r#"1 ICON "{ico_escaped}"
1 VERSIONINFO
FILEVERSION 0,1,0,0
PRODUCTVERSION 0,1,0,0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "OSMOO Technologies\0"
            VALUE "FileDescription", "OSMOO AI Operating Companion Setup\0"
            VALUE "FileVersion", "0.1.0.0\0"
            VALUE "InternalName", "OSMOO-Setup\0"
            VALUE "LegalCopyright", "Copyright (c) 2026 OSMOO Technologies\0"
            VALUE "OriginalFilename", "OSMOO-Setup.exe\0"
            VALUE "ProductName", "OSMOO AI Operating Companion\0"
            VALUE "ProductVersion", "0.1.0.0\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#
            );

            let rc_path = out_dir.join("osmoo.rc");
            let res_path = out_dir.join("osmoo.res");
            std::fs::write(&rc_path, rc_content).expect("Failed to write osmoo.rc");

            // Locate rc.exe in Windows Kits
            let rc_exe = find_rc_exe();
            if let Some(rc_tool) = rc_exe {
                let status = std::process::Command::new(rc_tool)
                    .args(["/fo", res_path.to_str().unwrap(), rc_path.to_str().unwrap()])
                    .status();

                if let Ok(s) = status {
                    if s.success() && res_path.exists() {
                        println!("cargo:rustc-link-arg={}", res_path.display());
                    }
                }
            }
        }
    }

    // 2. Package release payload into payload.zip
    let payload_path = out_dir.join("payload.zip");
    let zip_file = File::create(&payload_path).expect("Failed to create payload.zip");
    let mut zip = ZipWriter::new(zip_file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let release_dir = root_dir.join("target").join("release");

    // Add release executables
    let binaries = [
        ("OSMOO.exe", "bin/OSMOO.exe"),
        ("voxy-daemon.exe", "bin/voxy-daemon.exe"),
        ("voxy-overlay.exe", "bin/voxy-overlay.exe"),
    ];

    let profile = std::env::var("PROFILE").unwrap_or_default();
    for (src_name, dest_name) in binaries {
        let src_path = release_dir.join(src_name);
        if src_path.exists() {
            println!("cargo:rerun-if-changed={}", src_path.display());
            add_file_to_zip(&mut zip, &src_path, dest_name, options);
        } else if profile == "release" {
            panic!(
                "Required release binary {} not found at {}. Build the binaries first: cargo build --release -p voxy-daemon -p voxy-overlay -p voxy-desktop-ui --bin OSMOO",
                src_name,
                src_path.display()
            );
        } else {
            // Placeholder in case of test build without full release bin
            zip.start_file(dest_name, options).unwrap();
            zip.write_all(b"OSMOO_STUB").unwrap();
        }
    }

    // Add assets
    let asset_files = [
        ("assets/icons/osmoo.ico", "assets/icons/osmoo.ico"),
        (
            "assets/icons/osmoo_app_icon.png",
            "assets/icons/osmoo_app_icon.png",
        ),
        (
            "assets/icons/osmoo_mark_black.png",
            "assets/icons/osmoo_mark_black.png",
        ),
        (
            "assets/icons/osmoo_mark_white.png",
            "assets/icons/osmoo_mark_white.png",
        ),
        (
            "crates/desktop_ui/src/assets/osmoo_cat.png",
            "assets/osmoo_cat.png",
        ),
        (
            "crates/desktop_ui/src/assets/osmoo_cat_cropped.png",
            "assets/osmoo_cat_cropped.png",
        ),
        (
            "crates/desktop_ui/src/assets/osmoo_cat_shadow.png",
            "assets/osmoo_cat_shadow.png",
        ),
        (
            "crates/desktop_ui/src/assets/osmoo_cat_transparent.png",
            "assets/osmoo_cat_transparent.png",
        ),
        (
            "crates/desktop_ui/src/assets/osmoo_cat_new_transparent.png",
            "assets/osmoo_cat_new_transparent.png",
        ),
        (
            "crates/desktop_ui/src/assets/rig_body.png",
            "assets/rig_body.png",
        ),
        (
            "crates/desktop_ui/src/assets/rig_head.png",
            "assets/rig_head.png",
        ),
        (
            "crates/desktop_ui/src/assets/rig_tail.png",
            "assets/rig_tail.png",
        ),
        (
            "crates/desktop_ui/src/assets/rig_sunglasses.png",
            "assets/rig_sunglasses.png",
        ),
        (
            "crates/desktop_ui/src/assets/rig_ear_left.png",
            "assets/rig_ear_left.png",
        ),
        (
            "crates/desktop_ui/src/assets/rig_ear_right.png",
            "assets/rig_ear_right.png",
        ),
        (".env.example", ".env.example"),
        ("LICENSE", "LICENSE"),
    ];

    for (src_rel, dest_path) in asset_files {
        let src_path = root_dir.join(src_rel);
        if src_path.exists() {
            add_file_to_zip(&mut zip, &src_path, dest_path, options);
        }
    }

    zip.finish().expect("Failed to finalize payload.zip");
}

fn add_file_to_zip(zip: &mut ZipWriter<File>, src: &Path, dest: &str, options: SimpleFileOptions) {
    let mut file =
        File::open(src).unwrap_or_else(|e| panic!("Failed to open {}: {e}", src.display()));
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).unwrap();
    zip.start_file(dest, options).unwrap();
    zip.write_all(&buffer).unwrap();
}

fn find_rc_exe() -> Option<PathBuf> {
    let candidate =
        PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\rc.exe");
    if candidate.exists() {
        return Some(candidate);
    }

    // Try globbing Windows Kits
    let kits_dir = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    if let Ok(entries) = std::fs::read_dir(kits_dir) {
        for entry in entries.flatten() {
            let rc_cand = entry.path().join("x64").join("rc.exe");
            if rc_cand.exists() {
                return Some(rc_cand);
            }
        }
    }

    None
}
