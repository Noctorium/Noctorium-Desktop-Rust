//! Deezer: its catalogue, its charts and a listener's public profile, read through Deezer's own public API
//! as Noctorium-Base's `DeezerClient` reads it, and matched on YouTube Music to be played -- Deezer's audio
//! plays in Deezer's own players and nowhere else.

use crate::domain::{Album, Artist, Playlist, ProviderType, Track};
use regex::Regex;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const API: &str = "https://api.deezer.com";
/// Deezer takes fifty requests in five seconds; forty leaves room.
const PER_WINDOW: usize = 40;
const WINDOW: Duration = Duration::from_secs(5);
const PAGE: usize = 100;

fn wait_for_turn() {
    static SENT: OnceLock<Mutex<std::collections::VecDeque<Instant>>> = OnceLock::new();
    let sent = SENT.get_or_init(|| Mutex::new(Default::default()));
    loop {
        let mut sent = sent.lock().unwrap();
        while sent.front().is_some_and(|t| t.elapsed() >= WINDOW) {
            sent.pop_front();
        }
        if sent.len() < PER_WINDOW {
            sent.push_back(Instant::now());
            return;
        }
        let wait = WINDOW.saturating_sub(sent.front().unwrap().elapsed());
        drop(sent);
        std::thread::sleep(wait.max(Duration::from_millis(5)));
    }
}

fn read(url: &str) -> Result<Option<serde_json::Value>, String> {
    for attempt in 0..2 {
        wait_for_turn();
        let body: serde_json::Value = match ureq::get(url).set("Accept", "application/json").timeout(Duration::from_secs(15)).call() {
            Ok(r) => r.into_json().map_err(|_| "Deezer sent a reply this could not read.".to_string())?,
            Err(ureq::Error::Status(404, _)) => return Err("Deezer has nothing at that address.".into()),
            Err(ureq::Error::Status(code, _)) => return Err(format!("Deezer answered HTTP {code}.")),
            Err(e) => return Err(format!("Could not reach Deezer: {e}")),
        };
        match body["error"]["code"].as_i64() {
            None => return Ok(Some(body)),
            // "No data": an answer, and an empty one.
            Some(800) => return Ok(None),
            Some(4) if attempt == 0 => std::thread::sleep(WINDOW),
            Some(_) => {
                let message = body["error"]["message"].as_str().unwrap_or("refused the request");
                return Err(format!("Deezer: {message}"));
            }
        }
    }
    Err("Deezer is being asked too much at once. Try again in a moment.".into())
}

fn list(url: &str) -> Result<Vec<serde_json::Value>, String> {
    Ok(read(url)?.and_then(|b| b["data"].as_array().cloned()).unwrap_or_default())
}

/// Every page of a paged answer, following `next`, up to `limit` items.
fn pages(first: &str, limit: usize) -> Result<Vec<serde_json::Value>, String> {
    let mut gathered = Vec::new();
    let mut next = Some(first.to_string());
    for _ in 0..60 {
        let Some(url) = next.take() else { break };
        if gathered.len() >= limit {
            break;
        }
        match read(&url) {
            Ok(Some(page)) => {
                gathered.extend(page["data"].as_array().cloned().unwrap_or_default());
                next = page["next"].as_str().map(String::from);
            }
            Ok(None) => break,
            Err(why) if gathered.is_empty() => return Err(why),
            Err(_) => break,
        }
    }
    gathered.truncate(limit);
    Ok(gathered)
}

fn cover_of(item: &serde_json::Value) -> Option<String> {
    ["cover_xl", "cover_big", "picture_xl", "picture_big", "cover_medium", "picture_medium"]
        .iter()
        .find_map(|k| item[*k].as_str().filter(|s| !s.is_empty()).map(String::from))
}

fn artist_of(item: &serde_json::Value) -> Option<Artist> {
    let name = item["name"].as_str().filter(|n| !n.trim().is_empty())?;
    let id = item["id"].as_i64().map(|i| i.to_string()).unwrap_or_else(|| name.to_string());
    Some(Artist { id, name: name.to_string(), provider: ProviderType::Deezer })
}

pub fn track_of(item: &serde_json::Value, on_album: Option<&Album>) -> Option<Track> {
    if item["type"].as_str().is_some_and(|t| t != "track") {
        return None;
    }
    let id = item["id"].as_i64()?.to_string();
    let title = item["title"].as_str().filter(|t| !t.trim().is_empty())?.to_string();
    let mut artists: Vec<Artist> = item["contributors"].as_array().into_iter().flatten().filter_map(artist_of).collect();
    if artists.is_empty() {
        artists.extend(artist_of(&item["artist"]));
    }
    artists.dedup_by(|a, b| a.id == b.id);
    if artists.is_empty() {
        return None;
    }
    let album = on_album.cloned().or_else(|| {
        let entry = &item["album"];
        let title = entry["title"].as_str().filter(|t| !t.is_empty())?;
        Some(Album {
            id: entry["id"].as_i64().map(|i| i.to_string()).unwrap_or_else(|| title.to_string()),
            title: title.to_string(),
            artists: artists.iter().take(1).cloned().collect(),
            provider: ProviderType::Deezer,
            artwork_url: cover_of(entry),
        })
    });
    Some(Track {
        provider: ProviderType::Deezer,
        source_url: item["link"].as_str().map(String::from).unwrap_or_else(|| format!("https://www.deezer.com/track/{id}")),
        id,
        title,
        artwork_url: album.as_ref().and_then(|a| a.artwork_url.clone()),
        album,
        artists,
        duration_ms: item["duration"].as_i64().filter(|d| *d > 0).map(|d| d * 1000),
    })
}

