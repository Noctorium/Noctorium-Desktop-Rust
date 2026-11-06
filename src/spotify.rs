//! Spotify: a library to read, never a source to play from -- its songs are matched on YouTube Music when
//! played. Three ways in, as Noctorium-Base has them:
//!
//! - public playlists and albums by their link, read from Spotify's own embed page, no sign-in;
//! - the "Download your data" export, its playlists and Liked Songs imported as playlists of the listener's own;
//! - the listener's account, signed in with Spotify's own sign-in page (OAuth with PKCE, Noctorium's own
//!   application, a reply to the computer's loopback address), for search, playlists, Liked Songs, top and
//!   recent songs, and the heart.

use crate::domain::{Album, Artist, LocalPlaylist, Playlist, ProviderType, Track};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";
const API: &str = "https://api.spotify.com/v1";
const BUILT_IN_CLIENT_ID: &str = "675f916f3c524a56b6e3196b14b48935";
const REDIRECT_PORT: u16 = 8888;
const REDIRECT_PATH: &str = "/noctorium/spotify";
const SCOPES: &str = "playlist-read-private playlist-read-collaborative user-library-read user-library-modify playlist-modify-public playlist-modify-private user-top-read user-read-recently-played user-read-playback-state user-modify-playback-state";

fn song(id: &str, title: &str, artist: &str, album: Option<&Album>, duration_ms: Option<i64>, artwork: Option<String>) -> Track {
    Track {
        provider: ProviderType::Spotify,
        id: id.to_string(),
        title: title.to_string(),
        artists: artist
            .split(", ")
            .filter(|a| !a.trim().is_empty())
            .map(|a| Artist { id: a.to_string(), name: a.trim().to_string(), provider: ProviderType::Spotify })
            .collect(),
        album: album.cloned(),
        duration_ms,
        artwork_url: artwork.or_else(|| album.and_then(|a| a.artwork_url.clone())),
        source_url: format!("https://open.spotify.com/track/{id}"),
    }
}

// ---- public pages ----------------------------------------------------------------------------------

/// A playlist or album from the data in Spotify's embed page. Spotify shows a public playlist's first
/// hundred songs there; a longer one needs the account.
pub fn parse_embed(html: &str, kind: &str, id: &str) -> Result<Playlist, String> {
    static NEXT_DATA: OnceLock<Regex> = OnceLock::new();
    let pattern = NEXT_DATA.get_or_init(|| Regex::new(r#"(?is)<script[^>]*\bid\s*=\s*["']__NEXT_DATA__["'][^>]*>(.*?)</script>"#).unwrap());
    let changed = "Spotify's public page has changed and Noctorium cannot read it yet.";
    let script = pattern.captures(html).ok_or(changed)?;
    let data: serde_json::Value = serde_json::from_str(&script[1]).map_err(|_| changed)?;
    let props = &data["props"]["pageProps"];
    if matches!(props["status"].as_i64(), Some(403 | 404 | 410)) || props["state"]["data"]["entity"].is_null() {
        return Err(if kind == "album" {
            "Spotify has no album at that address.".into()
        } else {
            "This playlist is private or does not exist. Only public playlists can be read without signing in.".into()
        });
    }
    let entity = &props["state"]["data"]["entity"];
    let title = entity["name"].as_str().or(entity["title"].as_str()).ok_or(changed)?.trim().to_string();
    let owner = entity["subtitle"].as_str().unwrap_or("").trim().to_string();
    let cover = entity["coverArt"]["sources"]
        .as_array()
        .and_then(|s| s.iter().max_by_key(|x| x["width"].as_i64().unwrap_or(0)))
        .and_then(|s| s["url"].as_str())
        .map(String::from);
    let album = (kind == "album").then(|| Album {
        id: id.to_string(),
        title: title.clone(),
        artists: vec![Artist { id: owner.clone(), name: owner.clone(), provider: ProviderType::Spotify }],
        provider: ProviderType::Spotify,
        artwork_url: cover.clone(),
    });
    let mut seen = std::collections::HashSet::new();
    let tracks: Vec<Track> = entity["trackList"]
        .as_array()
        .ok_or(changed)?
        .iter()
        .filter(|e| e["entityType"].as_str().is_none_or(|t| t == "track"))
        .filter_map(|e| {
            let uri = e["uri"].as_str()?;
            let track_id = uri.strip_prefix("spotify:track:")?;
            let title = e["title"].as_str()?.trim();
            let credit = e["subtitle"].as_str()?.replace('\u{a0}', " ");
            seen.insert(track_id.to_string()).then(|| {
                song(track_id, title, credit.trim(), album.as_ref(), e["duration"].as_i64().filter(|d| *d > 0), None)
            })
        })
        .collect();
    Ok(Playlist {
        id: format!("{kind}:{id}"),
        title,
        provider: ProviderType::Spotify,
        owner_name: Some(owner).filter(|o| !o.is_empty()),
        artwork_url: cover,
        track_count: Some(tracks.len() as i64),
        tracks,
        source_url: Some(format!("https://open.spotify.com/{kind}/{id}")),
    })
}

/// `open.spotify.com/[intl-xx/]playlist/<id>` or `/album/<id>`: the kind and the id.
pub fn link_ids(url: &str) -> Option<(String, String)> {
    static LINK: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| Regex::new(r"open\.spotify\.com/(?:intl-[a-z-]+/)?(playlist|album)/([A-Za-z0-9]{22})").unwrap());
    let found = link.captures(url)?;
    Some((found[1].to_string(), found[2].to_string()))
}

pub fn is_spotify(url: &str) -> bool {
    url.to_lowercase().contains("open.spotify.com/")
}

/// A public playlist or album by its link. With the account, a playlist is read whole; without, its first hundred.
pub fn open_link(url: &str, account: Option<&Account>) -> Option<Result<Playlist, String>> {
    let (kind, id) = link_ids(url)?;
    if let (Some(account), "playlist") = (account, kind.as_str()) {
        if let Ok(found) = account.playlist(&id) {
            return Some(Ok(found));
        }
    }
    Some((|| {
        let html = ureq::get(&format!("https://open.spotify.com/embed/{kind}/{id}"))
            .set("User-Agent", BROWSER)
            .set("Accept", "text/html,application/xhtml+xml")
            .timeout(Duration::from_secs(20))
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(404 | 410, _) => "This playlist is private or does not exist.".to_string(),
                ureq::Error::Status(429, _) => "Spotify asked Noctorium to slow down. Try again in a few minutes.".to_string(),
                other => format!("Could not reach Spotify's public page: {other}"),
            })?
            .into_string()
            .map_err(|e| e.to_string())?;
        parse_embed(&html, &kind, &id)
    })())
}

