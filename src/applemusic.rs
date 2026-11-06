//! Apple Music: a catalogue and a library to read, never a source to play from -- its audio plays only in
//! Apple's own players, so its songs are matched on YouTube Music. Read through the interface Apple's own
//! web player uses, as Noctorium-Base's `AppleMusicClient` reads it: with the token that web player carries
//! in its scripts for the catalogue, and the listener's own `media-user-token` for their library.

use crate::domain::{Album, Artist, Playlist, ProviderType, Track};
use regex::Regex;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const API: &str = "https://api.music.apple.com/v1";
const ORIGIN: &str = "https://music.apple.com";
const WEB_PLAYER: &str = "https://music.apple.com/us/browse";
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

fn fetch_text(url: &str) -> Result<String, String> {
    ureq::get(url)
        .set("User-Agent", BROWSER)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| format!("Apple Music could not be reached: {e}"))?
        .into_string()
        .map_err(|e| e.to_string())
}

/// The token Apple's web player hands MusicKit, read out of its scripts once a run.
fn developer_token() -> Result<String, String> {
    static TOKEN: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    let kept = TOKEN.get_or_init(|| Mutex::new(None));
    if let Some(token) = kept.lock().unwrap().clone() {
        return Ok(token);
    }
    let page = fetch_text(WEB_PLAYER)?;
    let script = Regex::new(r#"<script[^>]+src="(/assets/[^"]+\.js)""#).unwrap();
    let jwt = Regex::new(r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+").unwrap();
    let mut scripts: Vec<String> = script.captures_iter(&page).map(|c| c[1].to_string()).collect();
    scripts.dedup();
    scripts.sort_by_key(|s| !s.contains("/index"));
    for path in scripts.iter().take(12) {
        let Ok(text) = fetch_text(&format!("{ORIGIN}{path}")) else { continue };
        if let Some(found) = jwt.find(&text) {
            let token = found.as_str().to_string();
            *kept.lock().unwrap() = Some(token.clone());
            return Ok(token);
        }
    }
    Err("Apple's web player has changed, and Noctorium could not find what it reads Apple Music with.".into())
}

fn read(url: &str, user_token: Option<&str>) -> Result<serde_json::Value, String> {
    let url = if url.starts_with("http") { url.to_string() } else { format!("https://api.music.apple.com{url}") };
    let mut request = ureq::get(&url)
        .set("Authorization", &format!("Bearer {}", developer_token()?))
        .set("Origin", ORIGIN)
        .set("User-Agent", BROWSER)
        .timeout(Duration::from_secs(20));
    if let Some(token) = user_token {
        request = request.set("Media-User-Token", token);
    }
    match request.call() {
        Ok(r) => r.into_json().map_err(|e| e.to_string()),
        Err(ureq::Error::Status(401 | 403, _)) if user_token.is_some() => {
            Err("Apple Music no longer accepts this sign-in. Sign in again in Services › Apple Music.".into())
        }
        Err(ureq::Error::Status(404, _)) => Err("Apple Music has nothing at that address.".into()),
        Err(e) => Err(format!("Apple Music did not answer: {e}")),
    }
}

fn pages(first: &str, user_token: Option<&str>, limit: usize) -> Result<Vec<serde_json::Value>, String> {
    let mut items = Vec::new();
    let mut next = Some(first.to_string());
    while let Some(url) = next.take() {
        if items.len() >= limit {
            break;
        }
        let page = read(&url, user_token)?;
        items.extend(page["data"].as_array().cloned().unwrap_or_default());
        next = page["next"].as_str().map(String::from);
    }
    items.truncate(limit);
    Ok(items)
}

fn artwork(attributes: &serde_json::Value) -> Option<String> {
    attributes["artwork"]["url"]
        .as_str()
        .map(|u| u.replace("{w}", "600").replace("{h}", "600").replace("{c}", "bb").replace("{f}", "jpg"))
}

pub fn song_of(item: &serde_json::Value, storefront: &str) -> Option<Track> {
    let kind = item["type"].as_str()?;
    if kind != "songs" && kind != "library-songs" {
        return None;
    }
    let a = &item["attributes"];
    let title = a["name"].as_str().filter(|t| !t.is_empty())?;
    let artist = a["artistName"].as_str().filter(|t| !t.is_empty())?;
    let catalog = a["playParams"]["catalogId"].as_str();
    let id = if kind == "library-songs" { catalog.or(item["id"].as_str())? } else { item["id"].as_str()? };
    let artists = vec![Artist { id: artist.to_string(), name: artist.to_string(), provider: ProviderType::AppleMusic }];
    let album = a["albumName"].as_str().filter(|t| !t.is_empty()).map(|title| Album {
        id: title.to_string(),
        title: title.to_string(),
        artists: artists.clone(),
        provider: ProviderType::AppleMusic,
        artwork_url: artwork(a),
    });
    Some(Track {
        provider: ProviderType::AppleMusic,
        id: id.to_string(),
        title: title.to_string(),
        artists,
        album,
        duration_ms: a["durationInMillis"].as_i64().filter(|d| *d > 0),
        artwork_url: artwork(a),
        source_url: a["url"].as_str().map(String::from).unwrap_or_else(|| format!("{ORIGIN}/{storefront}/song/{id}")),
    })
}

fn playlist_of(item: &serde_json::Value, storefront: &str) -> Option<Playlist> {
    let id = item["id"].as_str()?;
    let a = &item["attributes"];
    let kind = item["type"].as_str().unwrap_or("");
    let path = match kind {
        "albums" => format!("{ORIGIN}/{storefront}/album/{id}"),
        "library-playlists" => format!("{ORIGIN}/library/playlist/{id}"),
        _ => format!("{ORIGIN}/{storefront}/playlist/{id}"),
    };
    Some(Playlist {
        id: id.to_string(),
        title: a["name"].as_str()?.to_string(),
        provider: ProviderType::AppleMusic,
        owner_name: a["curatorName"].as_str().or(a["artistName"].as_str()).map(String::from),
        artwork_url: artwork(a),
        tracks: vec![],
        source_url: Some(a["url"].as_str().map(String::from).unwrap_or(path)),
        track_count: a["trackCount"].as_i64(),
    })
}

pub fn search(query: &str, storefront: &str, limit: usize) -> Result<Vec<Track>, String> {
    let term = crate::ytdlp::percent_encode(query.trim());
    let found = read(&format!("{API}/catalog/{storefront}/search?term={term}&types=songs&limit={}", limit.min(25)), None)?;
    Ok(found["results"]["songs"]["data"].as_array().into_iter().flatten().filter_map(|s| song_of(s, storefront)).collect())
}

/// What is most played in `storefront` just now: songs, and playlists.
pub fn charts(storefront: &str) -> Result<(Vec<Track>, Vec<Playlist>), String> {
    let found = read(&format!("{API}/catalog/{storefront}/charts?types=songs,playlists&limit=20"), None)?;
    let results = &found["results"];
    let songs = results["songs"][0]["data"].as_array().into_iter().flatten().filter_map(|s| song_of(s, storefront)).collect();
    let playlists = results["playlists"][0]["data"].as_array().into_iter().flatten().filter_map(|p| playlist_of(p, storefront)).collect();
    Ok((songs, playlists))
}

/// `music.apple.com/<storefront>/album/<name>/<id>` or `/playlist/<name>/<id>`, with its songs.
pub fn open_link(url: &str, user_token: Option<&str>) -> Option<Result<Playlist, String>> {
    static LINK: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| Regex::new(r"music\.apple\.com/(?:([a-z]{2})/)?(album|playlist)/(?:[^/?#]+/)?([A-Za-z0-9.]+)").unwrap());
    let library = url.contains("music.apple.com/library/playlist/");
    let found = link.captures(url)?;
    let storefront = found.get(1).map(|m| m.as_str()).unwrap_or("us").to_string();
    let (kind, id) = (found[2].to_string(), found[3].to_string());
    Some((|| {
        let (head, songs) = if library {
            let token = user_token.ok_or("A library playlist needs the Apple Music sign-in.")?;
            let head = read(&format!("{API}/me/library/playlists/{id}"), Some(token))?;
            (head, pages(&format!("{API}/me/library/playlists/{id}/tracks?limit=100"), Some(token), 5_000)?)
        } else {
            let head = read(&format!("{API}/catalog/{storefront}/{kind}s/{id}"), None)?;
            (head, pages(&format!("{API}/catalog/{storefront}/{kind}s/{id}/tracks?limit=100"), None, 5_000)?)
        };
        let item = &head["data"][0];
        let mut playlist = playlist_of(item, &storefront).ok_or("Apple Music did not say what that is.")?;
        playlist.tracks = songs.iter().filter_map(|s| song_of(s, &storefront)).collect();
        playlist.track_count = Some(playlist.tracks.len() as i64);
        Ok(playlist)
    })())
}

pub fn is_apple(url: &str) -> bool {
    url.to_lowercase().contains("music.apple.com/")
}

/// The listener's own Apple Music: where it is, its playlists and its songs.
pub struct Library {
    pub storefront: String,
    pub playlists: Vec<Playlist>,
    pub songs: Vec<Track>,
}

pub fn library(user_token: &str) -> Result<Library, String> {
    let storefront = read(&format!("{API}/me/storefront"), Some(user_token))?["data"][0]["id"].as_str().unwrap_or("us").to_string();
    let playlists = pages(&format!("{API}/me/library/playlists?limit=100"), Some(user_token), 500)?
        .iter()
        .filter_map(|p| playlist_of(p, &storefront))
        .collect();
    let songs = pages(&format!("{API}/me/library/songs?limit=100"), Some(user_token), 2_000)?
        .iter()
        .filter_map(|s| song_of(s, &storefront))
        .collect();
    Ok(Library { storefront, playlists, songs })
}

/// The listener's `media-user-token`, from a browser's cookies, where they are signed in to music.apple.com.
pub fn user_token_from(cookies: &[crate::accounts::Cookie]) -> Result<String, String> {
    cookies
        .iter()
        .find(|c| c.name == "media-user-token" && c.domain.ends_with("apple.com"))
        .map(|c| c.value.clone())
        .ok_or_else(|| "That browser is not signed in to music.apple.com.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_catalogue_song_is_read() {
        let item = json!({"id": "1440833098", "type": "songs", "attributes": {
            "name": "Get Lucky", "artistName": "Daft Punk", "albumName": "Random Access Memories",
            "durationInMillis": 369000, "url": "https://music.apple.com/us/album/get-lucky/1440833060?i=1440833098",
            "artwork": {"url": "https://is1-ssl.mzstatic.com/x/{w}x{h}{c}.{f}"}}});
        let song = song_of(&item, "us").unwrap();
        assert_eq!(song.provider, ProviderType::AppleMusic);
        assert_eq!(song.duration_ms, Some(369_000));
        assert_eq!(song.artwork_url.as_deref(), Some("https://is1-ssl.mzstatic.com/x/600x600bb.jpg"));
        assert_eq!(song.album.unwrap().title, "Random Access Memories");
    }
}
