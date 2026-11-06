//! mpv, driven over its JSON IPC: the player that makes the sound.
//!
//! As Noctorium-Base's `MpvPlaybackEngine` does it: one mpv per song, started with no config of its own, no
//! video, no window and no terminal, told where to listen for commands, and given the stream address yt-dlp
//! found -- never asked to run yt-dlp itself. mpv leaves when the song ends, and that is how the end of a
//! song is known.
//!
//! Everything happens on one thread of the engine's own. The window sends it commands and reads the state it
//! keeps, so a slow mpv can never freeze a frame, and two commands can never cross on the pipe.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Idle,
    /// mpv is running and has not yet said it is playing.
    Loading,
    Playing,
    Paused,
}

#[derive(Clone, Debug, Default)]
pub struct PlaybackState {
    pub status: Status,
    pub position_ms: i64,
    pub duration_ms: i64,
    /// What is playing, as mpv reads it: "opus 48 kHz", for the status line.
    pub format: Option<String>,
    /// Which song this state is about.
    pub generation: u64,
    /// The visualiser's last frame, while the meter is in the filters.
    pub analysis: Option<crate::audio::Analysis>,
    /// What the meter's levels need taking back: the booster's gain, which it measures after.
    pub analysis_offset_db: f32,
}

/// What the engine tells the window about, besides the state it keeps.
#[derive(Clone, Debug)]
pub enum EngineEvent {
    /// The song of `generation` came to its end; `error` says why if it stopped rather than finished.
    Ended { generation: u64, error: Option<String> },
}

/// How the engine plays: mpv's volume (1 is mpv's hundred), mute, speed, repeat-one, and the audio filters
/// -- the equaliser, evening out, the booster -- as one `af` chain. See [`crate::audio::Shaping`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Levels {
    pub volume: f32,
    pub muted: bool,
    pub speed: f32,
    pub looping: bool,
    pub filters: String,
}

enum Cmd {
    Play {
        generation: u64,
        url: String,
        user_agent: Option<String>,
        start_ms: i64,
        paused: bool,
    },
    Pause(bool),
    Seek(i64),
    Levels(Levels),
    Stop,
    SetPath(Option<PathBuf>),
}

pub struct Engine {
    commands: Sender<Cmd>,
    state: Arc<Mutex<PlaybackState>>,
    generation: AtomicU64,
}

impl Engine {
    pub fn new(
        mpv: Option<PathBuf>,
        levels: Levels,
        notify: impl Fn(EngineEvent) + Send + 'static,
    ) -> Engine {
        let (commands, receiver) = channel::<Cmd>();
        let state = Arc::new(Mutex::new(PlaybackState::default()));
        let shared = state.clone();
        std::thread::Builder::new()
            .name("mpv".into())
            .spawn(move || {
                let mut worker = Worker {
                    mpv,
                    levels,
                    session: None,
                    state: shared,
                    notify: Box::new(notify),
                };
                loop {
                    // Thirty frames a second while the visualiser measures; five otherwise.
                    let pace = if worker.levels.filters.contains(crate::audio::METER_LABEL) { 33 } else { 200 };
                    match receiver.recv_timeout(Duration::from_millis(pace)) {
                        Ok(command) => worker.handle(command),
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => {
                            worker.stop();
                            return;
                        }
                    }
                    worker.poll();
                }
            })
            .expect("the mpv thread could not be started");
        Engine {
            commands,
            state,
            generation: AtomicU64::new(0),
        }
    }

    pub fn state(&self) -> PlaybackState {
        self.state.lock().unwrap().clone()
    }

