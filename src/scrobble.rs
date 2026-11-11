//! Scrobbling: telling Last.fm, Libre.fm and ListenBrainz what was listened to. The same clients, the same
//! application and the same rule as Noctorium-Base's `scrobble` package: a song is counted once half of it
//! has been heard, or four minutes, whichever is first, and only if it is longer than thirty seconds; what
//! could not be sent is kept and sent later.

use crate::domain::{ProviderType, Track};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrobbleTrack {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_seconds: Option<i64>,
    pub source_url: String,
    pub provider: ProviderType,
}

impl ScrobbleTrack {
    pub fn from(track: &Track, duration_ms: i64) -> Self {
        let duration = if duration_ms > 0 { Some(duration_ms / 1000) } else { track.duration_ms.map(|d| d / 1000) };
        ScrobbleTrack {
            // An artist from a channel name the services leave on the song: "Band - Topic" would not match.
            title: track.title.clone(),
            artist: track.artist_line(),
            album: track.album.as_ref().map(|a| a.title.clone()).filter(|a| !a.is_empty()),
            duration_seconds: duration,
            source_url: track.source_url.clone(),
            provider: track.provider,
        }
    }
}

/// When a song counts: half of it, or four minutes, whichever comes first; never for thirty seconds or less.
pub fn threshold_ms(duration_ms: i64) -> Option<i64> {
    (duration_ms > 30_000).then(|| (duration_ms / 2).min(240_000))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Site {
    LastFm,
    LibreFm,
    ListenBrainz,
}

impl Site {
    pub const ALL: [Site; 3] = [Site::LastFm, Site::LibreFm, Site::ListenBrainz];
    pub fn name(self) -> &'static str {
        match self {
            Site::LastFm => "Last.fm",
            Site::LibreFm => "Libre.fm",
            Site::ListenBrainz => "ListenBrainz",
        }
    }
    fn api_root(self) -> &'static str {
        match self {
            Site::LastFm => "https://ws.audioscrobbler.com/2.0/",
            Site::LibreFm => "https://libre.fm/2.0/",
            Site::ListenBrainz => "https://api.listenbrainz.org",
        }
    }
    fn approval_page(self) -> &'static str {
        match self {
            Site::LastFm => "https://www.last.fm/api/auth/",
            Site::LibreFm => "https://libre.fm/api/auth/",
            Site::ListenBrainz => "https://listenbrainz.org/settings/",
        }
    }
    /// The name its sign-in is kept under, the Kotlin players' own.
    pub fn secret_name(self) -> &'static str {
        match self {
            Site::LastFm => "lastfm.session",
            Site::LibreFm => "librefm.session",
            Site::ListenBrainz => "listenbrainz.token",
        }
    }
}

/// Noctorium's own application on Last.fm, the Kotlin players', unless one is named in the environment.
fn api_key() -> String {
    std::env::var("NOCTORIUM_LASTFM_API_KEY").ok().filter(|k| !k.trim().is_empty()).unwrap_or_else(|| "394c9c370aa8da3149db90cd4d9261a0".into())
}

fn shared_secret() -> String {
    std::env::var("NOCTORIUM_LASTFM_SHARED_SECRET")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| "42b92ec5de8336e67701467d51b03e49".into())
}

const USER_AGENT: &str = concat!("Noctorium-Rust/", env!("CARGO_PKG_VERSION"), " scrobbler");

/// Audioscrobbler's signature: every parameter but `format`, sorted by name and run together, then the
/// shared secret, through MD5.
pub fn signature(parameters: &BTreeMap<String, String>, secret: &str) -> String {
    use md5::{Digest, Md5};
    let source: String =
        parameters.iter().filter(|(k, _)| *k != "format").map(|(k, v)| format!("{k}{v}")).collect::<String>() + secret;
    Md5::digest(source.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn signed(mut parameters: BTreeMap<String, String>) -> BTreeMap<String, String> {
    parameters.insert("api_key".into(), api_key());
    let sig = signature(&parameters, &shared_secret());
    parameters.insert("api_sig".into(), sig);
    parameters.insert("format".into(), "json".into());
    parameters
}

fn audioscrobbler_answer(site: Site, reply: Result<ureq::Response, ureq::Error>) -> Result<serde_json::Value, String> {
    let body: serde_json::Value = match reply {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(_, r)) => r.into_json().unwrap_or(serde_json::Value::Null),
        Err(e) => return Err(format!("{} could not be reached: {e}", site.name())),
    };
    if let Some(code) = body["error"].as_i64() {
        let message = body["message"].as_str().unwrap_or("refused");
        return Err(if code == 9 {
            format!("{} needs connecting again: it no longer accepts the old sign-in.", site.name())
        } else {
            format!("{}: {message}", site.name())
        });
    }
    Ok(body)
}

fn get(site: Site, parameters: BTreeMap<String, String>) -> Result<serde_json::Value, String> {
    let mut request = ureq::get(site.api_root()).set("User-Agent", USER_AGENT).timeout(Duration::from_secs(10));
    for (k, v) in &signed(parameters) {
        request = request.query(k, v);
    }
    audioscrobbler_answer(site, request.call())
}

fn post(site: Site, parameters: BTreeMap<String, String>) -> Result<serde_json::Value, String> {
    let form: Vec<(String, String)> = signed(parameters).into_iter().collect();
    let pairs: Vec<(&str, &str)> = form.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let reply = ureq::post(site.api_root()).set("User-Agent", USER_AGENT).timeout(Duration::from_secs(10)).send_form(&pairs);
    audioscrobbler_answer(site, reply)
}

/// The first step of connecting Last.fm or Libre.fm: a token, and the page where the listener allows it.
pub fn begin_authorization(site: Site) -> Result<(String, String), String> {
    let mut parameters = BTreeMap::new();
    parameters.insert("method".to_string(), "auth.getToken".to_string());
    let token = get(site, parameters)?["token"].as_str().map(String::from).ok_or("No token came back.")?;
    let url = format!("{}?api_key={}&token={token}", site.approval_page(), api_key());
    Ok((token, url))
}

/// The second: once allowed, the session. Answers the username and the session key.
pub fn complete_authorization(site: Site, token: &str) -> Result<(String, String), String> {
    let mut parameters = BTreeMap::new();
    parameters.insert("method".to_string(), "auth.getSession".to_string());
    parameters.insert("token".to_string(), token.to_string());
    let answer = get(site, parameters)?;
    let session = &answer["session"];
    let name = session["name"].as_str().ok_or("Not allowed yet.")?;
    let key = session["key"].as_str().ok_or("Not allowed yet.")?;
    Ok((name.to_string(), key.to_string()))
}

/// A ListenBrainz token's owner, or why it is not one.
pub fn validate_listenbrainz(token: &str) -> Result<String, String> {
    let body: serde_json::Value = ureq::get(&format!("{}/1/validate-token", Site::ListenBrainz.api_root()))
        .set("Authorization", &format!("Token {}", token.trim()))
        .set("User-Agent", USER_AGENT)
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("ListenBrainz did not accept it: {e}"))?
        .into_json()
        .map_err(|e| e.to_string())?;
    if body["valid"].as_bool() != Some(true) {
        return Err("ListenBrainz says that token is not valid.".into());
    }
    Ok(body["user_name"].as_str().unwrap_or("Connected").to_string())
}

