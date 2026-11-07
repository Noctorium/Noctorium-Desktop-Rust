//! Noctorium playlists: the same playlist on every Noctorium the listener uses -- the Kotlin desktop, the phone,
//! this -- kept in storage the listener owns. Base's `NoctoriumPlaylistSync.kt` and `GoogleDrivePlaylists.kt`,
//! with the same files and the same rule, so a playlist made on any of them is on all of them.
//!
//! One file a playlist, `<id>.noctorium-playlist.json`, in a folder called "Noctorium Playlists": inside a folder
//! that Google Drive, OneDrive or Dropbox already syncs, or in the listener's Google Drive itself. For each
//! playlist, whichever copy was changed last is the playlist, whole; a deletion is kept as a file saying so.

use crate::domain::{LocalPlaylist, Track};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const FORMAT: &str = "noctorium-playlist";
pub const VERSION: u32 = 1;
pub const SUFFIX: &str = ".noctorium-playlist.json";
pub const FOLDER: &str = "Noctorium Playlists";
/// Ninety days: long enough for every device to have seen a deletion.
pub const TOMBSTONE_LIFETIME_MS: i64 = 90 * 24 * 60 * 60 * 1000;

/// One Noctorium playlist as it is kept in the listener's storage. The same JSON Base writes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistFile {
    #[serde(default = "format")]
    pub format: String,
    #[serde(default = "version")]
    pub version: u32,
    pub id: String,
    #[serde(default)]
    pub title: String,
    pub updated_at: i64,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub device: String,
    #[serde(default, deserialize_with = "crate::domain::forgiving_tracks")]
    pub tracks: Vec<Track>,
}

fn format() -> String {
    FORMAT.into()
}

fn version() -> u32 {
    VERSION
}

impl PlaylistFile {
    pub fn file_name(&self) -> String {
        file_name_for(&self.id)
    }

    /// The playlist in `text`, or None when it is not one this version can read -- broken, not ours, or from a
    /// newer Noctorium -- which is then left exactly as it is rather than overwritten.
    pub fn decode(text: &str) -> Option<PlaylistFile> {
        serde_json::from_str::<PlaylistFile>(text).ok().filter(|f| f.format == FORMAT && f.version <= VERSION && !f.id.is_empty())
    }