// ---- the data export -------------------------------------------------------------------------------

/// What Spotify's "Download your data" holds that Noctorium imports: playlists, and Liked Songs.
#[derive(Default, Debug)]
pub struct Export {
    pub playlists: Vec<(String, Vec<Track>)>,
    pub liked: Vec<Track>,
}

fn exported_song(title: Option<&str>, artist: Option<&str>, album: Option<&str>, uri: Option<&str>) -> Option<Track> {
    let id = uri?.strip_prefix("spotify:track:")?;
    let title = title?.trim();
    let artist = artist?.trim();
    if title.is_empty() || artist.is_empty() {
        return None;
    }
    let album = album.filter(|a| !a.trim().is_empty()).map(|a| Album {
        id: a.to_string(),
        title: a.to_string(),
        artists: vec![],
        provider: ProviderType::Spotify,
        artwork_url: None,
    });
    Some(song(id, title, artist, album.as_ref(), None, None))
}

fn read_export_json(name: &str, text: &str, export: &mut Export) {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')) else { return };
    let lower = name.to_lowercase();
    let file = lower.rsplit(['/', '\\']).next().unwrap_or(&lower);
    let mut playlist = |entry: &serde_json::Value| {
        let name = entry["name"].as_str().map(str::trim).filter(|n| !n.is_empty()).unwrap_or("Untitled playlist");
        let items = entry["items"].as_array().or(entry["tracks"].as_array());
        let mut seen = std::collections::HashSet::new();
        let songs: Vec<Track> = items
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let t = if item["track"].is_object() { &item["track"] } else { item };
                exported_song(
                    t["trackName"].as_str().or(t["track"].as_str()),
                    t["artistName"].as_str().or(t["artist"].as_str()),
                    t["albumName"].as_str().or(t["album"].as_str()),
                    t["trackUri"].as_str().or(t["uri"].as_str()),
                )
            })
            .filter(|s| seen.insert(s.id.clone()))
            .collect();
        export.playlists.push((name.to_string(), songs));
    };
    if file.starts_with("playlist") {
        if let Some(list) = root["playlists"].as_array() {
            list.iter().for_each(&mut playlist);
        } else if let Some(list) = root.as_array() {
            list.iter().filter(|e| e.get("items").is_some() || e.get("tracks").is_some()).for_each(&mut playlist);
        }
    }
    if file.starts_with("yourlibrary") {
        for entry in root["tracks"].as_array().into_iter().flatten() {
            if let Some(s) = exported_song(
                entry["track"].as_str().or(entry["trackName"].as_str()),
                entry["artist"].as_str().or(entry["artistName"].as_str()),
                entry["album"].as_str().or(entry["albumName"].as_str()),
                entry["uri"].as_str().or(entry["trackUri"].as_str()),
            ) {
                if !export.liked.iter().any(|l| l.id == s.id) {
                    export.liked.push(s);
                }
            }
        }
    }
}

