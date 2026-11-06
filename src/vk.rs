//! VK's music, through the interface VK's own web player uses, with the listener's VK sign-in: the `p` and
//! `remixsid` cookies of a browser signed in to vk.ru, exchanged for a short-lived token. As Noctorium-Base's
//! `VkClient` reads it -- throttled to VK's pace, and its refusals worded for a reader.
//!
//! A song's address is its page, `vk.ru/audio<owner>_<id>`, with its access key after a `#`; its stream is
//! asked for at play, and played through [`crate::hlsrelay`], since VK encrypts some parts of it.

use crate::domain::{Album, Artist, Playlist, ProviderType, Track};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const API: &str = "https://api.vk.ru/method";
const WEB_TOKEN_URL: &str = "https://login.vk.ru/?act=web_token";
const WEB_APP_ID: &str = "6287487";
const API_VERSION: &str = "5.282";
const BROWSER: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:143.0) Gecko/20100101 Firefox/143.0";

#[derive(Clone, Debug, PartialEq)]
pub struct Cookies {
    pub p: String,
    pub remixsid: String,
}

impl Cookies {
    pub fn header(&self) -> String {
        format!("p={}; remixsid={}", self.p, self.remixsid)
    }

    pub fn from_jar(cookies: &[crate::accounts::Cookie]) -> Result<Cookies, String> {
        let find = |name: &str| {
            cookies
                .iter()
                .find(|c| c.name == name && (c.domain.ends_with("vk.ru") || c.domain.ends_with("vk.com")))
                .map(|c| c.value.clone())
        };
        Ok(Cookies {
            p: find("p").ok_or("That browser is not signed in to VK.")?,
            remixsid: find("remixsid").ok_or("That browser is not signed in to VK.")?,
        })
    }
}

#[derive(Clone, Debug)]
struct Token {
    access: String,
    user_id: i64,
    expires_at: i64,
}

/// A signed-in VK session.
#[derive(Clone, Debug)]
pub struct Session {
    pub cookies: Cookies,
    token: std::sync::Arc<Mutex<Option<Token>>>,
}

fn pace() {
    static LAST: OnceLock<Mutex<Instant>> = OnceLock::new();
    let last = LAST.get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(1)));
    let mut last = last.lock().unwrap();
    let wait = Duration::from_millis(400).saturating_sub(last.elapsed());
    std::thread::sleep(wait);
    *last = Instant::now();
}

impl Session {
    pub fn new(cookies: Cookies) -> Session {
        Session { cookies, token: std::sync::Arc::new(Mutex::new(None)) }
    }

    fn exchange(&self) -> Result<Token, String> {
        let reply = ureq::post(WEB_TOKEN_URL)
            .set("Cookie", &self.cookies.header())
            .set("Origin", "https://vk.ru")
            .set("Referer", "https://vk.ru/")
            .set("User-Agent", BROWSER)
            .timeout(Duration::from_secs(20))
            .send_form(&[("version", "1"), ("app_id", WEB_APP_ID)])
            .map_err(|e| format!("Could not reach VK: {e}"))?;
        let page: serde_json::Value = reply.into_json().map_err(|_| "VK answered the sign-in with something Noctorium could not read.".to_string())?;
        if page["type"].as_str() != Some("okay") {
            return Err(if page["error_info"].as_str() == Some("unauthorized") {
                "VK no longer recognises this sign-in. Sign in to VK again in Services.".into()
            } else {
                "VK refused the sign-in. Sign in to VK again in Services.".into()
            });
        }
        let data = &page["data"];
        Ok(Token {
            access: data["access_token"].as_str().ok_or("VK's sign-in reply carried no token.")?.to_string(),
            user_id: data["user_id"].as_i64().unwrap_or(0),
            expires_at: data["expires"].as_i64().unwrap_or(crate::store::now_seconds() + 900),
        })
    }

    fn token(&self) -> Result<Token, String> {
        let mut kept = self.token.lock().unwrap();
        if let Some(token) = kept.as_ref().filter(|t| t.expires_at - 120 > crate::store::now_seconds()) {
            return Ok(token.clone());
        }
        let fresh = self.exchange()?;
        *kept = Some(fresh.clone());
        Ok(fresh)
    }

