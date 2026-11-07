//! Opening Noctorium when the listener signs in to the computer, as the Kotlin desktop's `LaunchAtLogin`
//! does: the per-user Run entry on Windows, an XDG autostart file on Linux, a launch agent on a Mac. In a
//! window, or straight into the tray with `--tray`.

pub const TRAY_ARGUMENT: &str = "--tray";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Window,
    Tray,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Off => "Off",
            Mode::Window => "In a window",
            Mode::Tray => "In the tray",
        }
    }
}

fn program() -> Option<String> {
    std::env::current_exe().ok().map(|p| p.to_string_lossy().into_owned())
}

fn command(tray: bool) -> Option<String> {
    Some(format!("\"{}\"{}", program()?, if tray { format!(" {TRAY_ARGUMENT}") } else { String::new() }))
}

/// The per-user Run entry, read and written through the registry's own functions. It once ran reg.exe for
/// each: a program that starts reg.exe hidden to add itself to the Run key is what malware looks like to an
/// antivirus, and Windows Defender said so.
#[cfg(windows)]
mod entry {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE,
        REG_SZ,
    };

    const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Noctorium Rust";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    /// The Run key, opened for `access`, closed again once `work` is done with it.
    fn with_key<T>(access: u32, work: impl FnOnce(HKEY) -> T) -> Option<T> {
        let mut key: HKEY = std::ptr::null_mut();
        let path = wide(KEY);
        // SAFETY: `path` is a NUL-terminated UTF-16 string that outlives the call, and `key` is written by it.
        if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) } != ERROR_SUCCESS {
            return None;
        }
        let result = work(key);
        // SAFETY: `key` was opened above and is closed once.
        unsafe { RegCloseKey(key) };
        Some(result)
    }

    pub fn read() -> Option<String> {
        with_key(KEY_QUERY_VALUE, |key| {
            let name = wide(NAME);
            let mut kind = 0u32;
            let mut size = 0u32;
            // SAFETY: asks only for the size, with no buffer.
            let asked = unsafe { RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), &mut kind, std::ptr::null_mut(), &mut size) };
            if asked != ERROR_SUCCESS || kind != REG_SZ || size == 0 {
                return None;
            }
            let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
            // SAFETY: `buffer` holds `size` bytes, as the registry asked for.
            let read = unsafe { RegQueryValueExW(key, name.as_ptr(), std::ptr::null(), &mut kind, buffer.as_mut_ptr().cast(), &mut size) };
            if read != ERROR_SUCCESS {
                return None;
            }
            let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            Some(String::from_utf16_lossy(&buffer[..end]))
        })
        .flatten()
    }

    pub fn write(command: &str) -> bool {
        with_key(KEY_SET_VALUE, |key| {
            let (name, value) = (wide(NAME), wide(command));
            // SAFETY: `value` is NUL-terminated and its size in bytes includes the NUL, as REG_SZ wants.
            unsafe { RegSetValueExW(key, name.as_ptr(), 0, REG_SZ, value.as_ptr().cast(), (value.len() * 2) as u32) == ERROR_SUCCESS }
        })
        .unwrap_or(false)
    }

    pub fn remove() -> bool {
        read().is_none()
            || with_key(KEY_SET_VALUE, |key| {
                let name = wide(NAME);
                // SAFETY: `name` is NUL-terminated.
                unsafe { RegDeleteValueW(key, name.as_ptr()) == ERROR_SUCCESS }
            })
            .unwrap_or(false)
    }
}

#[cfg(target_os = "linux")]
mod entry {
    fn file() -> Option<std::path::PathBuf> {
        Some(dirs::config_dir()?.join("autostart").join("noctorium-rust.desktop"))
    }

    pub fn read() -> Option<String> {
        let text = std::fs::read_to_string(file()?).ok()?;
        text.lines().find_map(|l| l.strip_prefix("Exec=")).map(String::from)
    }

    pub fn write(command: &str) -> bool {
        let Some(path) = file() else { return false };
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let text = format!("[Desktop Entry]\nType=Application\nName=Noctorium\nExec={command}\nX-GNOME-Autostart-enabled=true\n");
        std::fs::write(path, text).is_ok()
    }

    pub fn remove() -> bool {
        file().is_none_or(|p| !p.exists() || std::fs::remove_file(p).is_ok())
    }
}

#[cfg(target_os = "macos")]
mod entry {
    fn file() -> Option<std::path::PathBuf> {
        Some(dirs::home_dir()?.join("Library").join("LaunchAgents").join("app.noctorium.rust.plist"))
    }

    pub fn read() -> Option<String> {
        let text = std::fs::read_to_string(file()?).ok()?;
        Some(text)
    }

    pub fn write(command: &str) -> bool {
        let Some(path) = file() else { return false };
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let arguments: String = command
            .split("\" ")
            .map(|p| p.trim_matches('"'))
            .map(|p| format!("<string>{p}</string>"))
            .collect();
        let text = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>app.noctorium.rust</string><key>ProgramArguments</key><array>{arguments}</array><key>RunAtLoad</key><true/></dict></plist>\n"
        );
        std::fs::write(path, text).is_ok()
    }

    pub fn remove() -> bool {
        file().is_none_or(|p| !p.exists() || std::fs::remove_file(p).is_ok())
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod entry {
    pub fn read() -> Option<String> {
        None
    }
    pub fn write(_: &str) -> bool {
        false
    }
    pub fn remove() -> bool {
        false
    }
}

/// How Noctorium opens at sign-in now.
pub fn mode() -> Mode {
    match entry::read() {
        None => Mode::Off,
        Some(command) if command.contains(TRAY_ARGUMENT) => Mode::Tray,
        Some(_) => Mode::Window,
    }
}

/// Changes it. True when it took.
pub fn set(mode: Mode) -> bool {
    let done = match mode {
        Mode::Off => entry::remove(),
        Mode::Window => command(false).is_some_and(|c| entry::write(&c)),
        Mode::Tray => command(true).is_some_and(|c| entry::write(&c)),
    };
    done && self::mode() == mode
}