    pub fn encode(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    fn of(playlist: &LocalPlaylist, device: &str) -> PlaylistFile {
        PlaylistFile {
            format: FORMAT.into(),
            version: VERSION,
            id: playlist.id.clone(),
            title: playlist.title.clone(),
            updated_at: playlist.updated_millis(),
            deleted: false,
            device: device.to_string(),
            tracks: playlist.tracks.clone(),
        }
    }

    fn into_local(self) -> LocalPlaylist {
        LocalPlaylist {
            id: self.id,
            title: self.title,
            tracks: self.tracks,
            created_at_epoch_seconds: self.updated_at / 1000,
            updated_at_epoch_seconds: self.updated_at / 1000,
            imported_from: None,
            synced: true,
            updated_at_millis: self.updated_at,
        }
    }
}

pub fn file_name_for(id: &str) -> String {
    format!("{id}{SUFFIX}")
}

/// The playlist's id from a file's name, None for a file that is not one of ours.
pub fn id_of(name: &str) -> Option<&str> {
    name.strip_suffix(SUFFIX).filter(|id| !id.is_empty())
}

/// What a merge leaves: the playlists to keep here, the files to write there, and the deletions to remember.
#[derive(Debug, PartialEq)]
pub struct Merge {
    pub local: Vec<LocalPlaylist>,
    pub uploads: Vec<PlaylistFile>,
    pub tombstones: BTreeMap<String, i64>,
}

/// This device's playlists and the ones in the listener's storage, brought together. Local playlists -- the
/// ones not `synced` -- are never touched and never leave. Pure, as Base's `mergeNoctoriumPlaylists` is.
pub fn merge(local: &[LocalPlaylist], tombstones: &BTreeMap<String, i64>, remote: &[PlaylistFile], device: &str, now_ms: i64) -> Merge {
    let remote_by: HashMap<&str, &PlaylistFile> = remote.iter().map(|f| (f.id.as_str(), f)).collect();
    let local_by: HashMap<&str, &LocalPlaylist> = local.iter().filter(|p| p.synced).map(|p| (p.id.as_str(), p)).collect();
    let mut kept: Vec<LocalPlaylist> = local.to_vec();
    let mut uploads = Vec::new();
    let mut stones = tombstones.clone();
    let mut ids: Vec<String> = local_by.keys().chain(remote_by.keys()).map(|id| id.to_string()).chain(tombstones.keys().cloned()).collect();
    ids.sort();
    ids.dedup();
    for id in ids {
        let mine = local_by.get(id.as_str()).copied();
        let theirs = remote_by.get(id.as_str()).copied();
        let gone = tombstones.get(&id).copied();
        let mine_at = mine.map(|p| p.updated_millis()).or(gone).unwrap_or(i64::MIN);
        let theirs_at = theirs.map(|f| f.updated_at).unwrap_or(i64::MIN);
        if let Some(theirs) = theirs.filter(|_| theirs_at > mine_at) {
            // Theirs is newer: take it, or take its deletion.
            kept.retain(|p| p.id != id);
            if theirs.deleted {
                stones.insert(id.clone(), theirs_at);
            } else {
                stones.remove(&id);
                kept.push(theirs.clone().into_local());
            }
        } else if let Some(mine) = mine.filter(|_| theirs.is_none() || mine_at > theirs_at) {
            uploads.push(PlaylistFile::of(mine, device));
        } else if let (None, Some(gone)) = (mine, gone) {
            if (theirs.is_none() || gone > theirs_at) && !theirs.is_some_and(|f| f.deleted) {
                uploads.push(PlaylistFile {
                    format: FORMAT.into(),
                    version: VERSION,
                    id: id.clone(),
                    title: String::new(),
                    updated_at: gone,
                    deleted: true,
                    device: device.to_string(),
                    tracks: vec![],
                });
            }
        }
    }
    stones.retain(|_, at| now_ms - *at <= TOMBSTONE_LIFETIME_MS);
    Merge { local: kept, uploads, tombstones: stones }
}

/// Where the listener chose to keep their Noctorium playlists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CloudKind {
    /// Nowhere yet: Noctorium playlists are kept here, and go there once somewhere is chosen.
    #[default]
    None,
    /// A folder on this computer that Google Drive, OneDrive, Dropbox or the like syncs.
    Folder,
    /// The listener's Google Drive, signed in to.
    GoogleDrive,
}

/// The listener's choice of where Noctorium playlists are kept. Base's `PlaylistCloudPreferences`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CloudSettings {
    pub kind: CloudKind,
    pub folder: String,
    /// The listener's own Google OAuth client's id. Its secret is kept with the other secrets.
    pub google_client_id: String,
}

/// The names the Google client's secret and the sign-in are kept under, with the other secrets.
pub const GOOGLE_SECRET: &str = "noctorium-playlists.google-client-secret";
pub const GOOGLE_TOKENS: &str = "noctorium-playlists.google-drive-tokens";

/// How syncing stands, for Settings to show.
#[derive(Clone, Debug, Default)]
pub struct SyncStatus {
    pub syncing: bool,
    pub last_synced_ms: Option<i64>,
    pub problem: Option<String>,
    pub signing_in: bool,
}

/// Where the listener keeps their Noctorium playlists.
pub trait Cloud {
    fn list(&mut self) -> Result<Vec<String>, String>;
    fn read(&mut self, name: &str) -> Result<Option<String>, String>;
    fn write(&mut self, name: &str, text: &str) -> Result<(), String>;
}

/// Reads every Noctorium playlist in `cloud`, merges them with this device's, and writes back what changed.
pub fn sync(cloud: &mut dyn Cloud, local: &[LocalPlaylist], tombstones: &BTreeMap<String, i64>, device: &str, now_ms: i64) -> Result<Merge, String> {
    let mut remote = Vec::new();
    for name in cloud.list()? {
        if id_of(&name).is_none() {
            continue;
        }
        if let Some(file) = cloud.read(&name)?.as_deref().and_then(PlaylistFile::decode) {
            remote.push(file);
        }
    }
    let merged = merge(local, tombstones, &remote, device, now_ms);
    for file in &merged.uploads {
        cloud.write(&file.file_name(), &file.encode())?;
    }
    Ok(merged)
}

/// A folder on this computer -- one Google Drive, OneDrive, Dropbox or Syncthing syncs, typically.
pub struct FolderCloud {
    parent: PathBuf,
}

