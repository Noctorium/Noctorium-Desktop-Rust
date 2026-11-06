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

#[cfg(windows)]
mod entry {
    const KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Noctorium Rust";

    fn reg(arguments: &[&str]) -> Option<String> {
        let mut command = std::process::Command::new("reg");
        command.args(arguments);
        crate::ytdlp::hide_console(&mut command);
        let output = command.output().ok()?;
        output.status.success().then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    }

    pub fn read() -> Option<String> {
        let output = reg(&["query", KEY, "/v", NAME])?;
        output.lines().find(|l| l.trim_start().starts_with(NAME)).and_then(|l| l.split("REG_SZ").nth(1)).map(|v| v.trim().to_string())
    }

    pub fn write(command: &str) -> bool {
        reg(&["add", KEY, "/v", NAME, "/t", "REG_SZ", "/d", command, "/f"]).is_some()
    }

    pub fn remove() -> bool {
        read().is_none() || reg(&["delete", KEY, "/v", NAME, "/f"]).is_some()
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
