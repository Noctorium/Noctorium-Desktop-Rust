//! The listener's own YouTube Music and SoundCloud: their playlists, liked songs, personal home page, and
//! liking a song there too. Read through the services' own web interfaces as Noctorium-Base's
//! `YouTubeMusicClient` and `SoundCloudAccountClient` read them, with the session of a browser the listener
//! is already signed in to -- taken by yt-dlp, which knows where every browser keeps its cookies.
//!
//! The Kotlin desktop signs in in a browser window of its own; this one borrows the session the listener's
//! browser already has. Either way it is the listener's own session, kept in the system's secret store.

use crate::domain::{Playlist, ProviderType, Track};
use sha1::{Digest, Sha1};
use std::path::Path;
use std::time::Duration;

const MUSIC: &str = "https://music.youtube.com";
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

/// One cookie from a Netscape cookie file.
#[derive(Clone, Debug, PartialEq)]
pub struct Cookie {
    pub domain: String,
    pub name: String,
    pub value: String,
    pub expires: i64,
}

/// The cookies in a Netscape cookie file, the format yt-dlp writes. `#HttpOnly_` lines are cookies too.
pub fn read_cookie_file(text: &str) -> Vec<Cookie> {
    text.lines()
        .filter_map(|line| {
            let line = line.strip_prefix("#HttpOnly_").unwrap_or(line);
            if line.starts_with('#') || line.trim().is_empty() {
                return None;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            (fields.len() >= 7).then(|| Cookie {
                domain: fields[0].trim_start_matches('.').to_string(),
                name: fields[5].to_string(),
                value: fields[6].to_string(),
                expires: fields[4].parse().unwrap_or(0),
            })
        })
        .collect()
}

/// A `Cookie` header of the live cookies set for `domain` or above it.
pub fn cookie_header(cookies: &[Cookie], domain: &str, now: i64) -> String {
    cookies
        .iter()
        .filter(|c| domain == c.domain || domain.ends_with(&format!(".{}", c.domain)))
        .filter(|c| c.expires == 0 || c.expires > now)
        .map(|c| format!("{}={}", c.name, c.value))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Has yt-dlp read `browser`'s cookies and write them to a file, and answers them. The file is deleted at
/// once: it holds every site's sign-in.
pub fn export_browser_cookies(yt_dlp: &Path, browser: &str, url: &str) -> Result<Vec<Cookie>, String> {
    let file = std::env::temp_dir().join(format!("noctorium-rust-jar-{}-{}.txt", std::process::id(), crate::store::now_seconds()));
    let mut command = std::process::Command::new(yt_dlp);
    command.args(["--cookies-from-browser", browser, "--cookies"]).arg(&file).args([
        "--simulate",
        "--skip-download",
        "--no-warnings",
        "--flat-playlist",
        "--playlist-items",
        "1",
        "--",
        url,
    ]);
    crate::ytdlp::hide_console(&mut command);
    let output = command.output().map_err(|e| format!("yt-dlp could not be started: {e}"))?;
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let _ = std::fs::remove_file(&file);
    if text.is_empty() {
        let said = String::from_utf8_lossy(&output.stderr);
        let why = said.lines().rev().find(|l| l.contains("ERROR")).unwrap_or("no cookies came back").trim().to_string();
        return Err(format!("Could not read {browser}'s sign-ins: {why}"));
    }
    Ok(read_cookie_file(&text))
}

// ---- YouTube Music ---------------------------------------------------------------------------------

/// Google's signature for a signed-in request: SHA-1 over the time, the SAPISID cookie and the origin.
pub fn sapisid_hash(sapisid: &str, origin: &str, epoch_seconds: i64) -> String {
    let digest: String = Sha1::digest(format!("{epoch_seconds} {sapisid} {origin}").as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("SAPISIDHASH {epoch_seconds}_{digest}")
}

pub fn sapisid_from(cookie_header: &str) -> Option<String> {
    cookie_header.split(';').map(str::trim).find_map(|pair| {
        ["SAPISID=", "__Secure-3PAPISID="]
            .iter()
            .find_map(|prefix| pair.strip_prefix(prefix).filter(|v| !v.is_empty()).map(String::from))
    })
}

/// A signed-in YouTube Music session: the browser's cookies for youtube.com.
#[derive(Clone, Debug, PartialEq)]
pub struct YouTube {
    pub cookies: String,
}

impl YouTube {
    pub fn from_cookies(cookies: &[Cookie]) -> Result<YouTube, String> {
        let header = cookie_header(cookies, "music.youtube.com", crate::store::now_seconds());
        if sapisid_from(&header).is_none() {
            return Err("That browser is not signed in to YouTube.".into());
        }
        Ok(YouTube { cookies: header })
    }

    fn post(&self, endpoint: &str, mut body: serde_json::Value) -> Result<serde_json::Value, String> {
        let version = crate::ytmusic::client_version();
        body["context"] = serde_json::json!({
            "client": {"clientName": "WEB_REMIX", "clientVersion": version, "hl": "en", "gl": "US"}
        });
        let sapisid = sapisid_from(&self.cookies).ok_or("The YouTube session has no SAPISID.")?;
        let now = crate::store::now_seconds();
        let reply = ureq::post(&format!("{MUSIC}/youtubei/v1/{endpoint}?prettyPrint=false"))
            .set("User-Agent", BROWSER)
            .set("Cookie", &self.cookies)
            .set("Authorization", &sapisid_hash(&sapisid, MUSIC, now))
            .set("X-Goog-AuthUser", "0")
            .set("X-YouTube-Client-Name", "67")
            .set("X-YouTube-Client-Version", &version)
            .set("Origin", MUSIC)
            .set("Referer", &format!("{MUSIC}/"))
            .set("Content-Type", "application/json")
            .timeout(Duration::from_secs(20))
            .send_string(&body.to_string());
        match reply {
            Ok(r) => r.into_json().map_err(|e| e.to_string()),
            Err(ureq::Error::Status(401 | 403, _)) => {
                Err("YouTube no longer accepts this sign-in. Sign in again in Settings › Accounts.".into())
            }
            Err(e) => Err(format!("YouTube Music did not answer: {e}")),
        }
    }

    /// The account's name, which also proves the session works.
    pub fn account_name(&self) -> Result<String, String> {
        let body = self.post("account/account_menu", serde_json::json!({}))?;
        let mut names = Vec::new();
        find_key(&body, "accountName", &mut names);
        names
            .into_iter()
            .find_map(runs_or_simple)
            .ok_or_else(|| "YouTube did not say who is signed in.".into())
    }

    /// The listener's playlists, every page of them. YouTube Music sends the library's shelf a couple of dozen
    /// at a time, with a token for the next lot; only the first page was read, so a listener with more
    /// playlists than that saw some of them and not the rest.
    pub fn playlists(&self) -> Result<Vec<Playlist>, String> {
        let mut body = self.post("browse", serde_json::json!({"browseId": "FEmusic_liked_playlists"}))?;
        let mut seen = std::collections::HashSet::new();
        let mut found = Vec::new();
        let mut last_token: Option<String> = None;
        for _ in 0..MAX_PLAYLIST_PAGES {
            let page = Self::playlists_on(&body, &mut seen);
            // A page that adds nothing, or no way onward, or a token come back unchanged, is the end.
            if page.is_empty() {
                break;
            }
            found.extend(page);
            let Some(token) = continuation(&body).filter(|t| Some(t) != last_token.as_ref()) else { break };
            body = match self.post("browse", serde_json::json!({"continuation": token})) {
                Ok(next) => next,
                // A page refused part way keeps what came before it.
                Err(_) => break,
            };
            last_token = Some(token);
        }
        Ok(found)
    }

    /// The playlists on one page of the library's shelf, leaving out any in `seen` and adding the rest to it.
    fn playlists_on(body: &serde_json::Value, seen: &mut std::collections::HashSet<String>) -> Vec<Playlist> {
        let mut cards = Vec::new();
        find_key(body, "musicTwoRowItemRenderer", &mut cards);
        cards
            .into_iter()
            .filter_map(|card| {
                let title = runs_or_simple(&card["title"])?;
                let browse = card["navigationEndpoint"]["browseEndpoint"]["browseId"].as_str()?;
                let id = browse.strip_prefix("VL")?.to_string();
                // The library's own lists -- liked music, episodes for later -- are shown elsewhere.
                if id == "LM" || id == "SE" || !seen.insert(id.clone()) {
                    return None;
                }
                let artwork = card["thumbnailRenderer"]["musicThumbnailRenderer"]["thumbnail"]["thumbnails"]
                    .as_array()
                    .and_then(|t| t.last())
                    .and_then(|t| t["url"].as_str())
                    .map(crate::ytmusic::bigger_artwork);
                Some(Playlist {
                    source_url: Some(format!("{MUSIC}/playlist?list={id}")),
                    id,
                    title,
                    provider: ProviderType::YoutubeMusic,
                    owner_name: runs_or_simple(&card["subtitle"]),
                    artwork_url: artwork,
                    tracks: vec![],
                    track_count: None,
                })
            })
            .collect()
    }

    /// The songs of a playlist, the listener's private ones and Liked Music (`LM`) too, page after page.
    pub fn playlist_tracks(&self, playlist_id: &str, limit: usize) -> Result<Vec<Track>, String> {
        let mut found: Vec<Track> = Vec::new();
        let mut body = self.post("browse", serde_json::json!({"browseId": format!("VL{playlist_id}")}))?;
        for _ in 0..20 {
            let page = crate::ytmusic::parse_songs(&body);
            let before = found.len();
            for track in page {
                if !found.iter().any(|t| t.id == track.id) {
                    found.push(track);
                }
            }
            if found.len() >= limit || found.len() == before {
                break;
            }
            let Some(token) = continuation(&body) else { break };
            body = self.post("browse", serde_json::json!({"continuation": token}))?;
        }
        found.truncate(limit);
        Ok(found)
    }

    pub fn liked_songs(&self, limit: usize) -> Result<Vec<Track>, String> {
        self.playlist_tracks("LM", limit)
    }

    /// The personal home page: each shelf of songs, by its title.
    pub fn home(&self) -> Result<Vec<(String, Vec<Track>)>, String> {
        let body = self.post("browse", serde_json::json!({"browseId": "FEmusic_home"}))?;
        let mut shelves = Vec::new();
        find_key(&body, "musicCarouselShelfRenderer", &mut shelves);
        Ok(shelves
            .into_iter()
            .filter_map(|shelf| {
                let title = runs_or_simple(&shelf["header"]["musicCarouselShelfBasicHeaderRenderer"]["title"])?;
                let tracks = crate::ytmusic::parse_songs(shelf);
                (!tracks.is_empty()).then_some((title, tracks))
            })
            .collect())
    }

    pub fn set_liked(&self, video_id: &str, liked: bool) -> Result<(), String> {
        let endpoint = if liked { "like/like" } else { "like/removelike" };
        self.post(endpoint, serde_json::json!({"target": {"videoId": video_id}})).map(|_| ())
    }

    pub fn add_to_playlist(&self, playlist_id: &str, video_id: &str) -> Result<(), String> {
        self.edit(playlist_id, vec![serde_json::json!({"action": "ACTION_ADD_VIDEO", "addedVideoId": video_id})])
    }

    /// Every playlist change but making and deleting one is a list of actions sent together. A 200 is not
    /// enough: the answer's own `status` says STATUS_SUCCEEDED when YouTube did what it was asked.
    fn edit(&self, playlist_id: &str, actions: Vec<serde_json::Value>) -> Result<(), String> {
        let id = playlist_id.strip_prefix("VL").unwrap_or(playlist_id);
        let reply = self.post("browse/edit_playlist", serde_json::json!({"playlistId": id, "actions": actions}))?;
        match reply["status"].as_str() {
            Some(status) if !status.contains("SUCCEEDED") => {
                Err(format!("YouTube Music did not make the change ({status}). The playlist may not be yours."))
            }
            _ => Ok(()),
        }
    }

    /// Makes a playlist, with songs in it from the start if there are any. Its id, to open it by.
    pub fn create_playlist(&self, title: &str, video_ids: &[String], privacy: Privacy) -> Result<String, String> {
        let title = title.trim();
        if title.is_empty() {
            return Err("Give the playlist a name first.".into());
        }
        let mut ids: Vec<&String> = Vec::new();
        for id in video_ids {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        let mut body = serde_json::json!({"title": title.chars().take(MAX_TITLE).collect::<String>(), "privacyStatus": privacy.name()});
        if !ids.is_empty() {
            body["videoIds"] = serde_json::json!(ids);
        }
        let reply = self.post("playlist/create", body)?;
        reply["playlistId"].as_str().map(String::from).ok_or_else(|| "YouTube Music did not make the playlist.".into())
    }

    /// Name, description and privacy at once, as YouTube Music's own "Edit playlist" saves: an action for each
    /// thing that changed and none for the rest.
    pub fn edit_playlist(&self, playlist_id: &str, edit: &PlaylistEdit) -> Result<(), String> {
        let mut actions = Vec::new();
        if let Some(title) = &edit.title {
            let title = title.trim();
            if title.is_empty() {
                return Err("Give the playlist a name first.".into());
            }
            actions.push(serde_json::json!({"action": "ACTION_SET_PLAYLIST_NAME", "playlistName": title.chars().take(MAX_TITLE).collect::<String>()}));
        }
        if let Some(description) = &edit.description {
            let text: String = description.trim().chars().take(MAX_DESCRIPTION).collect();
            actions.push(serde_json::json!({"action": "ACTION_SET_PLAYLIST_DESCRIPTION", "playlistDescription": text}));
        }
        if let Some(privacy) = edit.privacy {
            actions.push(serde_json::json!({"action": "ACTION_SET_PLAYLIST_PRIVACY", "playlistPrivacy": privacy.name()}));
        }
        if actions.is_empty() {
            return Ok(());
        }
        self.edit(playlist_id, actions)
    }

    pub fn remove_from_playlist(&self, playlist_id: &str, video_id: &str) -> Result<(), String> {
        self.edit(playlist_id, vec![serde_json::json!({"action": "ACTION_REMOVE_VIDEO_BY_VIDEO_ID", "removedVideoId": video_id})])
    }

    /// Tells YouTube Music the song was played, so it is in the account's history and its recommendations
    /// learn from it: the player is asked about the video as the account, and the reports its answer names
    /// are sent -- the playback report, which files it in history, and a first watch-time one. The audio
    /// still comes from wherever it always does.
    pub fn record_play(&self, video_id: &str) -> Result<(), String> {
        let cpn = playback_nonce();
        let player = self.post("player", serde_json::json!({"videoId": video_id, "cpn": cpn}))?;
        let tracking = &player["playbackTracking"];
        let playback = tracking["videostatsPlaybackUrl"]["baseUrl"].as_str().ok_or("YouTube gave no playback report address.")?;
        let common = format!("ver=2&c=WEB_REMIX&cpn={cpn}");
        self.get_signed(&with_query(playback, &common))?;
        if let Some(watchtime) = tracking["videostatsWatchtimeUrl"]["baseUrl"].as_str() {
            let _ = self.get_signed(&with_query(watchtime, &format!("{common}&st=0&et=5.54")));
        }
        Ok(())
    }

    fn get_signed(&self, url: &str) -> Result<(), String> {
        let sapisid = sapisid_from(&self.cookies).ok_or("The YouTube session has no SAPISID.")?;
        ureq::get(url)
            .set("User-Agent", BROWSER)
            .set("Cookie", &self.cookies)
            .set("Authorization", &sapisid_hash(&sapisid, MUSIC, crate::store::now_seconds()))
            .set("Origin", MUSIC)
            .set("Referer", &format!("{MUSIC}/"))
            .timeout(Duration::from_secs(15))
            .call()
            .map(|_| ())
            .map_err(|e| format!("YouTube did not take the report: {e}"))
    }

    /// The artist a song is by, as YouTube Music links it -- the artist page, which is also the channel to
    /// follow -- and whether the account follows them.
    pub fn artist_of(&self, video_id: &str) -> Result<FollowedArtist, String> {
        let next = self.post("next", serde_json::json!({"videoId": video_id, "isAudioOnly": true}))?;
        let (id, name) = first_artist_link(&next).ok_or("YouTube Music did not say whose song this is.")?;
        let page = self.post("browse", serde_json::json!({"browseId": id}))?;
        let mut flags = Vec::new();
        find_key(&page, "subscribeButtonRenderer", &mut flags);
        let following = flags.iter().find_map(|f| f["subscribed"].as_bool());
        Ok(FollowedArtist { id, name, following: following.unwrap_or(false) })
    }

    /// Follows an artist, or stops: subscribing to their channel, as YouTube calls it.
    pub fn set_following(&self, channel_id: &str, follow: bool) -> Result<(), String> {
        let endpoint = if follow { "subscription/subscribe" } else { "subscription/unsubscribe" };
        self.post(endpoint, serde_json::json!({"channelIds": [channel_id]})).map(|_| ())
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), String> {
        let id = playlist_id.strip_prefix("VL").unwrap_or(playlist_id);
        self.post("playlist/delete", serde_json::json!({"playlistId": id})).map(|_| ())
    }
}

/// An artist on YouTube Music, by their page's id, and whether the account follows them.
#[derive(Clone, Debug, PartialEq)]
pub struct FollowedArtist {
    pub id: String,
    pub name: String,
    pub following: bool,
}

/// The first byline run that links to an artist's page.
fn first_artist_link(value: &serde_json::Value) -> Option<(String, String)> {
    match value {
        serde_json::Value::Object(map) => {
            let browse = &value["navigationEndpoint"]["browseEndpoint"];
            let page = browse["browseEndpointContextSupportedConfigs"]["browseEndpointContextMusicConfig"]["pageType"].as_str();
            if let (Some("MUSIC_PAGE_TYPE_ARTIST"), Some(id), Some(name)) = (page, browse["browseId"].as_str(), value["text"].as_str()) {
                return Some((id.to_string(), name.to_string()));
            }
            map.values().find_map(first_artist_link)
        }
        serde_json::Value::Array(list) => list.iter().find_map(first_artist_link),
        _ => None,
    }
}

/// The client playback nonce that ties a playback's reports together, as YouTube's player makes one.
fn playback_nonce() -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
    let mut rng = rand::rng();
    (0..16).map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char).collect()
}

fn with_query(url: &str, extra: &str) -> String {
    format!("{url}{}{extra}", if url.contains('?') { "&" } else { "?" })
}

const MAX_TITLE: usize = 150;
const MAX_DESCRIPTION: usize = 5000;

/// Who can see a YouTube playlist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Privacy {
    Public,
    Unlisted,
    Private,
}

impl Privacy {
    pub const ALL: [Privacy; 3] = [Privacy::Public, Privacy::Unlisted, Privacy::Private];

    pub fn name(self) -> &'static str {
        match self {
            Privacy::Public => "PUBLIC",
            Privacy::Unlisted => "UNLISTED",
            Privacy::Private => "PRIVATE",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Privacy::Public => "Public",
            Privacy::Unlisted => "Unlisted",
            Privacy::Private => "Private",
        }
    }
}

