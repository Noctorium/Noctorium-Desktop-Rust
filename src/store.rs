//! Where Noctorium keeps things, and the files it keeps them in.
//!
//! Its own folder, beside the Kotlin desktop's rather than inside it: `%LOCALAPPDATA%\Noctorium Rust` on
//! Windows, `~/Library/Application Support/Noctorium Rust` on a Mac, `~/.local/share/noctorium-rust` on
//! Linux. The library files are written in the Kotlin players' own format, and on the first run the
//! playlists, pinned songs and recently played of a Noctorium already on this computer are read in -- read,
//! never written back, so the two can run side by side without either losing the other's edits.

use crate::bangs::SearchMode;
use crate::domain::{decode_tracks, LocalPlaylist, Playlist, ProviderType, Track};
use crate::queue::RepeatMode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const RECENT_LIMIT: usize = 60;

#[derive(Clone, Debug)]
pub struct Places {
    /// This player's folder. None only where the system names no home at all.
    pub root: Option<PathBuf>,
    /// The Kotlin desktop's folder, to borrow its playback tools and read its library from.
    pub neighbour: Option<PathBuf>,
}

impl Places {
    pub fn system() -> Self {
        if let Some(home) = std::env::var_os("NOCTORIUM_RUST_HOME").filter(|v| !v.is_empty()) {
            return Places {
                root: Some(PathBuf::from(home)),
                neighbour: None,
            };
        }
        let (root, neighbour) = if cfg!(target_os = "linux") {
            let share = dirs::data_dir();
            (
                share.as_ref().map(|d| d.join("noctorium-rust")),
                share.map(|d| d.join("noctorium")),
            )
        } else {
            // data_local_dir is %LOCALAPPDATA% on Windows and Application Support on a Mac.
            let base = dirs::data_local_dir();
            (
                base.as_ref().map(|d| d.join("Noctorium Rust")),
                base.map(|d| d.join("Noctorium")),
            )
        };
        Places { root, neighbour }
    }

    pub fn file(&self, name: &str) -> Option<PathBuf> {
        self.root.as_ref().map(|r| r.join(name))
    }

