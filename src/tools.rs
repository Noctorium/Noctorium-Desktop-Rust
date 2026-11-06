//! The two programs Noctorium plays music with, yt-dlp and mpv: where they are found, and fetching them.
//!
//! The order is Noctorium-Base's `BackendLocator`'s. An environment variable wins, because somebody who sets
//! one is answering exactly this question. Then this player's own `bin` folder, then the Kotlin desktop's
//! (somebody who already has it should not download a second mpv), then PATH -- and on a Mac, the folders
//! Homebrew and MacPorts install into, since an app opened from Finder starts with a bare PATH.
//!
//! Fetching is only ever done when asked, from the projects' own GitHub releases: yt-dlp's checked against
//! the SHA-256 sums it publishes beside itself, mpv from shinchiro's Windows builds, as the Kotlin desktop
//! does.

use crate::store::Places;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    YtDlp,
    Mpv,
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::YtDlp => "yt-dlp",
            Tool::Mpv => "mpv",
        }
    }

    fn environment(self) -> &'static str {
        match self {
            Tool::YtDlp => "NOCTORIUM_YTDLP_PATH",
            Tool::Mpv => "NOCTORIUM_MPV_PATH",
        }
    }

    pub fn executable(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Tool::YtDlp, true) => "yt-dlp.exe",
            (Tool::YtDlp, false) => "yt-dlp",
            (Tool::Mpv, true) => "mpv.exe",
            (Tool::Mpv, false) => "mpv",
        }
    }
}

pub fn find(tool: Tool, places: &Places) -> Option<PathBuf> {
    if let Some(configured) = std::env::var_os(tool.environment()).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(bin) = places.bin() {
        if tool == Tool::YtDlp {
            // The folder build, which starts in a second where the one-file build first unpacks itself.
            candidates.push(bin.join(YT_DLP_FOLDER).join(tool.executable()));
        }
        candidates.push(bin.join(tool.executable()));
    }
    if let Some(neighbour) = &places.neighbour {
        candidates.push(neighbour.join("bin").join(tool.executable()));
    }
    if cfg!(target_os = "macos") {
        for folder in ["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"] {
            candidates.push(Path::new(folder).join(tool.executable()));
        }
        if tool == Tool::Mpv {
            candidates.push(PathBuf::from("/Applications/mpv.app/Contents/MacOS/mpv"));
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|dir| dir.join(tool.executable())));
    }
    if cfg!(windows) {
        // Where Windows installers and package managers put them, none of which is always on PATH: the
        // mpv installer's own folder, Scoop's shims, Chocolatey's, and winget's links.
        let environment = |name: &str| std::env::var_os(name).map(PathBuf::from);
        let mut folders: Vec<PathBuf> = Vec::new();
        for base in [environment("ProgramFiles"), environment("ProgramFiles(x86)")].into_iter().flatten() {
            folders.extend([base.join("MPV Player"), base.join("mpv"), base.join("yt-dlp")]);
        }
        if let Some(home) = environment("USERPROFILE") {
            folders.push(home.join("scoop").join("shims"));
        }
        if let Some(data) = environment("ProgramData") {
            folders.push(data.join("chocolatey").join("bin"));
        }
        if let Some(local) = environment("LOCALAPPDATA") {
            folders.push(local.join("Microsoft").join("WinGet").join("Links"));
            folders.push(local.join("Programs").join("mpv"));
        }
        candidates.extend(folders.into_iter().map(|f| f.join(tool.executable())));
    }
    candidates.into_iter().find(|c| c.is_file())
}

/// Whether this player can fetch `tool` onto this computer itself, rather than saying how to get it.
pub fn fetches_itself(tool: Tool) -> bool {
    match tool {
        Tool::YtDlp => yt_dlp_asset().is_some(),
        Tool::Mpv => cfg!(all(windows, any(target_arch = "x86_64", target_arch = "aarch64"))),
    }
}

