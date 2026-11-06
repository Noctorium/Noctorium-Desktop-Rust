//! YouTube Music's own interface, the one its web player reads: for search and radio, signed out.
//!
//! yt-dlp's YouTube Music search answers with a title and an id and nothing else -- every row would read
//! "YouTube Music" where the artist belongs, with no length -- so this asks the way Noctorium-Base's
//! `YouTubeMusic` client does: the WEB_REMIX client, the "Songs" filter, and rows read column by column.
//! yt-dlp stays the fallback for when this does not answer.

use crate::domain::{Album, Artist, ProviderType, Track};
use regex::Regex;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const ORIGIN: &str = "https://music.youtube.com";
/// YouTube Music's own filter for the "Songs" tab, so albums, artists and playlists stay out.
const SONGS_ONLY: &str = "EgWKAQIIAWoKEAkQBRAKEAMQBA==";
const FIELD_SEPARATOR: &str = "•";
const NAME_SEPARATORS: [&str; 3] = [",", "&", "/"];
/// Used only when the page cannot be read for the current one; the interface accepts versions for months.
const FALLBACK_VERSION: &str = "1.20260601.01.00";
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

#[derive(Clone, Debug)]
struct Keys {
    api_key: Option<String>,
    version: String,
}

/// The key and client version the page publishes, read once: they change often enough that a build-time
/// copy would go stale, and seldom enough that once a run is plenty.
fn keys() -> Keys {
    static CACHED: OnceLock<Mutex<Option<Keys>>> = OnceLock::new();
    let cache = CACHED.get_or_init(|| Mutex::new(None));
    if let Some(keys) = cache.lock().unwrap().clone() {
        return keys;
    }
    let page = ureq::get(&format!("{ORIGIN}/"))
        .set("User-Agent", BROWSER)
        .set("Accept-Language", "en")
        // Past the consent page Europe is shown first, which carries no keys.
        .set("Cookie", "SOCS=CAI")
        .timeout(Duration::from_secs(15))
        .call()
        .ok()
        .and_then(|r| r.into_string().ok())
        .unwrap_or_default();
    let find = |pattern: &str| Regex::new(pattern).ok()?.captures(&page).map(|c| c[1].to_string());
    let keys = Keys {
        api_key: find(r#""INNERTUBE_API_KEY":"([^"]+)""#),
        version: find(r#""INNERTUBE_CLIENT_VERSION":"([^"]+)""#).unwrap_or_else(|| FALLBACK_VERSION.into()),
    };
    *cache.lock().unwrap() = Some(keys.clone());
    keys
}

/// The client version YouTube Music's page names, for signed-in requests made elsewhere.
pub fn client_version() -> String {
    keys().version
}

fn post(endpoint: &str, mut body: serde_json::Value) -> Result<serde_json::Value, String> {
    let keys = keys();
    body["context"] = serde_json::json!({
        "client": {"clientName": "WEB_REMIX", "clientVersion": keys.version, "hl": "en", "gl": "US"}
    });
    let mut url = format!("{ORIGIN}/youtubei/v1/{endpoint}?prettyPrint=false");
    if let Some(key) = keys.api_key {
        url.push_str("&key=");
        url.push_str(&key);
    }
    ureq::post(&url)
        .set("User-Agent", BROWSER)
        .set("Origin", ORIGIN)
        .set("Referer", &format!("{ORIGIN}/"))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(20))
        .send_string(&body.to_string())
        .map_err(|e| format!("YouTube Music did not answer: {e}"))?
        .into_json()
        .map_err(|e| format!("YouTube Music's answer could not be read: {e}"))
}

pub fn search_songs(query: &str, limit: usize) -> Result<Vec<Track>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(vec![]);
    }
    let body = post("search", serde_json::json!({"query": query, "params": SONGS_ONLY}))?;
    Ok(parse_songs(&body).into_iter().take(limit).collect())
}

/// The radio YouTube Music plays after `video_id`.
pub fn radio(video_id: &str, limit: usize) -> Result<Vec<Track>, String> {
    let body = post(
        "next",
        serde_json::json!({"videoId": video_id, "playlistId": format!("RDAMVM{video_id}"), "isAudioOnly": true}),
    )?;
    Ok(parse_radio(&body)
        .into_iter()
        .filter(|t| t.id != video_id)
        .take(limit)
        .collect())
}

