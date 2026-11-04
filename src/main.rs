// =============================================================================
//  Open-TmuxTabs (Rust) - tmux sessions on remote Linux hosts as Windows
//  Terminal tabs, as a native Windows GUI.
//
//  `core` holds the platform-independent logic (tested with `cargo test` on
//  any host); `ui` is the Win32 user interface and exists only on Windows.
// =============================================================================
#![cfg_attr(windows, windows_subsystem = "windows")]
#![cfg_attr(not(windows), allow(dead_code))]
#![allow(clippy::missing_safety_doc)]

mod core;
#[cfg(windows)]
mod ui;

#[cfg(windows)]
fn main() {
    ui::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Open-TmuxTabs is a Windows application. On this platform only `cargo test` is useful.");
}
