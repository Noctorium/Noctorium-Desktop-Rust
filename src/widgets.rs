//! Stream widgets: what Noctorium is playing, over a stream in OBS or Streamlabs, as a browser source. The
//! Kotlin players' seven looks, page for page, served from a small server of Noctorium's own; off until it is
//! switched on, and on the computer's loopback address unless the listener allows the network.
//!
//! - `/` and `/widget/<look>`: a page, with its script in it.
//! - `/api/now-playing`: what is playing, which a page reads about once a second.
//! - `/api/artwork?k=<key>`: the song's cover, fetched once and kept.

use crate::domain::Track;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const DEFAULT_PORT: u16 = 4851;
const SCRIPT_MARK: &str = "<!-- noctorium:widget-script -->";
const SCRIPT: &str = include_str!("../assets/widgets/widget.js");

/// The looks, with their names, what they are, and the size a browser source wants for them.
pub const LOOKS: [(&str, &str, &str, u32, u32); 7] = [
    ("glass", "Glass", "A frosted dark card with the cover, the title and the artist, the service's logo and a thin progress bar.", 640, 168),
    ("minimal", "Minimal", "Only words: a small \"Now playing\", the title and the artist, white with a soft shadow.", 640, 140),
    ("vinyl", "Vinyl", "A record that turns while the music plays, with the cover as its label and the song beside it.", 640, 220),
    ("neon", "Neon", "A dark see-through strip with a glowing outline, and the song written in light in your accent colour.", 680, 128),
    ("ticker", "Ticker", "A slim bar for the edge of the screen: the artist and the title on one line, over a hairline of progress.", 1280, 44),
    ("afterglow", "Afterglow", "A large cover over a soft glow in its own colours, with the title and the artist beneath.", 360, 500),
    ("windows98", "Windows 98", "A tiny Now Playing window from 1998: a navy title bar, a grey bevelled face, and a progress bar of blocks.", 600, 224),
];

fn page_source(look: &str) -> Option<&'static str> {
    Some(match look {
        "glass" => include_str!("../assets/widgets/glass.html"),
        "minimal" => include_str!("../assets/widgets/minimal.html"),
        "vinyl" => include_str!("../assets/widgets/vinyl.html"),
        "neon" => include_str!("../assets/widgets/neon.html"),
        "ticker" => include_str!("../assets/widgets/ticker.html"),
        "afterglow" => include_str!("../assets/widgets/afterglow.html"),
        "windows98" => include_str!("../assets/widgets/windows98.html"),
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub enabled: bool,
    pub port: u16,
    pub theme: String,
    pub show_cover: bool,
    pub show_progress: bool,
    pub show_service_logo: bool,
    pub hide_when_paused: bool,
    pub hide_after_seconds: u32,
    /// Listens on every address, so another computer in the house -- a streaming PC -- can show it.
    pub allow_network: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Preferences {
            enabled: false,
            port: DEFAULT_PORT,
            theme: "glass".into(),
            show_cover: true,
            show_progress: true,
            show_service_logo: true,
            hide_when_paused: true,
            hide_after_seconds: 5,
            allow_network: false,
        }
    }
}

/// What the widgets show, kept up to date by the window.
#[derive(Clone, Debug, Default)]
pub struct Playing {
    pub track: Option<Track>,
    /// `playing`, `paused`, `loading` or `stopped`.
    pub state: &'static str,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub accent: String,
    pub accent_from_artwork: bool,
}

pub fn track_key(track: &Track) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(track.queue_key().as_bytes()).iter().take(6).map(|b| format!("{b:02x}")).collect()
}

