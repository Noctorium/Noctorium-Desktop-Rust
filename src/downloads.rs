//! Music kept on this computer: downloads Noctorium plays without the network, and songs saved as files to
//! keep and take elsewhere -- an MP3 with its title, artist, album and cover, or the original audio. As
//! Noctorium-Base's `DownloadManager`, `MusicExport` and `Id3` have it.

use crate::domain::Track;
use crate::store::Places;
use crate::ytdlp::YtDlp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportFormat {
    Mp3,
    Original,
}

impl ExportFormat {
    pub fn name(self) -> &'static str {
        match self {
            ExportFormat::Mp3 => "MP3",
            ExportFormat::Original => "Original quality",
        }
    }
}

/// A song downloaded for playing offline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Downloaded {
    pub track: Track,
    /// The file's name inside the downloads folder.
    pub file: String,
    pub bytes: u64,
}

/// The downloads folder and its index, `downloads/index.json`.
pub struct Store {
    folder: Option<PathBuf>,
    pub entries: Vec<Downloaded>,
}

impl Store {
    pub fn open(places: &Places) -> Store {
        let folder = places.file("downloads");
        let entries: Vec<Downloaded> = folder
            .as_ref()
            .and_then(|f| std::fs::read_to_string(f.join("index.json")).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut store = Store { folder, entries };
        // A file deleted by hand is a download no longer.
        let folder = store.folder.clone();
        store.entries.retain(|e| folder.as_ref().is_some_and(|f| f.join(&e.file).is_file()));
        store
    }

    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    pub fn file_for(&self, track: &Track) -> Option<PathBuf> {
        let key = track.queue_key();
        let entry = self.entries.iter().find(|e| e.track.queue_key() == key)?;
        let path = self.folder.as_ref()?.join(&entry.file);
        path.is_file().then_some(path)
    }

    pub fn add(&mut self, entry: Downloaded) {
        let key = entry.track.queue_key();
        self.entries.retain(|e| e.track.queue_key() != key);
        self.entries.insert(0, entry);
        self.save();
    }

    pub fn remove(&mut self, track: &Track) {
        let key = track.queue_key();
        if let (Some(folder), Some(entry)) = (&self.folder, self.entries.iter().find(|e| e.track.queue_key() == key)) {
            let _ = std::fs::remove_file(folder.join(&entry.file));
        }
        self.entries.retain(|e| e.track.queue_key() != key);
        self.save();
    }

    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|e| e.bytes).sum()
    }

    fn save(&self) {
        let Some(folder) = &self.folder else { return };
        let _ = std::fs::create_dir_all(folder);
        if let Ok(text) = serde_json::to_string_pretty(&self.entries) {
            let temporary = folder.join("index.json.tmp");
            if std::fs::write(&temporary, text).is_ok() {
                let _ = std::fs::rename(&temporary, folder.join("index.json"));
            }
        }
    }
}

/// Downloads `track` into the downloads folder. Returns what to keep in the index.
pub fn download(backend: &YtDlp, places: &Places, track: &Track, progress: &dyn Fn(f32)) -> Result<Downloaded, String> {
    let folder = places.file("downloads").ok_or("There is no folder to keep downloads in.")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let stem = format!("{}-{}", track.provider.key_name().to_lowercase(), safe_name(&track.id));
    let produced = backend.download_audio(&track.source_url, &folder, &stem, "bestaudio/best", progress)?;
    let bytes = std::fs::metadata(&produced).map(|m| m.len()).unwrap_or(0);
    Ok(Downloaded {
        track: track.clone(),
        file: produced.file_name().unwrap_or_default().to_string_lossy().into_owned(),
        bytes,
    })
}

/// Saves `track` as a file in `folder`, to keep. An MP3 is made by mpv from the original and tagged here.
pub fn export(
    backend: &YtDlp,
    mpv: Option<&Path>,
    track: &Track,
    folder: &Path,
    format: ExportFormat,
    progress: &dyn Fn(f32),
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(folder).map_err(|e| format!("Could not make {}: {e}", folder.display()))?;
    let working = std::env::temp_dir().join("noctorium-rust").join("export");
    std::fs::create_dir_all(&working).map_err(|e| e.to_string())?;
    let stem = format!("{}-{}", std::process::id(), safe_name(&track.id));
    // The original first: m4a where there is a choice, because that plays on every phone.
    let original = backend.download_audio(&track.source_url, &working, &stem, "bestaudio[ext=m4a]/bestaudio", &|p| {
        progress(if format == ExportFormat::Mp3 { p * 0.8 } else { p })
    })?;
    let result = match format {
        ExportFormat::Original => {
            let extension = original.extension().and_then(|e| e.to_str()).unwrap_or("m4a").to_string();
            let name = available_name(folder, &file_name_for(track, &extension));
            let target = folder.join(name);
            move_file(&original, &target).map(|_| target)
        }
        ExportFormat::Mp3 => {
            let mpv = mpv.ok_or("mpv is needed to make an MP3. Settings › Playback tools can install it.")?;
            let target = folder.join(available_name(folder, &file_name_for(track, "mp3")));
            let made = to_mp3(mpv, &original, &target);
            let _ = std::fs::remove_file(&original);
            made?;
            progress(0.95);
            let cover = track.artwork_url.as_deref().and_then(fetch_cover);
            let tags = Tags {
                title: track.title.clone(),
                artist: track.artist_line(),
                album: track.album.as_ref().map(|a| a.title.clone()),
                cover,
            };
            retag(&target, &tags).map_err(|e| format!("The MP3 was made but could not be tagged: {e}"))?;
            Ok(target)
        }
    };
    progress(1.0);
    result
}