/// What to change about a YouTube playlist; what is `None` is left as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaylistEdit {
    pub title: Option<String>,
    pub description: Option<String>,
    pub privacy: Option<Privacy>,
}

fn find_key<'a>(value: &'a serde_json::Value, name: &str, into: &mut Vec<&'a serde_json::Value>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(found) = map.get(name) {
                into.push(found);
            }
            map.values().for_each(|v| find_key(v, name, into));
        }
        serde_json::Value::Array(list) => list.iter().for_each(|v| find_key(v, name, into)),
        _ => {}
    }
}

fn runs_or_simple(node: &serde_json::Value) -> Option<String> {
    if let Some(text) = node["simpleText"].as_str() {
        return Some(text.to_string()).filter(|t| !t.trim().is_empty());
    }
    let text: String = node["runs"].as_array()?.iter().filter_map(|r| r["text"].as_str()).collect();
    Some(text.trim().to_string()).filter(|t| !t.is_empty())
}

/// The pages of playlists read at most: a couple of dozen to a page, so a thousand and more.
const MAX_PLAYLIST_PAGES: usize = 50;

fn continuation(body: &serde_json::Value) -> Option<String> {
    let mut found = Vec::new();
    find_key(body, "continuationCommand", &mut found);
    let mut next = Vec::new();
    find_key(body, "nextContinuationData", &mut next);
    found
        .iter()
        .chain(next.iter())
        .find_map(|c| c["token"].as_str().or(c["continuation"].as_str()))
        .map(String::from)
}

