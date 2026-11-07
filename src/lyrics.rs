//! Lyrics from every source Noctorium-Base asks -- LRCLIB, Better Lyrics, Karalyr, SyncLRC, lyrics.ovh,
//! and Musixmatch, Happi and Genius for those with a key -- all at once, synced where they have them.
//!
//! LRCLIB as Noctorium-Base has it: the exact lookup by title, artist, album and length, and when that
//! finds nothing, or a placeholder, its search, with every candidate scored on how well its title, artist
//! and length agree and how complete it is.

use crate::domain::Track;
use regex::Regex;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq)]
pub struct LyricLine {
    pub text: String,
    /// When the line starts, for synced lyrics.
    pub start_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
    pub synced: bool,
    /// What it says where they came from: "Lyrics from LRCLIB".
    pub source: String,
    pub provider: Provider,
    /// The page they came from, for a source that offers only a link -- Genius.
    pub link: Option<String>,
    /// What every source answered, for choosing another; empty on the alternatives themselves.
    pub outcomes: Vec<Outcome>,
    /// Set when these are not the song's own lyrics as asked for, but the closest a looser search found: a
    /// song uploaded under somebody else's name. See [`crate::closematch`].
    pub close_match: Option<CloseMatch>,
}

/// The song a close match's lyrics belong to, as the source names it.
#[derive(Clone, Debug, PartialEq)]
pub struct CloseMatch {
    pub title: String,
    pub artist: String,
}

/// The lyrics sources, saved by the names Kotlin gives them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Provider {
    #[default]
    Lrclib,
    BetterLyrics,
    Karalyr,
    Synclrc,
    LyricsOvh,
    Musixmatch,
    Happi,
    Genius,
}

impl Provider {
    pub const ALL: [Provider; 8] = [
        Provider::Lrclib,
        Provider::BetterLyrics,
        Provider::Karalyr,
        Provider::Synclrc,
        Provider::LyricsOvh,
        Provider::Musixmatch,
        Provider::Happi,
        Provider::Genius,
    ];

