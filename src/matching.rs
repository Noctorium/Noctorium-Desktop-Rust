//! Playing a song from a service that only lets its own players make the sound -- Spotify, Apple Music,
//! Deezer, Tidal -- by finding the same recording on YouTube Music. Noctorium-Base's `ReferenceMatch`, rule
//! for rule: title, artist and length each scored, a remix or a cover never taken for the original, and
//! nothing taken at all below the line. Matches are remembered, `matches.json`, so a song is searched once.

use crate::domain::{ProviderType, Track};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

pub const ACCEPT_SCORE: f64 = 0.62;
const MAX_DURATION_GAP_MS: i64 = 25_000;
const CLOSE_DURATION_MS: i64 = 4_000;

const EDITION_WORDS: [&str; 28] = [
    "remaster", "remastered", "remasterd", "re-master", "re-mastered", "master", "mono", "stereo", "single", "album",
    "version", "edit", "radio", "deluxe", "expanded", "bonus", "track", "anniversary", "reissue", "re-issue",
    "digital", "explicit", "clean", "edition", "original", "recording", "the", "remastered.",
];

const REMIX_WORDS: [&str; 23] = [
    "remix", "bootleg", "flip", "mashup", "cover", "karaoke", "instrumental", "nightcore", "sped up", "spedup",
    "slowed", "reverb", "8d audio", "1 hour", "one hour", "hour loop", "loop", "live at", "live in", "live from",
    "tribute", "in the style of", "made famous by",
];

/// What to search YouTube Music for: the first artist and the title without its edition.
pub fn search_query(track: &Track) -> String {
    let title = stripped_title(&track.title);
    let artist = track.artists.first().map(|a| a.name.trim().to_string()).unwrap_or_default();
    [artist, title].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
}

/// The best candidate that clears the line, or none.
pub fn choose<'a>(reference: &Track, candidates: &'a [Track]) -> Option<&'a Track> {
    candidates
        .iter()
        .filter(|c| !c.provider.is_reference())
        .map(|c| (c, score(reference, c)))
        .filter(|(_, s)| *s >= ACCEPT_SCORE)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(c, _)| c)
}

/// The match, wearing the reference's own title and cover: the listener chose that song, not a video of it.
pub fn resolved(reference: &Track, matched: &Track) -> Track {
    Track {
        title: reference.title.clone(),
        artists: if matched.artists.is_empty() { reference.artists.clone() } else { matched.artists.clone() },
        album: matched.album.clone().or_else(|| reference.album.clone()),
        artwork_url: reference.artwork_url.clone().or_else(|| matched.artwork_url.clone()),
        duration_ms: matched.duration_ms.or(reference.duration_ms),
        ..matched.clone()
    }
}

pub fn score(reference: &Track, candidate: &Track) -> f64 {
    let artist = artist_score(reference, candidate);
    if artist <= 0.0 {
        return 0.0;
    }
    let title = title_score(&reference.title, &candidate.title);
    if title <= 0.0 {
        return 0.0;
    }
    let duration = duration_score(reference.duration_ms, candidate.duration_ms);
    if duration <= 0.0 {
        return 0.0;
    }
    title * 0.45 + artist * 0.35 + duration * 0.20
}

fn artist_score(reference: &Track, candidate: &Track) -> f64 {
    let wanted: Vec<String> = reference.artists.iter().map(|a| normalize(&a.name)).filter(|s| !s.is_empty()).collect();
    let Some(primary) = wanted.first() else { return 0.0 };
    let offered: Vec<String> = candidate
        .artists
        .iter()
        .map(|a| normalize(&a.name))
        .chain(std::iter::once(normalize(&candidate.title)))
        .filter(|s| !s.is_empty())
        .collect();
    if offered.is_empty() {
        return 0.0;
    }
    if offered.iter().any(|o| o == primary) {
        1.0
    } else if offered.iter().any(|o| o.contains(primary.as_str()) || primary.contains(o.as_str())) {
        0.9
    } else if wanted.iter().skip(1).any(|extra| offered.iter().any(|o| o.contains(extra.as_str()))) {
        0.7
    } else if tokens_overlap(primary, &offered.join(" ")) >= 0.6 {
        0.65
    } else {
        0.0
    }
}

