//! Connect: Noctorium on one device playing, pausing and handing over the music of Noctorium on another --
//! the desktop, the phone, the terminal player -- on the same network. Noctorium-Base's `connect` package,
//! byte for byte, so this player and the Kotlin ones find and trust each other:
//!
//! - every device announces itself by UDP broadcast every three seconds on 45633-45635, its announcement
//!   signed with HMAC-SHA256 under a key the listener's Noctorium account gives all their devices alike;
//! - each answers a small HTTP interface -- `GET /state`, `GET /queue`, `POST /command` -- whose requests are
//!   signed the same way, timestamped and never replayed.

use crate::domain::{Album, Artist, ProviderType, Track};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DISCOVERY_PORTS: [u16; 3] = [45_633, 45_634, 45_635];
pub const VERSION: i32 = 1;
const ANNOUNCE_EVERY: Duration = Duration::from_secs(3);
const PEER_FORGOTTEN_AFTER: Duration = Duration::from_secs(12);
const CLOCK_TOLERANCE_MS: i64 = 5 * 60 * 1000;
const HEADER_NONCE: &str = "x-noctorium-nonce";
const HEADER_TIMESTAMP: &str = "x-noctorium-timestamp";
const HEADER_AUTH: &str = "x-noctorium-auth";
const HEADER_DEVICE: &str = "x-noctorium-device";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeviceKind {
    Desktop,
    Phone,
    Terminal,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeviceRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub position_ms: i64,
    #[serde(default)]
    pub duration_ms: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementExtra {
    #[serde(default)]
    pub terminal: bool,
    #[serde(default)]
    pub now_playing: Option<NowPlaying>,
    #[serde(default)]
    pub driven_by: Option<DeviceRef>,
    #[serde(default)]
    pub driving: Option<DeviceRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub v: i32,
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub port: u16,
    pub stamp: i64,
    pub tag: String,
    #[serde(default)]
    pub extra: Option<String>,
    #[serde(default)]
    pub extra_tag: Option<String>,
}

impl Announcement {
    fn kind_name(&self) -> &'static str {
        match self.kind {
            DeviceKind::Desktop => "DESKTOP",
            DeviceKind::Phone => "PHONE",
            DeviceKind::Terminal => "TERMINAL",
        }
    }
    pub fn signable(&self) -> String {
        format!("{}|{}|{}|{}|{}|{}", self.v, self.id, self.name, self.kind_name(), self.port, self.stamp)
    }
    pub fn signable_with_extra(&self) -> String {
        format!("{}|{}", self.signable(), self.extra.clone().unwrap_or_default())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireArtist {
    pub id: String,
    pub name: String,
    pub provider: ProviderType,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireTrack {
    pub provider: ProviderType,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub artists: Vec<WireArtist>,
    #[serde(default)]
    pub album_id: Option<String>,
    #[serde(default)]
    pub album_title: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    pub source_url: String,
}

impl WireTrack {
    pub fn of(track: &Track) -> WireTrack {
        WireTrack {
            provider: track.provider,
            id: track.id.clone(),
            title: track.title.clone(),
            artists: track.artists.iter().map(|a| WireArtist { id: a.id.clone(), name: a.name.clone(), provider: a.provider }).collect(),
            album_id: track.album.as_ref().map(|a| a.id.clone()),
            album_title: track.album.as_ref().map(|a| a.title.clone()),
            duration_ms: track.duration_ms,
            artwork_url: track.artwork_url.clone(),
            source_url: track.source_url.clone(),
        }
    }

    pub fn track(&self) -> Track {
        Track {
            provider: self.provider,
            id: self.id.clone(),
            title: self.title.clone(),
            artists: self.artists.iter().map(|a| Artist { id: a.id.clone(), name: a.name.clone(), provider: a.provider }).collect(),
            album: self.album_title.as_ref().map(|t| Album {
                id: self.album_id.clone().unwrap_or_default(),
                title: t.clone(),
                artists: vec![],
                provider: self.provider,
                artwork_url: self.artwork_url.clone(),
            }),
            duration_ms: self.duration_ms,
            artwork_url: self.artwork_url.clone(),
            source_url: self.source_url.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteState {
    pub device_id: String,
    pub device_name: String,
    pub kind: DeviceKind,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub track: Option<WireTrack>,
    #[serde(default)]
    pub position_ms: i64,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default = "one")]
    pub volume: f32,
    #[serde(default)]
    pub queue_size: i32,
    #[serde(default = "minus_one")]
    pub queue_index: i32,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default = "off")]
    pub repeat: String,
    #[serde(default)]
    pub controlled_by: Option<String>,
    #[serde(default)]
    pub controlled_by_id: Option<String>,
    #[serde(default)]
    pub driving: Option<DeviceRef>,
    #[serde(default)]
    pub terminal: bool,
    #[serde(default)]
    pub queue_key: Option<String>,
    #[serde(default)]
    pub tells_whom_it_drives: bool,
}

fn one() -> f32 {
    1.0
}
fn minus_one() -> i32 {
    -1
}
fn off() -> String {
    "OFF".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteQueue {
    pub tracks: Vec<WireTrack>,
    pub index: i32,
    pub key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommandType {
    Play,
    Pause,
    Next,
    Previous,
    Seek,
    Volume,
    Shuffle,
    Repeat,
    TakeOver,
    HandBack,
    Jump,
    Add,
    Remove,
    Move,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    #[serde(rename = "type")]
    pub kind: CommandType,
    #[serde(default)]
    pub position_ms: Option<i64>,
    #[serde(default)]
    pub volume: Option<f32>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub repeat: Option<String>,
    #[serde(default)]
    pub tracks: Vec<WireTrack>,
    #[serde(default = "minus_one")]
    pub index: i32,
    #[serde(default)]
    pub from_device_id: Option<String>,
    #[serde(default)]
    pub from_device_name: Option<String>,
    #[serde(default)]
    pub to_index: Option<i32>,
}

impl Command {
    pub fn new(kind: CommandType) -> Command {
        Command {
            kind,
            position_ms: None,
            volume: None,
            enabled: None,
            repeat: None,
            tracks: vec![],
            index: -1,
            from_device_id: None,
            from_device_name: None,
            to_index: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Handover {
    pub tracks: Vec<WireTrack>,
    pub index: i32,
    pub position_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandReply {
    pub ok: bool,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub state: Option<RemoteState>,
    #[serde(default)]
    pub handover: Option<Handover>,
}

// ---- signatures ------------------------------------------------------------------------------------

pub fn sign(key: &str, message: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(key.as_bytes()).expect("HMAC takes any key");
    mac.update(message.as_bytes());
    mac.finalize().into_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn matches(key: &str, message: &str, presented: &str) -> bool {
    let expected = sign(key, message);
    // Compared whole, so the time taken says nothing about how much of it was right.
    expected.len() == presented.len() && expected.bytes().zip(presented.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

fn sha256_hex(text: &str) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn request_message(method: &str, path: &str, timestamp: i64, nonce: &str, body: &str) -> String {
    format!("{method}\n{path}\n{timestamp}\n{nonce}\n{}", sha256_hex(body))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn nonce() -> String {
    (0..16).map(|_| format!("{:02x}", rand::random::<u8>())).collect()
}

// ---- this device --------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Identity {
    pub id: String,
    pub name: String,
}

/// A device heard on the network.
#[derive(Clone, Debug, PartialEq)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub address: String,
    pub port: u16,
    pub now_playing: Option<NowPlaying>,
    pub seen: Instant,
}

/// What a command asks of this device: the command, and where to send the answer.
pub struct Asked {
    pub command: Command,
    pub reply: std::sync::mpsc::Sender<CommandReply>,
}

/// What this device says of itself, kept up to date by the window.
#[derive(Clone, Debug, Default)]
pub struct Here {
    pub state: Option<RemoteState>,
    pub queue: Option<RemoteQueue>,
    pub now_playing: Option<NowPlaying>,
}

struct Shared {
    key: Mutex<Option<String>>,
    here: Mutex<Here>,
    peers: Mutex<HashMap<String, Peer>>,
    nonces: Mutex<HashMap<String, i64>>,
}

/// Connect running: discovery and the server, until dropped.
pub struct Connect {
    shared: Arc<Shared>,
    pub identity: Identity,
    pub port: u16,
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for Connect {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = TcpStream::connect_timeout(&([127, 0, 0, 1], self.port).into(), Duration::from_millis(200));
    }
}

impl Connect {
    /// Starts discovery and the server. `asked` hears every command from another device.
    pub fn start(identity: Identity, key: String, asked: impl Fn(Asked) + Send + Sync + 'static) -> Result<Connect, String> {
        let server = TcpListener::bind(("0.0.0.0", 0)).map_err(|e| format!("Connect could not listen: {e}"))?;
        let port = server.local_addr().map_err(|e| e.to_string())?.port();
        let socket = DISCOVERY_PORTS
            .iter()
            .find_map(|p| UdpSocket::bind(("0.0.0.0", *p)).ok())
            .ok_or("Connect could not listen for other devices: every discovery port is taken.")?;
        socket.set_broadcast(true).map_err(|e| e.to_string())?;
        socket.set_read_timeout(Some(Duration::from_millis(500))).map_err(|e| e.to_string())?;
        let shared = Arc::new(Shared {
            key: Mutex::new(Some(key)),
            here: Mutex::new(Here::default()),
            peers: Mutex::new(HashMap::new()),
            nonces: Mutex::new(HashMap::new()),
        });
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let asked = Arc::new(asked);

        // Announcing, and hearing the others.
        {
            let (shared, stop, identity) = (shared.clone(), stop.clone(), identity.clone());
            let sender = socket.try_clone().map_err(|e| e.to_string())?;
            std::thread::Builder::new()
                .name("connect-announce".into())
                .spawn(move || {
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        if let Some(bytes) = announcement(&shared, &identity, port).and_then(|a| serde_json::to_vec(&a).ok()) {
                            for target in DISCOVERY_PORTS {
                                let _ = sender.send_to(&bytes, ("255.255.255.255", target));
                            }
                        }
                        shared.peers.lock().unwrap().retain(|_, p| p.seen.elapsed() < PEER_FORGOTTEN_AFTER);
                        std::thread::sleep(ANNOUNCE_EVERY);
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        {
            let (shared, stop, own) = (shared.clone(), stop.clone(), identity.id.clone());
            std::thread::Builder::new()
                .name("connect-listen".into())
                .spawn(move || {
                    let mut buffer = [0u8; 2048];
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        let Ok((length, from)) = socket.recv_from(&mut buffer) else { continue };
                        let Ok(heard) = serde_json::from_slice::<Announcement>(&buffer[..length]) else { continue };
                        if heard.id == own {
                            continue;
                        }
                        let Some(key) = shared.key.lock().unwrap().clone() else { continue };
                        if !believable(&heard, &key) {
                            continue;
                        }
                        let extra = extra_of(&heard, &key);
                        let kind = if extra.as_ref().is_some_and(|e| e.terminal) { DeviceKind::Terminal } else { heard.kind };
                        shared.peers.lock().unwrap().insert(
                            heard.id.clone(),
                            Peer {
                                id: heard.id,
                                name: heard.name,
                                kind,
                                address: from.ip().to_string(),
                                port: heard.port,
                                now_playing: extra.and_then(|e| e.now_playing),
                                seen: Instant::now(),
                            },
                        );
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        // The server.
        {
            let (shared, stop, asked) = (shared.clone(), stop.clone(), asked.clone());
            std::thread::Builder::new()
                .name("connect-server".into())
                .spawn(move || {
                    for connection in server.incoming().flatten() {
                        if stop.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        let (shared, asked) = (shared.clone(), asked.clone());
                        std::thread::spawn(move || {
                            let _ = serve(&shared, connection, &*asked);
                        });
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(Connect { shared, identity, port, stop })
    }

    pub fn peers(&self) -> Vec<Peer> {
        let mut peers: Vec<Peer> = self.shared.peers.lock().unwrap().values().cloned().collect();
        peers.sort_by(|a, b| a.name.cmp(&b.name));
        peers
    }

    pub fn show(&self, here: Here) {
        *self.shared.here.lock().unwrap() = here;
    }

    /// Sends a command to another device, signed. Answers its reply, or why there was none.
    pub fn command(&self, peer: &Peer, mut command: Command) -> Result<CommandReply, String> {
        command.from_device_id = Some(self.identity.id.clone());
        command.from_device_name = Some(self.identity.name.clone());
        let body = serde_json::to_string(&command).map_err(|e| e.to_string())?;
        let (status, text) = self.send(peer, "POST", "/command", &body)?;
        if status == 401 {
            return Err(format!(
                "{} did not accept this device. Both have to be signed in to the same Noctorium account, with their clocks right.",
                peer.name
            ));
        }
        serde_json::from_str(&text).map_err(|_| format!("{} said something unexpected.", peer.name))
    }

    /// Finds one of the listener's devices by its address, `host:port`, for a network that keeps devices from
    /// hearing each other's announcements, and keeps it among the peers as if it had been heard.
    pub fn add_address(&self, address: &str) -> Result<Peer, String> {
        let address = address.trim();
        let (host, port) = address
            .rsplit_once(':')
            .and_then(|(h, p)| Some((h.trim_matches(|c| c == '[' || c == ']').to_string(), p.parse::<u16>().ok()?)))
            .ok_or("Write the address as the device shows it, with its port: 192.168.1.20:45678.")?;
        let probe = Peer {
            id: String::new(),
            name: address.to_string(),
            kind: DeviceKind::Desktop,
            address: host,
            port,
            now_playing: None,
            seen: Instant::now(),
        };
        let state = self.state_of(&probe)?;
        if state.device_id == self.identity.id {
            return Err("That is this computer's own address.".into());
        }
        let peer = Peer {
            id: state.device_id.clone(),
            name: state.device_name.clone(),
            kind: if state.terminal { DeviceKind::Terminal } else { state.kind },
            now_playing: state.track.as_ref().map(|t| NowPlaying {
                title: t.title.clone(),
                artist: t.artists.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", "),
                provider: Some(t.provider.key_name().to_string()),
                artwork_url: t.artwork_url.clone(),
                playing: state.playing,
                position_ms: state.position_ms,
                duration_ms: state.duration_ms,
            }),
            ..probe
        };
        self.shared.peers.lock().unwrap().insert(peer.id.clone(), peer.clone());
        Ok(peer)
    }

    pub fn state_of(&self, peer: &Peer) -> Result<RemoteState, String> {
        let (_, text) = self.send(peer, "GET", "/state", "")?;
        serde_json::from_str(&text).map_err(|_| format!("{} said something unexpected.", peer.name))
    }

    fn send(&self, peer: &Peer, method: &str, path: &str, body: &str) -> Result<(u16, String), String> {
        let key = self.shared.key.lock().unwrap().clone().ok_or("This device is not signed in to Noctorium.")?;
        let timestamp = now_ms();
        let nonce = nonce();
        let signature = sign(&key, &request_message(method, path, timestamp, &nonce, body));
        let host = if peer.address.contains(':') { format!("[{}]", peer.address) } else { peer.address.clone() };
        let request = ureq::request(method, &format!("http://{host}:{}{path}", peer.port))
            .set(HEADER_DEVICE, &self.identity.id)
            .set(HEADER_NONCE, &nonce)
            .set(HEADER_TIMESTAMP, &timestamp.to_string())
            .set(HEADER_AUTH, &signature)
            .set("Content-Type", "application/json; charset=utf-8")
            .timeout(Duration::from_secs(8));
        let reply = if method == "POST" { request.send_string(body) } else { request.call() };
        match reply {
            Ok(r) => Ok((r.status(), r.into_string().unwrap_or_default())),
            Err(ureq::Error::Status(code, r)) => Ok((code, r.into_string().unwrap_or_default())),
            Err(_) => Err(format!("{} did not answer. Its firewall may be keeping Noctorium out on this network.", peer.name)),
        }
    }
}

fn announcement(shared: &Shared, identity: &Identity, port: u16) -> Option<Announcement> {
    let key = shared.key.lock().unwrap().clone()?;
    let mut mine = Announcement {
        v: VERSION,
        id: identity.id.clone(),
        name: identity.name.clone(),
        kind: DeviceKind::Desktop,
        port,
        stamp: now_ms() / 60_000,
        tag: String::new(),
        extra: None,
        extra_tag: None,
    };
    mine.tag = sign(&key, &mine.signable());
    let extra = AnnouncementExtra { terminal: false, now_playing: shared.here.lock().unwrap().now_playing.clone(), driven_by: None, driving: None };
    mine.extra = serde_json::to_string(&extra).ok();
    mine.extra_tag = Some(sign(&key, &mine.signable_with_extra()));
    Some(mine)
}

fn believable(heard: &Announcement, key: &str) -> bool {
    heard.v == VERSION && heard.port > 0 && (now_ms() / 60_000 - heard.stamp).abs() <= 5 && matches(key, &heard.signable(), &heard.tag)
}

fn extra_of(heard: &Announcement, key: &str) -> Option<AnnouncementExtra> {
    let (text, tag) = (heard.extra.as_ref()?, heard.extra_tag.as_ref()?);
    matches(key, &heard.signable_with_extra(), tag).then(|| serde_json::from_str(text).ok()).flatten()
}

fn serve(shared: &Shared, mut connection: TcpStream, asked: &(dyn Fn(Asked) + Send + Sync)) -> std::io::Result<()> {
    connection.set_read_timeout(Some(Duration::from_secs(10)))?;
    let mut reader = BufReader::new(connection.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("").to_uppercase();
    let path = parts.next().unwrap_or("").split('?').next().unwrap_or("").to_string();
    let mut headers: HashMap<String, String> = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = header.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }
    let length: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0).min(512 * 1024);
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let body = String::from_utf8_lossy(&body).into_owned();
    let respond = |connection: &mut TcpStream, status: u16, text: &str| -> std::io::Result<()> {
        let reason = match status {
            200 => "OK",
            401 => "Unauthorized",
            404 => "Not Found",
            503 => "Service Unavailable",
            _ => "Error",
        };
        write!(connection, "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len())
    };
    let Some(key) = shared.key.lock().unwrap().clone() else {
        return respond(&mut connection, 503, r#"{"ok":false,"message":"This device is not signed in to Noctorium."}"#);
    };
    // Signed, recent, and not heard before.
    let refused = (|| {
        let nonce = headers.get(HEADER_NONCE)?;
        let stamp: i64 = headers.get(HEADER_TIMESTAMP)?.parse().ok()?;
        let presented = headers.get(HEADER_AUTH)?;
        if (now_ms() - stamp).abs() > CLOCK_TOLERANCE_MS || !(8..=128).contains(&nonce.len()) {
            return None;
        }
        if !matches(&key, &request_message(&method, &path, stamp, nonce, &body), presented) {
            return None;
        }
        let mut seen = shared.nonces.lock().unwrap();
        let now = now_ms();
        seen.retain(|_, at| now - *at < CLOCK_TOLERANCE_MS * 2);
        if seen.insert(nonce.clone(), now).is_some() {
            return None;
        }
        Some(())
    })()
    .is_none();
    if refused {
        return respond(&mut connection, 401, r#"{"ok":false,"message":"Not authorised."}"#);
    }
    let here = shared.here.lock().unwrap().clone();
    let reply = match (method.as_str(), path.as_str()) {
        ("GET", "/state") => here.state.and_then(|s| serde_json::to_string(&s).ok()),
        ("GET", "/queue") => here.queue.and_then(|q| serde_json::to_string(&q).ok()),
        ("POST", "/command") => {
            let Ok(command) = serde_json::from_str::<Command>(&body) else {
                return respond(&mut connection, 400, r#"{"ok":false,"message":"Bad request."}"#);
            };
            let (sender, receiver) = std::sync::mpsc::channel();
            asked(Asked { command, reply: sender });
            let answer = receiver
                .recv_timeout(Duration::from_secs(8))
                .unwrap_or(CommandReply { ok: false, message: Some("The other device did not answer in time.".into()), state: None, handover: None });
            serde_json::to_string(&answer).ok()
        }
        _ => None,
    };
    match reply {
        Some(text) => respond(&mut connection, 200, &text),
        None => respond(&mut connection, 404, r#"{"ok":false,"message":"No such thing."}"#),
    }
}

/// The key of a queue, so another device can tell whether the queue it holds is still this one.
pub fn queue_key(tracks: &[Track]) -> String {
    let joined: String = tracks.iter().map(|t| t.queue_key()).collect::<Vec<_>>().join("\n");
    sha256_hex(&joined).chars().take(16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_announcement_is_signed_as_the_kotlin_players_sign_it() {
        let mut a = Announcement { v: 1, id: "d1".into(), name: "PC".into(), kind: DeviceKind::Desktop, port: 5000, stamp: 29_000_000, tag: String::new(), extra: None, extra_tag: None };
        assert_eq!(a.signable(), "1|d1|PC|DESKTOP|5000|29000000");
        a.tag = sign("key", &a.signable());
        assert!(matches("key", &a.signable(), &a.tag));
        assert!(!matches("other", &a.signable(), &a.tag));
        let wire = serde_json::to_value(&a).unwrap();
        assert_eq!(wire["kind"], "DESKTOP");
        assert!(wire.get("extraTag").is_some());
    }

    #[test]
    fn hmac_is_rfc_4231s() {
        // RFC 4231 test case 2.
        assert_eq!(sign("Jefe", "what do ya want for nothing?"), "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
    }

    #[test]
    fn a_request_message_names_everything_signed() {
        let message = request_message("POST", "/command", 7, "abcdefgh", "{}");
        assert!(message.starts_with("POST\n/command\n7\nabcdefgh\n"));
        assert!(message.ends_with("44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"));
    }

    #[test]
    fn commands_travel_as_the_kotlin_players_write_them() {
        let mut command = Command::new(CommandType::TakeOver);
        command.index = 2;
        let wire = serde_json::to_value(&command).unwrap();
        assert_eq!(wire["type"], "TAKE_OVER");
        let back: Command = serde_json::from_str(r#"{"type":"HAND_BACK"}"#).unwrap();
        assert_eq!(back.kind, CommandType::HandBack);
        assert_eq!(back.index, -1);
    }
}
