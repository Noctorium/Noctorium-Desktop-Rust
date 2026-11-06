//! Tidal: its catalogue and the listener's own collection, through Tidal's official interface
//! (`openapi.tidal.com/v2`), matched on YouTube Music to be played -- Tidal's audio plays only in its own
//! players. As in Noctorium-Base there is no application built in: the listener registers one of their own at
//! developer.tidal.com, its client id and secret read the catalogue, and its sign-in reads the collection.

use crate::domain::{Album, Artist, Playlist, ProviderType, Track};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const API: &str = "https://openapi.tidal.com/v2";
const TOKEN_ENDPOINT: &str = "https://auth.tidal.com/v1/oauth2/token";
const REDIRECT_PORT: u16 = 8889;
const REDIRECT_PATH: &str = "/noctorium/tidal";
const SCOPES: &str = "user.read collection.read playlists.read search.read recommendations.read";
pub const DEVELOPER_PORTAL: &str = "https://developer.tidal.com/dashboard";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub client_id: String,
    pub client_secret: String,
}

impl App {
    pub fn reads_catalogue(&self) -> bool {
        !self.client_id.trim().is_empty() && !self.client_secret.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub access: String,
    pub refresh: Option<String>,
    pub expires_at: i64,
}

fn redirect_uri() -> String {
    format!("http://127.0.0.1:{REDIRECT_PORT}{REDIRECT_PATH}")
}

fn token(form: &[(&str, &str)], basic: Option<&App>, keep_refresh: Option<&str>) -> Result<Tokens, String> {
    let mut request = ureq::post(TOKEN_ENDPOINT).timeout(Duration::from_secs(20));
    if let Some(app) = basic {
        let pair = format!("{}:{}", app.client_id.trim(), app.client_secret.trim());
        request = request.set("Authorization", &format!("Basic {}", standard_base64(pair.as_bytes())));
    }
    let answer: serde_json::Value = match request.send_form(form) {
        Ok(r) => r.into_json().map_err(|e| e.to_string())?,
        Err(ureq::Error::Status(_, r)) => {
            let body: serde_json::Value = r.into_json().unwrap_or_default();
            let why = body["error_description"].as_str().or(body["error"].as_str()).unwrap_or("no reason given");
            return Err(format!("Tidal refused: {why}. Check the client id and secret from {DEVELOPER_PORTAL}."));
        }
        Err(e) => return Err(format!("Could not reach Tidal: {e}")),
    };
    Ok(Tokens {
        access: answer["access_token"].as_str().ok_or("Tidal sent no access token.")?.to_string(),
        refresh: answer["refresh_token"].as_str().map(String::from).or(keep_refresh.map(String::from)),
        expires_at: crate::store::now_seconds() + answer["expires_in"].as_i64().unwrap_or(3600) - 60,
    })
}

fn standard_base64(bytes: &[u8]) -> String {
    let url = crate::oauth::base64url(bytes).replace('-', "+").replace('_', "/");
    let padding = (4 - url.len() % 4) % 4;
    url + &"=".repeat(padding)
}

/// A token for the catalogue alone, from the application's own id and secret.
pub fn catalogue_token(app: &App) -> Result<Tokens, String> {
    token(&[("grant_type", "client_credentials")], Some(app), None)
}

/// Signs the listener in with Tidal's own sign-in page.
pub fn sign_in(app: &App, open: impl Fn(&str)) -> Result<Tokens, String> {
    let (verifier, challenge) = crate::oauth::pkce();
    let state = crate::oauth::state();
    let url = format!(
        "https://login.tidal.com/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&code_challenge_method=S256&code_challenge={challenge}&state={state}",
        app.client_id.trim(),
        crate::ytdlp::percent_encode(&redirect_uri()),
        crate::ytdlp::percent_encode(SCOPES).replace('+', "%20"),
    );
    let code = crate::oauth::wait_for_code("Tidal", &url, REDIRECT_PORT, REDIRECT_PATH, &state, open)?;
    token(
        &[
            ("grant_type", "authorization_code"),
            ("client_id", app.client_id.trim()),
            ("code", &code),
            ("redirect_uri", &redirect_uri()),
            ("code_verifier", &verifier),
        ],
        None,
        None,
    )
}

/// A JSON:API document: what was asked for, and the resources it carried along by type and id.
struct Document {
    data: Vec<serde_json::Value>,
    included: HashMap<(String, String), serde_json::Value>,
    next: Option<String>,
}

impl Document {
    fn related(&self, item: &serde_json::Value, name: &str) -> Vec<&serde_json::Value> {
        item["relationships"][name]["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| self.included.get(&(r["type"].as_str()?.to_string(), r["id"].as_str()?.to_string())))
            .collect()
    }
}

/// A signed-in or catalogue-only session: its tokens, refreshed where they can be, and its country.
#[derive(Clone, Debug)]
pub struct Session {
    pub app: App,
    pub tokens: std::sync::Arc<std::sync::Mutex<Tokens>>,
    pub country: String,
}

impl Session {
    fn access(&self) -> Result<String, String> {
        let mut tokens = self.tokens.lock().unwrap();
        if crate::store::now_seconds() >= tokens.expires_at {
            *tokens = match tokens.refresh.clone() {
                Some(refresh) => token(
                    &[("grant_type", "refresh_token"), ("client_id", self.app.client_id.trim()), ("refresh_token", &refresh)],
                    None,
                    Some(&refresh),
                )?,
                None => catalogue_token(&self.app)?,
            };
        }
        Ok(tokens.access.clone())
    }