fn to_mp3(mpv: &Path, input: &Path, output: &Path) -> Result<(), String> {
    let mut command = std::process::Command::new(mpv);
    command.args([
        "--no-config",
        "--no-video",
        "--really-quiet",
        &format!("--o={}", output.display()),
        "--oac=libmp3lame",
        "--oacopts=b=320k",
        "--",
    ]);
    command.arg(input);
    crate::ytdlp::hide_console(&mut command);
    let status = command.status().map_err(|e| format!("mpv could not be started: {e}"))?;
    if status.success() && output.is_file() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(output);
        Err("mpv could not make the MP3.".into())
    }
}

fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    // A rename cannot cross drives; a copy can.
    std::fs::rename(from, to)
        .or_else(|_| std::fs::copy(from, to).map(|_| ()).and_then(|_| std::fs::remove_file(from)))
        .map_err(|e| format!("Could not save {}: {e}", to.display()))
}

fn fetch_cover(url: &str) -> Option<Vec<u8>> {
    let response = ureq::get(url).timeout(std::time::Duration::from_secs(15)).call().ok()?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(response.into_reader(), 8 << 20), &mut bytes).ok()?;
    // ID3 carries JPEG and PNG; anything else (WebP, often) is turned into a JPEG.
    if picture_mime(&bytes).is_some() {
        return Some(bytes);
    }
    let image = image::load_from_memory(&bytes).ok()?;
    let mut jpeg = Vec::new();
    image.to_rgb8().write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg).ok()?;
    Some(jpeg)
}

/// Where songs are saved to keep: the Desktop, as the Kotlin desktop has it, or home.
pub fn default_export_folder() -> Option<PathBuf> {
    dirs::desktop_dir().filter(|d| d.is_dir()).or_else(dirs::home_dir)
}

/// "Artist - Title.mp3", with what a file name cannot hold taken out.
pub fn file_name_for(track: &Track, extension: &str) -> String {
    let artist = track.artist_line().trim().to_string();
    let title = track.title.trim().to_string();
    let stem = if artist.is_empty() && title.is_empty() {
        "Unknown track".to_string()
    } else if artist.is_empty() {
        title
    } else if title.is_empty() {
        artist
    } else if title.to_lowercase().starts_with(&format!("{} -", artist.to_lowercase())) {
        // A title that already begins with the artist would otherwise read "X - X - Song".
        title
    } else {
        format!("{artist} - {title}")
    };
    format!("{}.{extension}", safe_name(&stem))
}

pub fn safe_name(raw: &str) -> String {
    const FORBIDDEN: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1",
        "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let replaced: String = raw
        .chars()
        .map(|c| if FORBIDDEN.contains(&c) { '-' } else if (c as u32) < 0x20 { ' ' } else { c })
        .collect();
    let spaced = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let dashes = regex::Regex::new(r" *- *(- *)+").unwrap();
    let joined = dashes.replace_all(&spaced, " - ");
    // Windows silently drops a trailing dot or space, so a name ending in one could not be found again.
    let trimmed: String = joined.trim().trim_end_matches(['.', ' ']).chars().take(120).collect();
    let trimmed = trimmed.trim_end_matches(['.', ' ']).to_string();
    if !trimmed.chars().any(char::is_alphanumeric) {
        return "Unknown track".into();
    }
    if RESERVED.contains(&trimmed.to_uppercase().as_str()) {
        format!("{trimmed} track")
    } else {
        trimmed
    }
}

/// `name`, or "name (2)" and so on where the folder already has one.
pub fn available_name(folder: &Path, name: &str) -> String {
    if !folder.join(name).exists() {
        return name.to_string();
    }
    let (stem, extension) = match name.rfind('.') {
        Some(dot) if dot > 0 => (&name[..dot], &name[dot..]),
        _ => (name, ""),
    };
    (2..1000)
        .map(|n| format!("{stem} ({n}){extension}"))
        .find(|candidate| !folder.join(candidate).exists())
        .unwrap_or_else(|| format!("{stem} ({}){extension}", crate::store::now_seconds()))
}

pub struct Tags {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub cover: Option<Vec<u8>>,
}