    /// Where the playback tools this player fetched are kept.
    pub fn bin(&self) -> Option<PathBuf> {
        self.file("bin")
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// A [`crate::theme::ThemePreset`] by the name Kotlin saves it under, `NOCTORIUM_NIGHT`.
    pub theme: String,
    pub volume: f32,
    pub muted: bool,
    pub speed: f32,
    pub autoplay: bool,
    pub search_mode: SearchMode,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    /// The browser whose YouTube and SoundCloud sign-ins yt-dlp may read, `firefox`; None for none.
    pub cookies_browser: Option<String>,
    pub lyrics_open: bool,
    /// The source the lyrics open on when it has them; every source is still asked.
    pub lyrics_source: crate::lyrics::Provider,
    /// Whether the queue left at closing is there again on opening.
    pub restore_queue: bool,
    /// Set once the Kotlin desktop's library has been read in, so it is read in once.
    pub imported_neighbour: bool,
    pub equalizer: crate::audio::Equalizer,
    /// The volume booster, a percentage: 100 is off.
    pub boost_percent: u32,
    pub even_loudness: bool,
    /// Skip the parts of YouTube videos SponsorBlock's viewers marked as not the music.
    pub sponsorblock: bool,
    /// Where songs saved to keep go; None for the Desktop.
    pub export_folder: Option<String>,
    pub export_format: crate::downloads::ExportFormat,
    pub discord: crate::discord::PresenceSettings,
    /// The listener's public Deezer profile, by its link or number.
    pub deezer_profile: Option<String>,
    /// The Apple Music storefront the catalogue is read in, `us`, `gb`; the account's own once signed in.
    pub apple_storefront: String,
    /// The listener's own Tidal application's client id. Its secret is kept with the other secrets.
    pub tidal_client_id: String,
    pub shortcuts: crate::shortcuts::Preferences,
    /// The bars under the cover on Now playing, measured from the sound itself.
    pub visualiser: bool,
    /// Closing the window leaves Noctorium in the tray, playing, rather than quitting.
    pub close_to_tray: bool,
    pub widgets: crate::widgets::Preferences,
    /// This computer's id among the listener's devices, made once.
    pub connect_device_id: String,
    /// Everything about the look beyond the theme: accent, typeface, corners, lyrics, Now playing.
    pub looks: crate::looks::Looks,
    /// The name this computer's Noctorium goes by. It stays on this computer.
    pub profile_name: String,
    /// A song goes in the history once half a minute of it has been heard; off adds nothing more.
    pub keep_history: bool,
    /// Whether this computer finds the listener's other devices and shows up on theirs.
    pub connect_enabled: bool,
    /// What the other devices call this computer; blank for its own name.
    pub connect_device_name: String,
    /// Devices added by their address, for networks that keep devices from hearing each other.
    pub connect_addresses: Vec<String>,
    /// A song picked on Home or from a search: a radio from it, or the rest of its shelf after it.
    pub home_play: HomePlay,
    pub autoplay_from: AutoplaySource,
    /// Autoplay leaves out what was played lately.
    pub autoplay_avoid_recent: bool,
    /// Seconds over which the sleep timer lowers the volume before it pauses; zero stops at once.
    pub sleep_fade_seconds: u32,
    /// Songs played here go into the YouTube Music account's history.
    pub youtube_history: bool,
    /// Spotify's songs: matched on YouTube Music and played here, or played by Spotify itself (Premium).
    pub spotify_playback: SpotifyPlayback,
    /// The Spotify device to play on, by its id; blank for whichever is active.
    pub spotify_device: String,
    /// Bandcamp genres, by tag, each a row of its best-sellers on Home.
    pub bandcamp_genres: Vec<String>,
    /// The services a search of all of them asks.
    pub hybrid_search: Vec<ProviderType>,
    /// A note from the tray when a song starts.
    pub announce_tracks: bool,
    pub start_page: StartPage,
    /// One request to GitHub at start, to say when a newer Noctorium is out.
    pub check_updates: bool,
    /// A newer version found is downloaded, checked and put in place by itself, ready at the next start.
    pub auto_update: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HomePlay {
    /// The song, then songs like it, as a radio started from it.
    #[default]
    Autoplay,
    /// The song, then the rest of the shelf or the results it was picked from.
    Shelf,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AutoplaySource {
    /// Radio from the same service as the last song.
    #[default]
    SameService,
    /// YouTube Music's radio after every song, whatever service it came from.
    YoutubeMusic,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpotifyPlayback {
    #[default]
    Matched,
    OnSpotify,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StartPage {
    #[default]
    Home,
    Search,
    Library,
    NowPlaying,
}

impl StartPage {
    pub const ALL: [StartPage; 4] = [StartPage::Home, StartPage::Search, StartPage::Library, StartPage::NowPlaying];

    pub fn label(self) -> &'static str {
        match self {
            StartPage::Home => "Home",
            StartPage::Search => "Search",
            StartPage::Library => "Library",
            StartPage::NowPlaying => "Now playing",
        }
    }
}

/// The services a search of all of them asks, unless the listener chose otherwise.
pub fn default_hybrid_search() -> Vec<ProviderType> {
    vec![
        ProviderType::YoutubeMusic,
        ProviderType::YoutubeVideo,
        ProviderType::Soundcloud,
        ProviderType::Bandcamp,
        ProviderType::Spotify,
        ProviderType::Vk,
    ]
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "NOCTORIUM_NIGHT".into(),
            volume: 0.72,
            muted: false,
            speed: 1.0,
            autoplay: true,
            search_mode: SearchMode::YoutubeMusic,
            repeat: RepeatMode::Off,
            shuffle: false,
            cookies_browser: None,
            lyrics_open: false,
            lyrics_source: crate::lyrics::Provider::default(),
            restore_queue: true,
            imported_neighbour: false,
            equalizer: crate::audio::Equalizer::default(),
            boost_percent: crate::audio::boost::OFF,
            even_loudness: false,
            sponsorblock: true,
            export_folder: None,
            export_format: crate::downloads::ExportFormat::Mp3,
            discord: crate::discord::PresenceSettings::default(),
            deezer_profile: None,
            apple_storefront: "us".into(),
            tidal_client_id: String::new(),
            shortcuts: crate::shortcuts::Preferences::default(),
            visualiser: true,
            close_to_tray: false,
            widgets: crate::widgets::Preferences::default(),
            connect_device_id: String::new(),
            looks: crate::looks::Looks::default(),
            profile_name: "Noctorium Listener".into(),
            keep_history: true,
            connect_enabled: true,
            connect_device_name: String::new(),
            connect_addresses: vec![],
            home_play: HomePlay::default(),
            autoplay_from: AutoplaySource::default(),
            autoplay_avoid_recent: true,
            sleep_fade_seconds: 0,
            youtube_history: true,
            spotify_playback: SpotifyPlayback::default(),
            spotify_device: String::new(),
            bandcamp_genres: vec!["electronic".into(), "hip-hop-rap".into(), "ambient".into()],
            hybrid_search: default_hybrid_search(),
            announce_tracks: false,
            start_page: StartPage::default(),
            check_updates: true,
            auto_update: true,
        }
    }
}

/// The queue as it was left.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedQueue {
    #[serde(deserialize_with = "forgiving")]
    pub tracks: Vec<Track>,
    pub current_index: isize,
    pub position_ms: i64,
}

fn forgiving<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Track>, D::Error> {
    let values: Vec<serde_json::Value> = Vec::deserialize(d)?;
    Ok(values
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect())
}

/// Everything the listener keeps, as it is in memory.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub playlists: Vec<LocalPlaylist>,
    pub liked: Vec<Track>,
    pub recent: Vec<Track>,
    /// Playlists on the services, followed by their links.
    pub followed: Vec<Playlist>,
}