fn collect<'a>(value: &'a serde_json::Value, name: &str, into: &mut Vec<&'a serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(found) = map.get(name).filter(|v| v.is_object()) {
                into.push(found);
            }
            for child in map.values() {
                collect(child, name, into);
            }
        }
        serde_json::Value::Array(list) => list.iter().for_each(|child| collect(child, name, into)),
        _ => {}
    }
}

fn runs_text(node: &serde_json::Value) -> Option<String> {
    let text: String = node["runs"]
        .as_array()?
        .iter()
        .filter_map(|r| r["text"].as_str())
        .collect();
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

/// Runs split into fields at " • ", with the separators between several artists dropped.
fn fields_of(runs: &serde_json::Value) -> Vec<Vec<String>> {
    let mut fields: Vec<Vec<String>> = vec![vec![]];
    for run in runs.as_array().into_iter().flatten() {
        let text = run["text"].as_str().unwrap_or("").trim();
        if text == FIELD_SEPARATOR {
            fields.push(vec![]);
        } else if !text.is_empty() && !NAME_SEPARATORS.contains(&text) {
            fields.last_mut().unwrap().push(text.to_string());
        }
    }
    fields.into_iter().filter(|f| !f.is_empty()).collect()
}

/// `3:04` or `1:02:03` as milliseconds, or None when the text is not a length.
pub fn duration_text_to_ms(text: &str) -> Option<i64> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let numbers: Vec<i64> = parts.iter().map(|p| p.trim().parse::<i64>().ok()).collect::<Option<_>>()?;
    if numbers[1..].iter().any(|n| *n >= 60) || numbers.iter().any(|n| *n < 0) {
        return None;
    }
    Some(numbers.iter().fold(0, |total, n| total * 60 + n) * 1000)
}

/// A cover at a size worth showing: Google's image links name their size, and the rows ask for 60 or 120.
pub fn bigger_artwork(url: &str) -> String {
    static SIZE: OnceLock<Regex> = OnceLock::new();
    let size = SIZE.get_or_init(|| Regex::new(r"=w\d+-h\d+").unwrap());
    if url.contains("googleusercontent.com") || url.contains("ggpht.com") {
        size.replace(url, "=w544-h544").into_owned()
    } else {
        url.to_string()
    }
}

fn last_thumbnail(list: &serde_json::Value) -> Option<String> {
    list.as_array()?.last()?["url"].as_str().map(bigger_artwork)
}

fn song(video_id: String, title: String, names: Vec<String>, album: Option<String>, duration_ms: Option<i64>, artwork: Option<String>) -> Track {
    let provider = ProviderType::YoutubeMusic;
    let artists: Vec<Artist> = names
        .into_iter()
        .map(|name| Artist { id: format!("YOUTUBE_MUSIC:{name}"), name, provider })
        .collect();
    Track {
        provider,
        album: album.map(|title| Album {
            id: format!("YOUTUBE_MUSIC:album:{title}"),
            title,
            artists: artists.clone(),
            provider,
            artwork_url: artwork.clone(),
        }),
        source_url: format!("{ORIGIN}/watch?v={video_id}"),
        id: video_id,
        title,
        artists,
        duration_ms,
        artwork_url: artwork,
    }
}

/// The songs of a search answer, each once. The columns are positional: the title first, then the
/// artists, the album and the length as fields with " • " between them.
pub fn parse_songs(body: &serde_json::Value) -> Vec<Track> {
    let mut rows = Vec::new();
    collect(body, "musicResponsiveListItemRenderer", &mut rows);
    let mut seen = std::collections::HashSet::new();
    rows.into_iter()
        .filter_map(|row| {
            let columns: Vec<&serde_json::Value> = row["flexColumns"]
                .as_array()?
                .iter()
                .map(|c| &c["musicResponsiveListItemFlexColumnRenderer"])
                .collect();
            let title = runs_text(&columns.first()?["text"])?;
            let video_id = row["playlistItemData"]["videoId"]
                .as_str()
                .or_else(|| {
                    columns.first()?["text"]["runs"].as_array()?.iter().find_map(|r| r["navigationEndpoint"]["watchEndpoint"]["videoId"].as_str())
                })?
                .to_string();
            let mut fields = fields_of(&columns.get(1)?["text"]["runs"]);
            let duration_ms = fields
                .last()
                .filter(|f| f.len() == 1)
                .and_then(|f| duration_text_to_ms(&f[0]));
            if duration_ms.is_some() {
                fields.pop();
            }
            let names = fields.first().cloned().filter(|n| !n.is_empty())?;
            let album = fields.get(1).map(|f| f.join(", ")).filter(|a| !a.is_empty());
            let artwork = last_thumbnail(&row["thumbnail"]["musicThumbnailRenderer"]["thumbnail"]["thumbnails"]);
            seen.insert(video_id.clone())
                .then(|| song(video_id, title, names, album, duration_ms, artwork))
        })
        .collect()
}