/// Replaces an MP3's ID3 tag with one saying what it is. Written beside and moved over it.
pub fn retag(file: &Path, tags: &Tags) -> std::io::Result<()> {
    let original = std::fs::read(file)?;
    let audio = &original[existing_tag_length(&original).min(original.len())..];
    let working = file.with_extension("mp3.tagging");
    let mut out = id3_tag(tags);
    out.extend_from_slice(audio);
    std::fs::write(&working, out)?;
    std::fs::rename(&working, file)
}

pub fn id3_tag(tags: &Tags) -> Vec<u8> {
    let mut frames = Vec::new();
    let mut text = |id: &str, value: &str| {
        let value: String = value.trim().chars().take(500).collect();
        if value.is_empty() {
            return;
        }
        let mut data = vec![1u8, 0xFF, 0xFE];
        data.extend(value.encode_utf16().flat_map(|u| u.to_le_bytes()));
        data.extend([0, 0]);
        frames.extend(frame(id, &data));
    };
    text("TIT2", &tags.title);
    text("TPE1", &tags.artist);
    if let Some(album) = &tags.album {
        text("TALB", album);
    }
    if let Some((cover, mime)) = tags.cover.as_ref().and_then(|c| picture_mime(c).map(|m| (c, m))) {
        let mut data = vec![0u8];
        data.extend(mime.as_bytes());
        data.extend([0, 3, 0]);
        data.extend(cover);
        frames.extend(frame("APIC", &data));
    }
    frames.extend(std::iter::repeat_n(0u8, 1024));
    let mut tag = b"ID3\x03\x00\x00".to_vec();
    let size = frames.len() as u32;
    tag.extend([(size >> 21) & 0x7F, (size >> 14) & 0x7F, (size >> 7) & 0x7F, size & 0x7F].map(|b| b as u8));
    tag.extend(frames);
    tag
}

fn frame(id: &str, data: &[u8]) -> Vec<u8> {
    let mut out = id.as_bytes().to_vec();
    out.extend((data.len() as u32).to_be_bytes());
    out.extend([0, 0]);
    out.extend(data);
    out
}

pub fn existing_tag_length(file: &[u8]) -> usize {
    if file.len() < 10 || &file[..3] != b"ID3" || file[6..10].iter().any(|b| b & 0x80 != 0) {
        return 0;
    }
    let size = file[6..10].iter().fold(0usize, |total, b| (total << 7) | (*b as usize & 0x7F));
    let footer = if file[5] & 0x10 != 0 { 10 } else { 0 };
    10 + size + footer
}

fn picture_mime(picture: &[u8]) -> Option<&'static str> {
    if picture.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if picture.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Artist, ProviderType};

    fn track(title: &str, artist: &str) -> Track {
        Track {
            provider: ProviderType::YoutubeMusic,
            id: "x".into(),
            title: title.into(),
            artists: vec![Artist { id: "a".into(), name: artist.into(), provider: ProviderType::YoutubeMusic }],
            album: None,
            duration_ms: None,
            artwork_url: None,
            source_url: "https://music.youtube.com/watch?v=x".into(),
        }
    }

    #[test]
    fn files_are_named_artist_dash_title_and_safely() {
        assert_eq!(file_name_for(&track("Song", "Band"), "mp3"), "Band - Song.mp3");
        assert_eq!(file_name_for(&track("Band - Song", "Band"), "mp3"), "Band - Song.mp3");
        assert_eq!(file_name_for(&track("What? / Why: \"no\"", "A"), "mp3"), "A - What - Why - no-.mp3");
        assert_eq!(safe_name("con"), "con track");
        assert_eq!(safe_name("???"), "Unknown track");
        assert_eq!(safe_name("trailing dots..."), "trailing dots");
    }

    #[test]
    fn a_tag_is_written_over_any_old_one() {
        let tags = Tags { title: "Song".into(), artist: "Band".into(), album: Some("Record".into()), cover: None };
        let tag = id3_tag(&tags);
        assert_eq!(&tag[..3], b"ID3");
        assert_eq!(existing_tag_length(&tag), tag.len());
        let dir = std::env::temp_dir().join(format!("noctorium-rust-id3-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("t.mp3");
        let mut old = id3_tag(&Tags { title: "Old".into(), artist: "Old".into(), album: None, cover: None });
        old.extend(b"AUDIO");
        std::fs::write(&file, &old).unwrap();
        retag(&file, &tags).unwrap();
        let written = std::fs::read(&file).unwrap();
        assert!(written.ends_with(b"AUDIO"));
        assert_eq!(existing_tag_length(&written), written.len() - 5);
    }

    #[test]
    fn a_taken_name_gets_a_number() {
        let dir = std::env::temp_dir().join(format!("noctorium-rust-names-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.mp3"), b"").unwrap();
        assert_eq!(available_name(&dir, "a.mp3"), "a (2).mp3");
        assert_eq!(available_name(&dir, "b.mp3"), "b.mp3");
    }
}
