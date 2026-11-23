//! The Noctorium account: one sign-in for the players and the website, and the listening statistics it
//! keeps -- Noctorium-Service, spoken to as Noctorium-Base's `NoctoriumAccountClient` speaks to it.

use serde::{Deserialize, Serialize};
use std::time::Duration;

fn service_override() -> Option<String> {
    std::env::var("NOCTORIUM_SERVICE_URL")
        .ok()
        .map(|u| u.trim().trim_end_matches('/').to_string())
        .filter(|u| !u.is_empty())
}

/// Listening statistics: listens reported, figures read, what is playing.
pub fn service_url() -> String {
    service_override().unwrap_or_else(|| "https://stats.noctorium.xyz".into())
}

/// Signing in and everything else about the account: its own server since 0.39.
pub fn account_url() -> String {
    service_override().unwrap_or_else(|| "https://account.noctorium.xyz".into())
}

/// Which server [path] belongs to: listening goes to the stats server, the rest to the account server.
fn server_for(path: &str) -> String {
    let stats = ["/api/stats", "/api/plays", "/api/now-playing", "/api/scrobble"]
        .iter()
        .any(|p| path == *p || path.starts_with(&format!("{p}?")));
    if stats { service_url() } else { account_url() }
}

/// Two-step verification, asked for after the password: the ways the account can be verified ("totp" for an
/// authenticator app, "telegram" for a code the login bot sends), whether strict mode needs them all, what is proved
/// already, and the service's words about the last try.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Challenge {
    pub ticket: String,
    pub methods: Vec<String>,
    pub strict: bool,
    pub done: Vec<String>,
    pub message: Option<String>,
}

impl Challenge {
    /// What is still to be given: in strict mode every way not yet proved, otherwise any one of them.
    pub fn still_needed(&self) -> Vec<String> {
        if self.strict {
            self.methods.iter().filter(|m| !self.done.contains(m)).cloned().collect()
        } else {
            self.methods.clone()
        }
    }
}

/// What a sign-in came to: signed in, or a code is asked for first.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    SignedIn(User, String),
    Verify(Challenge),
}