    fn call(&self, method: &str, params: &[(&str, String)]) -> Result<serde_json::Value, String> {
        for attempt in 0..4 {
            let token = self.token()?;
            pace();
            let mut form: Vec<(&str, String)> = params.to_vec();
            form.extend([
                ("access_token", token.access.clone()),
                ("v", API_VERSION.to_string()),
                ("lang", "en".to_string()),
                ("client_id", WEB_APP_ID.to_string()),
            ]);
            let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let page: serde_json::Value = ureq::post(&format!("{API}/{method}"))
                .set("User-Agent", BROWSER)
                .set("Origin", "https://vk.ru")
                .set("Referer", "https://vk.ru/")
                .timeout(Duration::from_secs(20))
                .send_form(&pairs)
                .map_err(|e| format!("Could not reach VK: {e}"))?
                .into_json()
                .map_err(|_| "VK answered with something Noctorium could not read.".to_string())?;
            let Some(code) = page["error"]["error_code"].as_i64() else { return Ok(page["response"].clone()) };
            match code {
                5 if attempt == 0 => *self.token.lock().unwrap() = None,
                5 => return Err("VK no longer accepts this sign-in. Sign in to VK again in Services.".into()),
                6 => std::thread::sleep(Duration::from_millis(1000 * (attempt as u64 + 1))),
                9 => return Err("VK has paused this for a while (flood control). Give it some minutes.".into()),
                29 => return Err("VK's limit on requests for this account is reached for now. Try again later.".into()),
                25 => return Err("VK wants you to confirm it is you. Open VK in a browser, confirm, and sign in again.".into()),
                15 | 201 => return Err("VK keeps this private.".into()),
                3 => return Err("VK no longer serves music to this kind of sign-in. Sign in to VK again.".into()),
                _ => return Err(format!("VK said: {}", page["error"]["error_msg"].as_str().unwrap_or("an error"))),
            }
        }
        Err("VK asked Noctorium to slow down. Try again in a moment.".into())
    }

    pub fn name(&self) -> Result<String, String> {
        let users = self.call("users.get", &[])?;
        let user = &users[0];
        let name = [user["first_name"].as_str(), user["last_name"].as_str()].into_iter().flatten().collect::<Vec<_>>().join(" ");
        Ok(if name.is_empty() { "VK".into() } else { name })
    }