/// What to tell somebody who has to install `tool` themselves.
pub fn how_to_install(tool: Tool) -> &'static str {
    match tool {
        Tool::Mpv if cfg!(target_os = "macos") => "Install it with Homebrew: brew install mpv",
        Tool::Mpv if cfg!(target_os = "linux") => {
            "Install it from your distribution: sudo apt install mpv, sudo dnf install mpv, or sudo pacman -S mpv"
        }
        Tool::YtDlp if cfg!(target_os = "linux") => "Install it from your distribution, or with pipx install yt-dlp",
        _ => "Use the Install button.",
    }
}

/// Where in `bin` the folder build of yt-dlp is unpacked.
pub const YT_DLP_FOLDER: &str = "yt-dlp";

/// yt-dlp's folder build for Windows: a zip of the program and the Python it runs on, unpacked once. The
/// one-file `yt-dlp.exe` unpacks the same seventeen megabytes into the temporary folder every time it is
/// started -- measured here at twenty-five seconds for `--version` -- and leaves them behind if it is ended.
fn yt_dlp_folder_asset() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("yt-dlp_win.zip")
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Some("yt-dlp_win_arm64.zip")
    } else {
        None
    }
}

fn yt_dlp_asset() -> Option<&'static str> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some("yt-dlp.exe")
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Some("yt-dlp_arm64.exe")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("yt-dlp_linux")
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some("yt-dlp_linux_aarch64")
    } else if cfg!(target_os = "macos") {
        Some("yt-dlp_macos")
    } else {
        None
    }
}

const USER_AGENT: &str = concat!("Noctorium-Rust/", env!("CARGO_PKG_VERSION"));

struct Release {
    tag: String,
    assets: Vec<(String, String)>,
}

fn latest_release(repository: &str) -> Result<Release, String> {
    let url = format!("https://api.github.com/repos/{repository}/releases/latest");
    let body: serde_json::Value = ureq::get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| format!("Could not reach GitHub for {repository}: {e}"))?
        .into_json()
        .map_err(|e| format!("GitHub's answer for {repository} could not be read: {e}"))?;
    let tag = body["tag_name"].as_str().unwrap_or("").to_string();
    let assets = body["assets"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some((
                        a["name"].as_str()?.to_string(),
                        a["browser_download_url"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Release { tag, assets })
}

fn download(url: &str, destination: &Path, progress: &dyn Fn(f32)) -> Result<(), String> {
    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| format!("The download failed: {e}"))?;
    let length: Option<u64> = response.header("Content-Length").and_then(|v| v.parse().ok());
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(destination).map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut written: u64 = 0;
    loop {
        let read = reader.read(&mut buffer).map_err(|e| format!("The download stopped: {e}"))?;
        if read == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buffer[..read]).map_err(|e| e.to_string())?;
        written += read as u64;
        if let Some(total) = length.filter(|t| *t > 0) {
            progress(written as f32 / total as f32);
        }
    }
    Ok(())
}