    pub fn display_name(self) -> &'static str {
        match self {
            Provider::Lrclib => "LRCLIB",
            Provider::BetterLyrics => "Better Lyrics",
            Provider::Karalyr => "Karalyr",
            Provider::Synclrc => "SyncLRC",
            Provider::LyricsOvh => "lyrics.ovh",
            Provider::Musixmatch => "Musixmatch",
            Provider::Happi => "Happi",
            Provider::Genius => "Genius",
        }
    }

    /// The environment variable holding the key a source needs, for those that need one.
    pub fn key_environment(self) -> Option<&'static str> {
        match self {
            Provider::Musixmatch => Some("NOCTORIUM_MUSIXMATCH_API_KEY"),
            Provider::Happi => Some("NOCTORIUM_HAPPI_API_KEY"),
            Provider::Genius => Some("NOCTORIUM_GENIUS_ACCESS_TOKEN"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Found,
    LinkOnly,
    NotFound,
    NeedsKey,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub provider: Provider,
    pub status: Status,
    pub lyrics: Option<Lyrics>,
    pub detail: String,
}

impl Outcome {
    fn none(provider: Provider, status: Status, detail: impl Into<String>) -> Outcome {
        Outcome { provider, status, lyrics: None, detail: detail.into() }
    }
}

impl Lyrics {
    /// The line being sung at `position_ms`, for synced lyrics.
    pub fn current_line(&self, position_ms: i64) -> Option<usize> {
        if !self.synced {
            return None;
        }
        self.lines
            .iter()
            .rposition(|line| line.start_ms.is_some_and(|start| start <= position_ms))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LyricsQuery {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_seconds: Option<i64>,
}

impl LyricsQuery {
    pub fn from(track: &Track) -> Self {
        let artist = track
            .artists
            .first()
            .map(|a| a.name.clone())
            .filter(|name| !name.starts_with("YouTube"))
            .unwrap_or_else(|| track.artist_line());
        LyricsQuery {
            title: clean_title(&track.title, &artist),
            artist,
            album: track.album.as_ref().map(|a| a.title.clone()),
            duration_seconds: track.duration_ms.map(|ms| ms / 1000),
        }
    }
}

/// The title without " (Official Video)", " | Lyrics" and the artist's name in front.
pub fn clean_title(title: &str, artist: &str) -> String {
    static SUFFIX: OnceLock<Regex> = OnceLock::new();
    static BRACKETED: OnceLock<Regex> = OnceLock::new();
    let suffix = SUFFIX.get_or_init(|| {
        Regex::new(r"(?i)\s*[|\-–—]\s*(official\s+)?(music\s+)?(video|audio|lyrics?|visuali[sz]er).*$").unwrap()
    });
    let bracketed = BRACKETED.get_or_init(|| {
        Regex::new(r"(?i)\s*[\[(](official\s+)?(music\s+)?(video|audio|lyrics?|visuali[sz]er)[\])].*$").unwrap()
    });
    let without = suffix.replace(title, "");
    let without = bracketed.replace(&without, "").trim().to_string();
    if artist.is_empty() {
        return without;
    }
    let leading = Regex::new(&format!(r"(?i)^{}\s*[-–—:]\s*", regex::escape(artist))).unwrap();
    leading.replace(&without, "").trim().to_string()
}

/// Lines from LRC text, or from plain text when the LRC has none.
pub fn from_lrc(synced: Option<&str>, plain: Option<&str>) -> (Vec<LyricLine>, bool) {
    static STAMP: OnceLock<Regex> = OnceLock::new();
    static ENHANCED: OnceLock<Regex> = OnceLock::new();
    let stamp = STAMP.get_or_init(|| Regex::new(r"\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?]").unwrap());
    let enhanced = ENHANCED.get_or_init(|| Regex::new(r"<\d{1,3}:\d{2}(?:[.:]\d{1,3})?>").unwrap());
    let mut timed: Vec<LyricLine> = Vec::new();
    for raw in synced.unwrap_or("").lines() {
        let text = enhanced.replace_all(&stamp.replace_all(raw, ""), "").trim().to_string();
        if text.is_empty() {
            continue;
        }
        for found in stamp.captures_iter(raw) {
            let minutes: i64 = found[1].parse().unwrap_or(0);
            let seconds: i64 = found[2].parse().unwrap_or(0);
            let fraction = found.get(3).map(|m| m.as_str()).unwrap_or("");
            let millis: i64 = match fraction.len() {
                0 => 0,
                1 => fraction.parse::<i64>().unwrap_or(0) * 100,
                2 => fraction.parse::<i64>().unwrap_or(0) * 10,
                _ => fraction[..3].parse().unwrap_or(0),
            };
            timed.push(LyricLine {
                text: text.clone(),
                start_ms: Some((minutes * 60 + seconds) * 1000 + millis),
            });
        }
    }
    if !timed.is_empty() {
        timed.sort_by_key(|l| l.start_ms);
        return (timed, true);
    }
    let lines = plain
        .unwrap_or("")
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.trim().is_empty())
        .map(|text| LyricLine {
            text: text.to_string(),
            start_ms: None,
        })
        .collect();
    (lines, false)
}

const USER_AGENT: &str = concat!(
    "Noctorium-Rust/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Noctorium/Noctorium-Desktop-Rust)"
);

fn get(url: &str, parameters: &[(&str, String)]) -> Result<Option<serde_json::Value>, String> {
    get_with(url, parameters, &[])
}

/// A JSON answer, None for a 404 -- and for 401 and 429 too, which mean "not for you, not now" rather than
/// that anything is wrong.
fn get_with(url: &str, parameters: &[(&str, String)], headers: &[(&str, &str)]) -> Result<Option<serde_json::Value>, String> {
    let mut request = ureq::get(url).set("User-Agent", USER_AGENT).timeout(Duration::from_secs(10));
    for (name, value) in parameters {
        request = request.query(name, value);
    }
    for (name, value) in headers {
        request = request.set(name, value);
    }
    let host = url.split('/').nth(2).unwrap_or(url).to_string();
    match request.call() {
        Ok(response) => response.into_json().map(Some).map_err(|e| e.to_string()),
        Err(ureq::Error::Status(404 | 401 | 429, _)) => Ok(None),
        Err(ureq::Error::Status(code, _)) => Err(format!("{host} returned {code}")),
        Err(e) => Err(format!("{host} could not be reached: {e}")),
    }
}

fn outcome(root: &serde_json::Value) -> Option<Lyrics> {
    if root["instrumental"].as_bool() == Some(true) {
        return Some(Lyrics {
            lines: vec![LyricLine {
                text: "♪ Instrumental track ♪".into(),
                start_ms: None,
            }],
            synced: false,
            source: "LRCLIB".into(),
            provider: Provider::Lrclib,
            link: None,
            outcomes: vec![],
            close_match: None,
        });
    }
    let (lines, synced) = from_lrc(root["syncedLyrics"].as_str(), root["plainLyrics"].as_str());
    // A line or two is a placeholder, not the song's lyrics.
    (lines.len() >= 3).then(|| Lyrics {
        lines,
        synced,
        source: "Lyrics from LRCLIB".into(),
        provider: Provider::Lrclib,
        link: None,
        outcomes: vec![],
        close_match: None,
    })
}

/// LRCLIB's lyrics in one of its answers, for the close match.
pub(crate) fn lrclib_lyrics(root: &serde_json::Value) -> Option<Lyrics> {
    outcome(root)
}

/// What LRCLIB's search answers to `parameters`, None when it could not be asked.
pub(crate) fn lrclib_search(parameters: &[(&str, String)]) -> Option<Vec<serde_json::Value>> {
    get("https://lrclib.net/api/search", parameters).ok().flatten().and_then(|found| found.as_array().cloned())
}

fn normalise(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn score(root: &serde_json::Value, lyrics: &Lyrics, query: &LyricsQuery) -> i64 {
    let (title, artist) = (
        normalise(root["trackName"].as_str().unwrap_or("")),
        normalise(root["artistName"].as_str().unwrap_or("")),
    );
    let (wanted_title, wanted_artist) = (normalise(&query.title), normalise(&query.artist));
    let title_score = if title == wanted_title {
        120
    } else if title.contains(&wanted_title) || wanted_title.contains(&title) {
        70
    } else {
        0
    };
    let artist_score = if artist == wanted_artist {
        90
    } else if artist.contains(&wanted_artist) || wanted_artist.contains(&artist) {
        55
    } else {
        0
    };
    let duration_score = match (query.duration_seconds, root["duration"].as_f64()) {
        (Some(wanted), Some(found)) => 35 - (found as i64 - wanted).abs().min(35),
        _ => 0,
    };
    let quality = (lyrics.lines.len() as i64).min(35) + if lyrics.synced { 30 } else { 10 };
    title_score + artist_score + duration_score + quality
}

/// The lyrics for `track` from every source at once: `preferred`'s when it has them, otherwise the first
/// source in Noctorium's order that does, with what all of them answered kept for choosing another. None
/// when nobody has them.
pub fn fetch(track: &Track, preferred: Provider) -> Result<Option<Lyrics>, String> {
    let query = LyricsQuery::from(track);
    let outcomes: Vec<Outcome> = std::thread::scope(|scope| {
        let handles: Vec<_> = Provider::ALL
            .iter()
            .map(|provider| {
                let query = &query;
                scope.spawn(move || {
                    // One retry, as Base gives each source, for a blip.
                    match ask(*provider, query) {
                        Err(_) => ask(*provider, query),
                        ok => ok,
                    }
                    .unwrap_or_else(|why| Outcome::none(*provider, Status::Error, why))
                })
            })
            .collect();
        handles
            .into_iter()
            .zip(Provider::ALL)
            .map(|(h, provider)| h.join().unwrap_or_else(|_| Outcome::none(provider, Status::Error, "failed")))
            .collect()
    });
    // No source has the song under its own name: the closest song that has lyrics, in LRCLIB's place.
    let mut outcomes = outcomes;
    if !outcomes.iter().any(|o| o.status == Status::Found) {
        if let Some(close) = crate::closematch::find(track) {
            for outcome in outcomes.iter_mut().filter(|o| o.provider == Provider::Lrclib) {
                *outcome = Outcome { provider: Provider::Lrclib, status: Status::Found, lyrics: Some(close.clone()), detail: close.source.clone() };
            }
        }
    }
    let found = |p: Provider| outcomes.iter().find(|o| o.provider == p && o.status == Status::Found).and_then(|o| o.lyrics.clone());
    let chosen = found(preferred)
        .or_else(|| Provider::ALL.iter().find_map(|p| found(*p)))
        .or_else(|| outcomes.iter().find(|o| o.status == Status::LinkOnly).and_then(|o| o.lyrics.clone()));
    if chosen.is_none() && outcomes.iter().all(|o| o.status == Status::Error) {
        return Err(outcomes.first().map(|o| o.detail.clone()).unwrap_or_default());
    }
    Ok(chosen.map(|mut lyrics| {
        lyrics.outcomes = outcomes;
        lyrics
    }))
}

/// One source's answer.
fn ask(provider: Provider, query: &LyricsQuery) -> Result<Outcome, String> {
    let key = provider.key_environment().and_then(|name| std::env::var(name).ok()).filter(|k| !k.trim().is_empty());
    let exact = || {
        let mut parameters = vec![("track_name", query.title.clone()), ("artist_name", query.artist.clone())];
        if let Some(album) = &query.album {
            parameters.push(("album_name", album.clone()));
        }
        if let Some(seconds) = query.duration_seconds {
            parameters.push(("duration", seconds.to_string()));
        }
        parameters
    };
    let found = |lines: Vec<LyricLine>, synced: bool, link: &str, source: &str| result(provider, lines, synced, Some(link.to_string()), source);
    let encode = crate::ytdlp::percent_encode;
    match provider {
        Provider::Lrclib => Ok(match lrclib(query)? {
            Some(lyrics) => Outcome { provider, status: Status::Found, lyrics: Some(lyrics), detail: String::new() },
            None => Outcome::none(provider, Status::NotFound, "No matching lyrics"),
        }),
        Provider::Karalyr => {
            let Some(root) = get("https://www.karalyr.com/api/get", &exact())? else {
                return Ok(Outcome::none(provider, Status::NotFound, "No karaoke match"));
            };
            let (lines, synced) = from_lrc(root["syncedLyrics"].as_str(), root["plainLyrics"].as_str());
            Ok(found(lines, synced, "https://karalyr.com", "Word-synced lyrics from Karalyr"))
        }
        Provider::Synclrc => {
            let mut parameters = vec![("track", query.title.clone()), ("artist", query.artist.clone())];
            if let Some(album) = &query.album {
                parameters.push(("album", album.clone()));
            }
            if let Some(seconds) = query.duration_seconds {
                parameters.push(("duration", seconds.to_string()));
            }
            let Some(root) = get("https://synclrc.dev/lyrics", &parameters)? else {
                return Ok(Outcome::none(provider, Status::NotFound, "No match"));
            };
            let synced_text = root["karaoke"].as_str().or(root["synced"].as_str());
            let (lines, synced) = from_lrc(synced_text, root["plain"].as_str());
            Ok(found(lines, synced, "https://synclrc.dev", "Lyrics from SyncLRC"))
        }
        Provider::LyricsOvh => {
            let url = format!("https://api.lyrics.ovh/v1/{}/{}", encode(&query.artist), encode(&query.title));
            let Some(root) = get(&url, &[])? else {
                return Ok(Outcome::none(provider, Status::NotFound, "No match"));
            };
            let (lines, synced) = from_lrc(None, root["lyrics"].as_str());
            Ok(found(lines, synced, "https://lyrics.ovh", "Lyrics from lyrics.ovh"))
        }
        Provider::BetterLyrics => {
            let mut parameters = vec![("s", query.title.clone()), ("a", query.artist.clone())];
            if let Some(album) = &query.album {
                parameters.push(("al", album.clone()));
            }
            if let Some(seconds) = query.duration_seconds {
                parameters.push(("d", seconds.to_string()));
            }
            let own_key = std::env::var("NOCTORIUM_BETTER_LYRICS_API_KEY").ok().filter(|k| !k.trim().is_empty());
            let headers: Vec<(&str, &str)> = own_key.as_deref().map(|k| vec![("X-API-Key", k)]).unwrap_or_default();
            let Some(root) = get_with("https://lyrics-api.boidu.dev/getLyrics", &parameters, &headers)? else {
                return Ok(Outcome::none(provider, Status::NotFound, "No cached match"));
            };
            let lines = from_ttml(root["ttml"].as_str().unwrap_or(""));
            let synced = lines.iter().any(|l| l.start_ms.is_some());
            Ok(found(lines, synced, "https://better-lyrics.boidu.dev", "Syllable-synced lyrics from Better Lyrics"))
        }
        Provider::Musixmatch => {
            let Some(key) = key else { return Ok(Outcome::none(provider, Status::NeedsKey, "NOCTORIUM_MUSIXMATCH_API_KEY")) };
            let parameters = [("q_track", query.title.clone()), ("q_artist", query.artist.clone()), ("apikey", key)];
            let Some(root) = get("https://api.musixmatch.com/ws/1.1/matcher.lyrics.get", &parameters)? else {
                return Ok(Outcome::none(provider, Status::NotFound, "No match"));
            };
            let lyrics = &root["message"]["body"]["lyrics"];
            let (lines, synced) = from_lrc(None, lyrics["lyrics_body"].as_str());
            let search = format!("https://www.musixmatch.com/search/{}", encode(&format!("{} {}", query.artist, query.title)));
            let attribution = lyrics["lyrics_copyright"].as_str().unwrap_or("Lyrics from Musixmatch").to_string();
            Ok(result(provider, lines, synced, Some(search), &attribution))
        }
        Provider::Happi => {
            let Some(key) = key else { return Ok(Outcome::none(provider, Status::NeedsKey, "NOCTORIUM_HAPPI_API_KEY")) };
            let parameters = [("artist", query.artist.clone()), ("track", query.title.clone())];
            let Some(root) = get_with("https://api.happi.dev/v1/lyrics", &parameters, &[("x-happi-token", &key)])? else {
                return Ok(Outcome::none(provider, Status::NotFound, "No match"));
            };
            let text = find_string(&root, &["lyrics", "plainLyrics", "text"]);
            let (lines, synced) = from_lrc(None, text.as_deref());
            Ok(found(lines, synced, "https://happi.dev", "Lyrics from Happi"))
        }
        Provider::Genius => {
            // Genius gives no lyrics through its API, only the page with them: a link, either way.
            let fallback = format!("https://genius.com/search?q={}", encode(&format!("{} {}", query.artist, query.title)));
            let url = match key {
                None => fallback,
                Some(token) => {
                    let bearer = format!("Bearer {token}");
                    get_with("https://api.genius.com/search", &[("q", format!("{} {}", query.artist, query.title))], &[("Authorization", &bearer)])?
                        .and_then(|root| root["response"]["hits"][0]["result"]["url"].as_str().map(String::from))
                        .unwrap_or(fallback)
                }
            };
            Ok(result(provider, vec![], false, Some(url), "Genius"))
        }
    }
}

/// An answer as Base judges one: a link alone when there are no lines, and nothing at all when there are so
/// few that they are more likely an error page than the song.
fn result(provider: Provider, lines: Vec<LyricLine>, synced: bool, link: Option<String>, source: &str) -> Outcome {
    let substantial = lines.is_empty() || (lines.len() >= 3 && lines.iter().map(|l| l.text.chars().count()).sum::<usize>() >= 40);
    if !substantial {
        return Outcome::none(provider, Status::NotFound, "Result was too short to trust");
    }
    if lines.is_empty() && link.is_none() {
        return Outcome::none(provider, Status::NotFound, "No matching lyrics");
    }
    let status = if lines.is_empty() { Status::LinkOnly } else { Status::Found };
    let lyrics = Lyrics { lines, synced, source: source.to_string(), provider, link, outcomes: vec![], close_match: None };
    Outcome { provider, status, lyrics: Some(lyrics), detail: String::new() }
}

fn find_string(value: &serde_json::Value, names: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => names
            .iter()
            .find_map(|n| map.get(*n).and_then(|v| v.as_str()).map(String::from))
            .or_else(|| map.values().find_map(|v| find_string(v, names))),
        _ => None,
    }
}

/// Lines from TTML, as Better Lyrics sends them: each `<p>` a line, starting at its `begin`.
pub fn from_ttml(ttml: &str) -> Vec<LyricLine> {
    static PARAGRAPH: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    static BEGIN: OnceLock<Regex> = OnceLock::new();
    let paragraph = PARAGRAPH.get_or_init(|| Regex::new(r"(?s)<(?:\w+:)?p\b([^>]*)>(.*?)</(?:\w+:)?p>").unwrap());
    let tag = TAG.get_or_init(|| Regex::new(r"<[^>]+>").unwrap());
    let begin = BEGIN.get_or_init(|| Regex::new(r#"\bbegin="([^"]+)""#).unwrap());
    let mut lines: Vec<LyricLine> = paragraph
        .captures_iter(ttml)
        .filter_map(|c| {
            let start = begin.captures(&c[1]).and_then(|b| ttml_time(&b[1]));
            let text = decode_entities(&tag.replace_all(&c[2], ""));
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then_some(LyricLine { text, start_ms: start })
        })
        .collect();
    lines.sort_by_key(|l| l.start_ms.unwrap_or(i64::MAX));
    lines
}

fn ttml_time(value: &str) -> Option<i64> {
    if !value.contains(':') {
        return value.trim_end_matches('s').parse::<f64>().ok().map(|s| (s * 1000.0) as i64);
    }
    let parts: Vec<&str> = value.split(':').collect();
    let seconds = match parts.as_slice() {
        [h, m, s] => h.parse::<f64>().ok()? * 3600.0 + m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?,
        [m, s] => m.parse::<f64>().ok()? * 60.0 + s.parse::<f64>().ok()?,
        _ => return None,
    };
    Some((seconds * 1000.0) as i64)
}

fn decode_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// LRCLIB: the exact lookup, then its search, the best candidate by score.
fn lrclib(query: &LyricsQuery) -> Result<Option<Lyrics>, String> {
    let query = query.clone();
    let mut exact = vec![("track_name", query.title.clone()), ("artist_name", query.artist.clone())];
    if let Some(album) = &query.album {
        exact.push(("album_name", album.clone()));
    }
    if let Some(seconds) = query.duration_seconds {
        exact.push(("duration", seconds.to_string()));
    }
    if let Some(found) = get("https://lrclib.net/api/get", &exact)?.as_ref().and_then(outcome) {
        return Ok(Some(found));
    }
    let search = get(
        "https://lrclib.net/api/search",
        &[("track_name", query.title.clone()), ("artist_name", query.artist.clone())],
    )?;
    let best = search
        .as_ref()
        .and_then(|s| s.as_array())
        .into_iter()
        .flatten()
        .filter_map(|root| outcome(root).map(|lyrics| (score(root, &lyrics, &query), lyrics)))
        .max_by_key(|(score, _)| *score)
        .map(|(_, mut lyrics)| {
            lyrics.source = "Matched from LRCLIB search".into();
            lyrics
        });
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synced_lines_are_read_with_their_times() {
        let lrc = "[00:12.50]First line\n[00:15.2]Second <00:16.00>line\n[01:02.345][01:30.00]Chorus\n[00:20.00]\n";
        let (lines, synced) = from_lrc(Some(lrc), None);
        assert!(synced);
        let times: Vec<_> = lines.iter().map(|l| l.start_ms.unwrap()).collect();
        assert_eq!(times, [12_500, 15_200, 62_345, 90_000]);
        assert_eq!(lines[1].text, "Second line");
    }

    #[test]
    fn plain_lyrics_stand_in_for_missing_synced_ones() {
        let (lines, synced) = from_lrc(Some(""), Some("one\n\ntwo\n"));
        assert!(!synced);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn the_current_line_follows_the_position() {
        let (lines, synced) = from_lrc(Some("[00:01.00]a\n[00:05.00]b\n[00:09.00]c"), None);
        let lyrics = Lyrics { lines, synced, source: String::new(), provider: Provider::Lrclib, link: None, outcomes: vec![], close_match: None };
        assert_eq!(lyrics.current_line(500), None);
        assert_eq!(lyrics.current_line(6_000), Some(1));
        assert_eq!(lyrics.current_line(60_000), Some(2));
    }

    #[test]
    fn ttml_lines_keep_their_start() {
        let ttml = r#"<tt><body><div><p begin="00:01.500" end="2"><span>Hello</span> <span>there &amp; you</span></p><p begin="0:00:00.250">First</p><p>Untimed</p></div></body></tt>"#;
        let lines = from_ttml(ttml);
        assert_eq!(lines[0].text, "First");
        assert_eq!(lines[0].start_ms, Some(250));
        assert_eq!(lines[1].text, "Hello there & you");
        assert_eq!(lines[1].start_ms, Some(1500));
        assert_eq!(lines[2].start_ms, None);
    }

    #[test]
    fn a_short_answer_is_not_trusted() {
        let short = vec![LyricLine { text: "404".into(), start_ms: None }];
        assert_eq!(result(Provider::Karalyr, short, false, None, "x").status, Status::NotFound);
        assert_eq!(result(Provider::Genius, vec![], false, Some("https://genius.com".into()), "Genius").status, Status::LinkOnly);
    }

    #[test]
    fn titles_lose_what_is_not_the_song() {
        assert_eq!(clean_title("Rick Astley - Never Gonna Give You Up (Official Video)", "Rick Astley"), "Never Gonna Give You Up");
        assert_eq!(clean_title("Song | Official Audio", "X"), "Song");
        assert_eq!(clean_title("Plain", "X"), "Plain");
    }
}