impl FolderCloud {
    pub fn new(parent: impl Into<PathBuf>) -> Self {
        FolderCloud { parent: parent.into() }
    }

    fn folder(&self) -> PathBuf {
        self.parent.join(FOLDER)
    }
}

impl Cloud for FolderCloud {
    fn list(&mut self) -> Result<Vec<String>, String> {
        if !self.parent.is_dir() {
            return Err(format!("The folder {} is not there. Is the drive it is on connected?", self.parent.display()));
        }
        let Ok(entries) = std::fs::read_dir(self.folder()) else { return Ok(vec![]) };
        Ok(entries.flatten().filter_map(|e| e.file_name().into_string().ok()).collect())
    }

    fn read(&mut self, name: &str) -> Result<Option<String>, String> {
        Ok(std::fs::read_to_string(self.folder().join(name)).ok())
    }

    fn write(&mut self, name: &str, text: &str) -> Result<(), String> {
        let folder = self.folder();
        std::fs::create_dir_all(&folder).map_err(|e| format!("Could not make {}: {e}", folder.display()))?;
        let temporary = folder.join(format!(".{name}.tmp"));
        std::fs::write(&temporary, text).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, folder.join(name)).map_err(|e| e.to_string())
    }
}

/// The folders on this computer a cloud's own app keeps in step, where they are found.
pub fn synced_folders() -> Vec<(&'static str, PathBuf)> {
    let home = dirs::home_dir().unwrap_or_default();
    let mut found: Vec<(&'static str, PathBuf)> = Vec::new();
    for letter in 'D'..='Z' {
        found.push(("Google Drive", PathBuf::from(format!("{letter}:\\My Drive"))));
    }
    if let Ok(entries) = std::fs::read_dir(home.join("Library/CloudStorage")) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with("GoogleDrive") {
                found.push(("Google Drive", entry.path().join("My Drive")));
            }
        }
    }
    if let Ok(one) = std::env::var("OneDrive") {
        found.push(("OneDrive", PathBuf::from(one)));
    }
    found.push(("OneDrive", home.join("OneDrive")));
    found.push(("Dropbox", home.join("Dropbox")));
    found.push(("iCloud Drive", home.join("Library/Mobile Documents/com~apple~CloudDocs")));
    let mut seen = HashSet::new();
    found.retain(|(_, path)| path.is_dir() && seen.insert(path.clone()));
    found
}

// ------------------------------------------------------------------------------------------- Google Drive

/// Only the files Noctorium makes: never the rest of the listener's Drive.
pub const GOOGLE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";
/// Base's, so one OAuth client serves every Noctorium. A "Desktop app" client takes any loopback port.
pub const GOOGLE_PORT: u16 = 8890;
pub const GOOGLE_PATH: &str = "/noctorium/google";

pub fn google_redirect() -> String {
    format!("http://127.0.0.1:{GOOGLE_PORT}{GOOGLE_PATH}")
}

/// A sign-in to Google Drive, kept with the other secrets. The same JSON Base keeps.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_epoch_seconds: i64,
}

/// Takes the listener through Google's own sign-in page, with PKCE and a loopback reply, and comes back with tokens.
pub fn google_sign_in(client_id: &str, client_secret: &str, open: impl Fn(&str)) -> Result<GoogleTokens, String> {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        return Err("Google Drive needs an OAuth client of your own: paste its client id and secret first.".into());
    }
    let (verifier, challenge) = crate::oauth::pkce();
    let state = crate::oauth::state();
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge_method=S256&code_challenge={challenge}&state={state}&access_type=offline&prompt=consent",
        encode(client_id),
        encode(&google_redirect()),
        encode(GOOGLE_SCOPE),
    );
    let code = crate::oauth::wait_for_code("Google Drive", &url, GOOGLE_PORT, GOOGLE_PATH, &state, open)?;
    token(&[
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", &code),
        ("redirect_uri", &google_redirect()),
        ("code_verifier", &verifier),
    ], None)
}

/// A fresh access token from the kept refresh token.
pub fn google_refresh(client_id: &str, client_secret: &str, refresh: &str) -> Result<GoogleTokens, String> {
    token(&[("grant_type", "refresh_token"), ("client_id", client_id), ("client_secret", client_secret), ("refresh_token", refresh)], Some(refresh))
}