fn title_score(reference: &str, candidate: &str) -> f64 {
    let wanted = normalize(&stripped_title(reference));
    let offered = normalize(candidate);
    if wanted.is_empty() || offered.is_empty() {
        return 0.0;
    }
    let wanted_remix = REMIX_WORDS.iter().any(|w| wanted.contains(w));
    let offered_remix = REMIX_WORDS.iter().any(|w| offered.contains(w));
    if offered_remix && !wanted_remix {
        return 0.0;
    }
    if offered == wanted {
        1.0
    } else if offered.contains(&wanted) {
        0.9
    } else if wanted.contains(&offered) {
        0.8
    } else {
        let overlap = tokens_overlap(&wanted, &offered);
        if overlap >= 0.6 { overlap } else { 0.0 }
    }
}

fn duration_score(wanted: Option<i64>, offered: Option<i64>) -> f64 {
    match (wanted, offered) {
        (Some(w), Some(o)) if w > 0 && o > 0 => {
            let gap = (w - o).abs();
            if gap > MAX_DURATION_GAP_MS {
                0.0
            } else if gap <= CLOSE_DURATION_MS {
                1.0
            } else {
                1.0 - (gap - CLOSE_DURATION_MS) as f64 / (MAX_DURATION_GAP_MS - CLOSE_DURATION_MS) as f64
            }
        }
        _ => 0.5,
    }
}

fn tokens_overlap(left: &str, right: &str) -> f64 {
    let l: std::collections::HashSet<&str> = left.split(' ').filter(|w| !w.is_empty()).collect();
    let r: std::collections::HashSet<&str> = right.split(' ').filter(|w| !w.is_empty()).collect();
    if l.is_empty() || r.is_empty() {
        return 0.0;
    }
    l.iter().filter(|w| r.contains(*w)).count() as f64 / l.len().min(r.len()) as f64
}

/// The title without what says which edition it is: "(2011 Remaster)", "- Radio Edit".
pub fn stripped_title(title: &str) -> String {
    static BRACKETED: OnceLock<Regex> = OnceLock::new();
    static YEAR: OnceLock<Regex> = OnceLock::new();
    let bracketed = BRACKETED.get_or_init(|| {
        Regex::new(r"(?i)[(\[](\d{4}\s)?(remaster(ed)?|re-?master(ed)?|deluxe|expanded|bonus track|anniversary|explicit|clean|album version|single version|mono|stereo)[^)\]]*[)\]]").unwrap()
    });
    let year = YEAR.get_or_init(|| Regex::new(r"^\d{4}$").unwrap());
    let mut working = bracketed.replace_all(title.trim(), " ").trim().to_string();
    // The last " - " or " – ", if everything after it is edition words.
    let dash = [" - ", " – "].iter().filter_map(|d| working.rfind(d).map(|i| (i, d.len()))).max_by_key(|(i, _)| *i);
    if let Some((index, length)) = dash {
        let tail = &working[index + length..];
        let words: Vec<String> = tail
            .split([' ', '/', '·', ','])
            .map(|w| w.trim().to_lowercase())
            .filter(|w| !w.is_empty())
            .collect();
        if !words.is_empty() && words.iter().all(|w| EDITION_WORDS.contains(&w.as_str()) || year.is_match(w)) {
            working = working[..index].trim().to_string();
        }
    }
    let collapsed = working.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches([' ', '-', '–', ',', '/']).to_string();
    if trimmed.is_empty() { title.trim().to_string() } else { trimmed }
}

