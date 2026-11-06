//! What is playing, on the listener's Discord profile: Discord's own local connection, a pipe on Windows
//! and a socket elsewhere, spoken to as Noctorium-Base's `DiscordIpcClient` speaks to it, with the card
//! built from the same templates and choices as `DiscordPresence`.

use crate::domain::{format_duration, ProviderType, Track};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// Noctorium's own application on Discord, the one the Kotlin players use, unless one is named.
const BUILT_IN_APPLICATION: &str = "1553165990995693588";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivityKind {
    Listening,
    Playing,
}

impl ActivityKind {
    pub fn name(self) -> &'static str {
        match self {
            ActivityKind::Listening => "Listening to",
            ActivityKind::Playing => "Playing",
        }
    }
    fn code(self) -> u8 {
        match self {
            ActivityKind::Listening => 2,
            ActivityKind::Playing => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Timestamps {
    Progress,
    Elapsed,
    Remaining,
    None,
}

impl Timestamps {
    pub const ALL: [Timestamps; 4] = [Timestamps::Progress, Timestamps::Elapsed, Timestamps::Remaining, Timestamps::None];
    pub fn name(self) -> &'static str {
        match self {
            Timestamps::Progress => "Progress bar",
            Timestamps::Elapsed => "Elapsed",
            Timestamps::Remaining => "Remaining",
            Timestamps::None => "None",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Paused {
    ShowPaused,
    Keep,
    Clear,
}

impl Paused {
    pub const ALL: [Paused; 3] = [Paused::ShowPaused, Paused::Keep, Paused::Clear];
    pub fn name(self) -> &'static str {
        match self {
            Paused::ShowPaused => "Show as paused",
            Paused::Keep => "Leave unchanged",
            Paused::Clear => "Hide the card",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Button {
    pub label: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PresenceSettings {
    pub enabled: bool,
    /// An application of the listener's own, for its name on the card; blank for Noctorium's.
    pub application_id: String,
    pub activity_kind: ActivityKind,
    pub details_template: String,
    pub state_template: String,
    pub large_text_template: String,
    pub show_cover: bool,
    pub small_text_template: String,
    pub timestamps: Timestamps,
    pub paused: Paused,
    pub paused_suffix: String,
    pub first_button: Button,
    pub second_button: Button,
    /// Says only that music is playing, not what.
    pub hide_track_details: bool,
    pub private_details: String,
}

impl Default for PresenceSettings {
    fn default() -> Self {
        PresenceSettings {
            enabled: false,
            application_id: String::new(),
            activity_kind: ActivityKind::Listening,
            details_template: "{title}".into(),
            state_template: "by {artist}".into(),
            large_text_template: "{album}".into(),
            show_cover: true,
            small_text_template: "{provider}".into(),
            timestamps: Timestamps::Progress,
            paused: Paused::ShowPaused,
            paused_suffix: " (paused)".into(),
            first_button: Button { label: "Listen on {provider}".into(), url: "{url}".into() },
            second_button: Button::default(),
            hide_track_details: false,
            private_details: "Listening to music".into(),
        }
    }
}

impl PresenceSettings {
    pub fn application(&self) -> String {
        let own = self.application_id.trim();
        if !own.is_empty() {
            return own.to_string();
        }
        std::env::var("NOCTORIUM_DISCORD_APPLICATION_ID")
            .ok()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| BUILT_IN_APPLICATION.into())
    }
}

struct Context<'a> {
    track: &'a Track,
    position_ms: i64,
    duration_ms: i64,
}

impl Context<'_> {
    fn value(&self, name: &str) -> String {
        match name {
            "title" => self.track.title.clone(),
            "artist" => self.track.artist_line(),
            "album" => self.track.album.as_ref().map(|a| a.title.clone()).unwrap_or_default(),
            "provider" => self.track.provider.display_name().into(),
            "url" => self.track.source_url.clone(),
            "position" => format_duration(self.position_ms),
            "duration" => format_duration(self.duration_ms),
            _ => String::new(),
        }
    }
}

const PLACEHOLDERS: [&str; 7] = ["title", "artist", "album", "provider", "url", "position", "duration"];

/// A template with its `{title}`, `{artist}` and the rest filled in. Empty when every placeholder in it
/// was, and with separators left dangling by an empty value tidied away.
fn render(template: &str, context: &Context) -> String {
    let used: Vec<&str> = PLACEHOLDERS.iter().copied().filter(|p| template.contains(&format!("{{{p}}}"))).collect();
    if !used.is_empty() && used.iter().all(|p| context.value(p).trim().is_empty()) {
        return String::new();
    }
    let mut text = template.to_string();
    for placeholder in used {
        text = text.replace(&format!("{{{placeholder}}}"), &context.value(placeholder));
    }
    let separators: &[char] = &['-', '·', '•', ',', '|', '/'];
    let mut tidied = text.split_whitespace().collect::<Vec<_>>().join(" ");
    while tidied.starts_with(separators) || tidied.ends_with(separators) {
        tidied = tidied.trim_matches(separators).trim().to_string();
    }
    tidied.chars().take(128).collect()
}

fn asset_key(provider: ProviderType) -> &'static str {
    match provider {
        ProviderType::YoutubeMusic => "youtube_music",
        ProviderType::YoutubeVideo => "youtube",
        ProviderType::Soundcloud => "soundcloud",
        ProviderType::Spotify => "spotify",
        ProviderType::Bandcamp => "bandcamp",
        ProviderType::Vk => "vk",
        ProviderType::AppleMusic => "apple_music",
        ProviderType::Deezer => "deezer",
        ProviderType::Tidal => "tidal",
        ProviderType::Local => "local",
    }
}

/// The card for `track`, or None for no card at all.
pub fn activity(
    settings: &PresenceSettings,
    track: &Track,
    position_ms: i64,
    duration_ms: i64,
    playing: bool,
    now_seconds: i64,
) -> Option<serde_json::Value> {
    if !playing && settings.paused == Paused::Clear {
        return None;
    }
    let context = Context { track, position_ms, duration_ms };
    let hidden = settings.hide_track_details;
    let suffix = if !playing && settings.paused == Paused::ShowPaused { settings.paused_suffix.as_str() } else { "" };
    let details = if hidden {
        settings.private_details.chars().take(128).collect()
    } else {
        render(&settings.details_template, &context)
    };
    let state = if hidden { String::new() } else { render(&settings.state_template, &context) };
    let mut card = serde_json::json!({"type": settings.activity_kind.code()});
    if !details.is_empty() {
        card["details"] = format!("{details}{suffix}").chars().take(128).collect::<String>().into();
    }
    if !state.is_empty() {
        card["state"] = state.into();
    }
    if playing && duration_ms > 0 {
        let start = now_seconds - position_ms / 1000;
        let end = now_seconds + (duration_ms - position_ms) / 1000;
        let stamps = match settings.timestamps {
            Timestamps::Progress => Some(serde_json::json!({"start": start, "end": end})),
            Timestamps::Elapsed => Some(serde_json::json!({"start": start})),
            Timestamps::Remaining => Some(serde_json::json!({"end": end})),
            Timestamps::None => None,
        };
        if let Some(stamps) = stamps {
            card["timestamps"] = stamps;
        }
    }
    let mut assets = serde_json::Map::new();
    if settings.show_cover && !hidden {
        if let Some(url) = track.artwork_url.as_ref().filter(|u| u.starts_with("http")) {
            assets.insert("large_image".into(), url.clone().into());
        }
    }
    let large_text = if hidden { String::new() } else { render(&settings.large_text_template, &context) };
    if !large_text.is_empty() {
        assets.insert("large_text".into(), large_text.into());
    }
    if settings.show_cover {
        assets.insert("small_image".into(), asset_key(track.provider).into());
    }
    let small_text = if hidden { String::new() } else { render(&settings.small_text_template, &context) };
    if !small_text.is_empty() {
        assets.insert("small_text".into(), small_text.into());
    }
    if !assets.is_empty() {
        card["assets"] = assets.into();
    }
    if !hidden {
        let buttons: Vec<serde_json::Value> = [&settings.first_button, &settings.second_button]
            .into_iter()
            .filter(|b| !b.label.trim().is_empty() && !b.url.trim().is_empty())
            .filter_map(|b| {
                let (label, url) = (render(&b.label, &context), render(&b.url, &context));
                (!label.is_empty() && url.starts_with("http"))
                    .then(|| serde_json::json!({"label": label.chars().take(31).collect::<String>(), "url": url}))
            })
            .take(2)
            .collect();
        if !buttons.is_empty() {
            card["buttons"] = buttons.into();
        }
    }
    Some(card)
}

fn frame(opcode: u32, payload: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + payload.len());
    bytes.extend(opcode.to_le_bytes());
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload.as_bytes());
    bytes
}

fn connect(application: &str) -> Option<Box<dyn Write + Send>> {
    for index in 0..10 {
        let mut candidate: Box<dyn Write + Send> = match open_endpoint(index) {
            Some(c) => c,
            None => continue,
        };
        let handshake = serde_json::json!({"v": 1, "client_id": application}).to_string();
        if candidate.write_all(&frame(0, &handshake)).and_then(|_| candidate.flush()).is_ok() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(windows)]
fn open_endpoint(index: u32) -> Option<Box<dyn Write + Send>> {
    let pipe = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{index}")).ok()?;
    Some(Box::new(pipe))
}

#[cfg(unix)]
fn open_endpoint(index: u32) -> Option<Box<dyn Write + Send>> {
    let bases: Vec<String> = ["XDG_RUNTIME_DIR", "TMPDIR"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .chain(["/tmp".to_string()])
        .collect();
    for base in bases {
        for path in [
            format!("{base}/discord-ipc-{index}"),
            format!("{base}/app/com.discordapp.Discord/discord-ipc-{index}"),
            format!("{base}/snap.discord/discord-ipc-{index}"),
        ] {
            if let Ok(socket) = std::os::unix::net::UnixStream::connect(&path) {
                return Some(Box::new(socket));
            }
        }
    }
    None
}

/// What the window asks of the presence thread.
pub enum Update {
    /// The card to show, None for none, and the application to show it as.
    Show(Option<serde_json::Value>, String),
    Off,
}

/// Talks to Discord on a thread of its own: connecting when there is something to show, trying again now
/// and then when Discord is not running, and sending no more often than Discord accepts.
pub struct Presence {
    sender: Sender<Update>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Presence {
    pub fn start() -> Presence {
        let (sender, receiver) = channel::<Update>();
        let connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = connected.clone();
        std::thread::Builder::new()
            .name("discord".into())
            .spawn(move || {
                let mut pipe: Option<(Box<dyn Write + Send>, String)> = None;
                let mut wanted: Option<(Option<serde_json::Value>, String)> = None;
                let mut sent_at = Instant::now() - Duration::from_secs(60);
                let mut tried_at = Instant::now() - Duration::from_secs(60);
                let mut dirty = false;
                let mut nonce = 0u64;
                loop {
                    match receiver.recv_timeout(Duration::from_millis(500)) {
                        Ok(Update::Show(card, application)) => {
                            wanted = Some((card, application));
                            dirty = true;
                        }
                        Ok(Update::Off) => {
                            wanted = None;
                            pipe = None;
                            flag.store(false, std::sync::atomic::Ordering::Relaxed);
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                    let Some((card, application)) = wanted.clone() else { continue };
                    if !dirty {
                        continue;
                    }
                    // Another application id means another connection.
                    if pipe.as_ref().is_some_and(|(_, app)| *app != application) {
                        pipe = None;
                    }
                    if pipe.is_none() {
                        if tried_at.elapsed() < Duration::from_secs(15) {
                            continue;
                        }
                        tried_at = Instant::now();
                        pipe = connect(&application).map(|p| (p, application.clone()));
                        flag.store(pipe.is_some(), std::sync::atomic::Ordering::Relaxed);
                        if pipe.is_none() {
                            continue;
                        }
                    }
                    // Discord takes five updates in twenty seconds; one in four is always inside that.
                    if sent_at.elapsed() < Duration::from_secs(4) {
                        continue;
                    }
                    nonce += 1;
                    let payload = serde_json::json!({
                        "cmd": "SET_ACTIVITY",
                        "args": {"pid": std::process::id(), "activity": card},
                        "nonce": nonce.to_string(),
                    })
                    .to_string();
                    let (writer, _) = pipe.as_mut().unwrap();
                    if writer.write_all(&frame(1, &payload)).and_then(|_| writer.flush()).is_ok() {
                        sent_at = Instant::now();
                        dirty = false;
                    } else {
                        pipe = None;
                        flag.store(false, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            })
            .expect("the Discord thread could not be started");
        Presence { sender, connected }
    }

    pub fn show(&self, card: Option<serde_json::Value>, application: String) {
        let _ = self.sender.send(Update::Show(card, application));
    }

    pub fn off(&self) {
        let _ = self.sender.send(Update::Off);
    }

    pub fn connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Artist;

    fn track() -> Track {
        Track {
            provider: ProviderType::YoutubeMusic,
            id: "x".into(),
            title: "Song".into(),
            artists: vec![Artist { id: "a".into(), name: "Band".into(), provider: ProviderType::YoutubeMusic }],
            album: None,
            duration_ms: Some(200_000),
            artwork_url: Some("https://i.ytimg.com/vi/x/hqdefault.jpg".into()),
            source_url: "https://music.youtube.com/watch?v=x".into(),
        }
    }

    #[test]
    fn the_card_says_what_is_playing() {
        let card = activity(&PresenceSettings::default(), &track(), 20_000, 200_000, true, 1_000).unwrap();
        assert_eq!(card["type"], 2);
        assert_eq!(card["details"], "Song");
        assert_eq!(card["state"], "by Band");
        assert_eq!(card["timestamps"]["start"], 980);
        assert_eq!(card["timestamps"]["end"], 1_180);
        assert_eq!(card["buttons"][0]["label"], "Listen on YouTube Music");
        assert!(card["assets"].get("large_text").is_none(), "an empty album leaves no text");
    }

    #[test]
    fn paused_and_private_cards() {
        let settings = PresenceSettings::default();
        let paused = activity(&settings, &track(), 0, 200_000, false, 0).unwrap();
        assert_eq!(paused["details"], "Song (paused)");
        assert!(paused.get("timestamps").is_none());
        let hidden = PresenceSettings { paused: Paused::Clear, ..settings.clone() };
        assert!(activity(&hidden, &track(), 0, 0, false, 0).is_none());
        let private = PresenceSettings { hide_track_details: true, ..settings };
        let card = activity(&private, &track(), 0, 200_000, true, 0).unwrap();
        assert_eq!(card["details"], "Listening to music");
        assert!(card.get("buttons").is_none());
    }

    #[test]
    fn templates_tidy_what_an_empty_value_leaves() {
        let t = track();
        let context = Context { track: &t, position_ms: 0, duration_ms: 0 };
        assert_eq!(render("{album} - {title}", &context), "Song");
        assert_eq!(render("{album}", &context), "");
        assert_eq!(render("plain words", &context), "plain words");
    }

    #[test]
    fn a_frame_is_opcode_length_and_payload() {
        assert_eq!(frame(1, "ab"), vec![1, 0, 0, 0, 2, 0, 0, 0, b'a', b'b']);
    }
}
