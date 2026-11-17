//! Artists' pages: pressing an artist's name opens their songs on the service the song came from -- YouTube Music's
//! artist for a YouTube Music song, the SoundCloud profile for a SoundCloud one -- the way a playlist opens, and Search
//! has Artists beside Music and Radio.
//!
//! A page is kept in the window's table of opened lists under a key of its own, [`key`], and read on a thread with
//! [`load`]: the profile's tracks where the song said whose they are (SoundCloud), and otherwise the songs a search of
//! that service for the artist finds that are theirs.

use crate::domain::{Playlist, ProviderType, Track};
use crate::ytdlp::YtDlp;

const PREFIX: &str = "noctorium-artist:";
const SEPARATOR: char = '\u{1}';

/// An artist, as a page is opened for them: their service, their name, and their profile's address where it is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtistRef {
    pub provider: ProviderType,
    pub name: String,
    pub profile: Option<String>,
    pub artwork: Option<String>,
}

const PROVIDERS: [ProviderType; 11] = [
    ProviderType::YoutubeMusic,
    ProviderType::YoutubeVideo,
    ProviderType::Soundcloud,
    ProviderType::Spotify,
    ProviderType::Bandcamp,
    ProviderType::Vk,
    ProviderType::AppleMusic,
    ProviderType::Deezer,
    ProviderType::Tidal,
    ProviderType::Local,
    ProviderType::Radio,
];

/// The first artist of `track`, with their SoundCloud profile where the song's address says it; None for a station, a
/// file on this computer or a song with no artist.
pub fn of_track(track: &Track) -> Option<ArtistRef> {
    if matches!(track.provider, ProviderType::Radio | ProviderType::Local) {
        return None;
    }
    let name = track.artists.first().map(|a| a.name.clone()).unwrap_or_else(|| track.artist_line());
    let name = name.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let profile = (track.provider == ProviderType::Soundcloud).then(|| soundcloud_profile(&track.source_url)).flatten();
    Some(ArtistRef { provider: track.provider, name, profile, artwork: None })
}

/// The key an artist's page is kept under among the opened lists.
pub fn key(artist: &ArtistRef) -> String {
    format!("{PREFIX}{}{SEPARATOR}{}{SEPARATOR}{}", artist.provider.key_name(), artist.name, artist.profile.as_deref().unwrap_or(""))
}

/// Whether an opened list's key is an artist's page.
pub fn is_key(key: &str) -> bool {
    key.starts_with(PREFIX)
}

/// The artist a page's key names.
pub fn parse(key: &str) -> Option<ArtistRef> {
    let rest = key.strip_prefix(PREFIX)?;
    let mut parts = rest.split(SEPARATOR);
    let provider_key = parts.next()?;
    let provider = PROVIDERS.into_iter().find(|p| p.key_name() == provider_key)?;
    let name = parts.next()?.to_string();
    let profile = parts.next().filter(|p| !p.is_empty()).map(String::from);
    Some(ArtistRef { provider, name, profile, artwork: None })
}

/// The SoundCloud profile a song's address is under: `https://soundcloud.com/odesza` for `.../odesza/a-moment-apart`.
pub fn soundcloud_profile(song_url: &str) -> Option<String> {
    let path = song_url.split("soundcloud.com/").nth(1)?.split('?').next()?.trim_matches('/');
    let (profile, rest) = path.split_once('/')?;
    (!profile.is_empty() && !rest.is_empty()).then(|| format!("https://soundcloud.com/{profile}"))
}

fn plain(name: &str) -> String {
    let lowered: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let words = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
    let words = words.strip_prefix("the ").unwrap_or(&words).to_string();
    words.strip_suffix(" topic").unwrap_or(&words).to_string()
}

/// Whether two artist names are the same artist as far as their words go: case, punctuation and a leading "The"
/// aside, or one the other with something added -- "Daft Punk" and "Daft Punk - Topic".
pub fn names_match(one: &str, other: &str) -> bool {
    let (a, b) = (plain(one), plain(other));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || (a.len() >= 3 && b.len() >= 3 && (a.starts_with(&format!("{b} ")) || b.starts_with(&format!("{a} "))))
}

