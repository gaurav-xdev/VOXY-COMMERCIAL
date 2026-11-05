#![cfg_attr(not(test), windows_subsystem = "windows")]

//! VOXY / OSMOO Desktop Companion Overlay Executable.
//!
//! Provides the transparent 2D animated Cat companion on the Windows desktop,
//! communicating with the background daemon over named pipe IPC.

fn main() {
    voxy_desktop_ui::overlay::run_overlay();
}