// ---- SoundCloud ------------------------------------------------------------------------------------

/// A signed-in SoundCloud session: the `oauth_token` its web player keeps in a cookie.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundCloud {
    pub token: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundCloudProfile {
    pub id: u64,
    pub name: String,
}

impl SoundCloud {
    pub fn from_cookies(cookies: &[Cookie]) -> Result<SoundCloud, String> {
        let now = crate::store::now_seconds();
        cookies
            .iter()
            .find(|c| c.name == "oauth_token" && c.domain.ends_with("soundcloud.com") && (c.expires == 0 || c.expires > now))
            .map(|c| SoundCloud { token: c.value.clone() })
            .ok_or_else(|| "That browser is not signed in to SoundCloud.".into())
    }

    fn call(&self, method: &str, path: &str) -> Result<serde_json::Value, String> {
        let id = crate::soundcloud::client_id().ok_or("SoundCloud could not be reached.")?;
        let separator = if path.contains('?') { '&' } else { '?' };
        let url = format!("https://api-v2.soundcloud.com{path}{separator}client_id={id}");
        let reply = ureq::request(method, &url)
            .set("User-Agent", BROWSER)
            .set("Authorization", &format!("OAuth {}", self.token))
            .set("Origin", "https://soundcloud.com")
            .timeout(Duration::from_secs(20))
            .call();
        match reply {
            Ok(r) => Ok(r.into_json().unwrap_or(serde_json::Value::Null)),
            Err(ureq::Error::Status(401, _)) => {
                Err("SoundCloud no longer accepts this sign-in. Sign in again in Settings › Accounts.".into())
            }
            Err(e) => Err(format!("SoundCloud did not answer: {e}")),
        }
    }