    pub fn my_audio(&self, limit: usize) -> Result<Vec<Track>, String> {
        let user = self.token()?.user_id;
        let mut out = Vec::new();
        while out.len() < limit {
            let page = self.call(
                "audio.get",
                &[("owner_id", user.to_string()), ("count", "200".into()), ("offset", out.len().to_string())],
            )?;
            let songs = audios(&page);
            let total = page["count"].as_u64().unwrap_or(0) as usize;
            let empty = songs.is_empty();
            out.extend(songs);
            if empty || out.len() >= total {
                break;
            }
        }
        Ok(out)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, String> {
        Ok(audios(&self.call(
            "audio.search",
            &[("q", query.trim().to_string()), ("count", limit.to_string()), ("auto_complete", "1".into()), ("sort", "2".into())],
        )?))
    }

    pub fn recommendations(&self) -> Result<Vec<Track>, String> {
        let user = self.token()?.user_id;
        Ok(audios(&self.call("audio.getRecommendations", &[("count", "30".into()), ("user_id", user.to_string())])?))
    }

    pub fn playlists(&self) -> Result<Vec<Playlist>, String> {
        let user = self.token()?.user_id;
        let page = self.call("audio.getPlaylists", &[("owner_id", user.to_string()), ("count", "100".into())])?;
        Ok(items(&page)
            .iter()
            .filter_map(|p| {
                let (owner, id) = (p["owner_id"].as_i64()?, p["id"].as_i64()?);
                let key = p["access_key"].as_str().unwrap_or("");
                let photo = if p["photo"].is_object() { &p["photo"] } else { &p["thumbs"][0] };
                Some(Playlist {
                    id: format!("{owner}_{id}"),
                    title: p["title"].as_str()?.to_string(),
                    provider: ProviderType::Vk,
                    owner_name: p["main_artists"][0]["name"].as_str().map(String::from),
                    artwork_url: photo["photo_600"].as_str().or(photo["photo_300"].as_str()).map(String::from),
                    tracks: vec![],
                    source_url: Some(format!("https://vk.ru/music/playlist/{owner}_{id}{}", if key.is_empty() { String::new() } else { format!("_{key}") })),
                    track_count: p["count"].as_i64(),
                })
            })
            .collect())
    }

    pub fn playlist(&self, owner: i64, id: i64, key: Option<&str>) -> Result<Vec<Track>, String> {
        let mut params = vec![("owner_id", owner.to_string()), ("album_id", id.to_string()), ("count", "1000".into())];
        if let Some(k) = key.filter(|k| !k.is_empty()) {
            params.push(("access_key", k.to_string()));
        }
        Ok(audios(&self.call("audio.get", &params)?))
    }

    /// Where a song's audio is now. VK's addresses expire, so they are asked for at play.
    pub fn stream(&self, source_url: &str) -> Result<String, String> {
        let (owner, id, key) = parse_source(source_url).ok_or("That is not a VK song.")?;
        let request = match key {
            Some(k) => format!("{owner}_{id}_{k}"),
            None => format!("{owner}_{id}"),
        };
        let found = self.call("audio.getById", &[("audios", request)])?;
        let url = found[0]["url"].as_str().filter(|u| !u.is_empty()).ok_or("VK will not play this song here.")?;
        Ok(url.to_string())
    }
}

fn items(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value.as_array().cloned().or_else(|| value["items"].as_array().cloned()).unwrap_or_default()
}

fn audios(value: &serde_json::Value) -> Vec<Track> {
    items(value).iter().filter_map(audio_of).collect()
}

pub fn audio_of(item: &serde_json::Value) -> Option<Track> {
    let (owner, id) = (item["owner_id"].as_i64()?, item["id"].as_i64()?);
    let title = item["title"].as_str().filter(|t| !t.is_empty())?;
    let artist = item["artist"].as_str().filter(|a| !a.is_empty()).unwrap_or("Unknown artist");
    let album = &item["album"];
    let thumb = if album["thumb"].is_object() { &album["thumb"] } else { &item["thumb"] };
    let artwork = thumb["photo_600"].as_str().or(thumb["photo_300"].as_str()).or(thumb["photo_1200"].as_str()).map(String::from);
    let key = item["access_key"].as_str().filter(|k| !k.is_empty());
    Some(Track {
        provider: ProviderType::Vk,
        id: format!("{owner}_{id}"),
        title: title.to_string(),
        artists: vec![Artist { id: artist.to_string(), name: artist.to_string(), provider: ProviderType::Vk }],
        album: album["title"].as_str().map(|t| Album {
            id: t.to_string(),
            title: t.to_string(),
            artists: vec![],
            provider: ProviderType::Vk,
            artwork_url: artwork.clone(),
        }),
        duration_ms: item["duration"].as_i64().filter(|d| *d > 0).map(|d| d * 1000),
        artwork_url: artwork,
        source_url: match key {
            Some(k) => format!("https://vk.ru/audio{owner}_{id}#vk-access={k}"),
            None => format!("https://vk.ru/audio{owner}_{id}"),
        },
    })
}

/// The owner, the id and the access key in a VK song's address.
pub fn parse_source(url: &str) -> Option<(i64, i64, Option<String>)> {
    let (page, fragment) = url.split_once('#').unwrap_or((url, ""));
    let rest = page.split("/audio").nth(1)?;
    let (owner, id) = rest.split_once('_')?;
    let id: String = id.chars().take_while(|c| c.is_ascii_digit()).collect();
    let key = fragment.split('&').find_map(|p| p.strip_prefix("vk-access=")).filter(|k| !k.is_empty()).map(String::from);
    if !page.contains("vk.ru/") && !page.contains("vk.com/") {
        return None;
    }
    Some((owner.parse().ok()?, id.parse().ok()?, key))
}

/// `vk.ru/music/playlist/<owner>_<id>[_<key>]`: its owner, id and access key.
pub fn playlist_ids(url: &str) -> Option<(i64, i64, Option<String>)> {
    let lower = url.to_lowercase();
    if !(lower.contains("vk.ru/") || lower.contains("vk.com/")) {
        return None;
    }
    let rest = url.split("playlist/").nth(1).or_else(|| url.split("audio_playlist").nth(1))?;
    let mut parts = rest.split(['?', '#', '/']).next()?.splitn(3, '_');
    let owner = parts.next()?.parse().ok()?;
    let id = parts.next()?.parse().ok()?;
    Some((owner, id, parts.next().map(String::from)))
}

/// The signed-in session, where search, links and playing can reach it.
pub fn current() -> Option<Session> {
    slot().lock().unwrap().clone()
}

pub fn set_current(session: Option<Session>) {
    *slot().lock().unwrap() = session;
}

fn slot() -> &'static Mutex<Option<Session>> {
    static SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(None))
}

pub fn is_vk_song(url: &str) -> bool {
    parse_source(url).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_song_and_its_address() {
        let item = json!({"owner_id": -2001, "id": 456, "title": "Song", "artist": "Band", "duration": 200,
            "access_key": "abc", "album": {"title": "Rec", "thumb": {"photo_600": "cover"}}});
        let track = audio_of(&item).unwrap();
        assert_eq!(track.source_url, "https://vk.ru/audio-2001_456#vk-access=abc");
        assert_eq!(parse_source(&track.source_url), Some((-2001, 456, Some("abc".into()))));
        assert_eq!(track.artwork_url.as_deref(), Some("cover"));
        assert_eq!(parse_source("https://example.com/audio1_2"), None);
    }

    #[test]
    fn playlist_links_give_their_ids() {
        assert_eq!(playlist_ids("https://vk.ru/music/playlist/-147845620_2949_9c2d4f"), Some((-147845620, 2949, Some("9c2d4f".into()))));
        assert_eq!(playlist_ids("https://vk.com/music/playlist/1_2"), Some((1, 2, None)));
    }
}
