//! What Noctorium is about: songs, the people who made them, and the services they come from.
//!
//! The same model as Noctorium-Base's `app.noctorium.domain`, and written to JSON the same way -- enum names
//! in capitals, fields in camelCase, absent values left out -- so a playlist saved by the Kotlin players
//! reads here, and one saved here reads there.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderType {
    YoutubeMusic,
    YoutubeVideo,
    Soundcloud,
    /// A library to read, never a source to play from: its songs are matched on YouTube Music when played.
    Spotify,
    Bandcamp,
    Vk,
    AppleMusic,
    Deezer,
    Tidal,
    Local,
    /// Internet radio: a station from anywhere in the world, from Radio Browser's open list of them -- see
    /// [`crate::radio`]. Its stream plays as it is, with no end, nothing to like and nothing to download.
    Radio,
}

impl ProviderType {
    pub fn display_name(self) -> &'static str {
        match self {
            ProviderType::YoutubeMusic => "YouTube Music",
            ProviderType::YoutubeVideo => "YouTube",
            ProviderType::Soundcloud => "SoundCloud",
            ProviderType::Spotify => "Spotify",
            ProviderType::Bandcamp => "Bandcamp",
            ProviderType::Vk => "VK Music",
            ProviderType::AppleMusic => "Apple Music",
            ProviderType::Deezer => "Deezer",
            ProviderType::Tidal => "Tidal",
            ProviderType::Local => "Local",
            ProviderType::Radio => "Radio",
        }
    }

    /// The name as Kotlin's enum has it, which is how queue keys are written.
    pub fn key_name(self) -> &'static str {
        match self {
            ProviderType::YoutubeMusic => "YOUTUBE_MUSIC",
            ProviderType::YoutubeVideo => "YOUTUBE_VIDEO",
            ProviderType::Soundcloud => "SOUNDCLOUD",
            ProviderType::Spotify => "SPOTIFY",
            ProviderType::Bandcamp => "BANDCAMP",
            ProviderType::Vk => "VK",
            ProviderType::AppleMusic => "APPLE_MUSIC",
            ProviderType::Deezer => "DEEZER",
            ProviderType::Tidal => "TIDAL",
            ProviderType::Local => "LOCAL",
            ProviderType::Radio => "RADIO",
        }
    }

    /// Two letters for the badge beside a song, where there is no room for the name.
    pub fn badge(self) -> &'static str {
        match self {
            ProviderType::YoutubeMusic => "YTM",
            ProviderType::YoutubeVideo => "YT",
            ProviderType::Soundcloud => "SC",
            ProviderType::Spotify => "SP",
            ProviderType::Bandcamp => "BC",
            ProviderType::Vk => "VK",
            ProviderType::AppleMusic => "AM",
            ProviderType::Deezer => "DZ",
            ProviderType::Tidal => "TD",
            ProviderType::Local => "PC",
            ProviderType::Radio => "FM",
        }
    }

    /// The service's own colour, for its badge.
    pub fn colour(self) -> [u8; 3] {
        match self {
            ProviderType::YoutubeMusic | ProviderType::YoutubeVideo => [0xFF, 0x00, 0x33],
            ProviderType::Soundcloud => [0xFF, 0x55, 0x00],
            ProviderType::Spotify => [0x1D, 0xB9, 0x54],
            ProviderType::Bandcamp => [0x40, 0x8E, 0xA3],
            ProviderType::Vk => [0x00, 0x77, 0xFF],
            ProviderType::AppleMusic => [0xFA, 0x24, 0x3C],
            ProviderType::Deezer => [0xA2, 0x38, 0xFF],
            ProviderType::Tidal => [0xCC, 0xCC, 0xCC],
            ProviderType::Local => [0x9E, 0x9E, 0x9E],
            ProviderType::Radio => [0x0F, 0x8A, 0x7E],
        }
    }

    /// A library to read rather than a source to play from: these services let only their own players make
    /// the sound, so their songs are matched on YouTube Music when played. See [`crate::matching`].
    pub fn is_reference(self) -> bool {
        matches!(self, ProviderType::Spotify | ProviderType::AppleMusic | ProviderType::Deezer | ProviderType::Tidal)
    }

    /// Whether this player can play the service's songs: itself through yt-dlp, or by matching them.
    pub fn plays_here(self) -> bool {
        self.playable_here() || self.is_reference() || matches!(self, ProviderType::Bandcamp | ProviderType::Vk | ProviderType::Radio)
    }

    /// Whether a song from the service can be downloaded or saved: its audio is read by yt-dlp.
    pub fn downloadable(self) -> bool {
        self.playable_here() || self == ProviderType::Bandcamp
    }

    /// Whether this is a radio station: a stream without an end, played by mpv as it is.
    pub fn is_radio(self) -> bool {
        self == ProviderType::Radio
    }

    /// Whether this player can play the service's songs itself: through yt-dlp, which reads these three.
    pub fn playable_here(self) -> bool {
        matches!(
            self,
            ProviderType::YoutubeMusic | ProviderType::YoutubeVideo | ProviderType::Soundcloud
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<Artist>,
    pub provider: ProviderType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub provider: ProviderType,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<Artist>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<Album>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
    pub source_url: String,
}

impl Track {
    pub fn artist_line(&self) -> String {
        self.artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The key a song is known by in the queue and the library, the same string Kotlin writes.
    pub fn queue_key(&self) -> String {
        format!("{}:{}", self.provider.key_name(), self.id)
    }
}

/// A playlist read from a service, or one followed by its link.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub title: String,
    pub provider: ProviderType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artwork_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track_count: Option<i64>,
}

/// A playlist the listener assembled inside Noctorium, free to mix every service. `playlists.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlaylist {
    pub id: String,
    pub title: String,
    #[serde(default, deserialize_with = "forgiving_tracks")]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub created_at_epoch_seconds: i64,
    #[serde(default)]
    pub updated_at_epoch_seconds: i64,
    /// Where an import made it from, kept as it was read so a Kotlin player's import finds it again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_from: Option<serde_json::Value>,
    /// A Noctorium playlist: on every Noctorium the listener uses, kept in their own storage. See
    /// [`crate::noctplaylists`]. False for a local one, which stays here. Kotlin's `synced`.
    #[serde(default)]
    pub synced: bool,
    /// When it was last changed, in milliseconds, for telling which of two copies is newer. Kotlin's `updatedAtMillis`.
    #[serde(default)]
    pub updated_at_millis: i64,
}