fn sha256_of(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Fetches `tool` into this player's `bin` folder. Returns what to tell the listener.
pub fn install(tool: Tool, places: &Places, progress: &dyn Fn(f32)) -> Result<String, String> {
    let bin = places.bin().ok_or("There is no folder to keep the tools in.")?;
    std::fs::create_dir_all(&bin).map_err(|e| format!("Could not make {}: {e}", bin.display()))?;
    match tool {
        Tool::YtDlp => install_yt_dlp(&bin, progress),
        Tool::Mpv => install_mpv(&bin, progress),
    }
}

fn install_yt_dlp(bin: &Path, progress: &dyn Fn(f32)) -> Result<String, String> {
    let release = latest_release("yt-dlp/yt-dlp")?;
    // The folder build where there is one and the release carries it; the one-file build otherwise.
    let folder_asset = yt_dlp_folder_asset().filter(|name| find_asset(&release, |a| a == *name).is_some());
    let asset = folder_asset.or(yt_dlp_asset()).ok_or("yt-dlp publishes no build for this computer.")?;
    let url = find_asset(&release, |name| name == asset).ok_or("yt-dlp's release has no build for this computer.")?;
    let partial = bin.join(format!("{asset}.partial"));
    download(&url, &partial, progress)?;
    let sums_url = find_asset(&release, |name| name == "SHA2-256SUMS").ok_or("yt-dlp's release has no checksums.")?;
    let sums = ureq::get(&sums_url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("Could not fetch yt-dlp's checksums: {e}"))?
        .into_string()
        .map_err(|e| e.to_string())?;
    let expected = expected_sum(&sums, asset).ok_or("yt-dlp's checksums do not list this build.")?;
    if sha256_of(&partial)? != expected {
        let _ = std::fs::remove_file(&partial);
        return Err("The yt-dlp that arrived does not match its published checksum, so it was thrown away.".into());
    }
    if folder_asset.is_some() {
        let target = bin.join(YT_DLP_FOLDER);
        let unpacking = bin.join(format!("{YT_DLP_FOLDER}.unpacking"));
        let _ = std::fs::remove_dir_all(&unpacking);
        let file = std::fs::File::open(&partial).map_err(|e| e.to_string())?;
        zip::ZipArchive::new(file)
            .and_then(|mut archive| archive.extract(&unpacking))
            .map_err(|e| format!("yt-dlp's zip could not be unpacked: {e}"))?;
        let _ = std::fs::remove_file(&partial);
        // The zip holds the program at its top or in one folder; either way it ends up as bin/yt-dlp/.
        let found = find_file(&unpacking, Tool::YtDlp.executable()).ok_or("yt-dlp's zip held no yt-dlp.exe.")?;
        let root = found.parent().unwrap_or(&unpacking).to_path_buf();
        let _ = std::fs::remove_dir_all(&target);
        std::fs::rename(&root, &target).map_err(|e| format!("Could not put yt-dlp in place: {e}"))?;
        let _ = std::fs::remove_dir_all(&unpacking);
    } else {
        let target = bin.join(Tool::YtDlp.executable());
        std::fs::rename(&partial, &target).map_err(|e| e.to_string())?;
        make_executable(&target);
    }
    Ok(format!("yt-dlp {} is installed.", release.tag))
}

/// The SHA-256 a `SHA2-256SUMS` file gives for `asset`.
fn expected_sum(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (sum, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == asset).then(|| sum.to_lowercase())
    })
}

fn install_mpv(bin: &Path, progress: &dyn Fn(f32)) -> Result<String, String> {
    if !fetches_itself(Tool::Mpv) {
        return Err(how_to_install(Tool::Mpv).into());
    }
    let architecture = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
    let release = latest_release("shinchiro/mpv-winbuild-cmake")?;
    // "-v3" is built for x86-64-v3 and raises an illegal instruction on a processor without AVX2; "mpv-dev"
    // is headers and a library rather than a program. Neither starts with "mpv-<arch>-" alone.
    let prefix = format!("mpv-{architecture}-");
    let v3 = format!("mpv-{architecture}-v3");
    let url = find_asset(&release, |name| {
        let lower = name.to_lowercase();
        lower.ends_with(".7z") && lower.starts_with(&prefix) && !lower.starts_with(&v3)
    })
    .ok_or("shinchiro's release has no mpv for this computer.")?;
    let archive = bin.join("mpv.7z.partial");
    download(&url, &archive, progress)?;
    let unpacked = bin.join("mpv-unpacked");
    let _ = std::fs::remove_dir_all(&unpacked);
    sevenz_rust2::decompress_file(&archive, &unpacked).map_err(|e| format!("The mpv archive could not be unpacked: {e}"))?;
    let _ = std::fs::remove_file(&archive);
    let found = find_file(&unpacked, "mpv.exe").ok_or("The mpv archive held no mpv.exe.")?;
    let folder = found.parent().unwrap_or(&unpacked).to_path_buf();
    for entry in std::fs::read_dir(&folder).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if path.is_file() {
            let _ = std::fs::rename(&path, bin.join(entry.file_name()));
        }
    }
    let _ = std::fs::remove_dir_all(&unpacked);
    Ok(format!("mpv {} is installed.", release.tag))
}

