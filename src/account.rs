//! The Noctorium account: one sign-in for the players and the website, and the listening statistics it
//! keeps -- Noctorium-Service, spoken to as Noctorium-Base's `NoctoriumAccountClient` speaks to it.

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub fn service_url() -> String {
    std::env::var("NOCTORIUM_SERVICE_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "https://noctorium-service.vercel.app".into())
}

#[derive(Clone, Debug, PartialEq)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub streams: i64,
    pub unique_tracks: i64,
    pub artists: i64,
    pub hours: f64,
}

/// One listen, as the service counts it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayReport {
    pub client_id: String,
    pub provider: String,
    pub track_id: String,
    pub title: String,
    pub artist: String,
    pub ms_played: i64,
    pub played_at: String,
}

/// Whether a listen counts: half a minute, or half the song for anything shorter. Without it, skipping
/// through a playlist would read afterwards as having listened to all of it.
pub fn listen_counts(heard_ms: i64, duration_ms: i64) -> bool {
    let needed = if duration_ms > 0 { (duration_ms / 2).min(30_000) } else { 30_000 };
    heard_ms >= needed
}

fn send(method: &str, path: &str, token: Option<&str>, body: Option<serde_json::Value>) -> Result<(u16, serde_json::Value), String> {
    let mut request = ureq::request(method, &format!("{}{path}", service_url()))
        .set("Accept", "application/json")
        .set("User-Agent", concat!("Noctorium-Rust/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20));
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    let reply = match body {
        Some(body) => request.send_json(body),
        None => request.call(),
    };
    match reply {
        Ok(r) => {
            let status = r.status();
            Ok((status, r.into_json().unwrap_or(serde_json::Value::Null)))
        }
        Err(ureq::Error::Status(status, r)) => Ok((status, r.into_json().unwrap_or(serde_json::Value::Null))),
        Err(_) => Err("Could not reach the Noctorium service.".into()),
    }
}

fn user_from(node: &serde_json::Value) -> Option<User> {
    let id = node["id"].as_i64().or_else(|| node["id"].as_str()?.parse().ok())?;
    let email = node["email"].as_str()?.to_string();
    let display_name = node["displayName"]
        .as_str()
        .map(String::from)
        .unwrap_or_else(|| email.split('@').next().unwrap_or("").to_string());
    Some(User { id, email, display_name })
}

fn authenticate(path: &str, body: serde_json::Value) -> Result<(User, String), String> {
    let (status, answer) = send("POST", path, None, Some(body))?;
    if !(200..300).contains(&status) {
        return Err(answer["error"].as_str().map(String::from).unwrap_or_else(|| format!("The service refused that (HTTP {status}).")));
    }
    let token = answer["token"].as_str().filter(|t| !t.is_empty()).ok_or("The service replied with something unexpected.")?;
    let user = user_from(&answer["user"]).ok_or("The service replied with something unexpected.")?;
    Ok((user, token.to_string()))
}

pub fn sign_up(email: &str, password: &str, display_name: &str) -> Result<(User, String), String> {
    authenticate(
        "/api/auth/signup",
        serde_json::json!({"email": email.trim(), "password": password, "displayName": display_name.trim()}),
    )
}

pub fn log_in(email: &str, password: &str) -> Result<(User, String), String> {
    authenticate("/api/auth/login", serde_json::json!({"email": email.trim(), "password": password}))
}

pub fn who_am_i(token: &str) -> Result<User, String> {
    let (status, answer) = send("GET", "/api/auth/me", Some(token), None)?;
    if status == 401 {
        return Err("The Noctorium account needs signing in again.".into());
    }
    user_from(&answer["user"]).ok_or_else(|| "The service did not say who is signed in.".into())
}

pub fn stats(token: &str) -> Result<Stats, String> {
    let (status, body) = send("GET", "/api/stats", Some(token), None)?;
    if !(200..300).contains(&status) {
        return Err(format!("The statistics could not be read (HTTP {status})."));
    }
    Ok(Stats {
        streams: body["streams"].as_i64().unwrap_or(0),
        unique_tracks: body["uniqueTracks"].as_i64().unwrap_or(0),
        artists: body["artists"].as_i64().unwrap_or(0),
        hours: body["hours"].as_f64().unwrap_or(0.0),
    })
}

/// The key the listener's devices sign Connect with, the same for all of them: from the account.
pub fn connect_key(token: &str) -> Result<String, String> {
    let (status, body) = send("GET", "/api/connect/key", Some(token), None)?;
    if !(200..300).contains(&status) {
        return Err(format!("The Noctorium service would not give the Connect key (HTTP {status})."));
    }
    body["key"].as_str().filter(|k| !k.is_empty()).map(String::from).ok_or_else(|| "The service sent no Connect key.".into())
}

/// Sends listens. True when the service took them.
pub fn submit(plays: &[PlayReport], token: &str) -> bool {
    if plays.is_empty() {
        return true;
    }
    matches!(send("POST", "/api/plays", Some(token), Some(serde_json::json!({"plays": plays}))), Ok((200..=299, _)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_listen_counts_after_half_a_minute_or_half_a_short_song() {
        assert!(!listen_counts(29_000, 200_000));
        assert!(listen_counts(30_000, 200_000));
        assert!(listen_counts(10_000, 20_000));
        assert!(!listen_counts(9_000, 20_000));
    }

    #[test]
    fn a_report_is_written_as_the_service_reads_it() {
        let report = PlayReport {
            client_id: "c".into(),
            provider: "YOUTUBE_MUSIC".into(),
            track_id: "x".into(),
            title: "t".into(),
            artist: "a".into(),
            ms_played: 1,
            played_at: "2026-11-05T00:00:00Z".into(),
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["msPlayed"], 1);
        assert_eq!(json["playedAt"], "2026-11-05T00:00:00Z");
    }
}
