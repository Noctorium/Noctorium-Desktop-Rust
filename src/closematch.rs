//! The lyrics of a song no source knows by the name it was given, found by a looser search -- Base's
//! `CloseMatch.kt`, with the same rules and the same thresholds, so the same song finds the same lyrics on
//! every Noctorium.
//!
//! A song uploaded by somebody else comes with their channel as its artist and a title such as
//! "Daft Punk - Get Lucky (slowed + reverb) [lyrics]": every source is asked for that title by that artist, and
//! none has it. This asks LRCLIB's search again with what is left of the title once the uploader's additions are
//! gone -- with the artist read out of the title where it is there -- and takes the candidate that matches it
//! best, if one matches well enough. What it finds is marked a close match, so the lyrics can say they may not be
//! this song's.

use crate::domain::Track;
use crate::lyrics::{CloseMatch, Lyrics};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

/// How many searches one song is worth: each is a request to a volunteer-run service.
const MAX_SEARCHES: usize = 4;
/// How alike a candidate has to be to be shown at all, from 0 to 1.
const THRESHOLD: f64 = 0.62;
/// Alike enough that searching on would only find the same song again.
const CERTAIN: f64 = 0.92;

/// A title to search for, and the artist with it where one could be told apart from the title.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Guess {
    pub title: String,
    pub artist: Option<String>,
}

/// The best close match for `track`, or None when nothing comes near enough to show.
pub fn find(track: &Track) -> Option<Lyrics> {
    find_with(track, super::lyrics::lrclib_search)
}

/// As [`find`], with LRCLIB's search answered by `search`: for the tests.
pub fn find_with(track: &Track, search: impl Fn(&[(&str, String)]) -> Option<Vec<serde_json::Value>>) -> Option<Lyrics> {
    let guesses = guesses_for(track);
    if guesses.is_empty() {
        return None;
    }
    let song_seconds = track.duration_ms.map(|ms| ms / 1000);
    let mut seen = HashSet::new();
    let mut best: Option<(f64, Lyrics)> = None;
    for guess in guesses.iter().take(MAX_SEARCHES) {
        let parameters: Vec<(&str, String)> = match &guess.artist {
            Some(artist) => vec![("track_name", guess.title.clone()), ("artist_name", artist.clone())],
            None => vec![("q", guess.title.clone())],
        };
        let Some(candidates) = search(&parameters) else { continue };
        for root in &candidates {
            if let Some(id) = root["id"].as_i64() {
                if !seen.insert(id) {
                    continue;
                }
            }
            let Some(title) = root["trackName"].as_str() else { continue };
            let artist = root["artistName"].as_str().unwrap_or("");
            let score = score(&guesses, title, artist, root["duration"].as_f64(), song_seconds);
            if score < THRESHOLD || best.as_ref().is_some_and(|(b, _)| score <= *b) {
                continue;
            }
            let Some(mut lyrics) = super::lyrics::lrclib_lyrics(root) else { continue };
            lyrics.close_match = Some(CloseMatch { title: title.to_string(), artist: artist.to_string() });
            lyrics.source = format!("Close match from LRCLIB: {title} by {artist}");
            best = Some((score, lyrics));
        }
        if best.as_ref().is_some_and(|(score, _)| *score >= CERTAIN) {
            break;
        }
    }
    best.map(|(_, lyrics)| lyrics)
}

/// What `track` might be called where the lyrics are: its title without what uploaders add to one, split at
/// its dash into artist and title when it has one, and the title alone, which LRCLIB's free search matches
/// against both. The track's own artist is used only where it is not a channel's name.
pub fn guesses_for(track: &Track) -> Vec<Guess> {
    let title = strip_uploader_noise(&track.title);
    if title.is_empty() {
        return vec![];
    }
    let artist = track.artists.first().map(|a| strip_uploader_noise(&a.name)).filter(|a| !a.is_empty() && !looks_like_a_channel(a));
    let mut guesses = Vec::new();
    static DASH: OnceLock<Regex> = OnceLock::new();
    let dash = DASH.get_or_init(|| Regex::new(r"\s+[-–—]\s+").unwrap());
    if let Some(found) = dash.find(&title) {
        let (left, right) = (title[..found.start()].trim(), title[found.end()..].trim());
        if !left.is_empty() && !right.is_empty() {
            guesses.push(Guess { title: right.to_string(), artist: Some(left.to_string()) });
            // "Song - Artist", the other way round, as some uploaders write it.
            guesses.push(Guess { title: left.to_string(), artist: Some(right.to_string()) });
        }
    }
    if let Some(artist) = &artist {
        guesses.push(Guess { title: title.clone(), artist: Some(artist.clone()) });
    }
    guesses.push(Guess {
        title: match &artist {
            Some(artist) => format!("{artist} {title}"),
            None => title.clone(),
        },
        artist: None,
    });
    // Last, the title alone: an uploader's name that looks like an artist's -- "shxhmir" -- would otherwise keep
    // every search to songs by nobody LRCLIB has heard of.
    if artist.is_some() {
        guesses.push(Guess { title: title.clone(), artist: None });
    }
    let mut unique = Vec::new();
    for guess in guesses {
        if !unique.contains(&guess) {
            unique.push(guess);
        }
    }
    unique
}