impl Library {
    pub fn is_liked(&self, track: &Track) -> bool {
        let key = track.queue_key();
        self.liked.iter().any(|t| t.queue_key() == key)
    }

    /// Likes it, or unlikes it if it was. Newest first.
    pub fn toggle_like(&mut self, track: &Track) -> bool {
        let key = track.queue_key();
        if let Some(i) = self.liked.iter().position(|t| t.queue_key() == key) {
            self.liked.remove(i);
            false
        } else {
            self.liked.insert(0, track.clone());
            true
        }
    }

    pub fn played(&mut self, track: &Track) {
        let key = track.queue_key();
        self.recent.retain(|t| t.queue_key() != key);
        self.recent.insert(0, track.clone());
        self.recent.truncate(RECENT_LIMIT);
    }

    pub fn playlist_mut(&mut self, id: &str) -> Option<&mut LocalPlaylist> {
        self.playlists.iter_mut().find(|p| p.id == id)
    }
}

pub fn load_settings(places: &Places) -> Settings {
    read_json(places.file("settings.json")).unwrap_or_default()
}

pub fn save_settings(places: &Places, settings: &Settings) {
    write_json(places.file("settings.json"), settings);
}

pub fn load_library(places: &Places) -> Library {
    Library {
        playlists: read_json(places.file("playlists.json")).unwrap_or_default(),
        liked: read_tracks(places.file("liked.json")),
        recent: read_tracks(places.file("recent.json")),
        followed: read_json(places.file("followed.json")).unwrap_or_default(),
    }
}

pub fn save_playlists(places: &Places, library: &Library) {
    write_json(places.file("playlists.json"), &library.playlists);
}

pub fn save_liked(places: &Places, library: &Library) {
    write_json(places.file("liked.json"), &library.liked);
}

pub fn save_recent(places: &Places, library: &Library) {
    write_json(places.file("recent.json"), &library.recent);
}

pub fn save_followed(places: &Places, library: &Library) {
    write_json(places.file("followed.json"), &library.followed);
}

/// What the listener calls a song here, over what its service calls it: uploader titles like "Artist - Song
/// (Official Video) [4K]" put right. Only on this computer; the service is not told.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackEdit {
    pub title: Option<String>,
    pub artist: Option<String>,
    /// The song as its service had it, to go back to.
    pub original: Option<Box<Track>>,
}

impl TrackEdit {
    pub fn is_empty(&self) -> bool {
        self.title.as_deref().is_none_or(|t| t.trim().is_empty()) && self.artist.as_deref().is_none_or(|a| a.trim().is_empty())
    }

    /// The song as the listener has it.
    pub fn apply(&self, track: &Track) -> Track {
        let mut edited = track.clone();
        if let Some(title) = self.title.as_deref().filter(|t| !t.trim().is_empty()) {
            edited.title = title.trim().to_string();
        }
        if let Some(artist) = self.artist.as_deref().filter(|a| !a.trim().is_empty()) {
            edited.artists = vec![crate::domain::Artist {
                id: format!("{}:{}", track.provider.key_name(), artist.trim()),
                name: artist.trim().to_string(),
                provider: track.provider,
            }];
        }
        edited
    }
}

