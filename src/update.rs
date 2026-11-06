//! Whether a newer Noctorium is out: the newest release in Noctorium-Installer, where every Noctorium is
//! published, asked once at start and compared by version. Off with `NOCTORIUM_NO_UPDATE_CHECK` or in
//! Settings › Updates. Nothing is downloaded without asking: the listener is told, and Install fetches the
//! release's terminal installer for this computer and runs it for Noctorium Rust, once this window has closed.

use std::time::Duration;

pub const RELEASES: &str = "https://github.com/Noctorium/Noctorium-Installer/releases/latest";
const LATEST: &str = "https://api.github.com/repos/Noctorium/Noctorium-Installer/releases/latest";

#[derive(Clone, Debug, PartialEq)]
pub struct Newer {
    pub version: String,
    pub url: String,
    /// The installer for this computer among the release's files, by its name and address.
    pub installer: Option<(String, String)>,
}

/// The release's terminal installer for this computer, which installs Noctorium Rust for whoever runs it.
/// None when the release has no Noctorium Rust for this computer, or no installer.
pub fn installer_for(assets: &[(String, String)]) -> Option<(String, String)> {
    let (ours, installer) = if cfg!(windows) {
        ("-windows-x64.zip", "noctorium-installer-cli-windows-x64.exe")
    } else if cfg!(target_os = "macos") {
        (if cfg!(target_arch = "aarch64") { "-macos-arm64.zip" } else { "-macos-x64.zip" }, "noctorium-installer-cli-macos")
    } else {
        ("-linux-x64.tar.gz", "noctorium-installer-cli-linux-x64")
    };
    assets.iter().find(|(name, _)| name.starts_with("noctorium-rust-") && name.ends_with(ours))?;
    assets.iter().find(|(name, _)| name == installer).cloned()
}

/// Downloads the release's terminal installer and starts it for Noctorium Rust, a few seconds from now, so
/// this window has closed by then and the installer can replace the program it is running from.
pub fn install(newer: &Newer) -> Result<(), String> {
    let (name, url) = newer.installer.clone().ok_or("This release has no Noctorium Rust for this computer: see its page.")?;
    let folder = std::env::temp_dir().join(format!("noctorium-update-{}", newer.version));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let path = folder.join(name.replace(['/', '\\'], "_"));
    let reply = ureq::get(&url)
        .set("User-Agent", concat!("Noctorium-Rust/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(600))
        .call()
        .map_err(|e| format!("The download failed: {e}"))?;
    let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    std::io::copy(&mut reply.into_reader(), &mut file).map_err(|e| format!("The download failed: {e}"))?;
    drop(file);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    // A console of its own on Windows, so its progress shows; a shell's sleep elsewhere, then the same.
    #[cfg(windows)]
    let started = {
        use std::os::windows::process::CommandExt;
        // One line for cmd, as it reads them: a window titled for the update, which waits, then installs.
        std::process::Command::new("cmd")
            .raw_arg(format!(
                r#"/c start "Noctorium update" cmd /c "timeout /t 4 /nobreak >nul & "{}" --product rust --yes""#,
                path.display()
            ))
            .spawn()
    };
    #[cfg(not(windows))]
    let started = std::process::Command::new("sh").arg("-c").arg("sleep 4; \"$0\" --product rust --yes").arg(&path).spawn();
    started.map(|_| ()).map_err(|e| format!("The installer would not start: {e}"))
}

/// `v1.2.3`, `1.2`, `1.2.3-beta` as numbers to compare; None for anything else.
pub fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let core = version.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>());
    let major = parts.next()?.ok()?;
    let minor = parts.next().unwrap_or(Ok(0)).ok()?;
    let patch = parts.next().unwrap_or(Ok(0)).ok()?;
    Some((major, minor, patch))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse(candidate), parse(current)), (Some(a), Some(b)) if a > b)
}

/// The newest release if it is newer than this one. Quiet about every failure: no release yet, no network.
pub fn check() -> Option<Newer> {
    if std::env::var_os("NOCTORIUM_NO_UPDATE_CHECK").is_some() {
        return None;
    }
    let body: serde_json::Value = ureq::get(LATEST)
        .set("User-Agent", concat!("Noctorium-Rust/", env!("CARGO_PKG_VERSION")))
        .set("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let tag = body["tag_name"].as_str()?;
    let assets: Vec<(String, String)> = body["assets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|a| Some((a["name"].as_str()?.to_string(), a["browser_download_url"].as_str()?.to_string())))
        .collect();
    is_newer(tag, crate::version()).then(|| Newer {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        url: body["html_url"].as_str().unwrap_or(RELEASES).to_string(),
        installer: installer_for(&assets),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers() {
        assert!(is_newer("v0.10.0", "0.9.9"));
        assert!(is_newer("1.0", "0.99.0"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("nonsense", "0.1.0"));
        assert_eq!(parse("v2.3.4-beta.1"), Some((2, 3, 4)));
    }

    #[test]
    fn the_installer_for_this_computer_is_picked() {
        let named = |names: &[&str]| names.iter().map(|n| (n.to_string(), format!("https://x/{n}"))).collect::<Vec<_>>();
        let release = named(&[
            "noctorium-rust-0.21.0-windows-x64.zip",
            "noctorium-rust-0.21.0-linux-x64.tar.gz",
            "noctorium-rust-0.21.0-macos-arm64.zip",
            "noctorium-rust-0.21.0-macos-x64.zip",
            "noctorium-installer-cli-windows-x64.exe",
            "noctorium-installer-cli-linux-x64",
            "noctorium-installer-cli-macos",
            "Noctorium-0.21.0-windows-x64.msi",
        ]);
        let picked = installer_for(&release).unwrap().0;
        assert!(picked.starts_with("noctorium-installer-cli-"), "{picked}");
        // A release from before Noctorium Rust was published has nothing to update to.
        assert!(installer_for(&named(&["noctorium-installer-cli-windows-x64.exe", "noctorium-installer-cli-linux-x64", "noctorium-installer-cli-macos"])).is_none());
    }
}