/// The reply to `/api/now-playing`, as Noctorium-Base's `NowPlayingFeed` writes it.
pub fn feed(playing: &Playing, preferences: &Preferences, server_time_ms: i64) -> serde_json::Value {
    let track = playing.track.as_ref().filter(|_| playing.state != "stopped");
    let duration = if playing.duration_ms > 0 { playing.duration_ms } else { track.and_then(|t| t.duration_ms).unwrap_or(0) };
    serde_json::json!({
        "state": if track.is_none() { "stopped" } else { playing.state },
        "playing": track.is_some() && playing.state == "playing",
        "track": track.map(|t| serde_json::json!({
            "key": track_key(t),
            "title": t.title,
            "artist": t.artist_line(),
            "album": t.album.as_ref().map(|a| a.title.clone()).filter(|a| !a.is_empty()),
            "service": {
                "id": t.provider.key_name().to_lowercase(),
                "name": t.provider.display_name(),
                "mark": crate::marks::path(t.provider),
                "viewBox": crate::marks::VIEWPORT,
                "colour": crate::marks::colour(t.provider),
            },
            "artwork": t.artwork_url.as_ref().map(|_| format!("/api/artwork?k={}", track_key(t))),
        })),
        "positionMs": if track.is_none() { 0 } else { playing.position_ms.clamp(0, if duration > 0 { duration } else { i64::MAX }) },
        "durationMs": duration.max(0),
        "serverTime": server_time_ms,
        "accent": playing.accent,
        "accentFromArtwork": playing.accent_from_artwork,
        "options": {
            "theme": preferences.theme,
            "cover": preferences.show_cover,
            "progress": preferences.show_progress,
            "logo": preferences.show_service_logo,
            "hidePaused": preferences.hide_when_paused,
            "hideAfter": preferences.hide_after_seconds,
        }
    })
}

struct Shared {
    playing: Mutex<Playing>,
    preferences: Mutex<Preferences>,
    covers: Mutex<HashMap<String, (String, Vec<u8>)>>,
}

/// The running server. Dropping it stops it.
pub struct Server {
    shared: Arc<Shared>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    pub address: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        // A connection of its own, to wake the accept loop so it sees it should stop.
        let _ = TcpStream::connect_timeout(&self.address.trim_start_matches("http://").parse().unwrap_or(([127, 0, 0, 1], DEFAULT_PORT).into()), Duration::from_millis(200));
    }
}