pub fn load_track_edits(places: &Places) -> std::collections::HashMap<String, TrackEdit> {
    read_json(places.file("track_edits.json")).unwrap_or_default()
}

pub fn save_track_edits(places: &Places, edits: &std::collections::HashMap<String, TrackEdit>) {
    write_json(places.file("track_edits.json"), edits);
}

/// One song heard here, and when: the profile's history, by day.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub track: Track,
    /// Seconds since 1970.
    pub played_at: i64,
}

/// How much history is kept: years of listening, without the file growing without end.
pub const HISTORY_LIMIT: usize = 5000;

pub fn load_history(places: &Places) -> Vec<HistoryEntry> {
    read_json(places.file("history.json")).unwrap_or_default()
}

pub fn save_history(places: &Places, history: &[HistoryEntry]) {
    write_json(places.file("history.json"), history);
}

/// The history grouped by the day it was played on, newest day first: "Today", "Yesterday", "Monday 3 November".
pub fn history_by_day(history: &[HistoryEntry], now: i64) -> Vec<(String, Vec<&HistoryEntry>)> {
    use chrono::{Datelike, Local, TimeZone};
    let today = Local.timestamp_opt(now, 0).single().map(|d| d.date_naive());
    let mut days: Vec<(chrono::NaiveDate, Vec<&HistoryEntry>)> = Vec::new();
    for entry in history {
        let Some(day) = Local.timestamp_opt(entry.played_at, 0).single().map(|d| d.date_naive()) else { continue };
        match days.iter_mut().find(|(d, _)| *d == day) {
            Some((_, entries)) => entries.push(entry),
            None => days.push((day, vec![entry])),
        }
    }
    days.sort_by_key(|day| std::cmp::Reverse(day.0));
    days.into_iter()
        .map(|(day, entries)| {
            let label = match today.map(|t| (t - day).num_days()) {
                Some(0) => "Today".to_string(),
                Some(1) => "Yesterday".to_string(),
                _ if Some(day.year()) == today.map(|t| t.year()) => day.format("%A %-d %B").to_string(),
                _ => day.format("%A %-d %B %Y").to_string(),
            };
            (label, entries)
        })
        .collect()
}

pub fn load_queue(places: &Places) -> SavedQueue {
    read_json(places.file("queue.json")).unwrap_or_default()
}

pub fn save_queue(places: &Places, queue: &SavedQueue) {
    write_json(places.file("queue.json"), queue);
}

/// What the Kotlin desktop on this computer has, read in once. Returns how many things were brought over.
pub fn import_neighbour(places: &Places, library: &mut Library) -> usize {
    let Some(neighbour) = places.neighbour.as_ref().filter(|n| n.is_dir()) else {
        return 0;
    };
    let mut brought = 0;
    let theirs: Vec<LocalPlaylist> =
        read_json(Some(neighbour.join("playlists.json"))).unwrap_or_default();
    for playlist in theirs {
        if !library.playlists.iter().any(|p| p.id == playlist.id) {
            library.playlists.push(playlist);
            brought += 1;
        }
    }
    for track in read_tracks(Some(neighbour.join("pinned.json"))) {
        if !library.is_liked(&track) {
            library.liked.push(track);
            brought += 1;
        }
    }
    if library.recent.is_empty() {
        library.recent = read_tracks(Some(neighbour.join("recent.json")));
        library.recent.truncate(RECENT_LIMIT);
        brought += library.recent.len();
    }
    brought
}

fn read_tracks(path: Option<PathBuf>) -> Vec<Track> {
    path.and_then(|p| fs::read_to_string(p).ok())
        .map(|text| decode_tracks(&text))
        .unwrap_or_default()
}

fn read_json<T: DeserializeOwned>(path: Option<PathBuf>) -> Option<T> {
    let text = fs::read_to_string(path?).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

/// Written beside and moved into place, so a crash halfway leaves the old file rather than half a new one.
fn write_json<T: Serialize + ?Sized>(path: Option<PathBuf>, value: &T) {
    let Some(path) = path else { return };
    if let Err(why) = try_write(&path, value) {
        eprintln!("Could not save {}: {why}", path.display());
    }
}

fn try_write<T: Serialize + ?Sized>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, path)
}

