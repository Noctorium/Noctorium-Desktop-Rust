//! yt-dlp: how Noctorium reads YouTube Music, YouTube and SoundCloud, and finds where a song's audio is.
//!
//! The same questions, asked the same way, as Noctorium-Base's `YtDlpService`. Every argument is its own
//! item -- nothing typed by anybody is ever put through a shell -- and every run has a time limit, so a
//! service that stops answering costs a message rather than a frozen search.

use crate::domain::{Album, Artist, Playlist, ProviderType, Track};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// How long a stream address is trusted. YouTube's last about six hours; half an hour is well inside that.
const ADDRESS_LIFETIME: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub url: String,
    /// The User-Agent yt-dlp was issued the address as, which mpv should present when fetching it.
    pub user_agent: Option<String>,
}

/// Which queue a run of yt-dlp waits in. Each run starts a Python, which is seconds of a processor's
/// time; on a two-core machine with the virus scanner busy, three at once -- two of the home page's rows
/// and the song just pressed -- took a search from nine seconds to forty, and the song past ninety. So
/// playing comes first: browsing waits while a song is being looked up, and browses one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lane {
    Browse,
    Play,
    /// Looking up a song before it is asked for -- the next in the queue, the one the pointer rests on --
    /// one at a time, beside the plays, so it never keeps a click waiting for a turn.
    Ahead,
    /// Downloads and songs being saved: one at a time, beside the others, since one can take minutes.
    Download,
}

/// How many runs of each lane may go at once.
const PLAYS_AT_ONCE: usize = 2;
const BROWSES_AT_ONCE: usize = 1;

#[derive(Default)]
struct Counts {
    playing: usize,
    play_waiting: usize,
    browsing: usize,
    downloading: usize,
    ahead: usize,
}

#[derive(Default)]
struct Turns {
    counts: Mutex<Counts>,
    freed: std::sync::Condvar,
}

impl Turns {
    fn take(&self, lane: Lane) -> Turn<'_> {
        let mut counts = self.counts.lock().unwrap();
        match lane {
            Lane::Play => {
                counts.play_waiting += 1;
                while counts.playing >= PLAYS_AT_ONCE {
                    counts = self.freed.wait(counts).unwrap();
                }
                counts.play_waiting -= 1;
                counts.playing += 1;
            }
            Lane::Browse => {
                while counts.browsing >= BROWSES_AT_ONCE || counts.playing > 0 || counts.play_waiting > 0 {
                    counts = self.freed.wait(counts).unwrap();
                }
                counts.browsing += 1;
            }
            Lane::Download => {
                while counts.downloading >= 1 {
                    counts = self.freed.wait(counts).unwrap();
                }
                counts.downloading += 1;
            }
            Lane::Ahead => {
                while counts.ahead >= 1 {
                    counts = self.freed.wait(counts).unwrap();
                }
                counts.ahead += 1;
            }
        }
        Turn { turns: self, lane }
    }

    /// A turn in `lane` if one is free now, rather than waiting for one: for looking ahead at what the pointer
    /// passes over, where a turn waited for would be for a song long since passed.
    fn try_take(&self, lane: Lane) -> Option<Turn<'_>> {
        let mut counts = self.counts.lock().unwrap();
        match lane {
            Lane::Ahead if counts.ahead == 0 => counts.ahead += 1,
            _ => return None,
        }
        Some(Turn { turns: self, lane })
    }
}

struct Turn<'a> {
    turns: &'a Turns,
    lane: Lane,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        let mut counts = self.turns.counts.lock().unwrap();
        match self.lane {
            Lane::Play => counts.playing = counts.playing.saturating_sub(1),
            Lane::Browse => counts.browsing = counts.browsing.saturating_sub(1),
            Lane::Download => counts.downloading = counts.downloading.saturating_sub(1),
            Lane::Ahead => counts.ahead = counts.ahead.saturating_sub(1),
        }
        drop(counts);
        self.turns.freed.notify_all();
    }
}

#[derive(Default)]
pub struct YtDlp {
    turns: Turns,
    path: RwLock<Option<PathBuf>>,
    cookies_browser: RwLock<Option<String>>,
    addresses: Mutex<HashMap<String, (Resolved, Instant)>>,
    /// The songs being looked up now, each with what its lookup comes to, so a second ask for the same song
    /// -- a click on the song being looked up ahead -- waits for that run rather than starting another.
    looking_up: Mutex<HashMap<String, Arc<Lookup>>>,
}

/// One run of yt-dlp for a song's audio, and its answer once it has one.
#[derive(Default)]
struct Lookup {
    answer: Mutex<Option<Result<Resolved, String>>>,
    done: std::sync::Condvar,
}