fn challenge_from(answer: &serde_json::Value, message: Option<String>) -> Option<Challenge> {
    let ticket = answer["challenge"].as_str().filter(|t| !t.is_empty())?.to_string();
    let list = |name: &str| {
        answer[name]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    Some(Challenge { ticket, methods: list("methods"), strict: answer["strict"].as_bool() == Some(true), done: list("done"), message })
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
    let mut request = ureq::request(method, &format!("{}{path}", server_for(path)))
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

fn authenticate(path: &str, body: serde_json::Value) -> Result<Outcome, String> {
    let (status, answer) = send("POST", path, None, Some(body))?;
    // Two-step verification: the password was right, and a code comes next. The first answer's `error` is worded for
    // apps too old to ask for one; a later one's says what was wrong with the code.
    let words = answer["error"].as_str().filter(|_| path.ends_with("/verify")).map(String::from);
    if let Some(asked) = challenge_from(&answer, words) {
        return Ok(Outcome::Verify(asked));
    }
    if !(200..300).contains(&status) {
        return Err(answer["error"].as_str().map(String::from).unwrap_or_else(|| format!("The service refused that (HTTP {status}).")));
    }
    let token = answer["token"].as_str().filter(|t| !t.is_empty()).ok_or("The service replied with something unexpected.")?;
    let user = user_from(&answer["user"]).ok_or("The service replied with something unexpected.")?;
    Ok(Outcome::SignedIn(user, token.to_string()))
}

/// The codes for [asked]'s sign-in; whichever are given count.
pub fn verify(asked: &Challenge, totp: &str, telegram: &str, backup: &str) -> Result<Outcome, String> {
    let mut body = serde_json::json!({ "challenge": asked.ticket });
    for (name, code) in [("totp", totp), ("telegram", telegram), ("backup", backup)] {
        if !code.trim().is_empty() {
            body[name] = code.trim().into();
        }
    }
    authenticate("/api/auth/challenge/verify", body)
}

/// Asks the login bot to send [asked]'s code on Telegram.
pub fn send_telegram_code(asked: &Challenge) -> Result<(), String> {
    let (status, answer) = send("POST", "/api/auth/challenge/telegram", None, Some(serde_json::json!({ "challenge": asked.ticket })))?;
    if (200..300).contains(&status) {
        return Ok(());
    }
    Err(answer["error"].as_str().unwrap_or("The code could not be sent.").to_string())
}

pub fn sign_up(email: &str, password: &str, display_name: &str) -> Result<Outcome, String> {
    authenticate(
        "/api/auth/signup",
        serde_json::json!({"email": email.trim(), "password": password, "displayName": display_name.trim()}),
    )
}

pub fn log_in(email: &str, password: &str) -> Result<Outcome, String> {
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

/// Signing in from a browser -- with Discord, Telegram or email: the code to show, the page to open, and the
/// secret this app alone keeps to collect its token with once the listener has allowed it there.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceStart {
    pub code: String,
    pub poll_token: String,
    pub url: String,
    pub expires_in: u64,
    pub interval: u64,
}

pub fn start_device_sign_in() -> Result<DeviceStart, String> {
    let (status, body) = send("POST", "/api/auth/device", None, Some(serde_json::json!({})))?;
    if !(200..300).contains(&status) {
        return Err(body["error"].as_str().map(String::from).unwrap_or_else(|| format!("The service refused that (HTTP {status}).")));
    }
    let text = |key: &str| body[key].as_str().filter(|v| !v.is_empty()).map(String::from).ok_or("The service replied with something unexpected.");
    Ok(DeviceStart {
        code: text("code")?,
        poll_token: text("pollToken")?,
        url: text("url")?,
        expires_in: body["expiresIn"].as_u64().unwrap_or(600),
        interval: body["interval"].as_u64().unwrap_or(3).clamp(1, 30),
    })
}

#[derive(Debug, PartialEq)]
pub enum DevicePoll {
    Waiting,
    SignedIn(User, String),
    /// Expired, already used or refused: the wait is over.
    Over(String),
    /// Not reached this time; worth asking again.
    Unreachable,
}

fn device_poll_from(status: u16, body: &serde_json::Value) -> DevicePoll {
    match status {
        202 => DevicePoll::Waiting,
        200..=299 => match (body["token"].as_str().filter(|t| !t.is_empty()), user_from(&body["user"])) {
            (Some(token), Some(user)) => DevicePoll::SignedIn(user, token.to_string()),
            _ => DevicePoll::Over("The service replied with something unexpected.".into()),
        },
        500..=599 => DevicePoll::Unreachable,
        _ => DevicePoll::Over(body["error"].as_str().unwrap_or("That sign-in did not go through. Start again.").to_string()),
    }
}

/// Waits, asking every few seconds, until the listener allows [start] in the browser, it lapses, or [cancelled] is set
/// (then `Ok(None)`). Blocking: run it off the interface's thread.
pub fn wait_for_device_sign_in(start: &DeviceStart, cancelled: &std::sync::atomic::AtomicBool) -> Result<Option<(User, String)>, String> {
    let until = std::time::Instant::now() + Duration::from_secs(start.expires_in);
    let path = format!("/api/auth/device/poll?token={}", url::form_urlencoded::byte_serialize(start.poll_token.as_bytes()).collect::<String>());
    while std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_secs(start.interval));
        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(None);
        }
        let poll = match send("GET", &path, None, None) {
            Ok((status, body)) => device_poll_from(status, &body),
            Err(_) => DevicePoll::Unreachable,
        };
        match poll {
            DevicePoll::SignedIn(user, token) => return Ok(Some((user, token))),
            DevicePoll::Over(why) => return Err(why),
            DevicePoll::Waiting | DevicePoll::Unreachable => {}
        }
    }
    Err("The code expired. Start again for a new one.".into())
}

#[cfg(test)]
mod device_tests {
    use super::*;

    #[test]
    fn a_challenge_is_read_and_strict_mode_needs_what_is_not_done() {
        let answer = serde_json::json!({"error": "old apps' words", "challenge": "t1", "methods": ["totp", "telegram"], "strict": true, "done": ["totp"]});
        let asked = challenge_from(&answer, None).unwrap();
        assert_eq!(asked.still_needed(), vec!["telegram".to_string()]);
        assert_eq!(asked.message, None);
        let either = Challenge { strict: false, done: vec![], ..asked };
        assert_eq!(either.still_needed().len(), 2);
        assert!(challenge_from(&serde_json::json!({"error": "no"}), None).is_none());
    }

    #[test]
    fn listening_goes_to_stats_and_the_rest_to_the_account() {
        assert_eq!(server_for("/api/plays"), service_url());
        assert_eq!(server_for("/api/stats?range=all"), service_url());
        assert_eq!(server_for("/api/auth/login"), account_url());
        assert_eq!(server_for("/api/scrobblers/lastfm"), account_url());
    }

    #[test]
    fn linked_scrobblers_are_read_and_a_missing_one_is_none() {
        let body = serde_json::json!({"lastfm": {"username": "ana", "sessionKey": "sk"}, "listenbrainz": null});
        assert_eq!(
            linked_from(&body),
            LinkedScrobblers { last_fm: Some(("ana".into(), "sk".into())), listen_brainz: None }
        );
    }