impl Server {
    pub fn start(preferences: Preferences) -> Result<Server, String> {
        let host = if preferences.allow_network { "0.0.0.0" } else { "127.0.0.1" };
        let listener = TcpListener::bind((host, preferences.port))
            .map_err(|_| format!("Port {} is taken by another program. Choose another one.", preferences.port))?;
        let address = format!("http://127.0.0.1:{}", preferences.port);
        let shared = Arc::new(Shared { playing: Mutex::new(Playing::default()), preferences: Mutex::new(preferences), covers: Mutex::new(HashMap::new()) });
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (serving, stopping) = (shared.clone(), stop.clone());
        std::thread::Builder::new()
            .name("stream-widgets".into())
            .spawn(move || {
                for connection in listener.incoming().flatten() {
                    if stopping.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    let shared = serving.clone();
                    std::thread::spawn(move || {
                        let _ = serve(&shared, connection);
                    });
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Server { shared, stop, address })
    }

    pub fn show(&self, playing: Playing) {
        *self.shared.playing.lock().unwrap() = playing;
    }

    pub fn set_preferences(&self, preferences: Preferences) {
        *self.shared.preferences.lock().unwrap() = preferences;
    }
}

fn serve(shared: &Shared, mut connection: TcpStream) -> std::io::Result<()> {
    connection.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(connection.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;
    let mut host = String::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some(value) = header.split_once(':').filter(|(k, _)| k.eq_ignore_ascii_case("host")).map(|(_, v)| v.trim()) {
            host = value.split(':').next().unwrap_or("").to_lowercase();
        }
    }
    let allow_network = shared.preferences.lock().unwrap().allow_network;
    // A web page that points a name of its own at this computer, to read what somebody is playing, is turned
    // away by the name it gave: only an address, or localhost, is answered.
    let by_address = host == "localhost" || host.parse::<std::net::IpAddr>().is_ok();
    let target = request.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let (status, kind, body): (u16, &str, Vec<u8>) = if !by_address || (!allow_network && host != "localhost" && host != "127.0.0.1") {
        (403, "text/plain", b"Not for this address.".to_vec())
    } else if path == "/" || path == "/widget" || path == "/widget/" || path.starts_with("/widget/") {
        let look = path.strip_prefix("/widget/").map(|l| l.trim_end_matches('/').to_lowercase()).unwrap_or_else(|| shared.preferences.lock().unwrap().theme.clone());
        match page_source(&look) {
            Some(page) => (200, "text/html; charset=utf-8", page.replace(SCRIPT_MARK, &format!("<script>\n{SCRIPT}\n</script>")).into_bytes()),
            None => (404, "text/plain", b"No such look.".to_vec()),
        }
    } else if path == "/api/now-playing" {
        let playing = shared.playing.lock().unwrap().clone();
        let preferences = shared.preferences.lock().unwrap().clone();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
        (200, "application/json", feed(&playing, &preferences, now).to_string().into_bytes())
    } else if path == "/api/artwork" {
        let key = query.split('&').find_map(|p| p.strip_prefix("k=")).unwrap_or("").to_string();
        match artwork(shared, &key) {
            Some((_, bytes)) => (200, "image/jpeg", bytes),
            None => (404, "text/plain", Vec::new()),
        }
    } else {
        (404, "text/plain", Vec::new())
    };
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        _ => "Not Found",
    };
    write!(
        connection,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    connection.write_all(&body)?;
    connection.flush()
}

/// The playing song's cover, fetched the first time a widget asks for it, and kept for the songs after.
fn artwork(shared: &Shared, key: &str) -> Option<(String, Vec<u8>)> {
    if let Some(kept) = shared.covers.lock().unwrap().get(key) {
        return Some(kept.clone());
    }
    let track = shared.playing.lock().unwrap().track.clone()?;
    if track_key(&track) != key {
        return None;
    }
    let response = ureq::get(track.artwork_url.as_ref()?).timeout(Duration::from_secs(15)).call().ok()?;
    let kind = response.content_type().to_string();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(response.into_reader(), 8 << 20), &mut bytes).ok()?;
    let mut covers = shared.covers.lock().unwrap();
    if covers.len() > 50 {
        covers.clear();
    }
    covers.insert(key.to_string(), (kind.clone(), bytes.clone()));
    Some((kind, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Artist, ProviderType};

    #[test]
    fn every_look_has_its_page_with_a_place_for_the_script() {
        for (id, ..) in LOOKS {
            assert!(page_source(id).unwrap().contains(SCRIPT_MARK), "{id}");
        }
    }

    #[test]
    fn the_feed_says_what_plays_as_the_kotlin_players_write_it() {
        let track = Track {
            provider: ProviderType::Soundcloud,
            id: "1".into(),
            title: "Song".into(),
            artists: vec![Artist { id: "a".into(), name: "Band".into(), provider: ProviderType::Soundcloud }],
            album: None,
            duration_ms: Some(200_000),
            artwork_url: Some("https://example.com/c.jpg".into()),
            source_url: "https://soundcloud.com/a/b".into(),
        };
        let playing = Playing { track: Some(track), state: "playing", position_ms: 5_000, duration_ms: 0, accent: "#B47CFF".into(), accent_from_artwork: false };
        let feed = feed(&playing, &Preferences::default(), 1);
        assert_eq!(feed["state"], "playing");
        assert_eq!(feed["track"]["service"]["id"], "soundcloud");
        assert_eq!(feed["durationMs"], 200_000);
        assert!(feed["track"]["artwork"].as_str().unwrap().starts_with("/api/artwork?k="));
        let stopped = Playing { state: "stopped", ..playing };
        assert!(super::feed(&stopped, &Preferences::default(), 1)["track"].is_null());
    }
}