/// How a lookup takes its turn.
#[derive(Clone, Copy, PartialEq)]
enum Asking {
    /// Somebody is waiting to hear it: a turn among the plays.
    Now,
    /// Ahead of time, waiting for a turn among the lookups ahead.
    Ahead,
    /// Ahead of time, only if a turn is free at once.
    IfFree,
}

pub type Backend = Arc<YtDlp>;

impl YtDlp {
    pub fn new(path: Option<PathBuf>) -> Backend {
        let backend = YtDlp::default();
        *backend.path.write().unwrap() = path;
        Arc::new(backend)
    }

    pub fn set_path(&self, path: Option<PathBuf>) {
        *self.path.write().unwrap() = path;
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.path.read().unwrap().clone()
    }

    pub fn set_cookies_browser(&self, browser: Option<String>) {
        *self.cookies_browser.write().unwrap() = browser.filter(|b| !b.trim().is_empty());
    }

    /// The flags that read a catalogue as the listener: their browser's sign-in, if they chose one.
    fn account_arguments(&self) -> Vec<String> {
        match self.cookies_browser.read().unwrap().as_ref() {
            Some(browser) => vec!["--cookies-from-browser".into(), browser.clone()],
            None => vec![],
        }
    }

    /// The flags for asking for a stream, which are not always the browsing ones. YouTube refuses a
    /// signed-in session arriving from something that is not a browser at the player, while the same song
    /// resolves at once with no cookies; SoundCloud needs its session for private and subscriber audio.
    fn playback_arguments(&self, provider: Option<ProviderType>) -> Vec<String> {
        match provider {
            Some(ProviderType::YoutubeMusic | ProviderType::YoutubeVideo) | None => vec![],
            Some(_) => self.account_arguments(),
        }
    }