impl LocalPlaylist {
    pub fn create(title: &str, now: i64) -> Self {
        LocalPlaylist {
            id: new_id(),
            title: title.trim().to_string(),
            tracks: Vec::new(),
            created_at_epoch_seconds: now,
            updated_at_epoch_seconds: now,
            imported_from: None,
            synced: false,
            updated_at_millis: now * 1000,
        }
    }

    /// When it was last changed, in milliseconds, from the seconds a playlist from before was kept with.
    pub fn updated_millis(&self) -> i64 {
        if self.updated_at_millis > 0 {
            self.updated_at_millis
        } else {
            self.updated_at_epoch_seconds * 1000
        }
    }

    /// Marks it changed now: always later than the copy it changes, even where another device's clock ran ahead
    /// of this one's, so an edit made here is never lost to the copy it was made to.
    pub fn touch(&mut self) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        self.updated_at_millis = now.max(self.updated_millis() + 1);
        self.updated_at_epoch_seconds = self.updated_at_millis / 1000;
    }
}

/// Reads a song at a time, so one from a service a later version added costs that song, not the playlist.
pub fn forgiving_tracks<'de, D>(deserializer: D) -> Result<Vec<Track>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let values: Vec<serde_json::Value> = Vec::deserialize(deserializer)?;
    Ok(values
        .into_iter()
        .filter_map(|value| serde_json::from_value(value).ok())
        .collect())
}

/// A list of songs read the same forgiving way, for the files that are nothing but one.
pub fn decode_tracks(text: &str) -> Vec<Track> {
    serde_json::from_str::<Vec<serde_json::Value>>(text)
        .map(|values| {
            values
                .into_iter()
                .filter_map(|value| serde_json::from_value(value).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// A random version-4 UUID, written as Kotlin's `UUID.randomUUID()` writes one.
pub fn new_id() -> String {
    let mut bytes: [u8; 16] = rand::random();
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// `3:07`, or `1:02:45` past the hour.
pub fn format_duration(ms: i64) -> String {
    let total = (ms.max(0) / 1000) as u64;
    let (hours, minutes, seconds) = (total / 3600, (total / 60) % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_track_as_kotlin_writes_it() {
        let json = r#"{"provider":"YOUTUBE_MUSIC","id":"abc","title":"Song",
            "artists":[{"id":"YOUTUBE_MUSIC:Band","name":"Band","provider":"YOUTUBE_MUSIC"}],
            "durationMs":187000,"sourceUrl":"https://music.youtube.com/watch?v=abc"}"#;
        let track: Track = serde_json::from_str(json).unwrap();
        assert_eq!(track.provider, ProviderType::YoutubeMusic);
        assert_eq!(track.queue_key(), "YOUTUBE_MUSIC:abc");
        assert_eq!(track.duration_ms, Some(187000));
        let written = serde_json::to_string(&track).unwrap();
        assert!(written.contains("\"sourceUrl\""));
        assert!(!written.contains("artworkUrl"), "absent values are left out: {written}");
    }

    #[test]
    fn reads_a_station_from_the_kotlin_players() {
        let json = r#"[{"provider":"RADIO","id":"9608b51d","title":"Radio Paradise",
            "artists":[{"id":"radio-country:US","name":"California, The United States Of America · rock","provider":"RADIO"}],
            "artworkUrl":"https://radioparadise.com/favicon.ico","sourceUrl":"http://stream.radioparadise.com/aac-320"}]"#;
        let tracks = decode_tracks(json);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].provider, ProviderType::Radio);
        assert_eq!(tracks[0].queue_key(), "RADIO:9608b51d");
        assert!(tracks[0].provider.plays_here() && !tracks[0].provider.playable_here() && !tracks[0].provider.downloadable());
        assert!(serde_json::to_string(&tracks[0]).unwrap().contains("\"provider\":\"RADIO\""));
    }

    #[test]
    fn one_unknown_song_costs_that_song_only() {
        let json = r#"{"id":"p","title":"Mixed","tracks":[
            {"provider":"SOMETHING_NEW","id":"x","title":"?","sourceUrl":"x"},
            {"provider":"SOUNDCLOUD","id":"1","title":"Kept","sourceUrl":"https://soundcloud.com/a/b"}]}"#;
        let playlist: LocalPlaylist = serde_json::from_str(json).unwrap();
        assert_eq!(playlist.tracks.len(), 1);
        assert_eq!(playlist.tracks[0].title, "Kept");
    }

    #[test]
    fn durations_read_as_a_clock() {
        assert_eq!(format_duration(187_000), "3:07");
        assert_eq!(format_duration(3_765_000), "1:02:45");
        assert_eq!(format_duration(-5), "0:00");
    }

    #[test]
    fn ids_look_like_uuids() {
        let id = new_id();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
    }
}