/// What a sign-in is kept as: the username and the key, for Last.fm and Libre.fm; the token for ListenBrainz.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Account {
    pub username: String,
    pub secret: String,
}

/// Tells `site` what is playing now, or that `track` was listened to from `started_at`.
pub fn send(site: Site, account: &Account, track: &ScrobbleTrack, started_at: Option<i64>) -> Result<(), String> {
    match site {
        Site::ListenBrainz => {
            let service = match track.provider {
                ProviderType::Soundcloud => "soundcloud.com",
                ProviderType::YoutubeMusic | ProviderType::YoutubeVideo => "youtube.com",
                ProviderType::Spotify => "spotify",
                ProviderType::Bandcamp => "bandcamp.com",
                ProviderType::Vk => "vk.com",
                ProviderType::AppleMusic => "music.apple.com",
                ProviderType::Deezer => "deezer.com",
                ProviderType::Tidal => "tidal.com",
                ProviderType::Local => "local",
                ProviderType::Radio => "radio",
            };
            let mut metadata = serde_json::json!({
                "artist_name": track.artist,
                "track_name": track.title,
                "additional_info": {
                    "media_player": "Noctorium",
                    "submission_client": "Noctorium",
                    "submission_client_version": env!("CARGO_PKG_VERSION"),
                    "origin_url": track.source_url,
                    "music_service": service,
                }
            });
            if let Some(album) = &track.album {
                metadata["release_name"] = album.clone().into();
            }
            if let Some(seconds) = track.duration_seconds {
                metadata["additional_info"]["duration_ms"] = (seconds * 1000).into();
            }
            let mut listen = serde_json::json!({"track_metadata": metadata});
            if let Some(at) = started_at {
                listen["listened_at"] = at.into();
            }
            let body = serde_json::json!({
                "listen_type": if started_at.is_some() { "single" } else { "playing_now" },
                "payload": [listen],
            });
            ureq::post(&format!("{}/1/submit-listens", site.api_root()))
                .set("Authorization", &format!("Token {}", account.secret))
                .set("User-Agent", USER_AGENT)
                .timeout(Duration::from_secs(10))
                .send_json(body)
                .map(|_| ())
                .map_err(|e| match e {
                    ureq::Error::Status(401, _) => "ListenBrainz needs connecting again.".to_string(),
                    other => format!("ListenBrainz: {other}"),
                })
        }
        Site::LastFm | Site::LibreFm => {
            let mut parameters = BTreeMap::new();
            let method = if started_at.is_some() { "track.scrobble" } else { "track.updateNowPlaying" };
            parameters.insert("method".to_string(), method.to_string());
            parameters.insert("sk".to_string(), account.secret.clone());
            parameters.insert("artist".to_string(), track.artist.clone());
            parameters.insert("track".to_string(), track.title.clone());
            if let Some(album) = &track.album {
                parameters.insert("album".to_string(), album.clone());
            }
            if let Some(seconds) = track.duration_seconds {
                parameters.insert("duration".to_string(), seconds.to_string());
            }
            if let Some(at) = started_at {
                parameters.insert("timestamp".to_string(), at.to_string());
            }
            post(site, parameters).map(|_| ())
        }
    }
}

/// A scrobble that could not be sent, kept to send later: `scrobble-queue.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub site: Site,
    pub track: ScrobbleTrack,
    pub started_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_threshold_is_half_or_four_minutes() {
        assert_eq!(threshold_ms(30_000), None);
        assert_eq!(threshold_ms(200_000), Some(100_000));
        assert_eq!(threshold_ms(900_000), Some(240_000));
    }

    #[test]
    fn the_signature_leaves_format_out_and_sorts() {
        let mut a = BTreeMap::new();
        a.insert("method".to_string(), "auth.getToken".to_string());
        a.insert("api_key".to_string(), "k".to_string());
        let mut b = a.clone();
        b.insert("format".to_string(), "json".to_string());
        assert_eq!(signature(&a, "s"), signature(&b, "s"));
        // md5("api_keykmethodauth.getTokens")
        assert_eq!(signature(&a, "s").len(), 32);
    }
}