    pub fn search(&self, provider: ProviderType, query: &str, limit: usize) -> Result<Vec<Track>, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(vec![]);
        }
        // YouTube Music's own interface first: yt-dlp's search of it carries no artists and no lengths.
        if provider == ProviderType::YoutubeMusic {
            if let Ok(found) = crate::ytmusic::search_songs(query, limit) {
                if !found.is_empty() {
                    return Ok(found);
                }
            }
        }
        // SoundCloud's own interface too: one request, where yt-dlp starts a Python.
        if provider == ProviderType::Soundcloud {
            if let Ok(found) = crate::soundcloud::search(query, limit) {
                if !found.is_empty() {
                    return Ok(found);
                }
            }
        }
        let target = match provider {
            ProviderType::YoutubeMusic => format!(
                "https://music.youtube.com/search?q={}&sp=EgWKAQIIAWoKEAkQBRAKEAMQBA%3D%3D",
                percent_encode(query)
            ),
            ProviderType::YoutubeVideo => format!("ytsearch{limit}:{query}"),
            ProviderType::Soundcloud => format!("scsearch{limit}:{query}"),
            other => return Err(format!("{} is not searched through yt-dlp", other.display_name())),
        };
        let mut arguments: Vec<String> = ["--flat-playlist", "--dump-single-json", "--no-warnings", "--no-playlist"]
            .map(String::from)
            .into();
        arguments.extend(self.account_arguments());
        arguments.extend(["--playlist-end".into(), limit.to_string(), "--".into(), target]);
        let output = self.run_retrying(&arguments, Duration::from_secs(90))?;
        Ok(entries(&parse(&output)?)
            .iter()
            .filter_map(|item| map_track(provider, item))
            .collect())
    }

    /// A playlist by its link: its name and its songs, in one flat request.
    pub fn playlist(&self, url: &str, limit: usize) -> Result<Playlist, String> {
        let provider = provider_of(url).ok_or("That is not a YouTube or SoundCloud link.")?;
        if provider == ProviderType::Soundcloud {
            if let Ok((title, owner_name, tracks)) = crate::soundcloud::playlist(url) {
                if !tracks.is_empty() {
                    return Ok(Playlist {
                        id: url.to_string(),
                        title,
                        provider,
                        owner_name,
                        artwork_url: tracks.first().and_then(|t| t.artwork_url.clone()),
                        track_count: Some(tracks.len() as i64),
                        tracks,
                        source_url: Some(url.to_string()),
                    });
                }
            }
        }
        let mut arguments: Vec<String> =
            ["--flat-playlist", "--dump-single-json", "--no-warnings"].map(String::from).into();
        arguments.extend(self.account_arguments());
        arguments.extend(["--playlist-end".into(), limit.to_string(), "--".into(), url.to_string()]);
        let root = parse(&self.run_retrying(&arguments, Duration::from_secs(90))?)?;
        let tracks: Vec<Track> = entries(&root)
            .iter()
            .filter_map(|item| map_track(provider, item))
            .collect();
        let title = string(&root, "title").unwrap_or_else(|| "Playlist".into());
        Ok(Playlist {
            id: string(&root, "id").unwrap_or_else(|| url.to_string()),
            title,
            provider,
            owner_name: string(&root, "uploader").or_else(|| string(&root, "channel")),
            artwork_url: best_thumbnail(&root).or_else(|| tracks.first().and_then(|t| t.artwork_url.clone())),
            track_count: Some(tracks.len() as i64),
            tracks,
            source_url: Some(url.to_string()),
        })
    }

    /// One song by its link, fully read.
    pub fn track(&self, url: &str) -> Result<Track, String> {
        let provider = provider_of(url).ok_or("That is not a YouTube or SoundCloud link.")?;
        let mut arguments: Vec<String> =
            ["--dump-single-json", "--skip-download", "--no-warnings", "--no-playlist"].map(String::from).into();
        arguments.extend(self.playback_arguments(Some(provider)));
        arguments.extend(["--".into(), url.to_string()]);
        let root = parse(&self.run_retrying(&arguments, Duration::from_secs(90))?)?;
        let track = map_track(provider, &root).ok_or("yt-dlp could not say what that link is.")?;
        Ok(enrich(track, &root))
    }

    /// Fills in the artist, album, length and cover a quick listing left out. Unchanged if nothing can be.
    pub fn enrich(&self, track: &Track) -> Track {
        if !has_placeholder_artist(track) {
            return track.clone();
        }
        let mut arguments: Vec<String> =
            ["--dump-single-json", "--skip-download", "--no-warnings", "--no-playlist"].map(String::from).into();
        arguments.extend(self.playback_arguments(Some(track.provider)));
        arguments.extend(["--".into(), track.source_url.clone()]);
        match self.run(&arguments, Duration::from_secs(45)).and_then(|o| parse(&o)) {
            Ok(root) => enrich(track.clone(), &root),
            Err(_) => track.clone(),
        }
    }

    /// Songs to carry on with after `seed`: YouTube's own radio for it, SoundCloud's related tracks, and a
    /// search for the artist when neither answers.
    pub fn radio(&self, seed: &Track, limit: usize) -> Result<Vec<Track>, String> {
        if seed.provider == ProviderType::YoutubeMusic {
            if let Ok(found) = crate::ytmusic::radio(&seed.id, limit) {
                if !found.is_empty() {
                    return Ok(found);
                }
            }
        }
        if seed.provider == ProviderType::Soundcloud {
            if let Ok(found) = crate::soundcloud::related(seed, limit) {
                let found: Vec<Track> = found.into_iter().filter(|t| t.id != seed.id).take(limit).collect();
                if !found.is_empty() {
                    return Ok(found);
                }
            }
        }
        let target = match seed.provider {
            ProviderType::YoutubeMusic => {
                Some(format!("https://music.youtube.com/watch?v={0}&list=RDAMVM{0}", seed.id))
            }
            ProviderType::YoutubeVideo => Some(format!("https://www.youtube.com/watch?v={0}&list=RD{0}", seed.id)),
            ProviderType::Soundcloud => Some(format!("{}/recommended", seed.source_url.trim_end_matches('/'))),
            _ => None,
        };
        if let Some(target) = target {
            let arguments: Vec<String> = vec![
                "--flat-playlist".into(),
                "--dump-single-json".into(),
                "--no-warnings".into(),
                "--playlist-end".into(),
                (limit + 1).to_string(),
                "--".into(),
                target,
            ];
            if let Ok(root) = self.run(&arguments, Duration::from_secs(90)).and_then(|o| parse(&o)) {
                let provider = if seed.provider == ProviderType::YoutubeVideo {
                    ProviderType::YoutubeVideo
                } else {
                    seed.provider
                };
                let found: Vec<Track> = entries(&root)
                    .iter()
                    .filter_map(|item| map_track(provider, item))
                    .filter(|t| t.id != seed.id)
                    .take(limit)
                    .collect();
                if !found.is_empty() {
                    return Ok(found);
                }
            }
        }
        let provider = if seed.provider.playable_here() { seed.provider } else { ProviderType::YoutubeMusic };
        let artist = seed.artists.first().map(|a| a.name.clone()).unwrap_or_default();
        let query = if artist.is_empty() || has_placeholder_artist(seed) { seed.title.clone() } else { artist };
        Ok(self
            .search(provider, &query, limit + 1)?
            .into_iter()
            .filter(|t| t.id != seed.id)
            .take(limit)
            .collect())
    }

    /// An address the player can read audio from. Remembered for half an hour, so a song played twice --
    /// or lined up next -- is not resolved twice: a run of yt-dlp is two to three seconds every time.
    pub fn resolve_audio(&self, source_url: &str) -> Result<Resolved, String> {
        self.resolve_audio_asking(source_url, Asking::Now)
    }

    /// Looks a song up before it is asked for, so that playing it is immediate: the next in the queue.
    pub fn prefetch_audio(&self, source_url: &str) {
        let _ = self.resolve_audio_asking(source_url, Asking::Ahead);
    }

    /// Looks up the song the pointer rests on, if a turn is free now. False when it was not looked up.
    pub fn prefetch_audio_if_free(&self, source_url: &str) -> bool {
        self.resolve_audio_asking(source_url, Asking::IfFree).is_ok()
    }

    /// Whether the song's address is known now, so playing it needs no run of yt-dlp.
    pub fn knows_audio(&self, source_url: &str) -> bool {
        self.addresses.lock().unwrap().get(source_url).is_some_and(|(_, at)| at.elapsed() < ADDRESS_LIFETIME)
    }

    fn resolve_audio_asking(&self, source_url: &str, asking: Asking) -> Result<Resolved, String> {
        if !(source_url.starts_with("https://") || source_url.starts_with("http://")) {
            return Err("Only HTTP media sources are accepted".into());
        }
        if let Some((resolved, at)) = self.addresses.lock().unwrap().get(source_url) {
            if at.elapsed() < ADDRESS_LIFETIME {
                return Ok(resolved.clone());
            }
        }
        // VK, through the listener's own sign-in; its HLS goes through the relay, which decrypts what mpv cannot.
        if crate::vk::is_vk_song(source_url) {
            let session = crate::vk::current().ok_or("Playing VK needs its sign-in: Services › VK.")?;
            let url = session.stream(source_url)?;
            let url = if url.contains(".m3u8") { crate::hlsrelay::relay_for(&url)? } else { url };
            return Ok(Resolved { url, user_agent: None });
        }
        // Bandcamp streams to anyone, from its own interface; the address expires, so it is asked for each time.
        if crate::bandcamp::is_bandcamp(source_url) {
            if let Ok(url) = crate::bandcamp::stream(source_url) {
                return Ok(Resolved { url, user_agent: None });
            }
        }
        // SoundCloud says where its audio is in one request, signed out. Its addresses last minutes, not
        // hours, so they are asked for each time rather than remembered.
        if provider_of(source_url) == Some(ProviderType::Soundcloud) && self.cookies_browser.read().unwrap().is_none() {
            if let Ok(url) = crate::soundcloud::stream(source_url) {
                return Ok(Resolved { url, user_agent: None });
            }
        }
        // A run for this song already under way -- looked up ahead, or asked for twice -- is waited for.
        let (lookup, ours) = {
            let mut looking_up = self.looking_up.lock().unwrap();
            match looking_up.get(source_url) {
                Some(running) => (running.clone(), false),
                None => {
                    let lookup = Arc::new(Lookup::default());
                    looking_up.insert(source_url.to_string(), lookup.clone());
                    (lookup, true)
                }
            }
        };
        if !ours {
            if asking == Asking::IfFree {
                return Err("already being looked up".into());
            }
            let mut answer = lookup.answer.lock().unwrap();
            while answer.is_none() {
                answer = lookup.done.wait(answer).unwrap();
            }
            return answer.clone().unwrap();
        }
        let answer = self.run_for_audio(source_url, asking);
        *lookup.answer.lock().unwrap() = Some(answer.clone());
        lookup.done.notify_all();
        self.looking_up.lock().unwrap().remove(source_url);
        answer
    }

    /// The run of yt-dlp itself, in the lane `asking` says.
    fn run_for_audio(&self, source_url: &str, asking: Asking) -> Result<Resolved, String> {
        let turn = match asking {
            Asking::Now => self.turns.take(Lane::Play),
            Asking::Ahead => self.turns.take(Lane::Ahead),
            Asking::IfFree => self.turns.try_take(Lane::Ahead).ok_or("no turn free")?,
        };
        let mut arguments: Vec<String> = vec![
            "--format".into(),
            "bestaudio/best".into(),
            "--print".into(),
            "%(url)s".into(),
            "--print".into(),
            "%(http_headers)j".into(),
            "--no-playlist".into(),
            "--no-warnings".into(),
        ];
        arguments.extend(self.playback_arguments(provider_of(source_url)));
        // What follows a # is Noctorium's own note, not part of the page.
        arguments.extend(["--".into(), source_url.split('#').next().unwrap_or(source_url).to_string()]);
        let output = self.run_retrying_with(turn, &arguments, Duration::from_secs(90))?;
        let resolved = read_resolved(&output).ok_or("yt-dlp returned no playable audio URL")?;
        let mut addresses = self.addresses.lock().unwrap();
        if addresses.len() > 200 {
            addresses.retain(|_, (_, at)| at.elapsed() < ADDRESS_LIFETIME);
        }
        addresses.insert(source_url.to_string(), (resolved.clone(), Instant::now()));
        Ok(resolved)
    }

    /// Forgets an address the service has refused, so the next ask goes back to it.
    pub fn forget_audio(&self, source_url: &str) {
        self.addresses.lock().unwrap().remove(source_url);
    }

    /// A quick failure -- the instant a laptop changes Wi-Fi -- is asked once more before anyone is told.
    fn run_retrying(&self, arguments: &[String], limit: Duration) -> Result<String, String> {
        self.run_retrying_in(Lane::Browse, arguments, limit)
    }

    /// As [`Self::run_retrying_in`], in a turn already taken.
    fn run_retrying_with(&self, turn: Turn<'_>, arguments: &[String], limit: Duration) -> Result<String, String> {
        let started = Instant::now();
        match self.run_watching_in(&turn, arguments, limit, None) {
            Err(why) if started.elapsed() < Duration::from_secs(5) && looks_transient(&why) => {
                std::thread::sleep(Duration::from_millis(600));
                self.run_watching_in(&turn, arguments, limit, None)
            }
            other => other,
        }
    }

    fn run_retrying_in(&self, lane: Lane, arguments: &[String], limit: Duration) -> Result<String, String> {
        let started = Instant::now();
        match self.run_in(lane, arguments, limit) {
            Err(why) if started.elapsed() < Duration::from_secs(5) && looks_transient(&why) => {
                std::thread::sleep(Duration::from_millis(600));
                self.run_in(lane, arguments, limit)
            }
            other => other,
        }
    }

    /// Runs yt-dlp and answers what it printed. Its output goes to files rather than pipes, read once it
    /// has finished: a yt-dlp started from the window, with its output piped back, was seen to finish its
    /// work and then never exit, and files leave nothing for either side to wait on.
    fn run(&self, arguments: &[String], limit: Duration) -> Result<String, String> {
        self.run_in(Lane::Browse, arguments, limit)
    }

    /// Waits for a turn in `lane`, then runs. The time limit starts with the run, not with the wait.
    fn run_in(&self, lane: Lane, arguments: &[String], limit: Duration) -> Result<String, String> {
        self.run_watching(lane, arguments, limit, None)
    }

    /// Downloads a song's audio into `folder` as `stem` and whatever extension it comes with, reporting
    /// progress from 0 to 1. Returns the file it made.
    pub fn download_audio(
        &self,
        source_url: &str,
        folder: &std::path::Path,
        stem: &str,
        format: &str,
        progress: &dyn Fn(f32),
    ) -> Result<PathBuf, String> {
        if !(source_url.starts_with("https://") || source_url.starts_with("http://")) {
            return Err("Only HTTP media sources are accepted".into());
        }
        let template = folder.join(format!("{stem}.%(ext)s"));
        let mut arguments: Vec<String> = vec![
            "--format".into(),
            format.into(),
            "--no-playlist".into(),
            "--no-warnings".into(),
            "--newline".into(),
            "--progress".into(),
            "--no-part".into(),
            "--no-continue".into(),
            "--output".into(),
            template.to_string_lossy().into_owned(),
        ];
        arguments.extend(self.playback_arguments(provider_of(source_url)));
        arguments.extend(["--".into(), source_url.split('#').next().unwrap_or(source_url).to_string()]);
        let watch = |output: &str| {
            if let Some(fraction) = last_progress(output) {
                progress(fraction);
            }
        };
        self.run_watching(Lane::Download, &arguments, Duration::from_secs(30 * 60), Some(&watch))?;
        std::fs::read_dir(folder)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_stem().and_then(|s| s.to_str()) == Some(stem)
                    && p.extension().is_some_and(|e| e != "part" && e != "ytdl")
            })
            .ok_or_else(|| "yt-dlp finished but left no file.".to_string())
    }

    fn run_watching(
        &self,
        lane: Lane,
        arguments: &[String],
        limit: Duration,
        watch: Option<&dyn Fn(&str)>,
    ) -> Result<String, String> {
        let waited = Instant::now();
        let turn = self.turns.take(lane);
        if cfg!(debug_assertions) && waited.elapsed() > Duration::from_secs(1) {
            eprintln!("yt-dlp {:?}: waited {:.1?} for a turn", arguments.last(), waited.elapsed());
        }
        self.run_watching_in(&turn, arguments, limit, watch)
    }

    /// Runs yt-dlp in a turn already taken.
    fn run_watching_in(&self, _turn: &Turn<'_>, arguments: &[String], limit: Duration, watch: Option<&dyn Fn(&str)>) -> Result<String, String> {
        static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = self
            .path()
            .ok_or("yt-dlp is not installed. Settings › Playback tools can install it.")?;
        let folder = std::env::temp_dir().join("noctorium-rust");
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = folder.join(format!("yt-dlp-{}-{run}", std::process::id()));
        let (out_path, err_path) = (base.with_extension("out"), base.with_extension("err"));
        let files = std::fs::File::create(&out_path).and_then(|out| Ok((out, std::fs::File::create(&err_path)?)));
        let (out_file, err_file) = files.map_err(|e| format!("yt-dlp's output has nowhere to go: {e}"))?;
        let mut command = Command::new(path);
        command
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::from(out_file))
            .stderr(Stdio::from(err_file));
        hide_console(&mut command);
        let read_back = || {
            let output = std::fs::read(&out_path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
            let errors = std::fs::read(&err_path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
            let _ = std::fs::remove_file(&out_path);
            let _ = std::fs::remove_file(&err_path);
            (output, errors)
        };
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(e) => {
                read_back();
                return Err(format!("yt-dlp could not be started: {e}"));
            }
        };
        let started = Instant::now();
        let mut last_watch = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() > limit => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let (_, errors) = read_back();
                    if cfg!(debug_assertions) {
                        let tail: Vec<&str> = errors.lines().rev().take(4).collect();
                        eprintln!("yt-dlp {:?}: gave up after {limit:?}; it said {tail:?}", arguments.last());
                    }
                    return Err("yt-dlp took too long to answer.".into());
                }
                Ok(None) => {
                    std::thread::sleep(Duration::from_millis(30));
                    if let Some(watch) = watch {
                        if last_watch.elapsed() > Duration::from_millis(400) {
                            last_watch = Instant::now();
                            if let Ok(bytes) = std::fs::read(&out_path) {
                                watch(&String::from_utf8_lossy(&bytes));
                            }
                        }
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        };
        let (output, errors) = read_back();
        if cfg!(debug_assertions) {
            let shown: Vec<&str> = arguments.iter().map(String::as_str).filter(|a| !a.starts_with("--")).collect();
            eprintln!("yt-dlp {shown:?}: {} in {:.1?}", status, started.elapsed());
        }
        if status.success() {
            Ok(output)
        } else {
            Err(explain(&errors))
        }
    }
}