/// The songs of a radio panel, in order, each once.
pub fn parse_radio(body: &serde_json::Value) -> Vec<Track> {
    let mut rows = Vec::new();
    collect(body, "playlistPanelVideoRenderer", &mut rows);
    let mut seen = std::collections::HashSet::new();
    rows.into_iter()
        .filter_map(|row| {
            let video_id = row["videoId"].as_str().filter(|v| !v.is_empty())?.to_string();
            let title = runs_text(&row["title"])?;
            let fields = fields_of(&row["longBylineText"]["runs"]);
            let names = fields.first().cloned()?;
            let album = fields
                .get(1)
                .map(|f| f.join(", "))
                .filter(|a| !a.is_empty() && !a.chars().all(|c| c.is_ascii_digit()));
            let duration_ms = runs_text(&row["lengthText"]).and_then(|t| duration_text_to_ms(&t));
            let artwork = last_thumbnail(&row["thumbnail"]["thumbnails"]);
            seen.insert(video_id.clone())
                .then(|| song(video_id, title, names, album, duration_ms, artwork))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn runs(texts: &[&str]) -> serde_json::Value {
        json!({"runs": texts.iter().map(|t| json!({"text": t})).collect::<Vec<_>>()})
    }

    #[test]
    fn a_search_row_is_read_column_by_column() {
        let body = json!({"contents": [{"musicResponsiveListItemRenderer": {
            "playlistItemData": {"videoId": "abc123"},
            "thumbnail": {"musicThumbnailRenderer": {"thumbnail": {"thumbnails": [
                {"url": "https://lh3.googleusercontent.com/x=w60-h60-l90-rj"},
                {"url": "https://lh3.googleusercontent.com/x=w120-h120-l90-rj"}]}}},
            "flexColumns": [
                {"musicResponsiveListItemFlexColumnRenderer": {"text": runs(&["Get Lucky"])}},
                {"musicResponsiveListItemFlexColumnRenderer": {"text": runs(&[
                    "Daft Punk", ", ", "Pharrell Williams", " • ", "Random Access Memories", " • ", "6:09"])}}
            ]}}]});
        let songs = parse_songs(&body);
        assert_eq!(songs.len(), 1);
        let s = &songs[0];
        assert_eq!(s.title, "Get Lucky");
        assert_eq!(s.artist_line(), "Daft Punk, Pharrell Williams");
        assert_eq!(s.album.as_ref().unwrap().title, "Random Access Memories");
        assert_eq!(s.duration_ms, Some(369_000));
        assert_eq!(s.artwork_url.as_deref(), Some("https://lh3.googleusercontent.com/x=w544-h544-l90-rj"));
        assert_eq!(s.source_url, "https://music.youtube.com/watch?v=abc123");
    }

    #[test]
    fn a_radio_row_drops_the_year_where_the_album_would_be() {
        let body = json!({"contents": [{"playlistPanelVideoRenderer": {
            "videoId": "r1", "title": runs(&["Reckoner"]),
            "longBylineText": runs(&["Radiohead", " • ", "2007"]),
            "lengthText": runs(&["4:50"])}}]});
        let songs = parse_radio(&body);
        assert_eq!(songs[0].artist_line(), "Radiohead");
        assert!(songs[0].album.is_none());
        assert_eq!(songs[0].duration_ms, Some(290_000));
    }

    #[test]
    fn lengths_are_read_and_other_text_is_not() {
        assert_eq!(duration_text_to_ms("3:04"), Some(184_000));
        assert_eq!(duration_text_to_ms("1:02:03"), Some(3_723_000));
        assert_eq!(duration_text_to_ms("2007"), None);
        assert_eq!(duration_text_to_ms("3:75"), None);
    }
}