fn token(form: &[(&str, &str)], kept_refresh: Option<&str>) -> Result<GoogleTokens, String> {
    let reply = ureq::post("https://oauth2.googleapis.com/token").timeout(Duration::from_secs(20)).send_form(form);
    let body: serde_json::Value = match reply {
        Ok(response) => response.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(_, response)) => {
            let body: serde_json::Value = response.into_json().unwrap_or_default();
            return Err(match body["error"].as_str().unwrap_or("") {
                "invalid_client" | "unauthorized_client" => "Google does not recognise that client. Check its id and secret in Google Cloud Console.".into(),
                "invalid_grant" => "Google no longer accepts that sign-in. Sign in to Google Drive again.".into(),
                _ => body["error_description"].as_str().unwrap_or("Google refused the sign-in.").to_string(),
            });
        }
        Err(e) => return Err(format!("Could not reach Google: {e}")),
    };
    let access = body["access_token"].as_str().ok_or("Google replied without an access token.")?;
    Ok(GoogleTokens {
        access_token: access.to_string(),
        refresh_token: body["refresh_token"].as_str().map(String::from).or(kept_refresh.map(String::from)),
        expires_at_epoch_seconds: crate::store::now_seconds() + body["expires_in"].as_i64().unwrap_or(3600) - 60,
    })
}

fn encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>().replace('+', "%20")
}

/// Noctorium playlists in a "Noctorium Playlists" folder in the listener's Google Drive, through Drive's API.
pub struct DriveCloud {
    token: String,
    api: String,
    folder: Option<String>,
    ids: HashMap<String, String>,
}

impl DriveCloud {
    pub fn new(token: String) -> Self {
        Self::with_api(token, "https://www.googleapis.com")
    }

    pub fn with_api(token: String, api: &str) -> Self {
        DriveCloud { token, api: api.trim_end_matches('/').to_string(), folder: None, ids: HashMap::new() }
    }

    fn get(&self, url: &str) -> Result<ureq::Response, String> {
        ureq::get(url).set("Authorization", &format!("Bearer {}", self.token)).timeout(Duration::from_secs(20)).call().map_err(problem)
    }

    /// The folder, made the first time. Noctorium can see only what it made, so any of that name is ours.
    fn folder(&mut self) -> Result<String, String> {
        if let Some(id) = &self.folder {
            return Ok(id.clone());
        }
        let query = format!("name = '{FOLDER}' and mimeType = 'application/vnd.google-apps.folder' and trashed = false");
        let found: serde_json::Value = self.get(&format!("{}/drive/v3/files?q={}&fields=files(id)&spaces=drive", self.api, encode(&query)))?.into_json().map_err(|e| e.to_string())?;
        let id = match found["files"][0]["id"].as_str() {
            Some(id) => id.to_string(),
            None => {
                let made: serde_json::Value = ureq::post(&format!("{}/drive/v3/files?fields=id", self.api))
                    .set("Authorization", &format!("Bearer {}", self.token))
                    .timeout(Duration::from_secs(20))
                    .send_json(serde_json::json!({ "name": FOLDER, "mimeType": "application/vnd.google-apps.folder" }))
                    .map_err(problem)?
                    .into_json()
                    .map_err(|e| e.to_string())?;
                made["id"].as_str().ok_or("Google Drive made the folder but did not say where.")?.to_string()
            }
        };
        self.folder = Some(id.clone());
        Ok(id)
    }
}

impl Cloud for DriveCloud {
    fn list(&mut self) -> Result<Vec<String>, String> {
        let parent = self.folder()?;
        self.ids.clear();
        let mut page: Option<String> = None;
        loop {
            let query = format!("'{parent}' in parents and trashed = false");
            let mut url = format!("{}/drive/v3/files?q={}&fields=nextPageToken,files(id,name)&pageSize=1000&spaces=drive", self.api, encode(&query));
            if let Some(token) = &page {
                url.push_str(&format!("&pageToken={}", encode(token)));
            }
            let root: serde_json::Value = self.get(&url)?.into_json().map_err(|e| e.to_string())?;
            for file in root["files"].as_array().into_iter().flatten() {
                if let (Some(name), Some(id)) = (file["name"].as_str(), file["id"].as_str()) {
                    self.ids.insert(name.to_string(), id.to_string());
                }
            }
            page = root["nextPageToken"].as_str().map(String::from);
            if page.is_none() {
                break;
            }
        }
        Ok(self.ids.keys().cloned().collect())
    }