fn playlist_of(item: &serde_json::Value, kind: &str) -> Option<Playlist> {
    let id = item["id"].as_i64()?.to_string();
    let title = item["title"].as_str().or(item["name"].as_str())?.to_string();
    Some(Playlist {
        source_url: Some(format!("https://www.deezer.com/{kind}/{id}")),
        id,
        title,
        provider: ProviderType::Deezer,
        owner_name: item["user"]["name"].as_str().or(item["artist"]["name"].as_str()).or(item["creator"]["name"].as_str()).map(String::from),
        artwork_url: cover_of(item),
        tracks: vec![],
        track_count: item["nb_tracks"].as_i64(),
    })
}

pub fn search(query: &str, limit: usize) -> Result<Vec<Track>, String> {
    let q = crate::ytdlp::percent_encode(query.trim());
    Ok(list(&format!("{API}/search?q={q}&limit={limit}"))?.iter().filter_map(|t| track_of(t, None)).collect())
}

/// The charts: songs, and playlists.
pub fn chart() -> Result<(Vec<Track>, Vec<Playlist>), String> {
    let chart = read(&format!("{API}/chart/0?limit=20"))?.unwrap_or_default();
    let tracks = chart["tracks"]["data"].as_array().into_iter().flatten().filter_map(|t| track_of(t, None)).collect();
    let playlists = chart["playlists"]["data"].as_array().into_iter().flatten().filter_map(|p| playlist_of(p, "playlist")).collect();
    Ok((tracks, playlists))
}

/// A playlist or an album by its link, `deezer.com/[xx/]playlist/123` or `/album/123`.
pub fn open_link(url: &str) -> Option<Result<Playlist, String>> {
    static LINK: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| Regex::new(r"deezer\.com/(?:[a-z]{2}(?:-[a-z]{2})?/)?(playlist|album)/(\d+)").unwrap());
    let found = link.captures(url)?;
    let (kind, id) = (found[1].to_string(), found[2].to_string());
    Some((|| {
        let page = read(&format!("{API}/{kind}/{id}"))?.ok_or("Deezer has nothing at that address.")?;
        let mut playlist = playlist_of(&page, &kind).ok_or("Deezer did not say what that is.")?;
        let album = (kind == "album").then(|| Album {
            id: id.clone(),
            title: playlist.title.clone(),
            artists: artist_of(&page["artist"]).into_iter().collect(),
            provider: ProviderType::Deezer,
            artwork_url: playlist.artwork_url.clone(),
        });
        playlist.tracks = pages(&format!("{API}/{kind}/{id}/tracks?limit={PAGE}"), 5_000)?
            .iter()
            .filter_map(|t| track_of(t, album.as_ref()))
            .collect();
        playlist.track_count = Some(playlist.tracks.len() as i64);
        Ok(playlist)
    })())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub id: i64,
    pub name: String,
    pub loved: Vec<Track>,
    pub playlists: Vec<Playlist>,
}

/// A listener's public profile, by its link or its number: their loved tracks and their playlists.
pub fn profile(text: &str) -> Result<Profile, String> {
    static PROFILE: OnceLock<Regex> = OnceLock::new();
    let pattern = PROFILE.get_or_init(|| Regex::new(r"deezer\.com/(?:[a-z]{2}(?:-[a-z]{2})?/)?profile/(\d+)").unwrap());
    let trimmed = text.trim();
    let id: i64 = if trimmed.chars().all(|c| c.is_ascii_digit()) && !trimmed.is_empty() {
        trimmed.parse().map_err(|_| "That is not a Deezer profile.")?
    } else {
        pattern.captures(trimmed).and_then(|c| c[1].parse().ok()).ok_or("Paste the link to your Deezer profile, deezer.com/profile/…")?
    };
    let user = read(&format!("{API}/user/{id}"))?.ok_or("Deezer has no profile by that number.")?;
    let loved = pages(&format!("{API}/user/{id}/tracks?limit={PAGE}"), 5_000)?.iter().filter_map(|t| track_of(t, None)).collect();
    let playlists = pages(&format!("{API}/user/{id}/playlists?limit={PAGE}"), 500)?
        .iter()
        .filter(|p| !p["is_loved_track"].as_bool().unwrap_or(false))
        .filter_map(|p| playlist_of(p, "playlist"))
        .collect();
    Ok(Profile { id, name: user["name"].as_str().unwrap_or("Deezer listener").to_string(), loved, playlists })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_track_is_read_with_its_contributors() {
        let item = json!({"id": 3135556, "title": "Harder, Better, Faster, Stronger", "duration": 224,
            "link": "https://www.deezer.com/track/3135556", "type": "track",
            "contributors": [{"id": 27, "name": "Daft Punk"}],
            "album": {"id": 302127, "title": "Discovery", "cover_xl": "https://e-cdns-images.dzcdn.net/x/1000x1000.jpg"}});
        let track = track_of(&item, None).unwrap();
        assert_eq!(track.provider, ProviderType::Deezer);
        assert_eq!(track.artist_line(), "Daft Punk");
        assert_eq!(track.duration_ms, Some(224_000));
        assert_eq!(track.album.as_ref().unwrap().title, "Discovery");
        assert!(track.artwork_url.is_some());
    }

    #[test]
    fn links_are_recognised() {
        assert!(open_link("https://www.youtube.com/playlist?list=x").is_none());
        assert!(profile("not a profile").is_err());
    }
}