    fn document(&self, url: &str) -> Result<Document, String> {
        let url = if url.starts_with("http") { url.to_string() } else { format!("{API}{url}") };
        let body: serde_json::Value = match ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", self.access()?))
            .set("Accept", "application/vnd.api+json")
            .timeout(Duration::from_secs(20))
            .call()
        {
            Ok(r) => r.into_json().map_err(|e| e.to_string())?,
            Err(ureq::Error::Status(404, _)) => return Err("Tidal has nothing at that address.".into()),
            Err(ureq::Error::Status(401 | 403, _)) => return Err("Tidal needs signing in again.".into()),
            Err(e) => return Err(format!("Tidal did not answer: {e}")),
        };
        let data = match &body["data"] {
            serde_json::Value::Array(list) => list.clone(),
            serde_json::Value::Null => vec![],
            single => vec![single.clone()],
        };
        let included = body["included"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| Some(((r["type"].as_str()?.to_string(), r["id"].as_str()?.to_string()), r.clone())))
            .collect();
        Ok(Document { data, included, next: body["links"]["next"].as_str().map(|n| format!("{API}{n}")) })
    }

    fn track_of(&self, item: &serde_json::Value, found: &Document) -> Option<Track> {
        if item["type"].as_str() != Some("tracks") {
            return None;
        }
        let id = item["id"].as_str()?;
        let a = &item["attributes"];
        let name = a["title"].as_str().filter(|t| !t.is_empty())?;
        // Tidal keeps a version -- "Remastered", "Live" -- apart from the title; a live take is another recording.
        let title = match a["version"].as_str().filter(|v| !v.is_empty()) {
            Some(version) => format!("{name} ({version})"),
            None => name.to_string(),
        };
        let artists: Vec<Artist> = found
            .related(item, "artists")
            .iter()
            .filter_map(|r| {
                let name = r["attributes"]["name"].as_str()?;
                Some(Artist { id: r["id"].as_str().unwrap_or(name).to_string(), name: name.to_string(), provider: ProviderType::Tidal })
            })
            .collect();
        if artists.is_empty() {
            return None;
        }
        let album = found.related(item, "albums").first().and_then(|r| {
            Some(Album {
                id: r["id"].as_str()?.to_string(),
                title: r["attributes"]["title"].as_str()?.to_string(),
                artists: artists.iter().take(1).cloned().collect(),
                provider: ProviderType::Tidal,
                artwork_url: None,
            })
        });
        Some(Track {
            provider: ProviderType::Tidal,
            id: id.to_string(),
            title,
            artists,
            album,
            duration_ms: iso_duration_ms(a["duration"].as_str().unwrap_or("")),
            artwork_url: None,
            source_url: format!("https://tidal.com/browse/track/{id}"),
        })
    }

