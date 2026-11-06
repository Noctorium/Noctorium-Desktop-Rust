//! SoundCloud's own interface, `api-v2.soundcloud.com`, signed out: search, related tracks, and where a
//! track's audio is. As Noctorium-Base's `SoundCloudClientIdProvider` and `SoundCloudRelatedClient` read it.
//!
//! Each answer is one HTTP request, where yt-dlp is a Python started each time -- ten to forty seconds on a
//! busy two-core machine against well under one. yt-dlp stays the fallback for when this does not answer.
//! Tracks SoundCloud only lets subscribers hear in full (its "SNIP" policy) are left out: they play thirty
//! seconds and stop, which reads as a broken player.

use crate::domain::{Artist, ProviderType, Track};
use regex::Regex;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const API: &str = "https://api-v2.soundcloud.com";
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

fn cache() -> &'static Mutex<Option<String>> {
    static CACHED: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    CACHED.get_or_init(|| Mutex::new(None))
}

fn fetch_text(url: &str) -> Option<String> {
    ureq::get(url)
        .set("User-Agent", BROWSER)
        .timeout(Duration::from_secs(15))
        .call()
        .ok()?
        .into_string()
        .ok()
}

/// The client id SoundCloud's own pages use, read out of the scripts the home page loads: there is none to
/// be issued, and it changes often enough that keeping one would break. Read once a run.
pub fn client_id() -> Option<String> {
    if let Some(id) = cache().lock().unwrap().clone() {
        return Some(id);
    }
    let page = fetch_text("https://soundcloud.com/")?;
    let script = Regex::new(r#"src="(https://[^"]+\.js[^"]*)""#).ok()?;
    let id_pattern = Regex::new(r#"client_id[=:]"?([A-Za-z0-9]{28,40})"?"#).ok()?;
    let scripts: Vec<String> = script.captures_iter(&page).map(|c| c[1].to_string()).collect();
    for url in scripts.iter().rev() {
        if let Some(found) = fetch_text(url).and_then(|body| id_pattern.captures(&body).map(|c| c[1].to_string())) {
            *cache().lock().unwrap() = Some(found.clone());
            return Some(found);
        }
    }
    None
}

fn forget_client_id() {
    *cache().lock().unwrap() = None;
}

/// A GET with the client id, trying a fresh id once if the old one has been retired.
fn get_json(path_and_query: &str) -> Result<serde_json::Value, String> {
    for attempt in 0..2 {
        let id = client_id().ok_or("SoundCloud could not be reached.")?;
        let separator = if path_and_query.contains('?') { '&' } else { '?' };
        let url = format!("{API}{path_and_query}{separator}client_id={id}");
        match ureq::get(&url).set("User-Agent", BROWSER).timeout(Duration::from_secs(15)).call() {
            Ok(response) => return response.into_json().map_err(|e| e.to_string()),
            Err(ureq::Error::Status(401 | 403, _)) if attempt == 0 => forget_client_id(),
            Err(e) => return Err(format!("SoundCloud did not answer: {e}")),
        }
    }
    Err("SoundCloud refused its own client id.".into())
}

/// A cover at a size worth showing: SoundCloud's links name their size, and "large" is 100 pixels.
pub fn bigger_artwork(url: &str) -> String {
    url.replace("-large.", "-t500x500.")
}

/// A track from the interface, or None for one that cannot be played in full here.
pub fn track_of(item: &serde_json::Value) -> Option<Track> {
    let policy = item["policy"].as_str().unwrap_or("").to_uppercase();
    if policy == "SNIP" || policy == "BLOCK" {
        return None;
    }
    let number = item["id"].as_u64()?;
    let page = item["permalink_url"].as_str().filter(|p| !p.is_empty())?;
    let title = item["title"].as_str().filter(|t| !t.trim().is_empty())?;
    let user = &item["user"];
    let name = user["username"].as_str().filter(|n| !n.is_empty()).unwrap_or("SoundCloud");
    let artwork = item["artwork_url"].as_str().or(user["avatar_url"].as_str()).map(bigger_artwork);
    Some(Track {
        provider: ProviderType::Soundcloud,
        id: number.to_string(),
        title: title.to_string(),
        artists: vec![Artist { id: format!("SOUNDCLOUD:{name}"), name: name.to_string(), provider: ProviderType::Soundcloud }],
        album: None,
        duration_ms: item["full_duration"].as_i64().or(item["duration"].as_i64()).filter(|d| *d > 0),
        artwork_url: artwork,
        source_url: page.to_string(),
    })
}

fn tracks_in(page: &serde_json::Value) -> Vec<Track> {
    page["collection"].as_array().into_iter().flatten().filter_map(track_of).collect()
}

pub fn search(query: &str, limit: usize) -> Result<Vec<Track>, String> {
    let q = crate::ytdlp::percent_encode(query.trim());
    // Asked for more than wanted, since previews are left out.
    let page = get_json(&format!("/search/tracks?q={q}&limit={}&linked_partitioning=1", (limit * 3).min(200)))?;
    Ok(tracks_in(&page).into_iter().take(limit).collect())
}

/// SoundCloud's own "related tracks" for one.
pub fn related(track: &Track, limit: usize) -> Result<Vec<Track>, String> {
    let number = match track.id.parse::<u64>() {
        Ok(n) => n,
        Err(_) => resolve(&track.source_url)?["id"].as_u64().ok_or("SoundCloud does not know that track.")?,
    };
    let page = get_json(&format!("/tracks/{number}/related?limit={limit}&linked_partitioning=1"))?;
    Ok(tracks_in(&page))
}

fn resolve(page_url: &str) -> Result<serde_json::Value, String> {
    let address = page_url.split(['#', '?']).next().unwrap_or(page_url);
    get_json(&format!("/resolve?url={}", crate::ytdlp::percent_encode(address)))
}

/// A track's tracks or a set's tracks, by its link.
pub fn playlist(page_url: &str) -> Result<(String, Option<String>, Vec<Track>), String> {
    let set = resolve(page_url)?;
    let title = set["title"].as_str().unwrap_or("Playlist").to_string();
    let owner = set["user"]["username"].as_str().map(String::from);
    let items: Vec<serde_json::Value> = set["tracks"].as_array().cloned().unwrap_or_default();
    // A set lists its first few tracks in full and the rest as ids only; those are asked for in fifties.
    let mut tracks: Vec<Track> = Vec::new();
    let mut missing: Vec<u64> = Vec::new();
    for item in &items {
        if item["title"].is_string() {
            if let Some(t) = track_of(item) {
                tracks.push(t);
            }
        } else if let Some(id) = item["id"].as_u64() {
            missing.push(id);
        }
    }
    for chunk in missing.chunks(50) {
        let ids = chunk.iter().map(u64::to_string).collect::<Vec<_>>().join("%2C");
        if let Ok(serde_json::Value::Array(found)) = get_json(&format!("/tracks?ids={ids}")) {
            tracks.extend(found.iter().filter_map(track_of));
        }
    }
    // In the set's own order, whichever request each came from.
    let order: Vec<u64> = items.iter().filter_map(|i| i["id"].as_u64()).collect();
    tracks.sort_by_key(|t| order.iter().position(|id| t.id == id.to_string()).unwrap_or(usize::MAX));
    Ok((title, owner, tracks))
}

/// Where a track's audio can be read: its progressive MP3 if it has one, its HLS stream otherwise.
pub fn stream(page_url: &str) -> Result<String, String> {
    let track = resolve(page_url)?;
    let transcodings: Vec<serde_json::Value> = track["media"]["transcodings"].as_array().cloned().unwrap_or_default();
    let pick = |protocol: &str| {
        transcodings
            .iter()
            .find(|t| t["format"]["protocol"].as_str() == Some(protocol) && !t["snipped"].as_bool().unwrap_or(false))
            .and_then(|t| t["url"].as_str())
            .map(String::from)
    };
    let address = pick("progressive").or_else(|| pick("hls")).ok_or("SoundCloud has no stream for this track.")?;
    let authorization = track["track_authorization"].as_str().unwrap_or("");
    let path = address.strip_prefix(API).unwrap_or(&address);
    let separator = if path.contains('?') { '&' } else { '?' };
    let answer = get_json(&format!("{path}{separator}track_authorization={authorization}"))?;
    answer["url"].as_str().map(String::from).ok_or_else(|| "SoundCloud gave no stream address.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_track_is_read_and_a_preview_left_out() {
        let item = json!({"id": 123, "title": "Song", "permalink_url": "https://soundcloud.com/a/song",
            "duration": 30000, "full_duration": 200000, "policy": "ALLOW",
            "artwork_url": "https://i1.sndcdn.com/artworks-x-large.jpg", "user": {"username": "Band"}});
        let track = track_of(&item).unwrap();
        assert_eq!(track.id, "123");
        assert_eq!(track.artist_line(), "Band");
        assert_eq!(track.duration_ms, Some(200_000));
        assert_eq!(track.artwork_url.as_deref(), Some("https://i1.sndcdn.com/artworks-x-t500x500.jpg"));
        let snip = json!({"id": 1, "title": "x", "permalink_url": "https://soundcloud.com/a/b", "policy": "SNIP"});
        assert!(track_of(&snip).is_none());
    }
}