    /// Plays `url`, and returns the number the song's end will be reported under.
    pub fn play(&self, url: String, user_agent: Option<String>, start_ms: i64, paused: bool) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let _ = self.commands.send(Cmd::Play {
            generation,
            url,
            user_agent,
            start_ms,
            paused,
        });
        generation
    }

    pub fn set_paused(&self, paused: bool) {
        let _ = self.commands.send(Cmd::Pause(paused));
    }

    pub fn seek(&self, position_ms: i64) {
        let _ = self.commands.send(Cmd::Seek(position_ms));
    }

    pub fn set_levels(&self, levels: Levels) {
        let _ = self.commands.send(Cmd::Levels(levels));
    }

    /// Lets go of whatever is playing: the player goes idle with nothing loaded.
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        let _ = self.commands.send(Cmd::Stop);
    }

    pub fn set_path(&self, path: Option<PathBuf>) {
        let _ = self.commands.send(Cmd::SetPath(path));
    }
}

struct Session {
    generation: u64,
    child: Child,
    endpoint: String,
    connection: Option<Connection>,
    started: Instant,
    /// Set once mpv has said where in the song it is, which is when it is really playing.
    heard: bool,
    paused: bool,
    format_read: bool,
}

struct Worker {
    mpv: Option<PathBuf>,
    levels: Levels,
    session: Option<Session>,
    state: Arc<Mutex<PlaybackState>>,
    notify: Box<dyn Fn(EngineEvent) + Send>,
}

impl Worker {
    fn handle(&mut self, command: Cmd) {
        match command {
            Cmd::Play {
                generation,
                url,
                user_agent,
                start_ms,
                paused,
            } => {
                self.stop();
                match self.start(generation, &url, user_agent.as_deref(), start_ms, paused) {
                    Ok(session) => {
                        *self.state.lock().unwrap() = PlaybackState {
                            status: Status::Loading,
                            position_ms: start_ms,
                            generation,
                            ..Default::default()
                        };
                        self.session = Some(session);
                    }
                    Err(why) => (self.notify)(EngineEvent::Ended {
                        generation,
                        error: Some(why),
                    }),
                }
            }
            Cmd::Pause(paused) => {
                if let Some(session) = self.session.as_mut() {
                    session.paused = paused;
                    send(session, serde_json::json!(["set_property", "pause", paused]));
                    let mut state = self.state.lock().unwrap();
                    if state.status != Status::Loading {
                        state.status = if paused { Status::Paused } else { Status::Playing };
                    }
                }
            }
            Cmd::Seek(position_ms) => {
                if let Some(session) = self.session.as_mut() {
                    let seconds = position_ms.max(0) as f64 / 1000.0;
                    send(session, serde_json::json!(["seek", seconds, "absolute"]));
                    self.state.lock().unwrap().position_ms = position_ms;
                }
            }
            Cmd::Levels(levels) => {
                let filters_changed = levels.filters != self.levels.filters;
                self.levels = levels.clone();
                if let Some(session) = self.session.as_mut() {
                    if filters_changed {
                        send(session, serde_json::json!(["set_property", "af", levels.filters]));
                    }
                    send(session, serde_json::json!(["set_property", "volume", levels.volume as f64 * 100.0]));
                    send(session, serde_json::json!(["set_property", "mute", levels.muted]));
                    send(session, serde_json::json!(["set_property", "speed", levels.speed as f64]));
                    send(
                        session,
                        serde_json::json!(["set_property", "loop-file", if levels.looping { "inf" } else { "no" }]),
                    );
                }
            }
            Cmd::Stop => {
                self.stop();
                *self.state.lock().unwrap() = PlaybackState::default();
            }
            Cmd::SetPath(path) => self.mpv = path,
        }
    }