    pub fn profile(&self) -> Result<SoundCloudProfile, String> {
        let me = self.call("GET", "/me")?;
        let user = if me["user"].is_object() { &me["user"] } else { &me };
        Ok(SoundCloudProfile {
            id: user["id"].as_u64().ok_or("SoundCloud did not say who is signed in.")?,
            name: user["username"].as_str().or(user["permalink"].as_str()).unwrap_or("SoundCloud").to_string(),
        })
    }

    pub fn likes(&self, user_id: u64, limit: usize) -> Result<Vec<Track>, String> {
        let page = self.call("GET", &format!("/users/{user_id}/track_likes?limit={}&linked_partitioning=1", limit.min(200)))?;
        Ok(page["collection"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|like| crate::soundcloud::track_of(&like["track"]))
            .collect())
    }

    pub fn playlists(&self, user_id: u64) -> Result<Vec<Playlist>, String> {
        let page = self.call("GET", &format!("/users/{user_id}/playlists?limit=50&linked_partitioning=1"))?;
        Ok(page["collection"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| {
                let url = p["permalink_url"].as_str()?;
                Some(Playlist {
                    id: p["id"].as_u64()?.to_string(),
                    title: p["title"].as_str()?.to_string(),
                    provider: ProviderType::Soundcloud,
                    owner_name: p["user"]["username"].as_str().map(String::from),
                    artwork_url: p["artwork_url"].as_str().map(crate::soundcloud::bigger_artwork),
                    tracks: vec![],
                    source_url: Some(url.to_string()),
                    track_count: p["track_count"].as_i64(),
                })
            })
            .collect())
    }