    fn read(&mut self, name: &str) -> Result<Option<String>, String> {
        if self.ids.is_empty() {
            self.list()?;
        }
        let Some(id) = self.ids.get(name).cloned() else { return Ok(None) };
        match self.get(&format!("{}/drive/v3/files/{id}?alt=media", self.api)) {
            Ok(response) => response.into_string().map(Some).map_err(|e| e.to_string()),
            Err(why) if why.contains("404") => Ok(None),
            Err(why) => Err(why),
        }
    }

    fn write(&mut self, name: &str, text: &str) -> Result<(), String> {
        let auth = format!("Bearer {}", self.token);
        if let Some(id) = self.ids.get(name) {
            ureq::request("PATCH", &format!("{}/upload/drive/v3/files/{id}?uploadType=media", self.api))
                .set("Authorization", &auth)
                .set("Content-Type", "application/json; charset=utf-8")
                .timeout(Duration::from_secs(20))
                .send_string(text)
                .map_err(problem)?;
            return Ok(());
        }
        let parent = self.folder()?;
        let boundary = format!("noctorium-{}", crate::domain::new_id());
        let metadata = serde_json::json!({ "name": name, "parents": [parent] });
        let body = format!(
            "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{text}\r\n--{boundary}--\r\n"
        );
        let made: serde_json::Value = ureq::post(&format!("{}/upload/drive/v3/files?uploadType=multipart&fields=id", self.api))
            .set("Authorization", &auth)
            .set("Content-Type", &format!("multipart/related; boundary={boundary}"))
            .timeout(Duration::from_secs(20))
            .send_string(&body)
            .map_err(problem)?
            .into_json()
            .map_err(|e| e.to_string())?;
        if let Some(id) = made["id"].as_str() {
            self.ids.insert(name.to_string(), id.to_string());
        }
        Ok(())
    }
}

fn problem(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(401, _) => "Google Drive no longer accepts the sign-in. Sign in again.".into(),
        ureq::Error::Status(code, _) => format!("Google Drive answered HTTP {code}."),
        ureq::Error::Transport(e) => format!("Could not reach Google Drive: {e}"),
    }
}