    fn start(
        &self,
        generation: u64,
        url: &str,
        user_agent: Option<&str>,
        start_ms: i64,
        paused: bool,
    ) -> Result<Session, String> {
        let mpv = self
            .mpv
            .as_ref()
            .ok_or("mpv is not installed. Settings › Playback tools can install it.")?;
        let endpoint = endpoint_for(generation);
        let mut arguments: Vec<String> = vec![
            "--no-config".into(),
            "--no-video".into(),
            "--force-window=no".into(),
            "--terminal=no".into(),
            "--input-terminal=no".into(),
            "--ytdl=no".into(),
            format!("--input-ipc-server={endpoint}"),
            format!("--volume={:.2}", self.levels.volume * 100.0),
            format!("--mute={}", if self.levels.muted { "yes" } else { "no" }),
            "--title=Noctorium".into(),
            "--network-timeout=20".into(),
            "--cache=yes".into(),
        ];
        if self.levels.looping {
            arguments.push("--loop-file=inf".into());
        }
        if !self.levels.filters.is_empty() {
            arguments.push(format!("--af={}", self.levels.filters));
        }
        if (self.levels.speed - 1.0).abs() > 0.001 {
            arguments.push(format!("--speed={:.2}", self.levels.speed));
        }
        if paused {
            arguments.push("--pause=yes".into());
        }
        if let Some(agent) = user_agent {
            arguments.push(format!("--user-agent={agent}"));
        }
        if start_ms > 0 {
            arguments.push(format!("--start=+{:.1}", start_ms as f64 / 1000.0));
        }
        arguments.push("--".into());
        arguments.push(url.to_string());
        #[cfg(unix)]
        let _ = std::fs::remove_file(&endpoint);
        let mut command = Command::new(mpv);
        command
            .args(&arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::ytdlp::hide_console(&mut command);
        let child = command.spawn().map_err(|e| format!("mpv could not be started: {e}"))?;
        Ok(Session {
            generation,
            child,
            endpoint,
            connection: None,
            started: Instant::now(),
            heard: false,
            paused,
            format_read: false,
        })
    }

    fn stop(&mut self) {
        if let Some(mut session) = self.session.take() {
            if let Some(connection) = session.connection.as_mut() {
                let _ = connection.request(serde_json::json!(["quit"]));
            }
            let deadline = Instant::now() + Duration::from_millis(300);
            while Instant::now() < deadline {
                if matches!(session.child.try_wait(), Ok(Some(_))) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(15));
            }
            let _ = session.child.kill();
            let _ = session.child.wait();
            #[cfg(unix)]
            let _ = std::fs::remove_file(&session.endpoint);
        }
    }

    fn poll(&mut self) {
        let Some(session) = self.session.as_mut() else { return };
        // Gone: the song ended, or mpv could not play it.
        if let Ok(Some(status)) = session.child.try_wait() {
            let generation = session.generation;
            let quick = session.started.elapsed() < Duration::from_secs(3);
            let heard = session.heard;
            self.session = None;
            *self.state.lock().unwrap() = PlaybackState {
                generation,
                ..Default::default()
            };
            let error = if status.success() && heard {
                None
            } else if !heard || quick {
                Some("mpv could not play this stream.".to_string())
            } else {
                None
            };
            (self.notify)(EngineEvent::Ended { generation, error });
            return;
        }
        if session.connection.is_none() {
            match Connection::open(&session.endpoint) {
                Ok(connection) => session.connection = Some(connection),
                Err(_) => return,
            }
        }
        let connection = session.connection.as_mut().unwrap();
        let position = connection.number("time-pos");
        let duration = connection.number("duration");
        let paused = connection.boolean("pause").unwrap_or(session.paused);
        if connection.broken {
            session.connection = None;
            return;
        }
        if !session.format_read && position.is_some() {
            let codec = connection.string("audio-codec-name");
            let rate = connection.number("audio-params/samplerate");
            session.format_read = true;
            let format = match (codec, rate) {
                (Some(c), Some(r)) => Some(format!("{c} {:.1} kHz", r / 1000.0).replace(".0 kHz", " kHz")),
                (Some(c), None) => Some(c),
                _ => None,
            };
            self.state.lock().unwrap().format = format;
        }
        let metering = self.levels.filters.contains(crate::audio::METER_LABEL);
        let analysis = if metering {
            connection
                .request(serde_json::json!(["get_property", format!("af-metadata/{}", crate::audio::METER_LABEL)]))
                .and_then(|v| crate::audio::analysis_from(&v, 0.0))
        } else {
            None
        };
        let mut state = self.state.lock().unwrap();
        state.analysis = analysis;
        if let Some(position) = position {
            session.heard = true;
            state.position_ms = (position * 1000.0) as i64;
            state.status = if paused { Status::Paused } else { Status::Playing };
        }
        if let Some(duration) = duration {
            state.duration_ms = (duration * 1000.0) as i64;
        }
    }
}