    pub fn set_liked(&self, user_id: u64, track_id: &str, liked: bool) -> Result<(), String> {
        let method = if liked { "PUT" } else { "DELETE" };
        self.call(method, &format!("/users/{user_id}/track_likes/{track_id}")).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One page of the library's playlist grid, and the token for the next if there is one.
    fn playlist_page(ids: &[&str], next: Option<&str>) -> serde_json::Value {
        let cards: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| {
                serde_json::json!({"musicTwoRowItemRenderer": {
                    "title": {"runs": [{"text": format!("Playlist {id}")}]},
                    "navigationEndpoint": {"browseEndpoint": {"browseId": format!("VL{id}")}}
                }})
            })
            .collect();
        let mut page = serde_json::json!({"contents": {"items": cards}});
        if let Some(token) = next {
            page["contents"]["continuations"] = serde_json::json!([{"nextContinuationData": {"continuation": token}}]);
        }
        page
    }

    #[test]
    fn every_page_of_playlists_is_read_once() {
        let mut seen = std::collections::HashSet::new();
        let first = playlist_page(&["PL1", "PL2", "LM"], Some("NEXT"));
        let page = YouTube::playlists_on(&first, &mut seen);
        assert_eq!(page.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["PL1", "PL2"], "Liked Music is left out");
        // The way on to the next lot, which was never followed: only the first page was ever shown.
        assert_eq!(continuation(&first).as_deref(), Some("NEXT"));
        // The next page's new playlists, and none read twice.
        let second = playlist_page(&["PL2", "PL3"], None);
        let page = YouTube::playlists_on(&second, &mut seen);
        assert_eq!(page.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["PL3"]);
        assert_eq!(page[0].title, "Playlist PL3");
        assert_eq!(continuation(&second), None, "the last page has no way onward");
    }