/// `title` without what an uploader adds to a song's name: "(slowed + reverb)", "[lyrics]", "| 1 hour",
/// "(official video)", "ft. somebody", "nightcore", emoji. Kept: remixes and versions, whose lyrics may differ.
pub fn strip_uploader_noise(title: &str) -> String {
    static RULES: OnceLock<Vec<Regex>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            // Anything in brackets that is about the upload rather than the song.
            r"(?i)[\[(【「][^\])】」]*(slowed|reverb|sped|speed|nightcore|lyric|letra|audio|video|visuali[sz]er|official|hd|hq|4k|8d|bass\s*boost|1\s*hour|extended|loop|clean|explicit|tiktok|edit audio|karaoke|instrumental version|cover art)[^\])】」]*[\])】」]",
            // The same, after a separator at the end: "Song | slowed + reverb", "Song // lyrics".
            r"\s*(\||//|~)\s*.*$",
            r"(?i)\b(slowed(\s*(and|&|\+)\s*reverb)?|sped\s*up|nightcore|reverb|8d\s*audio|bass\s*boosted)\b",
            // A featured artist is the song's, but no source's title has it.
            r"(?i)\s*[\[(]?\b(ft|feat|featuring)\b\.?\s+[^\])\-–—]*[\])]?",
            // Emoji and pictographs. (No surrogates to take as Kotlin does: a Rust string has none.)
            r"\p{So}",
        ]
        .iter()
        .map(|rule| Regex::new(rule).unwrap())
        .collect()
    });
    let mut text = title.to_string();
    for rule in rules {
        text = rule.replace_all(&text, " ").into_owned();
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ").trim_matches(|c| matches!(c, ' ' | '-' | '–' | '—' | ':' | '|')).to_string()
}

/// A YouTube channel's name rather than an artist's: "Daft Punk - Topic", "SomeMusicVEVO", "Chill Beats Uploads".
pub fn looks_like_a_channel(name: &str) -> bool {
    let lowered = name.to_lowercase();
    if lowered.ends_with(" - topic") || lowered.ends_with("vevo") || lowered.starts_with("youtube") {
        return true;
    }
    const WORDS: [&str; 13] = ["music", "records", "uploads", "lyrics", "beats", "vibes", "channel", "official", "tv", "sounds", "audio", "nightcore", "slowed"];
    lowered.split(|c: char| !c.is_alphanumeric()).any(|word| WORDS.contains(&word))
}