/// Reads the export: the zip Spotify emails, the folder it unpacks to, or one of its JSON files.
pub fn read_export(path: &std::path::Path) -> Result<Export, String> {
    let mut export = Export::default();
    let mut read_file = |name: &str, text: &str| {
        if name.to_lowercase().ends_with(".json") {
            read_export_json(name, text, &mut export);
        }
    };
    if path.is_dir() {
        let mut stack = vec![path.to_path_buf()];
        while let Some(folder) = stack.pop() {
            for entry in std::fs::read_dir(&folder).map_err(|e| e.to_string())?.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if let Ok(text) = std::fs::read_to_string(&p) {
                    read_file(&p.to_string_lossy(), &text);
                }
            }
        }
    } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip")) {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("That zip could not be opened: {e}"))?;
        for index in 0..archive.len() {
            let Ok(mut entry) = archive.by_index(index) else { continue };
            let Ok(name) = entry.name().map(|n| n.to_string()) else { continue };
            // Large files in an export are streaming history, not what is imported.
            if !name.to_lowercase().ends_with(".json") || entry.size() > 64 << 20 {
                continue;
            }
            let mut text = String::new();
            if entry.read_to_string(&mut text).is_ok() {
                read_file(&name, &text);
            }
        }
    } else {
        let text = std::fs::read_to_string(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        read_file(&path.to_string_lossy(), &text);
    }
    if export.playlists.is_empty() && export.liked.is_empty() {
        return Err("Nothing to import was found. Choose the zip Spotify emails, or the Playlist1.json and YourLibrary.json inside it.".into());
    }
    Ok(export)
}

/// The export, as the listener's own playlists: each made, or brought up to date if an earlier import made
/// it -- found again by the same mark the Kotlin players leave. Answers how many were made or updated.
pub fn import_into(export: Export, playlists: &mut Vec<LocalPlaylist>, now: i64) -> (usize, usize) {
    let mut made = 0;
    let mut updated = 0;
    let lists = std::iter::once(("Liked Songs".to_string(), "liked-songs".to_string(), export.liked))
        .chain(export.playlists.into_iter().map(|(name, songs)| (name.clone(), format!("playlist:{name}"), songs)));
    for (title, key, songs) in lists {
        if songs.is_empty() {
            continue;
        }
        let mark = serde_json::json!({"source": "spotify-data", "key": key, "importedAtEpochSeconds": now});
        let earlier = playlists.iter_mut().find(|p| {
            p.imported_from.as_ref().is_some_and(|m| m["source"] == "spotify-data" && m["key"].as_str() == Some(key.as_str()))
        });
        match earlier {
            Some(existing) => {
                existing.tracks = songs;
                existing.updated_at_epoch_seconds = now;
                existing.imported_from = Some(mark);
                updated += 1;
            }
            None => {
                let mut fresh = LocalPlaylist::create(&format!("{title} (Spotify)"), now);
                fresh.tracks = songs;
                fresh.imported_from = Some(mark);
                playlists.push(fresh);
                made += 1;
            }
        }
    }
    (made, updated)
}

// ---- the account -----------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub access: String,
    pub refresh: String,
    pub expires_at: i64,
}

fn client_id() -> String {
    std::env::var("NOCTORIUM_SPOTIFY_CLIENT_ID").ok().filter(|c| !c.trim().is_empty()).unwrap_or_else(|| BUILT_IN_CLIENT_ID.into())
}

fn redirect_uri() -> String {
    format!("http://127.0.0.1:{REDIRECT_PORT}{REDIRECT_PATH}")
}