/// The last "[download]  45.3% of ..." in yt-dlp's output, as a fraction.
fn last_progress(output: &str) -> Option<f32> {
    output.lines().rev().find_map(|line| {
        let rest = line.trim().strip_prefix("[download]")?;
        let percent = rest.split_whitespace().next()?.strip_suffix('%')?;
        percent.parse::<f32>().ok().map(|p| (p / 100.0).clamp(0.0, 1.0))
    })
}

/// yt-dlp's complaint, worded for a reader: its last ERROR line, without the prefix.
fn explain(stderr: &str) -> String {
    stderr
        .lines()
        .rev()
        .find(|line| line.contains("ERROR"))
        .map(|line| line.split_once("ERROR:").map(|(_, rest)| rest).unwrap_or(line).trim().to_string())
        .filter(|line| !line.is_empty())
        .unwrap_or_else(|| "yt-dlp could not do that.".into())
}

fn looks_transient(why: &str) -> bool {
    let lower = why.to_lowercase();
    ["timed out", "connection", "temporary failure", "network", "getaddrinfo", "reset by peer"]
        .iter()
        .any(|word| lower.contains(word))
}

/// Hides the console window a console program would otherwise flash up on Windows.
pub fn hide_console(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

pub fn provider_of(url: &str) -> Option<ProviderType> {
    let lower = url.to_lowercase();
    if lower.contains("music.youtube.com") {
        Some(ProviderType::YoutubeMusic)
    } else if lower.contains("youtube.com") || lower.contains("youtu.be") {
        Some(ProviderType::YoutubeVideo)
    } else if lower.contains("soundcloud.com") {
        Some(ProviderType::Soundcloud)
    } else {
        None
    }
}

/// Whether a link is to many songs rather than one.
pub fn is_playlist_link(url: &str) -> bool {
    let lower = url.to_lowercase();
    (lower.contains("list=") && !lower.contains("watch?v=")) || lower.contains("/playlist") || lower.contains("/sets/")
        || lower.contains("/browse/")
}

fn read_resolved(output: &str) -> Option<Resolved> {
    let lines: Vec<&str> = output.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let url = lines.iter().find(|l| l.starts_with("http"))?.to_string();
    let user_agent = lines
        .iter()
        .find(|l| l.starts_with('{'))
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .and_then(|headers| headers["User-Agent"].as_str().map(String::from));
    Some(Resolved { url, user_agent })
}

fn parse(output: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(output.trim()).map_err(|_| "yt-dlp's answer could not be read.".to_string())
}

fn entries(root: &serde_json::Value) -> Vec<serde_json::Value> {
    root["entries"].as_array().cloned().unwrap_or_default()
}

fn string(item: &serde_json::Value, name: &str) -> Option<String> {
    item.get(name)?.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

/// The cleaned-up name of whoever made it: YouTube's "Artist - Topic" channels lose the " - Topic".
fn clean_artist(name: &str) -> String {
    let trimmed = name.trim();
    let lower = trimmed.to_lowercase();
    if let Some(stripped) = lower.strip_suffix("- topic") {
        trimmed[..stripped.len()].trim().trim_end_matches('-').trim().to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn map_track(provider: ProviderType, item: &serde_json::Value) -> Option<Track> {
    let id = string(item, "id")?;
    let artists_field = item["artists"]
        .as_array()
        .and_then(|list| list.first())
        .and_then(|a| a.as_str())
        .map(String::from);
    let uploader = string(item, "artist")
        .or(artists_field)
        .or_else(|| string(item, "uploader"))
        .or_else(|| string(item, "channel"))
        .map(|name| clean_artist(&name))
        .unwrap_or_else(|| provider.display_name().to_string());
    let url = string(item, "webpage_url")
        .or_else(|| string(item, "original_url"))
        .or_else(|| {
            let raw = string(item, "url")?;
            if raw.starts_with("http") {
                Some(raw)
            } else {
                match provider {
                    ProviderType::YoutubeMusic => Some(format!("https://music.youtube.com/watch?v={raw}")),
                    ProviderType::YoutubeVideo => Some(format!("https://www.youtube.com/watch?v={raw}")),
                    _ => None,
                }
            }
        })?;
    // A YouTube Music search answers with www.youtube.com links; the song is still YouTube Music's.
    let url = if provider == ProviderType::YoutubeMusic && url.contains("www.youtube.com/watch") {
        url.replace("www.youtube.com", "music.youtube.com")
    } else {
        url
    };
    let title = string(item, "track")
        .filter(|_| provider != ProviderType::YoutubeVideo)
        .or_else(|| string(item, "title"))
        .or_else(|| title_from_url(&url))?;
    let duration_ms = item["duration"].as_f64().map(|seconds| (seconds * 1000.0) as i64);
    let artwork_url = best_thumbnail(item).or_else(|| match provider {
        ProviderType::YoutubeMusic | ProviderType::YoutubeVideo => Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg")),
        _ => None,
    });
    let album = string(item, "album").map(|title| Album {
        id: format!("{}:album:{title}", provider.key_name()),
        title,
        artists: vec![],
        provider,
        artwork_url: artwork_url.clone(),
    });
    Some(Track {
        provider,
        artists: vec![Artist {
            id: format!("{}:{uploader}", provider.key_name()),
            name: uploader,
            provider,
        }],
        id,
        title,
        album,
        duration_ms,
        artwork_url,
        source_url: url,
    })
}

/// The track with what a full read of its page says, where that says more.
fn enrich(track: Track, item: &serde_json::Value) -> Track {
    let raw = string(item, "artist")
        .map(|a| a.split(',').next().unwrap_or("").to_string())
        .or_else(|| string(item, "uploader"))
        .or_else(|| string(item, "channel"))
        .or_else(|| string(item, "creator").map(|a| a.split(',').next().unwrap_or("").to_string()));
    let Some(name) = raw
        .map(|n| clean_artist(&n))
        .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case(track.provider.display_name()))
    else {
        return track;
    };
    let artist = Artist {
        id: format!("{}:{name}", track.provider.key_name()),
        name,
        provider: track.provider,
    };
    let title = if track.provider == ProviderType::YoutubeVideo {
        string(item, "title")
    } else {
        string(item, "track").or_else(|| string(item, "title"))
    }
    .unwrap_or_else(|| track.title.clone());
    let artwork = best_thumbnail(item).or(track.artwork_url.clone());
    let album = string(item, "album")
        .map(|title| Album {
            id: format!("{}:album:{title}", track.provider.key_name()),
            title,
            artists: vec![artist.clone()],
            provider: track.provider,
            artwork_url: artwork.clone(),
        })
        .or(track.album.clone());
    Track {
        title,
        artists: vec![artist],
        album,
        duration_ms: item["duration"].as_f64().map(|s| (s * 1000.0) as i64).or(track.duration_ms),
        artwork_url: artwork,
        ..track
    }
}

pub fn has_placeholder_artist(track: &Track) -> bool {
    track.artists.is_empty()
        || track.artists.iter().all(|a| {
            let name = a.name.trim();
            name.is_empty()
                || ["YouTube Music", "YouTube", "SoundCloud"].iter().any(|p| name.eq_ignore_ascii_case(p))
        })
}

/// The smallest cover still large enough to look sharp. yt-dlp's own `thumbnail` is the original upload
/// -- a SoundCloud cover measured at 271 KB against 65 KB for its 500px variant.
pub fn best_thumbnail(item: &serde_json::Value) -> Option<String> {
    let list = item["thumbnails"].as_array().cloned().unwrap_or_default();
    let sized: Vec<(i64, String)> = list
        .iter()
        .filter_map(|t| Some((t["width"].as_i64()?, t["url"].as_str()?.to_string())))
        .collect();
    sized
        .iter()
        .filter(|(w, _)| *w >= 250)
        .min_by_key(|(w, _)| *w)
        .map(|(_, u)| u.clone())
        .or_else(|| string(item, "thumbnail"))
        .or_else(|| sized.iter().max_by_key(|(w, _)| *w).map(|(_, u)| u.clone()))
        .or_else(|| list.last().and_then(|t| t["url"].as_str()).map(String::from))
}

fn title_from_url(url: &str) -> Option<String> {
    let slug = url.trim_end_matches('/').rsplit('/').next()?.split('?').next()?;
    let words = slug.replace(['-', '_'], " ");
    let words = words.trim();
    if words.is_empty() || words.starts_with("watch") {
        return None;
    }
    let mut chars = words.chars();
    let first = chars.next()?.to_uppercase().collect::<String>();
    Some(first + chars.as_str())
}

pub fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_youtube_music_search_entry_becomes_a_song() {
        let item = json!({
            "id": "dQw4w9WgXcQ", "title": "Never Gonna Give You Up", "channel": "Rick Astley - Topic",
            "url": "https://www.youtube.com/watch?v=dQw4w9WgXcQ", "duration": 213.0
        });
        let track = map_track(ProviderType::YoutubeMusic, &item).unwrap();
        assert_eq!(track.artist_line(), "Rick Astley");
        assert_eq!(track.source_url, "https://music.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(track.duration_ms, Some(213_000));
        assert_eq!(track.artwork_url.as_deref(), Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg"));
    }

    #[test]
    fn a_soundcloud_stub_is_named_from_its_link() {
        let item = json!({"id": "123", "url": "https://soundcloud.com/someone/a-quiet-song"});
        let track = map_track(ProviderType::Soundcloud, &item).unwrap();
        assert_eq!(track.title, "A quiet song");
        assert_eq!(track.artist_line(), "SoundCloud");
        assert!(has_placeholder_artist(&track));
    }

    #[test]
    fn the_smallest_sharp_cover_is_chosen() {
        let item = json!({"thumbnails": [
            {"url": "small", "width": 100}, {"url": "right", "width": 300}, {"url": "huge", "width": 2000}
        ], "thumbnail": "original"});
        assert_eq!(best_thumbnail(&item).as_deref(), Some("right"));
        assert_eq!(best_thumbnail(&json!({"thumbnail": "original"})).as_deref(), Some("original"));
    }

    #[test]
    fn the_address_and_its_user_agent_are_read_together() {
        let output = "https://rr1.googlevideo.com/x\n{\"User-Agent\": \"Mozilla/5.0 test\", \"Accept\": \"*/*\"}\n";
        let resolved = read_resolved(output).unwrap();
        assert_eq!(resolved.url, "https://rr1.googlevideo.com/x");
        assert_eq!(resolved.user_agent.as_deref(), Some("Mozilla/5.0 test"));
    }

    #[test]
    fn links_are_told_apart() {
        assert_eq!(provider_of("https://soundcloud.com/a/b"), Some(ProviderType::Soundcloud));
        assert_eq!(provider_of("https://youtu.be/x"), Some(ProviderType::YoutubeVideo));
        assert!(is_playlist_link("https://music.youtube.com/playlist?list=PL123"));
        assert!(is_playlist_link("https://soundcloud.com/a/sets/b"));
        assert!(!is_playlist_link("https://music.youtube.com/watch?v=x&list=RDAMVMx"));
    }

    #[test]
    fn errors_read_as_yt_dlps_last_complaint() {
        let stderr = "WARNING: something\nERROR: [youtube] x: Video unavailable\n";
        assert_eq!(explain(stderr), "[youtube] x: Video unavailable");
    }

    #[test]
    fn download_progress_is_read_from_the_last_line() {
        let output = "[download] Destination: x.m4a\n[download]   3.1% of 4.2MiB\n[download]  45.0% of 4.2MiB at 1MiB/s\n";
        assert_eq!(last_progress(output), Some(0.45));
        assert_eq!(last_progress("nothing yet"), None);
    }

    #[test]
    fn queries_are_encoded_for_a_link() {
        assert_eq!(percent_encode("daft punk & co"), "daft+punk+%26+co");
    }
}

#[cfg(test)]
mod turn_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn browsing_waits_while_a_song_is_looked_up() {
        let turns = Arc::new(Turns::default());
        let play = turns.take(Lane::Play);
        let (tx, rx) = std::sync::mpsc::channel();
        let waiting = turns.clone();
        std::thread::spawn(move || {
            let _turn = waiting.take(Lane::Browse);
            tx.send(()).unwrap();
        });
        assert!(rx.recv_timeout(Duration::from_millis(200)).is_err(), "browsing went ahead of playing");
        drop(play);
        assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok(), "browsing goes once playing is done");
    }

    #[test]
    fn playing_does_not_wait_for_browsing() {
        let turns = Turns::default();
        let _browse = turns.take(Lane::Browse);
        let _play = turns.take(Lane::Play);
    }
}