/// The deletions this device remembers, between launches.
pub fn load_tombstones(path: Option<&Path>) -> BTreeMap<String, i64> {
    path.and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_tombstones(path: Option<&Path>, tombstones: &BTreeMap<String, i64>) {
    if let (Some(path), Ok(text)) = (path, serde_json::to_string(tombstones)) {
        let _ = std::fs::write(path, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProviderType;

    fn song(id: &str) -> Track {
        Track {
            provider: ProviderType::YoutubeMusic,
            id: id.into(),
            title: format!("Song {id}"),
            artists: vec![],
            album: None,
            duration_ms: None,
            artwork_url: None,
            source_url: format!("https://music.youtube.com/watch?v={id}"),
        }
    }

    fn playlist(id: &str, at: i64, songs: &[&str]) -> LocalPlaylist {
        LocalPlaylist {
            id: id.into(),
            title: format!("List {id}"),
            tracks: songs.iter().map(|s| song(s)).collect(),
            created_at_epoch_seconds: 0,
            updated_at_epoch_seconds: 0,
            imported_from: None,
            synced: true,
            updated_at_millis: at,
        }
    }

    fn file(id: &str, at: i64, songs: &[&str], deleted: bool) -> PlaylistFile {
        PlaylistFile {
            format: FORMAT.into(),
            version: VERSION,
            id: id.into(),
            title: format!("List {id}"),
            updated_at: at,
            deleted,
            device: String::new(),
            tracks: songs.iter().map(|s| song(s)).collect(),
        }
    }

    const NOW: i64 = 10_000_000;

    #[test]
    fn a_playlist_made_here_is_written_there_and_one_from_elsewhere_comes_here() {
        let out = merge(&[playlist("p", 100, &["a"])], &BTreeMap::new(), &[], "Rust", NOW);
        assert_eq!(out.uploads.len(), 1);
        let into = merge(&[], &BTreeMap::new(), &[file("p", 100, &["a", "b"], false)], "Rust", NOW);
        assert!(into.local[0].synced);
        assert_eq!(into.local[0].tracks.len(), 2);
    }

    #[test]
    fn the_newer_copy_wins_whole() {
        let theirs = merge(&[playlist("p", 100, &["a"])], &BTreeMap::new(), &[file("p", 200, &["a", "b"], false)], "Rust", NOW);
        assert_eq!(theirs.local[0].tracks.len(), 2);
        assert!(theirs.uploads.is_empty());
        let mine = merge(&[playlist("p", 300, &["c"])], &BTreeMap::new(), &[file("p", 200, &["a"], false)], "Rust", NOW);
        assert_eq!(mine.uploads[0].tracks[0].id, "c");
    }

    #[test]
    fn deletions_travel_both_ways_and_an_edit_after_one_wins() {
        let stones: BTreeMap<String, i64> = [("p".to_string(), 500)].into();
        let out = merge(&[], &stones, &[file("p", 100, &["a"], false)], "Rust", NOW);
        assert!(out.uploads[0].deleted && out.local.is_empty());
        let there = merge(&[playlist("p", 100, &["a"])], &BTreeMap::new(), &[file("p", 500, &[], true)], "Rust", NOW);
        assert!(there.local.is_empty());
        let edited = merge(&[playlist("p", 900, &["a"])], &BTreeMap::new(), &[file("p", 500, &[], true)], "Rust", NOW);
        assert_eq!(edited.local.len(), 1);
        assert!(!edited.uploads[0].deleted);
    }

    #[test]
    fn local_playlists_never_leave() {
        let mut mine = playlist("p", 100, &["a"]);
        mine.synced = false;
        assert!(merge(&[mine], &BTreeMap::new(), &[], "Rust", NOW).uploads.is_empty());
    }

    #[test]
    fn a_file_from_base_reads_here_and_a_newer_format_is_left_alone() {
        let base = r#"{"format":"noctorium-playlist","version":1,"id":"p","title":"Road trip","updatedAt":1794000000000,"deleted":false,"device":"Phone","tracks":[{"provider":"YOUTUBE_MUSIC","id":"a","title":"Song a","sourceUrl":"https://music.youtube.com/watch?v=a"}]}"#;
        let read = PlaylistFile::decode(base).unwrap();
        assert_eq!(read.title, "Road trip");
        assert_eq!(read.tracks.len(), 1);
        assert!(PlaylistFile::decode(r#"{"format":"noctorium-playlist","version":99,"id":"p","updatedAt":1}"#).is_none());
        assert!(id_of("notes.txt").is_none());
    }

    #[test]
    fn two_devices_sharing_a_folder_end_up_with_the_same_playlists() {
        let folder = std::env::temp_dir().join(format!("noctorium-rust-sync-{}", crate::domain::new_id()));
        std::fs::create_dir_all(&folder).unwrap();
        let mut cloud = FolderCloud::new(&folder);
        let desktop = sync(&mut cloud, &[playlist("road", 100, &["a"])], &BTreeMap::new(), "Desktop", NOW).unwrap();
        let phone = sync(&mut cloud, &[playlist("gym", 200, &["c"])], &BTreeMap::new(), "Phone", NOW).unwrap();
        assert_eq!(phone.local.len(), 2);
        let again = sync(&mut cloud, &desktop.local, &desktop.tombstones, "Desktop", NOW).unwrap();
        assert_eq!(again.local.len(), 2);
        assert!(again.uploads.is_empty());
        assert!(folder.join(FOLDER).join(file_name_for("gym")).is_file());
        let _ = std::fs::remove_dir_all(folder);
    }
}

#[cfg(test)]
mod written {
    /// Prints a playlist file exactly as this port writes it, for checking that Base reads it.
    #[test]
    #[ignore]
    fn print_a_file_as_written_here() {
        let mut playlist = crate::domain::LocalPlaylist::create("From Rust", 1);
        playlist.synced = true;
        playlist.tracks.push(crate::domain::Track {
            provider: crate::domain::ProviderType::Soundcloud,
            id: "s1".into(),
            title: "A SoundCloud song".into(),
            artists: vec![crate::domain::Artist { id: "a".into(), name: "Band".into(), provider: crate::domain::ProviderType::Soundcloud }],
            album: None,
            duration_ms: Some(200_000),
            artwork_url: None,
            source_url: "https://soundcloud.com/band/s1".into(),
        });
        println!("{}", super::PlaylistFile::of(&playlist, "Rust").encode());
    }
}