/// Signs in: opens Spotify's own sign-in page in the browser with `open`, and waits up to five minutes for
/// its answer at the loopback address. The secret half of PKCE never leaves this function.
pub fn sign_in(open: impl Fn(&str)) -> Result<Tokens, String> {
    let (verifier, challenge) = crate::oauth::pkce();
    let state = crate::oauth::state();
    let url = format!(
        "https://accounts.spotify.com/authorize?client_id={}&response_type=code&redirect_uri={}&code_challenge_method=S256&code_challenge={challenge}&state={state}&scope={}",
        client_id(),
        crate::ytdlp::percent_encode(&redirect_uri()),
        crate::ytdlp::percent_encode(SCOPES).replace('+', "%20"),
    );
    let code = crate::oauth::wait_for_code("Spotify", &url, REDIRECT_PORT, REDIRECT_PATH, &state, open)?;
    exchange(&[
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", &redirect_uri()),
        ("client_id", &client_id()),
        ("code_verifier", &verifier),
    ], None)
}

fn exchange(form: &[(&str, &str)], keep_refresh: Option<&str>) -> Result<Tokens, String> {
    let answer: serde_json::Value = match ureq::post("https://accounts.spotify.com/api/token").timeout(Duration::from_secs(20)).send_form(form) {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(_, r)) => {
            let body: serde_json::Value = r.into_json().unwrap_or_default();
            return Err(format!("Spotify refused the sign-in: {}", body["error_description"].as_str().or(body["error"].as_str()).unwrap_or("no reason given")));
        }
        Err(e) => return Err(format!("Could not reach Spotify: {e}")),
    };
    Ok(Tokens {
        access: answer["access_token"].as_str().ok_or("Spotify sent no access token.")?.to_string(),
        refresh: answer["refresh_token"].as_str().map(String::from).or(keep_refresh.map(String::from)).ok_or("Spotify sent no refresh token.")?,
        expires_at: crate::store::now_seconds() + answer["expires_in"].as_i64().unwrap_or(3600) - 60,
    })
}

/// The account signed in, where the services' search and links can reach it.
pub fn current() -> Option<Account> {
    signed_in().lock().unwrap().clone()
}

pub fn set_current(account: Option<Account>) {
    *signed_in().lock().unwrap() = account;
}

fn signed_in() -> &'static std::sync::Mutex<Option<Account>> {
    static ACCOUNT: OnceLock<std::sync::Mutex<Option<Account>>> = OnceLock::new();
    ACCOUNT.get_or_init(|| std::sync::Mutex::new(None))
}

/// A signed-in Spotify account. Its tokens are refreshed when they run out; whoever holds it saves them again.
#[derive(Clone, Debug)]
pub struct Account {
    pub tokens: std::sync::Arc<std::sync::Mutex<Tokens>>,
}

impl Account {
    pub fn new(tokens: Tokens) -> Account {
        Account { tokens: std::sync::Arc::new(std::sync::Mutex::new(tokens)) }
    }

    fn access(&self) -> Result<String, String> {
        let mut tokens = self.tokens.lock().unwrap();
        if crate::store::now_seconds() >= tokens.expires_at {
            let refresh = tokens.refresh.clone();
            *tokens = exchange(&[("grant_type", "refresh_token"), ("refresh_token", &refresh), ("client_id", &client_id())], Some(&refresh))
                .map_err(|_| "Spotify needs signing in again.".to_string())?;
        }
        Ok(tokens.access.clone())
    }

    fn call(&self, method: &str, path: &str) -> Result<serde_json::Value, String> {
        self.call_with(method, path, None)
    }

    fn call_with(&self, method: &str, path: &str, body: Option<&serde_json::Value>) -> Result<serde_json::Value, String> {
        let url = if path.starts_with("http") { path.to_string() } else { format!("{API}{path}") };
        for attempt in 0..2 {
            let request = ureq::request(method, &url)
                .set("Authorization", &format!("Bearer {}", self.access()?))
                .timeout(Duration::from_secs(20));
            let reply = match body {
                Some(body) => request.send_json(body.clone()),
                None if method == "PUT" || method == "POST" => request.set("Content-Length", "0").send_string(""),
                None => request.call(),
            };
            match reply {
                Ok(r) => return Ok(r.into_json().unwrap_or(serde_json::Value::Null)),
                Err(ureq::Error::Status(401, _)) if attempt == 0 => self.tokens.lock().unwrap().expires_at = 0,
                Err(ureq::Error::Status(429, r)) if attempt == 0 => {
                    let wait = r.header("Retry-After").and_then(|v| v.parse().ok()).unwrap_or(2u64).min(10);
                    std::thread::sleep(Duration::from_secs(wait));
                }
                Err(ureq::Error::Status(401, _)) => return Err("Spotify needs signing in again.".into()),
                Err(e) => return Err(format!("Spotify did not answer: {e}")),
            }
        }
        Err("Spotify did not answer.".into())
    }