    #[test]
    fn device_polls_read_as_the_service_means_them() {
        assert_eq!(device_poll_from(202, &serde_json::json!({"pending": true})), DevicePoll::Waiting);
        // An account made with Discord may have no email: it comes back empty, and still signs in.
        let ok = serde_json::json!({"token": "tok", "user": {"id": 9, "email": "", "displayName": "Disco"}});
        assert_eq!(device_poll_from(200, &ok), DevicePoll::SignedIn(User { id: 9, email: String::new(), display_name: "Disco".into() }, "tok".into()));
        assert_eq!(device_poll_from(410, &serde_json::json!({"error": "Expired."})), DevicePoll::Over("Expired.".into()));
        assert_eq!(device_poll_from(503, &serde_json::Value::Null), DevicePoll::Unreachable);
        assert!(matches!(device_poll_from(200, &serde_json::json!({})), DevicePoll::Over(_)));
    }
}

/// Last.fm and ListenBrainz as linked to the account, each as (username, session key or token): what this program
/// connects itself with when it signs in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LinkedScrobblers {
    pub last_fm: Option<(String, String)>,
    pub listen_brainz: Option<(String, String)>,
}

fn linked_from(body: &serde_json::Value) -> LinkedScrobblers {
    let pair = |part: &serde_json::Value, secret: &str| {
        Some((part["username"].as_str()?.to_string(), part[secret].as_str()?.to_string()))
    };
    LinkedScrobblers {
        last_fm: pair(&body["lastfm"], "sessionKey"),
        listen_brainz: pair(&body["listenbrainz"], "token"),
    }
}

pub fn scrobblers(token: &str) -> Result<LinkedScrobblers, String> {
    let (status, body) = send("GET", "/api/scrobblers", Some(token), None)?;
    if !(200..300).contains(&status) {
        return Err(format!("The Noctorium service would not say what is linked (HTTP {status})."));
    }
    Ok(linked_from(&body))
}

/// Saves a connection made in this program to the account, for every other Noctorium to take.
pub fn save_scrobbler(token: &str, site: crate::scrobble::Site, account: &crate::scrobble::Account) -> bool {
    let (path, body) = match site {
        crate::scrobble::Site::LastFm => (
            "/api/scrobblers/lastfm",
            serde_json::json!({"username": account.username, "sessionKey": account.secret}),
        ),
        crate::scrobble::Site::ListenBrainz => ("/api/scrobblers/listenbrainz", serde_json::json!({"token": account.secret})),
        crate::scrobble::Site::LibreFm => return false,
    };
    matches!(send("PUT", path, Some(token), Some(body)), Ok((200..=299, _)))
}

/// What is playing now, for the Discord status and Telegram bio the account service shows: the song, or null when
/// the music stops. Fire and forget; the next one corrects a missed one.
pub fn now_playing(token: &str, track: Option<serde_json::Value>) {
    let _ = send("POST", "/api/now-playing", Some(token), Some(serde_json::json!({ "track": track })));
}

/// A track as the service's now-playing reads it.
pub fn now_playing_track(track: &crate::domain::Track, position_ms: i64, duration_ms: i64) -> serde_json::Value {
    let https = |s: &str| s.starts_with("https://");
    let mut value = serde_json::json!({
        "title": track.title,
        "artist": track.artist_line(),
        "provider": serde_json::to_value(track.provider).unwrap_or_default(),
        "positionMs": position_ms.max(0),
    });
    if duration_ms > 0 {
        value["durationMs"] = duration_ms.into();
    }
    if let Some(album) = &track.album {
        value["album"] = album.title.clone().into();
    }
    if let Some(art) = track.artwork_url.as_deref().filter(|a| https(a)) {
        value["artworkUrl"] = art.into();
    }
    if https(&track.source_url) {
        value["url"] = track.source_url.clone().into();
    }
    value
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

    #[test]
    fn now_playing_carries_the_song_and_only_https_links() {
        let track = crate::domain::Track {
            provider: crate::domain::ProviderType::YoutubeMusic,
            id: "x".into(),
            title: "One More Time".into(),
            artists: vec![],
            album: None,
            duration_ms: Some(320_000),
            artwork_url: Some("http://insecure/a.jpg".into()),
            source_url: "https://music.youtube.com/watch?v=x".into(),
        };
        let value = now_playing_track(&track, 20_000, 320_000);
        assert_eq!(value["title"], "One More Time");
        assert_eq!(value["positionMs"], 20_000);
        assert_eq!(value["durationMs"], 320_000);
        assert!(value.get("artworkUrl").is_none(), "an http cover was sent");
        assert_eq!(value["url"], "https://music.youtube.com/watch?v=x");
    }
}