fn normalise(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// How alike a candidate is to the best of the guesses, from 0 to 1: the title counts most, then the artist
/// where a guess has one, with a little for a length close to the song's -- not much, since a slowed or sped-up
/// upload is the right song at the wrong length.
pub fn score(guesses: &[Guess], title: &str, artist: &str, candidate_seconds: Option<f64>, song_seconds: Option<i64>) -> f64 {
    let (candidate_title, candidate_artist) = (normalise(title), normalise(artist));
    let both = format!("{candidate_artist} {candidate_title}");
    let best = guesses
        .iter()
        .map(|guess| {
            let guess_title = normalise(&guess.title);
            match &guess.artist {
                None => {
                    // Free text: the candidate's artist and title together against it, with the title in it.
                    let inside = similarity(&candidate_title, &guess_title, true);
                    if inside < 0.6 {
                        0.0
                    } else {
                        0.55 * similarity(&both, &guess_title, false) + 0.45 * inside
                    }
                }
                Some(guess_artist) => 0.7 * similarity(&candidate_title, &guess_title, false) + 0.3 * similarity(&candidate_artist, &normalise(guess_artist), false),
            }
        })
        .fold(0.0, f64::max);
    let length_bonus = match (candidate_seconds, song_seconds) {
        (Some(candidate), Some(song)) if song > 0 => {
            // Slowed uploads run up to a third longer, sped-up ones as much shorter.
            if (0.75..=1.35).contains(&(candidate / song as f64)) {
                0.05
            } else {
                -0.05
            }
        }
        _ => 0.0,
    };
    (best + length_bonus).clamp(0.0, 1.0)
}

/// How many words two names share, from 0 to 1 (the Dice coefficient of their words). With `containment`, how
/// many of `a`'s words are in `b` -- for a title that has to be found within a longer free text.
pub fn similarity(a: &str, b: &str, containment: bool) -> f64 {
    let left: HashSet<&str> = a.split(' ').filter(|w| !w.is_empty()).collect();
    let right: HashSet<&str> = b.split(' ').filter(|w| !w.is_empty()).collect();
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let shared = left.intersection(&right).count() as f64;
    if containment {
        shared / left.len() as f64
    } else {
        2.0 * shared / (left.len() + right.len()) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Artist, ProviderType};

    fn upload(title: &str, channel: &str) -> Track {
        Track {
            provider: ProviderType::YoutubeVideo,
            id: "x".into(),
            title: title.into(),
            artists: vec![Artist { id: "c".into(), name: channel.into(), provider: ProviderType::YoutubeVideo }],
            album: None,
            duration_ms: Some(290_000),
            artwork_url: None,
            source_url: "https://www.youtube.com/watch?v=x".into(),
        }
    }

    fn candidate(id: i64, title: &str, artist: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id, "trackName": title, "artistName": artist, "duration": 248.0, "plainLyrics": "x",
            "syncedLyrics": "[00:01.00]Like the legend of the phoenix\n[00:04.00]Our ends were beginnings\n[00:07.00]What keeps the planet spinning\n[00:10.00]The force from the beginning",
        })
    }

    #[test]
    fn an_uploaders_additions_come_off_the_title() {
        assert_eq!(strip_uploader_noise("Daft Punk - Get Lucky (slowed + reverb) [lyrics]"), "Daft Punk - Get Lucky");
        assert_eq!(strip_uploader_noise("Get Lucky | 1 hour version"), "Get Lucky");
        assert_eq!(strip_uploader_noise("Get Lucky ft. Pharrell Williams"), "Get Lucky");
        assert_eq!(strip_uploader_noise("Get Lucky (Official Audio) ✨"), "Get Lucky");
        assert_eq!(strip_uploader_noise("Get Lucky sped up"), "Get Lucky");
        assert_eq!(strip_uploader_noise("Get Lucky (Daft Punk Remix)"), "Get Lucky (Daft Punk Remix)");
    }

    #[test]
    fn a_channel_is_told_from_an_artist() {
        assert!(looks_like_a_channel("Daft Punk - Topic"));
        assert!(looks_like_a_channel("DaftPunkVEVO"));
        assert!(looks_like_a_channel("Chill Vibes Music"));
        assert!(!looks_like_a_channel("Daft Punk"));
    }

    #[test]
    fn a_title_with_a_dash_is_read_as_artist_and_song() {
        let guesses = guesses_for(&upload("Daft Punk - Get Lucky (slowed + reverb)", "Chill Vibes Music"));
        assert_eq!(guesses[0], Guess { title: "Get Lucky".into(), artist: Some("Daft Punk".into()) });
        assert!(guesses.iter().all(|g| g.artist.as_deref() != Some("Chill Vibes Music")));
    }

    #[test]
    fn a_reupload_finds_the_songs_lyrics_marked_as_a_close_match() {
        let asked = std::cell::Cell::new(0);
        let found = find_with(&upload("Daft Punk - Get Lucky (slowed + reverb) [lyrics]", "Chill Vibes Music"), |_| {
            asked.set(asked.get() + 1);
            Some(vec![candidate(7, "Get Lucky", "Daft Punk")])
        })
        .unwrap();
        assert_eq!(found.close_match, Some(CloseMatch { title: "Get Lucky".into(), artist: "Daft Punk".into() }));
        assert_eq!(found.lines.len(), 4);
        assert_eq!(asked.get(), 1, "a near-certain match needs no more searching");
    }

    /// Found on a real phone: an uploader whose name looks like an artist's, and no source that knows them.
    #[test]
    fn an_uploader_who_looks_like_an_artist_still_gets_the_title_searched_on_its_own() {
        let asked = std::cell::Cell::new(0);
        let found = find_with(&upload("Get Lucky", "shxhmir"), |parameters| {
            asked.set(asked.get() + 1);
            if parameters.iter().any(|(_, v)| v.contains("shxhmir")) {
                Some(vec![])
            } else {
                Some(vec![candidate(7, "Get Lucky", "Daft Punk")])
            }
        });
        assert_eq!(found.unwrap().close_match.unwrap().artist, "Daft Punk");
        assert_eq!(asked.get(), 3, "the title alone, after the two with the uploader's name");
    }

    #[test]
    fn the_best_of_several_candidates_is_taken() {
        let found = find_with(&upload("Daft Punk - Get Lucky", "Some Uploads"), |_| {
            Some(vec![candidate(1, "Lucky", "Somebody Else"), candidate(2, "Get Lucky", "Daft Punk"), candidate(3, "Get Lucky (Radio Edit)", "Daft Punk")])
        });
        assert_eq!(found.unwrap().close_match.unwrap().title, "Get Lucky");
    }

    #[test]
    fn nothing_near_enough_is_no_match_rather_than_somebody_elses_lyrics() {
        let track = upload("Daft Punk - Get Lucky", "Some Uploads");
        assert!(find_with(&track, |_| Some(vec![candidate(1, "Harder Better Faster Stronger", "Daft Punk"), candidate(2, "Lucky Star", "Madonna")])).is_none());
        assert!(find_with(&track, |_| Some(vec![])).is_none());
        assert!(find_with(&track, |_| None).is_none());
    }
}