    /// Songs by their ids, with their artists and albums.
    fn tracks(&self, ids: &[String]) -> Result<Vec<Track>, String> {
        let mut out = Vec::new();
        for chunk in ids.chunks(20) {
            let filter = chunk.iter().map(|id| format!("filter%5Bid%5D={id}")).collect::<Vec<_>>().join("&");
            let found = self.document(&format!("/tracks?countryCode={}&{filter}&include=artists,albums", self.country))?;
            let by_id: HashMap<&str, Track> = found
                .data
                .iter()
                .filter_map(|t| Some((t["id"].as_str()?, self.track_of(t, &found)?)))
                .collect();
            out.extend(chunk.iter().filter_map(|id| by_id.get(id.as_str()).cloned()));
        }
        Ok(out)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, String> {
        let text = crate::ytdlp::percent_encode(query.trim()).replace('+', "%20");
        let found = self.document(&format!("/searchResults/{text}?countryCode={}&explicitFilter=include&include=tracks", self.country))?;
        let ids: Vec<String> = found
            .data
            .first()
            .map(|r| r["relationships"]["tracks"]["data"].as_array().into_iter().flatten().filter_map(|t| t["id"].as_str().map(String::from)).take(limit).collect())
            .unwrap_or_default();
        self.tracks(&ids)
    }

    pub fn me(&self) -> Result<(String, String, String), String> {
        let found = self.document("/users/me")?;
        let user = found.data.first().ok_or("Tidal would not say whose account this is.")?;
        let a = &user["attributes"];
        let name = a["username"].as_str().filter(|n| !n.is_empty()).map(String::from).unwrap_or_else(|| "Tidal".into());
        Ok((user["id"].as_str().unwrap_or("").to_string(), name, a["country"].as_str().unwrap_or("US").to_uppercase()))
    }

    /// The listener's collection: its songs, and its playlists.
    pub fn collection(&self, user_id: &str) -> Result<(Vec<Track>, Vec<Playlist>), String> {
        let mut ids = Vec::new();
        let mut next = Some(format!("/userCollections/{user_id}/relationships/tracks?countryCode={}&locale=en-US", self.country));
        while let Some(url) = next.take() {
            if ids.len() >= 2_000 {
                break;
            }
            let page = self.document(&url)?;
            ids.extend(page.data.iter().filter_map(|t| t["id"].as_str().map(String::from)));
            next = page.next;
        }
        let tracks = self.tracks(&ids)?;
        let found = self.document(&format!("/userCollections/{user_id}/relationships/playlists?countryCode={}&locale=en-US&include=playlists", self.country))?;
        let playlists = found
            .data
            .iter()
            .filter_map(|r| found.included.get(&("playlists".to_string(), r["id"].as_str()?.to_string())))
            .filter_map(|p| {
                let id = p["id"].as_str()?;
                Some(Playlist {
                    id: id.to_string(),
                    title: p["attributes"]["name"].as_str()?.to_string(),
                    provider: ProviderType::Tidal,
                    owner_name: None,
                    artwork_url: None,
                    tracks: vec![],
                    source_url: Some(format!("https://tidal.com/browse/playlist/{id}")),
                    track_count: p["attributes"]["numberOfItems"].as_i64(),
                })
            })
            .collect();
        Ok((tracks, playlists))
    }

    /// A playlist's songs, by its link.
    pub fn playlist(&self, id: &str) -> Result<Vec<Track>, String> {
        let mut ids = Vec::new();
        let mut next = Some(format!("/playlists/{id}/relationships/items?countryCode={}", self.country));
        while let Some(url) = next.take() {
            if ids.len() >= 5_000 {
                break;
            }
            let page = self.document(&url)?;
            ids.extend(page.data.iter().filter(|t| t["type"].as_str() == Some("tracks")).filter_map(|t| t["id"].as_str().map(String::from)));
            next = page.next;
        }
        self.tracks(&ids)
    }
}

/// `PT3M45S`, `PT1H2M3.5S` as milliseconds.
pub fn iso_duration_ms(text: &str) -> Option<i64> {
    let rest = text.strip_prefix("PT")?;
    let mut total = 0.0;
    let mut number = String::new();
    for c in rest.chars() {
        match c {
            'H' => total += number.parse::<f64>().ok()? * 3600.0,
            'M' => total += number.parse::<f64>().ok()? * 60.0,
            'S' => total += number.parse::<f64>().ok()?,
            _ => {
                number.push(c);
                continue;
            }
        }
        number.clear();
    }
    (total > 0.0).then_some((total * 1000.0) as i64)
}

/// `tidal.com/browse/playlist/<id>` or `tidal.com/playlist/<id>`: the playlist's id.
pub fn playlist_id(url: &str) -> Option<String> {
    let lower = url.to_lowercase();
    if !lower.contains("tidal.com/") {
        return None;
    }
    let after = url.split("/playlist/").nth(1)?;
    Some(after.split(['?', '#', '/']).next()?.to_string()).filter(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_lengths_are_read() {
        assert_eq!(iso_duration_ms("PT3M45S"), Some(225_000));
        assert_eq!(iso_duration_ms("PT1H2M3.5S"), Some(3_723_500));
        assert_eq!(iso_duration_ms("3:45"), None);
    }

    #[test]
    fn basic_authorization_is_padded_base64() {
        assert_eq!(standard_base64(b"id:secret"), "aWQ6c2VjcmV0");
        assert_eq!(standard_base64(b"ab"), "YWI=");
    }

    #[test]
    fn playlist_links_give_their_id() {
        assert_eq!(playlist_id("https://tidal.com/browse/playlist/1b087082-ab54-4e7d-a0d3-b1cf1cf18ebc?u").as_deref(), Some("1b087082-ab54-4e7d-a0d3-b1cf1cf18ebc"));
        assert_eq!(playlist_id("https://example.com/playlist/x"), None);
    }
}