/// The artists among what a search found that are who was searched for: what Artists in Search lists, a few from each
/// service, with a cover from one of their songs.
pub fn found(query: &str, results: &[(ProviderType, Vec<Track>)]) -> Vec<ArtistRef> {
    let mut artists: Vec<ArtistRef> = Vec::new();
    for (provider, tracks) in results {
        let mut from_this = 0;
        for track in tracks {
            for artist in &track.artists {
                if from_this >= 6 || !names_match(&artist.name, query) {
                    continue;
                }
                if artists.iter().any(|a| a.provider == *provider && plain(&a.name) == plain(&artist.name)) {
                    continue;
                }
                let profile = (*provider == ProviderType::Soundcloud).then(|| soundcloud_profile(&track.source_url)).flatten();
                artists.push(ArtistRef { provider: *provider, name: artist.name.clone(), profile, artwork: track.artwork_url.clone() });
                from_this += 1;
            }
        }
    }
    artists
}

/// An artist's page, read: the SoundCloud profile's tracks where it is known, and otherwise the songs a search of
/// their service for them finds that are theirs (every song found, when none says so).
pub fn load(backend: &YtDlp, artist: &ArtistRef) -> Result<Playlist, String> {
    let tracks = match (&artist.profile, artist.provider) {
        (Some(profile), ProviderType::Soundcloud) => backend
            .playlist(&format!("{profile}/tracks"), 200)
            .map(|p| p.tracks)
            .or_else(|_| songs_by(backend, artist))?,
        _ => songs_by(backend, artist)?,
    };
    Ok(Playlist {
        id: key(artist),
        title: artist.name.clone(),
        provider: artist.provider,
        owner_name: Some(format!("Artist · {}", artist.provider.display_name())),
        artwork_url: artist.artwork.clone().or_else(|| tracks.iter().find_map(|t| t.artwork_url.clone())),
        track_count: Some(tracks.len() as i64),
        tracks,
        source_url: artist.profile.clone(),
    })
}

fn songs_by(backend: &YtDlp, artist: &ArtistRef) -> Result<Vec<Track>, String> {
    let found = crate::services::search(backend, artist.provider, &artist.name, 50)?;
    let theirs: Vec<Track> = found.iter().filter(|t| t.artists.iter().any(|a| names_match(&a.name, &artist.name))).cloned().collect();
    Ok(if theirs.is_empty() { found } else { theirs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Artist;

    fn song(provider: ProviderType, artist: &str, url: &str) -> Track {
        Track {
            provider,
            id: url.into(),
            title: "Song".into(),
            artists: vec![Artist { id: artist.into(), name: artist.into(), provider }],
            album: None,
            duration_ms: None,
            artwork_url: Some("https://cover".into()),
            source_url: url.into(),
        }
    }

    #[test]
    fn a_page_key_says_the_artist_and_back() {
        let odesza = of_track(&song(ProviderType::Soundcloud, "ODESZA", "https://soundcloud.com/odesza/a-moment-apart")).unwrap();
        assert_eq!(odesza.profile.as_deref(), Some("https://soundcloud.com/odesza"));
        let k = key(&odesza);
        assert!(is_key(&k));
        assert_eq!(parse(&k), Some(odesza));
        let daft = of_track(&song(ProviderType::YoutubeMusic, "Daft Punk", "https://music.youtube.com/watch?v=x")).unwrap();
        assert_eq!(parse(&key(&daft)).unwrap().profile, None);
        assert!(of_track(&song(ProviderType::Radio, "France", "https://radio")).is_none());
    }

    #[test]
    fn names_are_the_same_artist_when_only_spelling_differs() {
        assert!(names_match("Daft Punk - Topic", "daft punk"));
        assert!(names_match("The Weeknd", "Weeknd"));
        assert!(!names_match("Daft Punk", "Punk"));
    }

    #[test]
    fn artists_in_search_are_who_was_searched_for() {
        let results = vec![(
            ProviderType::YoutubeMusic,
            vec![song(ProviderType::YoutubeMusic, "Daft Punk", "a"), song(ProviderType::YoutubeMusic, "Someone Else", "b"), song(ProviderType::YoutubeMusic, "Daft Punk", "c")],
        )];
        let artists = found("daft punk", &results);
        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].name, "Daft Punk");
        assert_eq!(artists[0].artwork.as_deref(), Some("https://cover"));
    }
}