/// Lower case, accents taken off, everything but letters and digits a single space.
pub fn normalize(text: &str) -> String {
    let plain: String = text
        .to_lowercase()
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Matches made, by the reference's queue key: `matches.json`.
#[derive(Default)]
pub struct Store {
    path: Option<std::path::PathBuf>,
    matches: HashMap<String, Track>,
}

impl Store {
    pub fn open(places: &crate::store::Places) -> Store {
        let path = places.file("matches.json");
        let matches = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Store { path, matches }
    }

    pub fn get(&self, reference: &Track) -> Option<Track> {
        self.matches.get(&reference.queue_key()).cloned()
    }

    pub fn remember(&mut self, reference: &Track, matched: &Track) {
        self.matches.insert(reference.queue_key(), matched.clone());
        if let (Some(path), Ok(text)) = (&self.path, serde_json::to_string(&self.matches)) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// The YouTube Music recording of `reference`, from the store or by searching for it.
pub fn find(store: &std::sync::Mutex<Store>, backend: &crate::ytdlp::YtDlp, reference: &Track) -> Result<Track, String> {
    if let Some(known) = store.lock().unwrap().get(reference) {
        return Ok(known);
    }
    let query = search_query(reference);
    let candidates = backend.search(ProviderType::YoutubeMusic, &query, 10)?;
    let chosen = choose(reference, &candidates)
        .ok_or_else(|| format!("“{}” could not be found on YouTube Music.", reference.title))?;
    let matched = resolved(reference, chosen);
    store.lock().unwrap().remember(reference, &matched);
    Ok(matched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Artist;

    fn track(provider: ProviderType, title: &str, artist: &str, ms: Option<i64>) -> Track {
        Track {
            provider,
            id: title.into(),
            title: title.into(),
            artists: vec![Artist { id: artist.into(), name: artist.into(), provider }],
            album: None,
            duration_ms: ms,
            artwork_url: None,
            source_url: "https://music.youtube.com/watch?v=x".into(),
        }
    }

    #[test]
    fn the_same_recording_is_found_and_a_remix_is_not() {
        let reference = track(ProviderType::Spotify, "Around the World - Radio Edit", "Daft Punk", Some(240_000));
        let candidates = vec![
            track(ProviderType::YoutubeMusic, "Around the World (Remix)", "Daft Punk", Some(240_000)),
            track(ProviderType::YoutubeMusic, "Around the World", "Daft Punk", Some(241_000)),
            track(ProviderType::YoutubeMusic, "Around the World", "Someone Else", Some(240_000)),
        ];
        let chosen = choose(&reference, &candidates).unwrap();
        assert_eq!(chosen.artists[0].name, "Daft Punk");
        assert_eq!(chosen.title, "Around the World");
    }

    #[test]
    fn a_length_far_off_is_refused() {
        let reference = track(ProviderType::Deezer, "Song", "Band", Some(200_000));
        let far = track(ProviderType::YoutubeMusic, "Song", "Band", Some(400_000));
        assert!(choose(&reference, &[far]).is_none());
    }

    #[test]
    fn editions_are_stripped_and_accents_dropped() {
        assert_eq!(stripped_title("Bohemian Rhapsody - 2011 Remaster"), "Bohemian Rhapsody");
        assert_eq!(stripped_title("Song (Remastered 2009)"), "Song");
        assert_eq!(stripped_title("Mr. Blue Sky - Live"), "Mr. Blue Sky - Live");
        assert_eq!(normalize("Beyoncé – Halo!"), "beyonce halo");
    }

    #[test]
    fn the_match_wears_the_references_title_and_cover() {
        let mut reference = track(ProviderType::Spotify, "Song", "Band", None);
        reference.artwork_url = Some("spotify-cover".into());
        let matched = track(ProviderType::YoutubeMusic, "Song (Official Audio)", "Band", Some(1));
        let result = resolved(&reference, &matched);
        assert_eq!(result.title, "Song");
        assert_eq!(result.provider, ProviderType::YoutubeMusic);
        assert_eq!(result.artwork_url.as_deref(), Some("spotify-cover"));
    }
}