    #[test]
    fn a_netscape_jar_is_read_http_only_lines_included() {
        let jar = "# Netscape HTTP Cookie File\n\
            .youtube.com\tTRUE\t/\tTRUE\t1999999999\tSAPISID\tabc/def\n\
            #HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t1999999999\tSID\tsecret\n\
            .youtube.com\tTRUE\t/\tTRUE\t1\tOLD\tgone\n\
            .example.com\tTRUE\t/\tFALSE\t0\tother\tx\n";
        let cookies = read_cookie_file(jar);
        assert_eq!(cookies.len(), 4);
        let header = cookie_header(&cookies, "music.youtube.com", 1_000_000);
        assert_eq!(header, "SAPISID=abc/def; SID=secret");
        assert_eq!(sapisid_from(&header).as_deref(), Some("abc/def"));
    }

    #[test]
    fn the_signature_is_googles() {
        // SHA-1 of "1700000000 abc https://music.youtube.com".
        let hash = sapisid_hash("abc", MUSIC, 1_700_000_000);
        assert!(hash.starts_with("SAPISIDHASH 1700000000_"));
        assert_eq!(hash.len(), "SAPISIDHASH 1700000000_".len() + 40);
    }

    #[test]
    fn the_artist_is_read_from_the_byline() {
        let next = serde_json::json!({"contents": [{"runs": [
            {"text": "Song"},
            {"text": "Daft Punk", "navigationEndpoint": {"browseEndpoint": {"browseId": "UC123",
                "browseEndpointContextSupportedConfigs": {"browseEndpointContextMusicConfig": {"pageType": "MUSIC_PAGE_TYPE_ARTIST"}}}}}
        ]}]});
        assert_eq!(first_artist_link(&next), Some(("UC123".into(), "Daft Punk".into())));
        assert_eq!(playback_nonce().len(), 16);
        assert_eq!(with_query("https://x/y?a=1", "b=2"), "https://x/y?a=1&b=2");
    }

    #[test]
    fn soundcloud_takes_its_oauth_cookie() {
        let cookies = read_cookie_file(".soundcloud.com\tTRUE\t/\tTRUE\t0\toauth_token\t2-123-abc\n");
        assert_eq!(SoundCloud::from_cookies(&cookies).unwrap().token, "2-123-abc");
        assert!(SoundCloud::from_cookies(&[]).is_err());
    }
}