fn find_asset(release: &Release, wanted: impl Fn(&str) -> bool) -> Option<String> {
    release
        .assets
        .iter()
        .find(|(name, _)| wanted(name))
        .map(|(_, url)| url.clone())
}

fn find_file(root: &Path, name: &str) -> Option<PathBuf> {
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if entry.file_name().to_string_lossy().eq_ignore_ascii_case(name) {
            return Some(path);
        }
    }
    None
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn make_executable(_: &Path) {}

/// How old a yt-dlp may be before Noctorium suggests updating it. YouTube changes often enough that a
/// yt-dlp a few months old is more likely than not to be refused.
pub const YT_DLP_STALE_DAYS: i64 = 90;

/// How many days old a yt-dlp is, from its version, which is its release date: `2026.07.04`.
pub fn yt_dlp_age_days(version: &str) -> Option<i64> {
    let date = version.trim().split(['.', '-']).take(3).collect::<Vec<_>>().join("-");
    let released = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok()?;
    Some((chrono::Local::now().date_naive() - released).num_days())
}

/// A tool's version, for Settings: the first line it prints for `--version`.
pub fn version(tool: Tool, path: &Path) -> Option<String> {
    let mut command = std::process::Command::new(path);
    command.arg("--version");
    crate::ytdlp::hide_console(&mut command);
    let output = command.output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let first = text.lines().next()?.trim().to_string();
    Some(match tool {
        Tool::Mpv => first.split_whitespace().take(2).collect::<Vec<_>>().join(" "),
        Tool::YtDlp => first,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_variable_wins() {
        let dir = std::env::temp_dir().join(format!("noctorium-rust-tools-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("my-yt-dlp");
        std::fs::write(&fake, b"").unwrap();
        // SAFETY: tests touching this variable run in this test alone.
        unsafe { std::env::set_var("NOCTORIUM_YTDLP_PATH", &fake) };
        let places = Places { root: None, neighbour: None };
        assert_eq!(find(Tool::YtDlp, &places), Some(fake));
        unsafe { std::env::remove_var("NOCTORIUM_YTDLP_PATH") };
    }

    #[test]
    fn checksums_are_found_by_asset_name() {
        let sums = "aaa  yt-dlp.exe\nbbb  yt-dlp_win.zip\nccc *yt-dlp_linux\n";
        assert_eq!(expected_sum(sums, "yt-dlp_win.zip").as_deref(), Some("bbb"));
        assert_eq!(expected_sum(sums, "yt-dlp_linux").as_deref(), Some("ccc"));
        assert_eq!(expected_sum(sums, "yt-dlp_macos"), None);
    }

    #[test]
    fn a_yt_dlp_version_is_its_release_date() {
        assert!(yt_dlp_age_days("2020.01.01").unwrap() > 365);
        assert!(yt_dlp_age_days("2026.07.04.232921").is_some(), "nightly builds carry a time after the date");
        assert_eq!(yt_dlp_age_days("not a version"), None);
    }

    #[test]
    fn own_folder_comes_before_the_neighbours() {
        let dir = std::env::temp_dir().join(format!("noctorium-rust-order-{}", std::process::id()));
        let (mine, theirs) = (dir.join("mine"), dir.join("theirs"));
        std::fs::create_dir_all(mine.join("bin")).unwrap();
        std::fs::create_dir_all(theirs.join("bin")).unwrap();
        std::fs::write(theirs.join("bin").join(Tool::Mpv.executable()), b"").unwrap();
        let places = Places { root: Some(mine.clone()), neighbour: Some(theirs.clone()) };
        assert_eq!(find(Tool::Mpv, &places), Some(theirs.join("bin").join(Tool::Mpv.executable())));
        std::fs::write(mine.join("bin").join(Tool::Mpv.executable()), b"").unwrap();
        assert_eq!(find(Tool::Mpv, &places), Some(mine.join("bin").join(Tool::Mpv.executable())));
    }
}
