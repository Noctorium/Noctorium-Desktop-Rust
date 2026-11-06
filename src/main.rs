//! Noctorium: the window, and the few words it says on the command line.
//!
//! A window program on Windows, so that opening it from the Start menu does not flash a console up first.
//! Debug builds keep the console, which is where anything worth reading while working on it goes.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;

const USAGE: &str = "\
Noctorium Rust

Noctorium for the desktop, written in Rust: YouTube Music, SoundCloud and more, on yt-dlp and mpv.

  --version   the version
  --tray      start in the tray, the window minimised
  --help      this

  NOCTORIUM_RUST_HOME    keep settings and the library in this folder
  NOCTORIUM_YTDLP_PATH   the yt-dlp to use
  NOCTORIUM_MPV_PATH     the mpv to use
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let said = |flags: &[&str]| arguments.iter().any(|a| flags.contains(&a.as_str()));
    // Printed to whatever standard output is: on Windows that is nothing from a double click, and a pipe or a
    // file when run as `"Noctorium Rust.exe" --version | more`, which is what the packaging script does.
    if said(&["--version", "-V"]) {
        println!("Noctorium Rust {}", noctorium::version());
        return ExitCode::SUCCESS;
    }
    let in_tray = said(&[noctorium::startup::TRAY_ARGUMENT]);
    if said(&["--help", "-h"]) {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let listener = match noctorium::instance::claim() {
        // Noctorium is already open, and has been asked to come to the front.
        noctorium::instance::Instance::Second => return ExitCode::SUCCESS,
        noctorium::instance::Instance::First(listener) => listener,
    };
    noctorium::platform::tie_children_to_this_process();
    match noctorium::gui::run(listener, in_tray) {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) => {
            complain(&format!("Noctorium could not open its window.\n\n{why}"));
            ExitCode::FAILURE
        }
    }
}

/// Says why there is no window, somewhere it will be seen: a window program on Windows has nowhere to print.
#[cfg(windows)]
fn complain(text: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (text, title) = (wide(text), wide("Noctorium"));
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }
}

#[cfg(not(windows))]
fn complain(text: &str) {
    eprintln!("{text}");
}