fn send(session: &mut Session, command: serde_json::Value) {
    if let Some(connection) = session.connection.as_mut() {
        let _ = connection.request(command);
        if connection.broken {
            session.connection = None;
        }
    }
}

fn endpoint_for(generation: u64) -> String {
    let process = std::process::id();
    if cfg!(windows) {
        format!(r"\\.\pipe\noctorium-rust-{process}-{generation}")
    } else {
        std::env::temp_dir()
            .join(format!("noctorium-rust-{process}-{generation}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

/// One connection to an mpv's IPC, told first to keep its events to itself, so every line read is an answer.
struct Connection {
    reader: BufReader<Box<dyn std::io::Read + Send>>,
    writer: Box<dyn Write + Send>,
    next_id: u64,
    broken: bool,
}

impl Connection {
    fn open(endpoint: &str) -> std::io::Result<Connection> {
        #[cfg(windows)]
        let (reader, writer): (Box<dyn std::io::Read + Send>, Box<dyn Write + Send>) = {
            let pipe = std::fs::OpenOptions::new().read(true).write(true).open(endpoint)?;
            let writer = pipe.try_clone()?;
            (Box::new(pipe), Box::new(writer))
        };
        #[cfg(unix)]
        let (reader, writer): (Box<dyn std::io::Read + Send>, Box<dyn Write + Send>) = {
            let socket = std::os::unix::net::UnixStream::connect(endpoint)?;
            socket.set_read_timeout(Some(Duration::from_secs(2)))?;
            let writer = socket.try_clone()?;
            (Box::new(socket), Box::new(writer))
        };
        let mut connection = Connection {
            reader: BufReader::new(reader),
            writer,
            next_id: 0,
            broken: false,
        };
        connection.request(serde_json::json!(["disable_event", "all"]));
        if connection.broken {
            return Err(std::io::Error::other("mpv did not answer"));
        }
        Ok(connection)
    }

    /// Sends a command and reads its answer: None when mpv said it could not, or has gone.
    fn request(&mut self, command: serde_json::Value) -> Option<serde_json::Value> {
        if self.broken {
            return None;
        }
        self.next_id += 1;
        let id = self.next_id;
        let line = serde_json::json!({"command": command, "request_id": id}).to_string() + "\n";
        if self.writer.write_all(line.as_bytes()).and_then(|_| self.writer.flush()).is_err() {
            self.broken = true;
            return None;
        }
        // Lines that are not the answer -- an event sent before mpv was told not to -- are skipped, a few.
        for _ in 0..100 {
            let mut reply = String::new();
            match self.reader.read_line(&mut reply) {
                Ok(0) | Err(_) => {
                    self.broken = true;
                    return None;
                }
                Ok(_) => {}
            }
            let Ok(answer) = serde_json::from_str::<serde_json::Value>(&reply) else { continue };
            if answer["request_id"].as_u64() != Some(id) {
                continue;
            }
            return (answer["error"].as_str() == Some("success")).then(|| answer["data"].clone());
        }
        self.broken = true;
        None
    }

    fn number(&mut self, name: &str) -> Option<f64> {
        self.request(serde_json::json!(["get_property", name]))?.as_f64()
    }

    fn boolean(&mut self, name: &str) -> Option<bool> {
        self.request(serde_json::json!(["get_property", name]))?.as_bool()
    }

    fn string(&mut self, name: &str) -> Option<String> {
        self.request(serde_json::json!(["get_property", name]))?
            .as_str()
            .map(String::from)
    }
}