    /// Every page of a paged answer, up to `limit` items.
    fn pages(&self, first: &str, limit: usize) -> Result<Vec<serde_json::Value>, String> {
        let mut items = Vec::new();
        let mut next = Some(first.to_string());
        while let Some(url) = next.take() {
            if items.len() >= limit {
                break;
            }
            let page = self.call("GET", &url)?;
            items.extend(page["items"].as_array().cloned().unwrap_or_default());
            next = page["next"].as_str().map(String::from);
        }
        items.truncate(limit);
        Ok(items)
    }

    /// The devices Spotify can play on now: the listener's phone, computer, speakers. (id, name, active)
    pub fn devices(&self) -> Result<Vec<(String, String, bool)>, String> {
        let answer = self.call("GET", "/me/player/devices")?;
        Ok(answer["devices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| Some((d["id"].as_str()?.to_string(), d["name"].as_str()?.to_string(), d["is_active"].as_bool().unwrap_or(false))))
            .collect())
    }

    /// Plays a song in the listener's Spotify, on `device` or whichever is active. Needs Premium.
    pub fn play(&self, track_id: &str, device: &str) -> Result<(), String> {
        let path = if device.is_empty() { "/me/player/play".to_string() } else { format!("/me/player/play?device_id={device}") };
        self.call_with("PUT", &path, Some(&serde_json::json!({"uris": [format!("spotify:track:{track_id}")]})))
            .map(|_| ())
            .map_err(|why| {
                if why.contains("404") {
                    "Spotify has no device to play on: open Spotify on one first.".into()
                } else if why.contains("403") {
                    "Playing on Spotify needs Premium.".into()
                } else {
                    why
                }
            })
    }

    pub fn display_name(&self) -> Result<String, String> {
        let me = self.call("GET", "/me")?;
        Ok(me["display_name"].as_str().or(me["id"].as_str()).unwrap_or("Spotify").to_string())
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, String> {
        let q = crate::ytdlp::percent_encode(query.trim());
        let answer = self.call("GET", &format!("/search?type=track&limit={}&q={q}", limit.min(50)))?;
        Ok(answer["tracks"]["items"].as_array().into_iter().flatten().filter_map(api_track).collect())
    }

    pub fn playlists(&self) -> Result<Vec<Playlist>, String> {
        Ok(self
            .pages("/me/playlists?limit=50", 500)?
            .iter()
            .filter_map(|p| {
                let id = p["id"].as_str()?;
                Some(Playlist {
                    id: format!("playlist:{id}"),
                    title: p["name"].as_str()?.to_string(),
                    provider: ProviderType::Spotify,
                    owner_name: p["owner"]["display_name"].as_str().map(String::from),
                    artwork_url: p["images"][0]["url"].as_str().map(String::from),
                    tracks: vec![],
                    source_url: Some(format!("https://open.spotify.com/playlist/{id}")),
                    track_count: p["tracks"]["total"].as_i64(),
                })
            })
            .collect())
    }

    pub fn playlist(&self, id: &str) -> Result<Playlist, String> {
        let info = self.call("GET", &format!("/playlists/{id}?fields=name,owner(display_name),images"))?;
        let tracks: Vec<Track> = self
            .pages(&format!("/playlists/{id}/tracks?limit=100"), 5_000)?
            .iter()
            .filter_map(|item| api_track(&item["track"]))
            .collect();
        Ok(Playlist {
            id: format!("playlist:{id}"),
            title: info["name"].as_str().unwrap_or("Playlist").to_string(),
            provider: ProviderType::Spotify,
            owner_name: info["owner"]["display_name"].as_str().map(String::from),
            artwork_url: info["images"][0]["url"].as_str().map(String::from),
            track_count: Some(tracks.len() as i64),
            tracks,
            source_url: Some(format!("https://open.spotify.com/playlist/{id}")),
        })
    }

    pub fn liked_songs(&self, limit: usize) -> Result<Vec<Track>, String> {
        Ok(self.pages("/me/tracks?limit=50", limit)?.iter().filter_map(|i| api_track(&i["track"])).collect())
    }

    pub fn top_tracks(&self) -> Result<Vec<Track>, String> {
        Ok(self.call("GET", "/me/top/tracks?limit=20")?["items"].as_array().into_iter().flatten().filter_map(api_track).collect())
    }

    pub fn recently_played(&self) -> Result<Vec<Track>, String> {
        let mut seen = std::collections::HashSet::new();
        Ok(self.call("GET", "/me/player/recently-played?limit=50")?["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| api_track(&i["track"]))
            .filter(|t| seen.insert(t.id.clone()))
            .take(20)
            .collect())
    }

    pub fn set_saved(&self, track_id: &str, saved: bool) -> Result<(), String> {
        self.call(if saved { "PUT" } else { "DELETE" }, &format!("/me/tracks?ids={track_id}")).map(|_| ())
    }
}

fn api_track(item: &serde_json::Value) -> Option<Track> {
    let id = item["id"].as_str()?;
    let artist = item["artists"].as_array()?.iter().filter_map(|a| a["name"].as_str()).collect::<Vec<_>>().join(", ");
    let album = item["album"]["name"].as_str().map(|title| Album {
        id: item["album"]["id"].as_str().unwrap_or(title).to_string(),
        title: title.to_string(),
        artists: vec![],
        provider: ProviderType::Spotify,
        artwork_url: item["album"]["images"][0]["url"].as_str().map(String::from),
    });
    Some(song(id, item["name"].as_str()?, &artist, album.as_ref(), item["duration_ms"].as_i64(), None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_embed_page_is_read() {
        let html = r#"<html><script id="__NEXT_DATA__" type="application/json">{"props":{"pageProps":{"state":{"data":{"entity":{
            "type":"playlist","name":"Mix","subtitle":"Someone","coverArt":{"sources":[{"url":"small","width":60},{"url":"big","width":640}]},
            "trackList":[{"uri":"spotify:track:4uLU6hMCjMI75M1A2tKUQC","title":"Song","subtitle":"Band, Other","duration":200000,"entityType":"track"},
                         {"uri":"spotify:episode:x","title":"Talk","subtitle":"Pod","entityType":"episode"}]}}}}}}</script></html>"#;
        let playlist = parse_embed(html, "playlist", "37i9dQZF1DXcBWIGoYBM5M").unwrap();
        assert_eq!(playlist.title, "Mix");
        assert_eq!(playlist.artwork_url.as_deref(), Some("big"));
        assert_eq!(playlist.tracks.len(), 1);
        assert_eq!(playlist.tracks[0].artists.len(), 2);
        assert_eq!(playlist.tracks[0].duration_ms, Some(200_000));
    }

    #[test]
    fn links_give_their_kind_and_id() {
        assert_eq!(
            link_ids("https://open.spotify.com/intl-de/playlist/37i9dQZF1DXcBWIGoYBM5M?si=x"),
            Some(("playlist".into(), "37i9dQZF1DXcBWIGoYBM5M".into()))
        );
        assert!(link_ids("https://open.spotify.com/track/abc").is_none());
    }

    #[test]
    fn an_export_imports_once_and_updates_after() {
        let mut export = Export::default();
        read_export_json(
            "Spotify Account Data/Playlist1.json",
            r#"{"playlists":[{"name":"Road","items":[{"track":{"trackName":"Song","artistName":"Band","albumName":"Rec","trackUri":"spotify:track:4uLU6hMCjMI75M1A2tKUQC"}},{"episode":{}}]}]}"#,
            &mut export,
        );
        read_export_json("YourLibrary.json", r#"{"tracks":[{"artist":"Band","album":"Rec","track":"Song","uri":"spotify:track:4uLU6hMCjMI75M1A2tKUQC"}]}"#, &mut export);
        assert_eq!(export.playlists[0].1.len(), 1);
        assert_eq!(export.liked.len(), 1);
        let mut playlists = Vec::new();
        assert_eq!(import_into(export, &mut playlists, 1), (2, 0));
        let mut again = Export::default();
        again.liked.push(playlists[0].tracks[0].clone());
        assert_eq!(import_into(again, &mut playlists, 2), (0, 1));
        assert_eq!(playlists.len(), 2);
    }
}