pub fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProviderType;

    fn temp_places(name: &str) -> Places {
        let root = std::env::temp_dir().join(format!("noctorium-rust-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        Places {
            root: Some(root.join("mine")),
            neighbour: Some(root.join("theirs")),
        }
    }

    fn track(id: &str) -> Track {
        Track {
            provider: ProviderType::Soundcloud,
            id: id.into(),
            title: id.into(),
            artists: vec![],
            album: None,
            duration_ms: None,
            artwork_url: None,
            source_url: format!("https://soundcloud.com/a/{id}"),
        }
    }

    #[test]
    fn an_edit_renames_only_what_was_changed() {
        let track = Track {
            provider: ProviderType::YoutubeVideo,
            id: "v".into(),
            title: "Band - Song (Official Video) [4K]".into(),
            artists: vec![crate::domain::Artist { id: "c".into(), name: "BandVEVO".into(), provider: ProviderType::YoutubeVideo }],
            album: None,
            duration_ms: None,
            artwork_url: None,
            source_url: "https://www.youtube.com/watch?v=v".into(),
        };
        let edit = TrackEdit { title: Some("Song".into()), artist: None, original: None };
        let edited = edit.apply(&track);
        assert_eq!(edited.title, "Song");
        assert_eq!(edited.artist_line(), "BandVEVO");
        assert_eq!(edited.queue_key(), track.queue_key());
        assert!(TrackEdit { title: Some("  ".into()), artist: None, original: None }.is_empty());
    }

    #[test]
    fn history_is_grouped_by_day_newest_first() {
        use chrono::{Local, TimeZone};
        let track = Track {
            provider: ProviderType::YoutubeMusic,
            id: "a".into(),
            title: "A".into(),
            artists: vec![],
            album: None,
            duration_ms: None,
            artwork_url: None,
            source_url: "https://music.youtube.com/watch?v=a".into(),
        };
        let noon = Local.with_ymd_and_hms(2026, 11, 5, 12, 0, 0).unwrap().timestamp();
        let history = vec![
            HistoryEntry { track: track.clone(), played_at: noon - 86_400 },
            HistoryEntry { track: track.clone(), played_at: noon - 60 },
            HistoryEntry { track: track.clone(), played_at: noon - 120 },
        ];
        let days = history_by_day(&history, noon);
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].0, "Today");
        assert_eq!(days[0].1.len(), 2);
        assert_eq!(days[1].0, "Yesterday");
    }

    #[test]
    fn settings_survive_a_round_trip_and_unknown_fields_are_ignored() {
        let places = temp_places("settings");
        let settings = Settings { theme: "AFTERGLOW".into(), volume: 0.3, ..Settings::default() };
        save_settings(&places, &settings);
        assert_eq!(load_settings(&places), settings);
        fs::write(places.file("settings.json").unwrap(), r#"{"theme":"NORD","fromTheFuture":1}"#).unwrap();
        let read = load_settings(&places);
        assert_eq!(read.theme, "NORD");
        assert!(read.autoplay, "what was not written takes its default");
    }

    #[test]
    fn the_neighbours_library_is_read_in_without_duplicates() {
        let places = temp_places("import");
        let theirs = places.neighbour.clone().unwrap();
        fs::create_dir_all(&theirs).unwrap();
        let playlist = LocalPlaylist {
            tracks: vec![track("1")],
            ..LocalPlaylist::create("Theirs", 1)
        };
        fs::write(theirs.join("playlists.json"), serde_json::to_string(&vec![playlist]).unwrap()).unwrap();
        fs::write(theirs.join("pinned.json"), serde_json::to_string(&vec![track("2")]).unwrap()).unwrap();
        let mut library = Library::default();
        assert_eq!(import_neighbour(&places, &mut library), 2);
        assert_eq!(import_neighbour(&places, &mut library), 0);
        assert_eq!(library.playlists[0].title, "Theirs");
        assert!(library.is_liked(&track("2")));
    }

    #[test]
    fn recently_played_moves_a_replayed_song_to_the_top() {
        let mut library = Library::default();
        library.played(&track("a"));
        library.played(&track("b"));
        library.played(&track("a"));
        let ids: Vec<_> = library.recent.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
    }
}
