//! The window.
//!
//! Immediate mode, as Noctorium Stats is: every frame draws whatever the state is now, and the state changes
//! only here -- from what the listener did, collected as [`Action`]s while the frame is drawn and carried out
//! after it, and from what the threads talking to yt-dlp, mpv and LRCLIB have sent back as [`Event`]s.
//! Nothing waits on the network or on a program on the thread that draws.

mod capture;
mod icons;
mod pages;
mod player;
mod settings;
mod widgets;

use crate::bangs::{self, SearchMode};
use crate::domain::{LocalPlaylist, Playlist, ProviderType, Track};
use crate::lyrics::{self, Lyrics};
use crate::mpv::{Engine, EngineEvent, Levels, Status};
use crate::queue::{Queue, RepeatMode};
use crate::store::{self, Library, Places, Settings};
use crate::theme::{self, Palette};
use crate::tools::{self, Tool};
use crate::ytdlp::{self, Backend, Resolved, YtDlp};
use eframe::egui::{self, Color32, Key, Modifiers};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

pub const TITLE: &str = "Noctorium";

/// Opens the window, and returns when it is closed.
///
/// With wgpu first, and with OpenGL if wgpu cannot open one: each has machines the other does not run on.
/// `NOCTORIUM_RENDERER=glow` asks for OpenGL straight away.
pub fn run(listener: Option<std::net::TcpListener>, in_tray: bool) -> Result<(), String> {
    let listener = std::sync::Arc::new(std::sync::Mutex::new(listener));
    let places = Places::system();
    let wanted_glow = std::env::var("NOCTORIUM_RENDERER").is_ok_and(|r| r.eq_ignore_ascii_case("glow"));
    let order = if wanted_glow {
        vec![eframe::Renderer::Glow]
    } else {
        vec![eframe::Renderer::Wgpu, eframe::Renderer::Glow]
    };
    let mut failures = Vec::new();
    for renderer in order {
        match open_window(places.clone(), renderer, listener.clone(), in_tray) {
            Ok(()) => return Ok(()),
            Err(why) => failures.push(format!("{renderer:?}: {why}")),
        }
    }
    Err(failures.join("
"))
}

fn open_window(
    places: Places,
    renderer: eframe::Renderer,
    listener: std::sync::Arc<std::sync::Mutex<Option<std::net::TcpListener>>>,
    in_tray: bool,
) -> Result<(), String> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_app_id("noctorium-rust")
            .with_inner_size([1240.0, 800.0])
            .with_min_inner_size([860.0, 560.0])
            .with_icon(widgets::app_icon()),
        renderer,
        ..Default::default()
    };
    eframe::run_native(
        TITLE,
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let mut app = App::new(&cc.egui_ctx, places);
            // Before the first frame, which already writes in bold.
            theme::install_fonts(&cc.egui_ctx, app.settings.looks.font);
            app.applied_font = Some(app.settings.looks.font);
            if in_tray {
                cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                app.window_minimised = true;
            }
            if let Some(listener) = listener.lock().unwrap().take() {
                let (events, ctx) = (app.events.clone(), cc.egui_ctx.clone());
                crate::instance::listen(listener, move || {
                    let _ = events.send(Event::ShowWindow);
                    ctx.request_repaint();
                });
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string())
}

#[derive(Clone, Debug, PartialEq)]
pub enum Page {
    Home,
    Search,
    Library,
    Liked,
    Recent,
    Playlist(String),
    Remote(String),
    Queue,
    Downloads,
    Settings,
    /// One page of Settings.
    SettingsAt(settings::SettingsPage),
    Shortcuts,
    NowPlaying,
    Connect,
    /// The likes of the listener's own account on a service.
    ServiceLikes(ProviderType),
    /// The services beyond YouTube and SoundCloud: Deezer, Spotify, Apple Music, Tidal, Bandcamp, VK.
    Services,
    /// The profile: its name, and the history of what was played here, by day.
    Profile,
}

/// What a frame's drawing asked for.
#[derive(Clone, Debug)]
pub enum Action {
    PlayList { tracks: Vec<Track>, index: usize },
    PlayShuffled(Vec<Track>),
    PlayNext(Track),
    AddToQueue(Track),
    AddAllToQueue(Vec<Track>),
    ToggleLike(Track),
    StartRadio(Track),
    AddToPlaylist { id: String, track: Track },
    AddToNewPlaylist(Track),
    RemoveFromPlaylist { id: String, index: usize },
    MoveInPlaylist { id: String, from: usize, to: usize },
    CreatePlaylist(String),
    RenamePlaylist { id: String, title: String },
    DeletePlaylist(String),
    SaveAsLocal(Playlist),
    Open(Page),
    Back,
    Search,
    OpenLink(String),
    Follow(Playlist),
    Unfollow(String),
    TogglePlay,
    Next,
    Previous,
    Seek(i64),
    SetVolume(f32),
    ToggleMute,
    ToggleShuffle,
    CycleRepeat,
    Stop,
    QueueJump(usize),
    QueueRemove(usize),
    QueueMove { from: usize, to: usize },
    QueueShuffleUpcoming,
    QueueClearUpcoming,
    QueueClear,
    PlaySuggestion(usize),
    KeepSuggestion(usize),
    SetTheme(&'static str),
    SetAutoplay(bool),
    SetRestoreQueue(bool),
    SetSpeed(f32),
    SetCookies(Option<String>),
    SetSearchMode(SearchMode),
    InstallTool(Tool),
    ImportNeighbour,
    OpenFolder,
    OpenUrl(String),
    Copy(String),
    ToggleLyrics,
    RefreshHome,
    ShowShortcuts(bool),
    /// A search typed for the window's pictures, see [`capture`].
    CaptureSearch(String),
    SetSleep(Option<u64>),
    SleepAtEndOfSong,
    SetEqualizer(crate::audio::Equalizer),
    SetBoost(u32),
    SetEvenLoudness(bool),
    SetSponsorBlock(bool),
    SetVisualiser(bool),
    Download(Track),
    DownloadAll(Vec<Track>),
    RemoveDownload(Track),
    Export(Track, crate::downloads::ExportFormat),
    SetExportFolder(Option<String>),
    SetExportFormat(crate::downloads::ExportFormat),
    OpenDownloadsFolder,
    SetDiscord(crate::discord::PresenceSettings),
    OpenQuickSearch,
    CloseQuickSearch,
    /// Types into the quick search bar, for the window's pictures.
    QuickSearchType(String),
    ConnectScrobbler(crate::scrobble::Site),
    SignIn(ProviderType),
    AccountLogIn { email: String, password: String },
    AccountSignUp { email: String, password: String, name: String },
    AccountSignOut,
    SetDeezerProfile(Option<String>),
    SpotifySignIn,
    SetShortcut { id: &'static str, global: bool, chord: Option<String> },
    FocusSearch,
    ResetShortcut(&'static str),
    SetGlobalShortcuts(bool),
    Capture(Option<(&'static str, bool)>),
    StopAtStart,
    QueueClearAllButCurrent,
    SetBoostBy(i32),
    ToggleWindow,
    SetCloseToTray(bool),
    SetWidgets(crate::widgets::Preferences),
    ConnectSend(crate::connect::Peer),
    ConnectBringBack(crate::connect::Peer),
    ConnectCommand(crate::connect::Peer, crate::connect::CommandType),
    SetStartup(crate::startup::Mode),
    AppleSignIn,
    VkSignIn,
    VkSignOut,
    AppleSignOut,
    SetTidalApp { client_id: String, client_secret: String },
    TidalSignIn,
    TidalSignOut,
    SpotifySignOut,
    ImportSpotifyData(String),
    SignOut(ProviderType),
    AddToServicePlaylist { playlist_id: String, title: String, track: Track },
    CancelAuthorization,
    ConnectListenBrainz(String),
    DisconnectScrobbler(crate::scrobble::Site),
    QuickSearchAll(String),
    SetLooks(Box<crate::looks::Looks>),
    /// Every plain setting at once, from Settings: saved, with whatever changing it needs done.
    SetSettings(Box<Settings>),
    /// A song picked on Home or from a search: a radio from it, or the rest of its shelf, as Settings say.
    PlayPicked { tracks: Vec<Track>, index: usize },
    ClearHistory,
    /// Saves what the listener calls a song here; None for both forgets the edit.
    EditTrack { track: Track, title: Option<String>, artist: Option<String> },
    ToggleStopAfter,
    /// Opens the edit details dialog for a song, with what it is called now.
    EditDetails(Track, String, String),
    AddConnectAddress(String),
    RemoveConnectAddress(String),
    /// Follows the artist of a YouTube song, or stops.
    SetFollowing { video_id: String, channel_id: String, follow: bool },
    RunDiagnostics,
    InstallUpdate,
    /// Look for a newer version now.
    CheckUpdates,
    /// Close, and start again as the version put in place.
    RestartToUpdate,
    LoadSpotifyDevices,
    /// Looks up the artist of a YouTube song, for the follow button.
    LoadArtist(String),
    /// Finds songs like the one playing, for Now playing's Related.
    LoadRelated(Track),
    /// Shows another source's lyrics for the song playing.
    ChooseLyrics(crate::lyrics::Provider),
    SetLyricsSource(crate::lyrics::Provider),
    /// Searches Settings, for the window's pictures, see [`capture`].
    CaptureSettingsQuery(String),
    /// Makes a playlist on YouTube Music with these songs (the YouTube ones among them).
    YouTubeCreate { title: String, tracks: Vec<Track> },
    YouTubeEdit { id: String, url: String, edit: crate::accounts::PlaylistEdit },
    YouTubeRemove { id: String, url: String, track: Track },
    YouTubeDelete { id: String, url: String },
}

/// A Spotify device: its id, its name, and whether it is the one playing.
pub type SpotifyDevice = (String, String, bool);

/// What Settings › Diagnostics looks at, gathered on the window's thread and checked on another.
struct Diagnose {
    yt_dlp: Option<std::path::PathBuf>,
    mpv: Option<std::path::PathBuf>,
    folder: Option<std::path::PathBuf>,
    connect: Result<u16, String>,
    widgets: Option<String>,
}

impl Diagnose {
    /// Each check: its name, whether it passed, and what was found.
    fn run(self) -> Vec<(String, bool, String)> {
        // The two programs answer side by side: each can take a while to start on a busy computer.
        let mut checks: Vec<(String, bool, String)> = std::thread::scope(|scope| {
            [(Tool::YtDlp, self.yt_dlp.clone()), (Tool::Mpv, self.mpv.clone())]
                .map(|(tool, path)| {
                    scope.spawn(move || match path {
                        Some(path) => match tools::version(tool, &path) {
                            Some(version) => (tool.name().to_string(), true, format!("{version} at {}", path.display())),
                            None => (tool.name().to_string(), false, format!("Found at {} but it would not start", path.display())),
                        },
                        None => (tool.name().to_string(), false, "Not installed: Settings › Playback tools installs it".into()),
                    })
                })
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| ("Tools".into(), false, "The check failed".into())))
                .collect()
        });
        checks.push(match self.folder {
            Some(folder) => {
                let probe = folder.join(".noctorium-write-test");
                let ok = std::fs::create_dir_all(&folder).and_then(|_| std::fs::write(&probe, b"ok")).is_ok();
                let _ = std::fs::remove_file(&probe);
                ("Storage".into(), ok, if ok { format!("Writes to {}", folder.display()) } else { format!("Cannot write to {}", folder.display()) })
            }
            None => ("Storage".into(), false, "No folder for the library".into()),
        });
        // The services all at once, each with its own time.
        let reached: Vec<(String, bool, String)> = std::thread::scope(|scope| {
            [
                ("YouTube Music", "https://music.youtube.com/"),
                ("SoundCloud", "https://soundcloud.com/"),
                ("LRCLIB lyrics", "https://lrclib.net/api/search?q=hello"),
                ("Noctorium service", "https://noctorium-service.vercel.app/"),
                ("GitHub (updates, tools)", "https://api.github.com/"),
            ]
            .map(|(name, url)| {
                scope.spawn(move || {
                    let started = Instant::now();
                    let answer = ureq::get(url).timeout(Duration::from_secs(10)).call();
                    let took = started.elapsed().as_millis();
                    match answer {
                        Ok(_) | Err(ureq::Error::Status(..)) => (name.to_string(), true, format!("Answered in {took} ms")),
                        Err(e) => (name.to_string(), false, format!("Unreachable: {e}")),
                    }
                })
            })
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| ("Network".into(), false, "The check failed".into())))
            .collect()
        });
        checks.extend(reached);
        checks.push(match self.connect {
            Ok(port) => ("Connect".into(), true, format!("Listening on port {port}")),
            Err(why) => ("Connect".into(), false, why),
        });
        checks.push(match self.widgets {
            Some(address) => ("Stream widgets".into(), true, format!("Served at {address}")),
            None => ("Stream widgets".into(), true, "Off".into()),
        });
        checks
    }
}

/// What the threads send back.
enum Event {
    Search { id: u64, provider: ProviderType, result: Result<Vec<Track>, String> },
    Home { id: u64, slot: usize, row: HomeRow },
    Resolved { request: u64, track: Track, start_ms: i64, paused: bool, result: Result<Resolved, String> },
    Engine(EngineEvent),
    Remote { url: String, result: Result<Playlist, String> },
    Link { result: Result<Track, String> },
    Radio { seed: String, result: Result<Vec<Track>, String> },
    Lyrics { key: String, result: Result<Option<Lyrics>, String> },
    Enriched(Track),
    ArtworkColour { url: String, colour: Option<Color32> },
    ToolProgress(Tool, f32),
    ToolDone { tool: Tool, result: Result<String, String> },
    ToolVersions { yt_dlp: Option<String>, mpv: Option<String> },
    Segments { key: String, segments: Vec<crate::sponsorblock::Segment> },
    HotKey(crate::platform::Pressed),
    Tray(crate::tray::Command),
    ConnectKey(Result<String, String>),
    ConnectAsked(crate::connect::Asked),
    ConnectDone { message: Result<String, String>, handover: Option<crate::connect::Handover> },
    JobProgress { key: String, progress: f32 },
    ShowWindow,
    QuickResults { id: u64, result: Result<Vec<Track>, String> },
    Scrobbled { site: crate::scrobble::Site, failed: Option<(crate::scrobble::Pending, String)> },
    AuthorizationBegun { site: crate::scrobble::Site, result: Result<(String, String), String> },
    AuthorizationAnswered { site: crate::scrobble::Site, result: Result<(String, String), String> },
    ListenBrainzChecked { token: String, result: Result<String, String> },
    SignedIn { provider: ProviderType, result: Result<SignedIn, String> },
    ServiceLibrary(ServicePiece),
    ServiceDone { message: Result<String, String> },
    /// Work whose answer needs nothing done.
    Quiet,
    Artist { video_id: String, result: Result<crate::accounts::FollowedArtist, String> },
    ConnectAdded { address: String, result: Result<crate::connect::Peer, String>, quiet: bool },
    Diagnostics(Vec<(String, bool, String)>),
    UpdateStarted(Result<(), String>),
    /// The new version is in place of this one, ready at the next start: its version, or why not.
    UpdateStaged(Result<String, String>),
    SpotifyDevices(Result<Vec<(String, String, bool)>, String>),
    Related { key: String, result: Result<Vec<Track>, String> },
    /// A YouTube playlist changed: the library is read again, and the playlist itself if it was open.
    YouTubeChanged { message: Result<String, String>, url: Option<String>, gone: bool },
    AccountSignedIn { result: Result<(crate::account::User, String), String>, quietly: bool },
    AccountStats(Result<crate::account::Stats, String>),
    DeezerChart(Result<Chart, String>),
    DeezerProfile(Result<crate::deezer::Profile, String>),
    BandcampDiscover { newest: bool, result: Result<Vec<Track>, String> },
    SpotifySignedIn(Result<(crate::spotify::Tokens, String), String>),
    AppleCharts(Result<Chart, String>),
    VkSignedIn(Result<(crate::vk::Cookies, String), String>),
    VkLibrary { music: Result<Vec<Track>, String>, playlists: Vec<Playlist>, recommended: Vec<Track> },
    AppleSignedIn(Result<(String, crate::applemusic::Library), String>),
    TidalReady { result: Result<crate::tidal::Tokens, String>, signed_in: bool },
    TidalCollection(Result<(String, Vec<Track>, Vec<Playlist>), String>),
    SpotifyLibrary(SpotifyPiece),
    SpotifyImported(Result<crate::spotify::Export, String>),
    PlaysReported { sent: Vec<crate::account::PlayReport>, ok: bool },
    Update(Option<crate::update::Newer>),
    Downloaded { key: String, result: Result<crate::downloads::Downloaded, String> },
    Exported { key: String, title: String, result: Result<std::path::PathBuf, String> },
}

/// One part of the Spotify library, as it arrives.
pub enum SpotifyPiece {
    Name(String),
    Playlists(Vec<Playlist>),
    Likes(Result<Vec<Track>, String>),
    Top(Vec<Track>),
    Recent(Vec<Track>),
}

/// A service the listener has just signed in to.
pub enum SignedIn {
    YouTube(crate::accounts::YouTube, String),
    SoundCloud(crate::accounts::SoundCloud, crate::accounts::SoundCloudProfile),
}

/// One part of an account's library, as it arrives.
pub enum ServicePiece {
    YouTubePlaylists(Vec<Playlist>),
    YouTubeLikes(Result<Vec<Track>, String>),
    SoundCloudPlaylists(Vec<Playlist>),
    SoundCloudLikes(Result<Vec<Track>, String>),
    YouTubeHome(Result<Vec<(String, Vec<Track>)>, String>),
}

fn sign_in_soundcloud(session: crate::accounts::SoundCloud) -> Result<SignedIn, String> {
    let profile = session.profile()?;
    Ok(SignedIn::SoundCloud(session, profile))
}

/// The sleep timer: a time to pause at, or the end of the song playing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sleep {
    At(Instant),
    EndOfSong,
}

/// The minutes the sleep timer offers, as the Kotlin players do.
pub const SLEEP_PRESETS: [u64; 5] = [15, 30, 45, 60, 90];

fn levels_for(settings: &Settings, repeat: RepeatMode, meter: bool) -> Levels {
    let shaping = crate::audio::Shaping {
        equalizer: settings.equalizer.clone(),
        boost_percent: settings.boost_percent,
        volume: settings.volume,
        even_loudness: settings.even_loudness,
    };
    Levels {
        volume: (shaping.mpv_volume() / 100.0) as f32,
        muted: settings.muted,
        speed: settings.speed,
        looping: repeat == RepeatMode::One,
        filters: {
            let chain = shaping.filters();
            match (meter, chain.is_empty()) {
                (false, _) => chain,
                (true, true) => crate::audio::spectrum_meter(),
                (true, false) => format!("{chain},{}", crate::audio::spectrum_meter()),
            }
        },
    }
}

#[derive(Clone, Debug)]
pub struct HomeRow {
    /// The service the row is from, for hiding the rows of a service in Settings › Browsing.
    pub provider: ProviderType,
    pub title: String,
    pub subtitle: String,
    pub tracks: Result<Vec<Track>, String>,
}

/// One service's answer to a search: None while it is still being asked.
pub type ProviderResults = (ProviderType, Option<Result<Vec<Track>, String>>);

#[derive(Default)]
pub struct SearchState {
    pub id: u64,
    pub query: String,
    pub mode: Option<SearchMode>,
    pub results: Vec<ProviderResults>,
}

pub enum Loadable<T> {
    Loading,
    Ready(T),
    Failed(String),
}

/// What text fields and open dialogs hold between frames.
#[derive(Default)]
pub struct Inputs {
    pub settings_query: String,
    pub listenbrainz_token: String,
    pub tidal_client_id: String,
    pub tidal_client_secret: String,
    pub spotify_export: String,
    pub deezer_profile: String,
    pub account_email: String,
    pub account_password: String,
    pub account_name: String,
    pub account_new: bool,
    pub search: String,
    pub focus_search: bool,
    pub new_playlist: String,
    pub follow_link: String,
    pub renaming: Option<(String, String)>,
    /// The YouTube playlist being edited: its id, name, description and privacy.
    pub youtube_edit: Option<(String, String, String, crate::accounts::Privacy)>,
    pub confirm_delete: Option<String>,
    pub scrubbing: Option<f32>,
    pub profile_name: Option<String>,
    /// The song whose details are being edited, with the title and artist typed so far.
    pub editing: Option<(Track, String, String)>,
    pub connect_address: String,
    pub connect_name: Option<String>,
    /// How much of the history the profile shows; more on asking.
    pub history_shown: usize,
}

pub struct Toast {
    pub text: String,
    pub error: bool,
    pub at: Instant,
}

pub struct ToolInfo {
    pub path: Option<std::path::PathBuf>,
    pub version: Option<String>,
    pub installing: Option<f32>,
}

pub struct App {
    ctx: egui::Context,
    places: Places,
    pub settings: Settings,
    pub library: Library,
    pub queue: Queue,
    backend: Backend,
    engine: Engine,
    events: Sender<Event>,
    receiver: Receiver<Event>,

    pub page: Page,
    history: Vec<Page>,
    inputs: Inputs,
    pub search: SearchState,
    next_id: u64,
    pub home: Vec<Option<HomeRow>>,
    home_id: u64,
    pub remote: HashMap<String, Loadable<Playlist>>,

    /// The newest ask for a stream address; older answers are thrown away.
    play_request: u64,
    /// The song whose address is being looked up.
    pub resolving: Option<Track>,
    /// The engine's number for the song playing, to know whose end it reports.
    generation: u64,
    /// The song mpv has, which is the queue's current one unless the queue moved since.
    pub loaded: Option<Track>,
    /// Where to start the queue's current song when play is pressed, after a restore.
    resume_at: i64,
    retried: HashSet<String>,
    prefetched: HashSet<String>,

    pub lyrics: HashMap<String, Loadable<Option<Lyrics>>>,
    artwork_colours: HashMap<String, Option<Color32>>,
    glow_colour: Option<Color32>,
    pub toasts: Vec<Toast>,
    pub yt_dlp: ToolInfo,
    pub mpv: ToolInfo,
    pub show_shortcuts: bool,
    pub palette: Palette,
    /// The palette, corners and skin egui was last dressed in, and the typeface; done again when they change.
    applied_look: Option<(Palette, crate::looks::CornerStyle, theme::Skin)>,
    applied_font: Option<crate::looks::FontChoice>,
    applied_text_size: Option<f32>,
    applied_animations: Option<bool>,
    /// What was played here, newest first, for the profile.
    pub play_history: Vec<store::HistoryEntry>,
    /// The Connect key, kept so Connect can start again when it is switched on or renamed.
    connect_key: Option<String>,
    last_connect_probe: Instant,
    /// How loud the sleep timer's fade has left the music: 1 until it starts.
    sleep_fade: f32,
    /// The artists of YouTube songs, by video, for the follow button.
    pub artists: HashMap<String, Loadable<crate::accounts::FollowedArtist>>,
    pub diagnostics: Option<Loadable<Vec<(String, bool, String)>>>,
    pub spotify_devices: Option<Result<Vec<SpotifyDevice>, String>>,
    pub installing_update: bool,
    /// The version put in place of this one, ready at the next start.
    pub update_ready: Option<String>,
    /// A check for a newer version is under way.
    pub checking_update: bool,
    /// When a newer version was last looked for, so one left open for days still finds it.
    pub update_checked: Instant,
    /// Songs like each song, by its key, for Now playing's Related.
    pub related: HashMap<String, Loadable<Vec<Track>>>,
    /// What the listener calls songs here, by their key.
    pub track_edits: HashMap<String, store::TrackEdit>,
    /// When the page in view was opened, for it to fade in.
    pub page_opened: Instant,
    queue_dirty: Option<Instant>,
    last_title: String,
    capture: Option<capture::Capture>,
    pub sleep: Option<Sleep>,
    skipper: crate::sponsorblock::Skipper,
    /// The media keys the system let Noctorium have.
    pub media_keys: Vec<&'static str>,
    hot_keys: Option<crate::platform::HotKeys>,
    /// The actions behind the global chords registered, by their order in the registration.
    pub global_actions: Vec<usize>,
    /// Which of those the system granted.
    pub global_granted: Vec<bool>,
    /// A shortcut being changed: the action's id, and whether it is the global chord.
    pub capturing: Option<(&'static str, bool)>,
    window_minimised: bool,
    tray: Option<crate::tray::Tray>,
    pub widget_server: Option<crate::widgets::Server>,
    pub connect: Option<std::sync::Arc<crate::connect::Connect>>,
    pub connect_problem: Option<String>,
    /// The device being driven from here, by its id, and the name it went by.
    pub driving: Option<(String, String)>,
    /// Who is driving this device, by name.
    pub driven_by: Option<String>,
    widgets_fed_at: Option<Instant>,
    tray_shown: (Option<String>, bool),
    /// Set by Quit in the tray: the window really closes, rather than going to the tray.
    quitting: bool,
    told_about_tray: bool,
    pub startup_mode: crate::startup::Mode,
    pub downloads: crate::downloads::Store,
    /// Downloads and saves under way, by song, with how far along each is.
    pub jobs: HashMap<String, (String, f32)>,
    pub presence: crate::discord::Presence,
    /// What the Discord card was last made from: the song, whether it played, and when it was sent.
    presence_sent: Option<(Option<String>, bool, i64, Instant)>,
    pub newer: Option<crate::update::Newer>,
    pub quick: Option<QuickSearch>,
    pub scrobblers: std::collections::BTreeMap<crate::scrobble::Site, crate::scrobble::Account>,
    /// A Last.fm or Libre.fm connection waiting for the listener to allow it in the browser.
    pub authorizing: Option<Authorizing>,
    listening: Listening,
    pending_scrobbles: Vec<crate::scrobble::Pending>,
    pub youtube: Option<crate::accounts::YouTube>,
    pub soundcloud: Option<(crate::accounts::SoundCloud, crate::accounts::SoundCloudProfile)>,
    pub youtube_name: Option<String>,
    /// The service being signed in to, while it is.
    pub signing_in: Option<ProviderType>,
    pub service_library: ServiceLibrary,
    pub services: ServicesData,
    matches: std::sync::Arc<std::sync::Mutex<crate::matching::Store>>,
    /// The Noctorium account: who, its token, and its statistics once read.
    pub account: Option<(crate::account::User, String)>,
    pub account_stats: Option<crate::account::Stats>,
    pub account_busy: bool,
    unreported_plays: Vec<crate::account::PlayReport>,
}

/// A chart: its songs, and its playlists.
pub type Chart = (Vec<Track>, Vec<Playlist>);

/// What the Services page shows, read when it is first opened.
#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct ServicesData {
    pub loaded: bool,
    pub deezer_chart: Option<Result<Chart, String>>,
    pub deezer_profile: Option<Result<crate::deezer::Profile, String>>,
    pub bandcamp_top: Option<Result<Vec<Track>, String>>,
    pub spotify_name: Option<String>,
    pub vk_name: Option<String>,
    pub vk_signing_in: bool,
    pub vk_music: Option<Result<Vec<Track>, String>>,
    pub vk_playlists: Vec<Playlist>,
    pub vk_recommended: Vec<Track>,
    pub apple_charts: Option<Result<Chart, String>>,
    pub apple_signed_in: bool,
    pub apple_library: Option<Result<(Vec<Playlist>, Vec<Track>), String>>,
    pub tidal_ready: bool,
    pub tidal_name: Option<String>,
    pub tidal_signing_in: bool,
    pub tidal_collection: Option<Result<(Vec<Track>, Vec<Playlist>), String>>,
    pub spotify_signing_in: bool,
    pub spotify_playlists: Vec<Playlist>,
    pub spotify_likes: Option<Result<Vec<Track>, String>>,
    pub spotify_top: Vec<Track>,
    pub spotify_recent: Vec<Track>,
    pub bandcamp_new: Option<Result<Vec<Track>, String>>,
}

/// What the listener's own accounts hold.
#[derive(Default)]
pub struct ServiceLibrary {
    pub youtube_playlists: Vec<Playlist>,
    pub youtube_likes: Option<Result<Vec<Track>, String>>,
    pub soundcloud_playlists: Vec<Playlist>,
    pub soundcloud_likes: Option<Result<Vec<Track>, String>>,
}

pub struct Authorizing {
    pub site: crate::scrobble::Site,
    pub token: Option<String>,
    pub url: Option<String>,
    pub asked_at: Instant,
    pub attempts: u32,
    pub asking: bool,
}

/// How much of the song playing has been heard, for scrobbling.
#[derive(Default)]
struct Listening {
    key: Option<String>,
    generation: u64,
    heard_ms: i64,
    duration_ms: i64,
    track: Option<Track>,
    started_at: i64,
    last_tick: Option<Instant>,
    scrobbled: bool,
}

/// The quick search bar, Ctrl+F: results as you type, over whatever page is open.
pub struct QuickSearch {
    pub text: String,
    pub searched: String,
    pub id: u64,
    pub typed_at: Instant,
    pub results: Option<Result<Vec<Track>, String>>,
    pub selected: usize,
    pub focus: bool,
}

impl App {
    fn new(ctx: &egui::Context, places: Places) -> App {
        let mut settings = store::load_settings(&places);
        let mut library = store::load_library(&places);
        if !settings.imported_neighbour {
            let brought = store::import_neighbour(&places, &mut library);
            settings.imported_neighbour = true;
            store::save_settings(&places, &settings);
            if brought > 0 {
                store::save_playlists(&places, &library);
                store::save_liked(&places, &library);
                store::save_recent(&places, &library);
            }
        }
        let (events, receiver) = channel();
        let yt_dlp_path = tools::find(Tool::YtDlp, &places);
        let mpv_path = tools::find(Tool::Mpv, &places);
        let backend = YtDlp::new(yt_dlp_path.clone());
        backend.set_cookies_browser(settings.cookies_browser.clone());
        let notify = {
            let events = events.clone();
            let ctx = ctx.clone();
            move |event: EngineEvent| {
                let _ = events.send(Event::Engine(event));
                ctx.request_repaint();
            }
        };
        let levels = levels_for(&settings, settings.repeat, false);
        let engine = Engine::new(mpv_path.clone(), levels, notify);
        let mut queue = Queue::default();
        queue.repeat = settings.repeat;
        queue.shuffle = settings.shuffle;
        let mut resume_at = 0;
        if settings.restore_queue {
            let saved = store::load_queue(&places);
            resume_at = saved.position_ms.max(0);
            queue.restore(saved.tracks, saved.current_index, settings.repeat);
            queue.shuffle = settings.shuffle;
        }
        let palette = theme::palette_for(&settings.theme, &settings.looks, None);
        let places_for_downloads = places.clone();
        let mut app = App {
            ctx: ctx.clone(),
            places,
            library,
            queue,
            backend,
            engine,
            events,
            receiver,
            page: match settings.start_page {
                store::StartPage::Home => Page::Home,
                store::StartPage::Search => Page::Search,
                store::StartPage::Library => Page::Library,
                store::StartPage::NowPlaying => Page::NowPlaying,
            },
            history: Vec::new(),
            inputs: Inputs::default(),
            search: SearchState::default(),
            next_id: 0,
            home: Vec::new(),
            home_id: 0,
            remote: HashMap::new(),
            play_request: 0,
            resolving: None,
            generation: 0,
            loaded: None,
            resume_at,
            retried: HashSet::new(),
            prefetched: HashSet::new(),
            lyrics: HashMap::new(),
            artwork_colours: HashMap::new(),
            glow_colour: None,
            toasts: Vec::new(),
            yt_dlp: ToolInfo { path: yt_dlp_path, version: None, installing: None },
            mpv: ToolInfo { path: mpv_path, version: None, installing: None },
            show_shortcuts: false,
            palette,
            applied_look: None,
            applied_font: None,
            applied_text_size: None,
            applied_animations: None,
            play_history: store::load_history(&places_for_downloads),
            connect_key: None,
            last_connect_probe: Instant::now(),
            sleep_fade: 1.0,
            artists: HashMap::new(),
            diagnostics: None,
            spotify_devices: None,
            installing_update: false,
            update_ready: None,
            checking_update: false,
            update_checked: Instant::now(),
            related: HashMap::new(),
            page_opened: Instant::now(),
            track_edits: store::load_track_edits(&places_for_downloads),
            queue_dirty: None,
            last_title: String::new(),
            capture: capture::Capture::from_environment(),
            sleep: None,
            skipper: crate::sponsorblock::Skipper::default(),
            media_keys: Vec::new(),
            hot_keys: None,
            global_actions: Vec::new(),
            global_granted: Vec::new(),
            capturing: None,
            window_minimised: false,
            tray: None,
            widget_server: None,
            connect: None,
            connect_problem: None,
            driving: None,
            driven_by: None,
            widgets_fed_at: None,
            tray_shown: (None, false),
            quitting: false,
            told_about_tray: false,
            startup_mode: crate::startup::Mode::Off,
            downloads: crate::downloads::Store::open(&places_for_downloads),
            jobs: HashMap::new(),
            presence: crate::discord::Presence::start(),
            presence_sent: None,
            newer: None,
            quick: None,
            scrobblers: std::collections::BTreeMap::new(),
            authorizing: None,
            listening: Listening::default(),
            pending_scrobbles: Vec::new(),
            youtube: None,
            soundcloud: None,
            youtube_name: None,
            signing_in: None,
            service_library: ServiceLibrary::default(),
            services: ServicesData::default(),
            matches: std::sync::Arc::new(std::sync::Mutex::new(crate::matching::Store::open(&places_for_downloads))),
            account: None,
            account_stats: None,
            account_busy: false,
            unreported_plays: Vec::new(),
            settings,
        };
        app.read_tool_versions();
        app.load_home();
        if app.settings.check_updates {
            app.checking_update = true;
            app.spawn(|| Event::Update(crate::update::check()));
        }
        for site in crate::scrobble::Site::ALL {
            if let Some(account) = crate::secrets::load(&app.places, site.secret_name())
                .and_then(|text| serde_json::from_str::<crate::scrobble::Account>(&text).ok())
            {
                app.scrobblers.insert(site, account);
            }
        }
        if let Some(cookies) = crate::secrets::load(&app.places, "youtube.cookies") {
            app.youtube = Some(crate::accounts::YouTube { cookies });
            app.youtube_name = crate::secrets::load(&app.places, "youtube.name");
        }
        if let Some(tokens) = crate::secrets::load(&app.places, "spotify.tokens")
            .and_then(|t| serde_json::from_str::<crate::spotify::Tokens>(&t).ok())
        {
            crate::spotify::set_current(Some(crate::spotify::Account::new(tokens)));
            app.load_spotify();
        }
        crate::services::sessions().apple_storefront = app.settings.apple_storefront.clone();
        if let Some(cookies) = crate::secrets::load(&app.places, "vk.cookies")
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| Some(crate::vk::Cookies { p: v["p"].as_str()?.to_string(), remixsid: v["remixsid"].as_str()?.to_string() }))
        {
            let session = crate::vk::Session::new(cookies.clone());
            crate::vk::set_current(Some(session.clone()));
            app.spawn(move || Event::VkSignedIn(session.name().map(|name| (cookies, name))));
        }
        if let Some(token) = crate::secrets::load(&app.places, "applemusic.token") {
            crate::services::sessions().apple_token = Some(token.clone());
            app.services.apple_signed_in = true;
            app.spawn(move || Event::AppleSignedIn(crate::applemusic::library(&token).map(|library| (token, library))));
        }
        if let Some(tokens) = crate::secrets::load(&app.places, "tidal.tokens").and_then(|t| serde_json::from_str::<crate::tidal::Tokens>(&t).ok()) {
            app.spawn(move || Event::TidalReady { result: Ok(tokens), signed_in: true });
        } else {
            app.start_tidal_catalogue();
        }
        if let Some(token) = crate::secrets::load(&app.places, "soundcloud.token") {
            let session = crate::accounts::SoundCloud { token };
            app.spawn(move || Event::SignedIn { provider: ProviderType::Soundcloud, result: sign_in_soundcloud(session) });
        }
        if let Some(token) = crate::secrets::load(&app.places, "noctorium.token") {
            app.spawn(move || Event::AccountSignedIn { result: crate::account::who_am_i(&token).map(|user| (user, token)), quietly: true });
        }
        app.unreported_plays = app
            .places
            .file("plays-queue.json")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        app.load_service_library();
        if app.youtube.is_some() {
            app.load_home();
        }
        app.pending_scrobbles = app
            .places
            .file("scrobble-queue.json")
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        app.start_hot_keys();
        let icon = widgets::app_icon();
        let (events, ctx) = (app.events.clone(), app.ctx.clone());
        app.tray = crate::tray::Tray::start(icon.rgba, icon.width, move |command| {
            let _ = events.send(Event::Tray(command));
            ctx.request_repaint();
        });
        app.startup_mode = crate::startup::mode();
        if app.settings.widgets.enabled {
            app.start_widgets();
        }
        if app.yt_dlp.path.is_none() || app.mpv.path.is_none() {
            app.toast("yt-dlp or mpv is missing. Settings › Playback tools can install them.", true);
        }
        app
    }

    // ---- threads ----------------------------------------------------------------------------------

    /// Runs `work` on a thread of its own and posts what it answers.
    fn spawn(&self, work: impl FnOnce() -> Event + Send + 'static) {
        let events = self.events.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let event = work();
            let _ = events.send(event);
            ctx.request_repaint();
        });
    }

    fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn toast(&mut self, text: impl Into<String>, error: bool) {
        let text = text.into();
        self.toasts.retain(|t| t.text != text);
        self.toasts.push(Toast { text, error, at: Instant::now() });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    fn read_tool_versions(&self) {
        let (yt, mpv) = (self.yt_dlp.path.clone(), self.mpv.path.clone());
        self.spawn(move || Event::ToolVersions {
            yt_dlp: yt.and_then(|p| tools::version(Tool::YtDlp, &p)),
            mpv: mpv.and_then(|p| tools::version(Tool::Mpv, &p)),
        });
    }

    // ---- home and search --------------------------------------------------------------------------

    fn load_home(&mut self) {
        self.home_id += 1;
        let id = self.home_id;
        let mut searches: Vec<(String, String, ProviderType, String)> = Vec::new();
        if let Some(artist) = self
            .library
            .recent
            .iter()
            .find(|t| t.provider.playable_here() && !ytdlp::has_placeholder_artist(t))
            .and_then(|t| t.artists.first())
        {
            searches.push((
                format!("More from {}", artist.name),
                "From what you played last".into(),
                ProviderType::YoutubeMusic,
                artist.name.clone(),
            ));
        }
        // Each row says what it is: the same fixed searches as the Kotlin players without an account.
        searches.extend([
            ("Indie electronic".into(), "Fresh on YouTube Music".into(), ProviderType::YoutubeMusic, "indie electronic music".into()),
            ("New electronic".into(), "Rising on SoundCloud".into(), ProviderType::Soundcloud, "new electronic music".into()),
            ("Ambient and focus".into(), "Long players, no vocals".into(), ProviderType::YoutubeMusic, "ambient focus music".into()),
            ("Lo-fi and remixes".into(), "Uploads and edits".into(), ProviderType::Soundcloud, "lofi remix".into()),
        ]);
        let looks = &self.settings.looks;
        searches.retain(|(_, _, provider, _)| crate::looks::HomePart::of(*provider).is_none_or(|part| looks.shows_home(part)));
        let genres: Vec<(String, String)> = if looks.shows_home(crate::looks::HomePart::Bandcamp) {
            self.settings
                .bandcamp_genres
                .iter()
                .filter_map(|tag| crate::bandcamp::GENRES.iter().find(|(t, _)| t == tag).map(|(t, n)| (t.to_string(), n.to_string())))
                .collect()
        } else {
            vec![]
        };
        self.home = vec![None; searches.len() + genres.len()];
        // Bandcamp's best-sellers in each genre picked, after the searches; they need no yt-dlp.
        for (offset, (tag, name)) in genres.into_iter().enumerate() {
            let slot = searches.len() + offset;
            self.spawn(move || Event::Home {
                id,
                slot,
                row: HomeRow {
                    provider: ProviderType::Bandcamp,
                    title: format!("{name} on Bandcamp"),
                    subtitle: "Best-selling this week".into(),
                    tracks: crate::bandcamp::discover_in(false, Some(&tag)),
                },
            });
        }
        if self.yt_dlp.path.is_none() {
            return;
        }
        if let Some(session) = self.youtube.clone().filter(|_| self.settings.looks.shows_home(crate::looks::HomePart::YoutubeMusic)) {
            self.spawn(move || Event::ServiceLibrary(ServicePiece::YouTubeHome(session.home())));
        }
        for (slot, (title, subtitle, provider, query)) in searches.into_iter().enumerate() {
            let backend = self.backend.clone();
            self.spawn(move || Event::Home {
                id,
                slot,
                row: HomeRow { provider, title, subtitle, tracks: crate::services::search(&backend, provider, &query, 12) },
            });
        }
    }

    fn run_search(&mut self) {
        let text = self.inputs.search.trim().to_string();
        if text.is_empty() {
            return;
        }
        if text.starts_with("http://") || text.starts_with("https://") || crate::sharelink::is_link(&text) {
            self.open_link(text);
            return;
        }
        let parsed = bangs::parse(&text);
        let mode = parsed.mode().unwrap_or(self.settings.search_mode);
        if !mode.offered_here() {
            self.toast(
                format!("{} needs a sign-in this version does not have yet. Try !ytm or !sc.", mode.display_name()),
                true,
            );
            return;
        }
        let providers: Vec<ProviderType> = if mode == SearchMode::Hybrid {
            let chosen: Vec<ProviderType> = self
                .settings
                .hybrid_search
                .iter()
                .copied()
                .filter(|p| match p {
                    ProviderType::Spotify => crate::spotify::current().is_some(),
                    ProviderType::Vk => crate::vk::current().is_some(),
                    ProviderType::Tidal => crate::services::sessions().tidal.is_some(),
                    _ => true,
                })
                .collect();
            if chosen.is_empty() { crate::services::provider_for(mode) } else { chosen }
        } else {
            crate::services::provider_for(mode)
        };
        let id = self.fresh_id();
        self.search = SearchState {
            id,
            query: parsed.query.clone(),
            mode: Some(mode),
            results: providers.iter().map(|p| (*p, None)).collect(),
        };
        for provider in providers {
            let backend = self.backend.clone();
            let query = parsed.query.clone();
            self.spawn(move || Event::Search { id, provider, result: crate::services::search(&backend, provider, &query, 20) });
        }
        self.open(Page::Search);
    }

    fn open_link(&mut self, url: String) {
        if crate::sharelink::is_link(&url) {
            // A playlist somebody sent: all of it is in the link, so it opens at once.
            let url: String = url.chars().filter(|c| !c.is_whitespace()).collect();
            match crate::sharelink::decode(&url) {
                Some((title, tracks)) => {
                    let playlist = Playlist {
                        id: crate::domain::new_id(),
                        title,
                        provider: ProviderType::Local,
                        owner_name: Some("Shared with you".into()),
                        artwork_url: tracks.iter().find_map(|t| t.artwork_url.clone()),
                        track_count: Some(tracks.len() as i64),
                        tracks,
                        source_url: None,
                    };
                    self.remote.insert(url.clone(), Loadable::Ready(playlist));
                    self.open(Page::Remote(url));
                }
                None => self.toast("That playlist link is damaged or from a newer Noctorium.", true),
            }
            return;
        }
        if !crate::services::recognises(&url) {
            self.toast("That is not a link Noctorium can open.", true);
            return;
        }
        if ytdlp::is_playlist_link(&url) || ytdlp::provider_of(&url).is_none() {
            self.load_remote(&url);
            self.open(Page::Remote(url));
        } else {
            self.toast("Opening the link…", false);
            let backend = self.backend.clone();
            self.spawn(move || Event::Link { result: backend.track(&url) });
        }
    }

    fn load_remote(&mut self, url: &str) {
        if matches!(self.remote.get(url), Some(Loadable::Ready(_)) | Some(Loadable::Loading)) {
            return;
        }
        self.remote.insert(url.to_string(), Loadable::Loading);
        let backend = self.backend.clone();
        let url = url.to_string();
        let youtube = self.youtube.clone().filter(|_| url.contains("youtube.com") && url.contains("list="));
        if ytdlp::provider_of(&url).is_none() && crate::services::recognises(&url) {
            // A service read through its own interface, on a thread, so the window does not wait.
            let url_for_thread = url.clone();
            self.spawn(move || Event::Remote {
                result: crate::services::open_link(&url_for_thread).unwrap_or_else(|| Err("That link could not be opened.".into())),
                url: url_for_thread,
            });
            return;
        }
        let known = self.service_library.youtube_playlists.iter().find(|p| p.source_url.as_deref() == Some(url.as_str())).cloned();
        self.spawn(move || {
            // Signed in, a YouTube playlist is read as the listener: their private ones open too.
            if let Some(session) = youtube {
                let id = url.split("list=").nth(1).unwrap_or("").split('&').next().unwrap_or("").to_string();
                if let Ok(tracks) = session.playlist_tracks(&id, 500) {
                    if !tracks.is_empty() {
                        let base = known.unwrap_or(Playlist {
                            id: id.clone(),
                            title: "Playlist".into(),
                            provider: ProviderType::YoutubeMusic,
                            owner_name: None,
                            artwork_url: None,
                            tracks: vec![],
                            source_url: Some(url.clone()),
                            track_count: None,
                        });
                        let playlist = Playlist {
                            artwork_url: base.artwork_url.clone().or_else(|| tracks.first().and_then(|t| t.artwork_url.clone())),
                            track_count: Some(tracks.len() as i64),
                            tracks,
                            ..base
                        };
                        return Event::Remote { result: Ok(playlist), url };
                    }
                }
            }
            Event::Remote { result: backend.playlist(&url, 300), url }
        });
    }

    fn open(&mut self, page: Page) {
        let metering = self.page == Page::NowPlaying;
        self.open_page(page);
        // The meter costs mpv something, so it is in the filters only while Now playing shows it.
        if metering != (self.page == Page::NowPlaying) {
            self.engine.set_levels(self.levels());
        }
    }

    fn open_page(&mut self, page: Page) {
        if self.page != page {
            self.page_opened = Instant::now();
            let previous = std::mem::replace(&mut self.page, page.clone());
            self.history.push(previous);
            if self.history.len() > 50 {
                self.history.remove(0);
            }
        }
        let reads_services = match &page {
            Page::Services => true,
            Page::SettingsAt(at) => at.reads_services(),
            _ => false,
        };
        if reads_services && !self.services.loaded {
            self.services.loaded = true;
            self.spawn(|| Event::DeezerChart(crate::deezer::chart()));
            let storefront = self.settings.apple_storefront.clone();
            self.spawn(move || Event::AppleCharts(crate::applemusic::charts(&storefront)));
            self.spawn(|| Event::BandcampDiscover { newest: false, result: crate::bandcamp::discover(false) });
            self.spawn(|| Event::BandcampDiscover { newest: true, result: crate::bandcamp::discover(true) });
            self.load_deezer_profile();
        }
        if let Page::Remote(url) = &page {
            let url = url.clone();
            self.load_remote(&url);
        }
    }

    // ---- playback ---------------------------------------------------------------------------------

    fn levels(&self) -> Levels {
        let mut levels = levels_for(&self.settings, self.queue.repeat, self.page == Page::NowPlaying && self.settings.visualiser);
        levels.volume *= self.sleep_fade;
        levels
    }

    fn apply_levels(&self) {
        self.engine.set_levels(self.levels());
        self.save_settings();
    }

    /// Counts what is heard of the song playing -- only while it plays -- and tells the connected services:
    /// now playing as it starts, a scrobble once enough of it has been heard.
    fn track_listening(&mut self) {
        let state = self.engine.state();
        let key = self.loaded.as_ref().map(Track::queue_key);
        if key != self.listening.key || self.generation != self.listening.generation {
            self.record_listen();
            self.listening = Listening {
                key: key.clone(),
                generation: self.generation,
                started_at: store::now_seconds(),
                ..Listening::default()
            };
            if let (Some(track), false) = (self.loaded.clone(), self.scrobblers.is_empty()) {
                let scrobble = crate::scrobble::ScrobbleTrack::from(&track, 0);
                for site in self.scrobblers.keys().copied().collect::<Vec<_>>() {
                    self.send_scrobble(site, scrobble.clone(), None);
                }
            }
        }
        let now = Instant::now();
        if state.status == Status::Playing && key.is_some() {
            if let Some(last) = self.listening.last_tick {
                // Bounded, so a frame held up by a sleeping computer does not count the sleep.
                let speed = self.settings.speed.clamp(0.5, 2.0) as f64;
                self.listening.heard_ms += (now.duration_since(last).as_millis().min(2_000) as f64 * speed) as i64;
            }
            self.listening.last_tick = Some(now);
        } else {
            self.listening.last_tick = None;
        }
        let duration = if state.duration_ms > 0 { state.duration_ms } else { 0 };
        if duration > 0 {
            self.listening.duration_ms = duration;
        }
        if self.listening.track.is_none() {
            self.listening.track = self.loaded.clone();
        }
        let due = crate::scrobble::threshold_ms(duration).is_some_and(|t| self.listening.heard_ms >= t);
        if due && !self.listening.scrobbled && !self.scrobblers.is_empty() {
            self.listening.scrobbled = true;
            if let Some(track) = self.loaded.clone() {
                let scrobble = crate::scrobble::ScrobbleTrack::from(&track, duration);
                let started_at = self.listening.started_at;
                for site in self.scrobblers.keys().copied().collect::<Vec<_>>() {
                    self.send_scrobble(site, scrobble.clone(), Some(started_at));
                }
            }
        }
    }

    /// Notes the listen just left for the Noctorium account, if enough of it was heard to be one.
    fn record_listen(&mut self) {
        let Some(track) = self.listening.track.clone() else { return };
        if !crate::account::listen_counts(self.listening.heard_ms, self.listening.duration_ms) {
            return;
        }
        if self.settings.keep_history {
            self.play_history.insert(0, store::HistoryEntry { track: track.clone(), played_at: self.listening.started_at });
            self.play_history.truncate(store::HISTORY_LIMIT);
            store::save_history(&self.places, &self.play_history);
        }
        if self.account.is_none() {
            return;
        }
        let artist = track.artist_line();
        self.unreported_plays.push(crate::account::PlayReport {
            client_id: crate::domain::new_id(),
            provider: track.provider.key_name().into(),
            track_id: track.id.clone(),
            title: track.title.clone(),
            artist: if artist.trim().is_empty() { "Unknown artist".into() } else { artist },
            ms_played: self.listening.heard_ms.max(0),
            played_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        });
        self.report_plays();
    }

    fn report_plays(&mut self) {
        let Some((_, token)) = self.account.clone() else { return };
        if self.unreported_plays.is_empty() {
            return;
        }
        let batch: Vec<_> = std::mem::take(&mut self.unreported_plays);
        self.save_unreported_plays();
        self.spawn(move || {
            let ok = crate::account::submit(&batch, &token);
            Event::PlaysReported { sent: batch, ok }
        });
    }

    fn save_unreported_plays(&self) {
        if let Some(path) = self.places.file("plays-queue.json") {
            if let Ok(text) = serde_json::to_string(&self.unreported_plays) {
                let _ = std::fs::write(path, text);
            }
        }
    }

    fn refresh_account_stats(&self) {
        if let Some((_, token)) = self.account.clone() {
            self.spawn(move || Event::AccountStats(crate::account::stats(&token)));
        }
    }

    fn send_scrobble(&self, site: crate::scrobble::Site, track: crate::scrobble::ScrobbleTrack, started_at: Option<i64>) {
        let Some(account) = self.scrobblers.get(&site).cloned() else { return };
        self.spawn(move || {
            let result = crate::scrobble::send(site, &account, &track, started_at);
            // Only a scrobble is worth keeping to send again; a missed "now playing" is out of date anyway.
            let failed = match (result, started_at) {
                (Err(why), Some(at)) => Some((crate::scrobble::Pending { site, track, started_at: at }, why)),
                (Err(why), None) if why.contains("connecting again") => {
                    Some((crate::scrobble::Pending { site, track, started_at: 0 }, why))
                }
                _ => None,
            };
            Event::Scrobbled { site, failed }
        });
    }

    fn save_pending_scrobbles(&self) {
        if let Some(path) = self.places.file("scrobble-queue.json") {
            if let Ok(text) = serde_json::to_string_pretty(&self.pending_scrobbles) {
                let _ = std::fs::write(path, text);
            }
        }
    }

    /// While a Last.fm or Libre.fm connection waits in the browser, asks every two seconds, for two minutes.
    fn poll_authorization(&mut self) {
        let Some(auth) = self.authorizing.as_mut() else { return };
        let Some(token) = auth.token.clone() else { return };
        if auth.asking || auth.asked_at.elapsed() < Duration::from_secs(2) {
            return;
        }
        if auth.attempts >= 60 {
            let site = auth.site;
            self.authorizing = None;
            self.toast(format!("{} was not allowed in time. Connect again to try once more.", site.name()), true);
            return;
        }
        auth.attempts += 1;
        auth.asking = true;
        auth.asked_at = Instant::now();
        let site = auth.site;
        self.ctx.request_repaint_after(Duration::from_secs(2));
        self.spawn(move || Event::AuthorizationAnswered { site, result: crate::scrobble::complete_authorization(site, &token) });
    }

    /// Keeps the Discord card in step: sent when the song, or playing and pausing, changes, and when the
    /// position has drifted from where the card's clock says it is -- a seek.
    fn update_presence(&mut self) {
        if !self.settings.discord.enabled {
            return;
        }
        let state = self.engine.state();
        let track = self.loaded.clone();
        let key = track.as_ref().map(Track::queue_key);
        let playing = state.status == Status::Playing;
        let now = store::now_seconds();
        let expected_start = now - state.position_ms / 1000;
        let changed = match &self.presence_sent {
            None => true,
            Some((sent_key, sent_playing, sent_start, _)) => {
                *sent_key != key || *sent_playing != playing || (playing && (sent_start - expected_start).abs() > 3)
            }
        };
        if !changed || (track.is_some() && state.status == Status::Loading) {
            return;
        }
        let card = track.as_ref().and_then(|t| {
            let duration = if state.duration_ms > 0 { state.duration_ms } else { t.duration_ms.unwrap_or(0) };
            crate::discord::activity(&self.settings.discord, t, state.position_ms, duration, playing, now)
        });
        self.presence.show(card, self.settings.discord.application());
        self.presence_sent = Some((key, playing, expected_start, Instant::now()));
    }

    /// The quick search bar searches once typing has paused for a moment.
    fn run_quick_search(&mut self) {
        let mode = self.settings.search_mode;
        let Some(quick) = self.quick.as_mut() else { return };
        let text = quick.text.trim().to_string();
        if text.is_empty() || text == quick.searched || quick.typed_at.elapsed() < Duration::from_millis(400) {
            return;
        }
        quick.searched = text.clone();
        quick.results = None;
        self.next_id += 1;
        let id = self.next_id;
        quick.id = id;
        let parsed = bangs::parse(&text);
        let chosen = parsed.mode().filter(|m| m.offered_here()).unwrap_or(mode);
        let provider = crate::services::provider_for(chosen).first().copied().unwrap_or(ProviderType::YoutubeMusic);
        let backend = self.backend.clone();
        self.spawn(move || Event::QuickResults { id, result: crate::services::search(&backend, provider, &parsed.query, 8) });
    }

    fn load_vk(&mut self) {
        let Some(session) = crate::vk::current() else { return };
        self.spawn(move || Event::VkLibrary {
            music: session.my_audio(1_000),
            playlists: session.playlists().unwrap_or_default(),
            recommended: session.recommendations().unwrap_or_default(),
        });
    }

    fn tidal_app(&self) -> crate::tidal::App {
        crate::tidal::App {
            client_id: self.settings.tidal_client_id.clone(),
            client_secret: crate::secrets::load(&self.places, "tidal.secret").unwrap_or_default(),
        }
    }

    /// Tidal's catalogue, through the listener's own application.
    fn start_tidal_catalogue(&mut self) {
        let app = self.tidal_app();
        if app.reads_catalogue() {
            self.spawn(move || Event::TidalReady { result: crate::tidal::catalogue_token(&app), signed_in: false });
        }
    }

    fn save_spotify_tokens(&self) {
        if let Some(account) = crate::spotify::current() {
            let tokens = account.tokens.lock().unwrap().clone();
            if let Ok(text) = serde_json::to_string(&tokens) {
                crate::secrets::save(&self.places, "spotify.tokens", &text);
            }
        }
    }

    fn load_spotify(&mut self) {
        let Some(account) = crate::spotify::current() else { return };
        let a = account.clone();
        self.spawn(move || Event::SpotifyLibrary(SpotifyPiece::Playlists(a.playlists().unwrap_or_default())));
        let a = account.clone();
        self.spawn(move || Event::SpotifyLibrary(SpotifyPiece::Likes(a.liked_songs(2_000))));
        let a = account.clone();
        self.spawn(move || Event::SpotifyLibrary(SpotifyPiece::Top(a.top_tracks().unwrap_or_default())));
        let a = account.clone();
        self.spawn(move || Event::SpotifyLibrary(SpotifyPiece::Recent(a.recently_played().unwrap_or_default())));
        if self.services.spotify_name.is_none() {
            self.spawn(move || match account.display_name() {
                Ok(name) => Event::SpotifyLibrary(SpotifyPiece::Name(name)),
                Err(why) => Event::SpotifyLibrary(SpotifyPiece::Likes(Err(why))),
            });
        }
    }

    fn load_deezer_profile(&mut self) {
        if let Some(profile) = self.settings.deezer_profile.clone() {
            self.spawn(move || Event::DeezerProfile(crate::deezer::profile(&profile)));
        }
    }

    /// Reads what the signed-in accounts hold, each part on its own thread.
    fn load_service_library(&mut self) {
        if let Some(session) = self.youtube.clone() {
            let s = session.clone();
            self.spawn(move || Event::ServiceLibrary(ServicePiece::YouTubePlaylists(s.playlists().unwrap_or_default())));
            let s = session.clone();
            self.spawn(move || Event::ServiceLibrary(ServicePiece::YouTubeLikes(s.liked_songs(500))));
        }
        if let Some((session, profile)) = self.soundcloud.clone() {
            let (s, id) = (session.clone(), profile.id);
            self.spawn(move || Event::ServiceLibrary(ServicePiece::SoundCloudPlaylists(s.playlists(id).unwrap_or_default())));
            self.spawn(move || Event::ServiceLibrary(ServicePiece::SoundCloudLikes(session.likes(id, 200))));
        }
    }

    fn start_download(&mut self, track: Track) {
        let key = track.queue_key();
        if !track.provider.downloadable() || self.downloads.file_for(&track).is_some() || self.jobs.contains_key(&key) {
            return;
        }
        self.jobs.insert(key.clone(), (format!("Downloading “{}”", track.title), 0.0));
        let (backend, places) = (self.backend.clone(), self.places.clone());
        let (events, ctx) = (self.events.clone(), self.ctx.clone());
        self.spawn(move || {
            let progress_key = key.clone();
            let progress = |p: f32| {
                let _ = events.send(Event::JobProgress { key: progress_key.clone(), progress: p });
                ctx.request_repaint();
            };
            Event::Downloaded { key, result: crate::downloads::download(&backend, &places, &track, &progress) }
        });
    }

    /// The sleep timer's word for the player bar: "23m", "40s", "End".
    pub fn sleep_label(&self) -> Option<String> {
        match self.sleep? {
            Sleep::EndOfSong => Some("End".into()),
            Sleep::At(at) => {
                let seconds = at.saturating_duration_since(Instant::now()).as_secs() + 1;
                Some(if seconds >= 60 { format!("{}m", seconds.div_ceil(60)) } else { format!("{seconds}s") })
            }
        }
    }

    /// The sleep timer and SponsorBlock, looked at every frame.
    fn watch(&mut self) {
        if let Some(Sleep::At(at)) = self.sleep {
            if Instant::now() >= at {
                self.sleep = None;
                self.engine.set_paused(true);
                self.toast("Sleep timer: paused. Good night.", false);
                if self.sleep_fade < 1.0 {
                    // Back to the volume it was, for whenever the music is played again.
                    self.sleep_fade = 1.0;
                    self.engine.set_levels(self.levels());
                }
            } else if self.settings.sleep_fade_seconds > 0 {
                // Quieter over the timer's last seconds rather than stopping dead.
                let left = at.saturating_duration_since(Instant::now()).as_secs_f32();
                let fade = (left / self.settings.sleep_fade_seconds as f32).clamp(0.0, 1.0);
                if (fade - self.sleep_fade).abs() > 0.02 || (fade == 0.0 && self.sleep_fade != 0.0) {
                    self.sleep_fade = fade;
                    self.engine.set_levels(self.levels());
                }
            }
        }
        if self.settings.sponsorblock && self.loaded.is_some() {
            let state = self.engine.state();
            if state.status == Status::Playing {
                if let Some(segment) = self.skipper.skip_from(state.position_ms) {
                    self.engine.seek(segment.end_ms);
                    self.toast(format!("Skipped {}", segment.description()), false);
                }
            }
        }
    }

    fn play_tracks(&mut self, tracks: Vec<Track>, index: usize) {
        if tracks.is_empty() {
            return;
        }
        self.queue.play_queue(tracks, index);
        self.queue.clear_suggestions();
        self.start_current(0, false);
        self.queue_changed();
    }

    /// Looks up the queue's current song and plays it from `start_ms`.
    fn start_current(&mut self, start_ms: i64, paused: bool) {
        let Some(track) = self.queue.current().cloned() else { return };
        if !track.provider.plays_here() {
            self.toast(format!("{} songs cannot be played by this version yet.", track.provider.display_name()), true);
            return;
        }
        self.resume_at = 0;
        self.play_request += 1;
        let request = self.play_request;
        // A downloaded song plays from its file: no lookup, and no network.
        if let Some(file) = self.downloads.file_for(&track) {
            self.resolving = None;
            self.generation = self.engine.play(file.to_string_lossy().into_owned(), None, start_ms, paused);
            self.started(track);
            self.fetch_lyrics_if_open();
            return;
        }
        if track.provider == ProviderType::Spotify && self.settings.spotify_playback == store::SpotifyPlayback::OnSpotify {
            if let Some(account) = crate::spotify::current() {
                self.engine.stop();
                self.loaded = None;
                self.resolving = None;
                let device = self.settings.spotify_device.clone();
                let title = track.title.clone();
                self.spawn(move || Event::ServiceDone { message: account.play(&track.id, &device).map(|_| format!("Playing “{title}” on Spotify")) });
                return;
            }
        }
        self.resolving = Some(track.clone());
        self.engine.stop();
        self.loaded = None;
        let backend = self.backend.clone();
        let matches = self.matches.clone();
        self.spawn(move || {
            // A song from a service that plays only in its own players is matched on YouTube Music first.
            let track = if track.provider.is_reference() {
                match crate::matching::find(&matches, &backend, &track) {
                    Ok(matched) => matched,
                    Err(why) => return Event::Resolved { request, track, start_ms, paused, result: Err(why) },
                }
            } else {
                track
            };
            let result = backend.resolve_audio(&track.source_url);
            Event::Resolved { request, track, start_ms, paused, result }
        });
        self.fetch_lyrics_if_open();
    }

    fn started(&mut self, track: Track) {
        self.skipper = crate::sponsorblock::Skipper::default();
        if self.settings.sponsorblock
            && matches!(track.provider, ProviderType::YoutubeMusic | ProviderType::YoutubeVideo)
        {
            let (key, id) = (track.queue_key(), track.id.clone());
            self.spawn(move || Event::Segments { key, segments: crate::sponsorblock::segments_for(&id) });
        }
        self.library.played(&track);
        store::save_recent(&self.places, &self.library);
        self.loaded = Some(track.clone());
        if let Some(url) = track.artwork_url.clone() {
            self.fetch_artwork_colour(url);
        }
        if ytdlp::has_placeholder_artist(&track) {
            let backend = self.backend.clone();
            let unenriched = track.clone();
            self.spawn(move || Event::Enriched(backend.enrich(&unenriched)));
        }
        if self.settings.youtube_history && matches!(track.provider, ProviderType::YoutubeMusic | ProviderType::YoutubeVideo) {
            if let Some(session) = self.youtube.clone() {
                let id = track.id.clone();
                self.spawn(move || {
                    if let Err(why) = session.record_play(&id) {
                        eprintln!("YouTube Music history: {why}");
                    }
                    Event::Quiet
                });
            }
        }
        if self.settings.announce_tracks && (self.window_minimised || !self.ctx.input(|i| i.focused)) {
            crate::platform::announce(&track.title, &track.artist_line());
        }
        self.prefetch_upcoming();
        self.maybe_fetch_radio();
    }

    /// Looks up what plays next while this song plays, so the step to it is immediate.
    fn prefetch_upcoming(&mut self) {
        let Some(next) = self.queue.upcoming().cloned() else { return };
        if !next.provider.playable_here() || !self.prefetched.insert(next.source_url.clone()) {
            return;
        }
        let backend = self.backend.clone();
        std::thread::spawn(move || {
            let _ = backend.resolve_audio(&next.source_url);
        });
    }

    /// Autoplay: when the queue is down to its last song, finds songs to carry on with.
    fn maybe_fetch_radio(&mut self) {
        if !self.settings.autoplay || self.queue.repeat != RepeatMode::Off {
            return;
        }
        let Some(current) = self.queue.current().cloned() else { return };
        if !self.queue.up_next().is_empty() || !current.provider.playable_here() {
            return;
        }
        let seed = current.queue_key();
        if self.queue.suggestions_seed.as_deref() == Some(seed.as_str()) {
            return;
        }
        self.queue.suggestions_seed = Some(seed.clone());
        let backend = self.backend.clone();
        let matches = self.matches.clone();
        let from_youtube = self.settings.autoplay_from == store::AutoplaySource::YoutubeMusic
            && !matches!(current.provider, ProviderType::YoutubeMusic | ProviderType::YoutubeVideo);
        self.spawn(move || {
            // YouTube Music's radio after every song: a song from elsewhere is found there first.
            let seed_track = if from_youtube {
                let matched = if current.provider.is_reference() {
                    crate::matching::find(&matches, &backend, &current).ok()
                } else {
                    backend.search(ProviderType::YoutubeMusic, &format!("{} {}", current.title, current.artist_line()), 1).ok().and_then(|mut f| f.pop())
                };
                matched.unwrap_or(current)
            } else {
                current
            };
            Event::Radio { seed, result: backend.radio(&seed_track, 20) }
        });
    }

    fn toggle_play(&mut self) {
        let state = self.engine.state();
        match state.status {
            Status::Playing | Status::Loading if self.loaded.is_some() => self.engine.set_paused(true),
            Status::Paused => self.engine.set_paused(false),
            _ if self.resolving.is_some() => {}
            _ => {
                if self.queue.current().is_some() {
                    let at = self.resume_at;
                    self.start_current(at, false);
                }
            }
        }
    }

    fn next(&mut self) {
        if self.queue.next(false).is_some() {
            self.start_current(0, false);
            self.queue_changed();
        }
    }

    fn previous(&mut self) {
        // A few seconds in, Previous goes back to the start of this song; earlier, to the one before.
        let state = self.engine.state();
        if self.loaded.is_some() && state.position_ms > 4_000 {
            self.engine.seek(0);
            return;
        }
        if self.queue.previous().is_some() {
            self.start_current(0, false);
            self.queue_changed();
        } else if self.loaded.is_some() {
            self.engine.seek(0);
        }
    }

    fn stop(&mut self) {
        self.play_request += 1;
        self.resolving = None;
        self.loaded = None;
        self.engine.stop();
    }

    fn song_ended(&mut self, generation: u64, error: Option<String>) {
        if generation != self.generation {
            return;
        }
        if let Some(why) = error {
            let Some(track) = self.loaded.clone() else { return };
            self.loaded = None;
            // An address can be refused once it is old or bound to a network the computer has left. Once
            // more, from the service, before anybody is told.
            if self.retried.insert(track.queue_key()) {
                self.backend.forget_audio(&track.source_url);
                self.start_current(0, false);
            } else if matches!(track.provider, ProviderType::YoutubeMusic | ProviderType::YoutubeVideo) {
                // What a refused YouTube stream nearly always means: YouTube changed, and yt-dlp has not.
                self.toast(
                    format!("YouTube refused “{}”. Updating yt-dlp in Settings › Playback tools usually fixes this.", track.title),
                    true,
                );
            } else {
                self.toast(format!("{}: {why}", track.title), true);
            }
            return;
        }
        self.loaded = None;
        if self.sleep == Some(Sleep::EndOfSong) {
            self.sleep = None;
            self.toast("Sleep timer: stopped at the end of the song. Good night.", false);
            return;
        }
        if self.queue.next(true).is_some() {
            self.start_current(0, false);
            self.queue_changed();
        }
    }

    fn queue_changed(&mut self) {
        self.queue_dirty = Some(Instant::now());
    }

    fn save_queue(&mut self) {
        let position = if self.loaded.is_some() { self.engine.state().position_ms } else { self.resume_at };
        store::save_queue(
            &self.places,
            &store::SavedQueue {
                tracks: self.queue.tracks.clone(),
                current_index: self.queue.current_index,
                position_ms: position,
            },
        );
        self.queue_dirty = None;
    }

    fn save_settings(&self) {
        store::save_settings(&self.places, &self.settings);
    }

    // ---- lyrics and artwork -----------------------------------------------------------------------

    fn fetch_lyrics_if_open(&mut self) {
        if !self.settings.lyrics_open {
            return;
        }
        let Some(track) = self.queue.current().cloned() else { return };
        let key = track.queue_key();
        if self.lyrics.contains_key(&key) {
            return;
        }
        self.lyrics.insert(key.clone(), Loadable::Loading);
        let preferred = self.settings.lyrics_source;
        self.spawn(move || Event::Lyrics { key, result: lyrics::fetch(&track, preferred) });
    }

    fn fetch_artwork_colour(&mut self, url: String) {
        if self.artwork_colours.contains_key(&url) {
            return;
        }
        let wanted = theme::preset(&self.settings.theme).glow.is_some_and(|g| g.follows_artwork)
            || self.settings.looks.wants_artwork_colour();
        if !wanted {
            return;
        }
        self.artwork_colours.insert(url.clone(), None);
        self.spawn(move || {
            let colour = ureq::get(&url)
                .timeout(Duration::from_secs(15))
                .call()
                .ok()
                .and_then(|r| {
                    let mut bytes = Vec::new();
                    let mut limited = std::io::Read::take(r.into_reader(), 4 << 20);
                    std::io::Read::read_to_end(&mut limited, &mut bytes).ok()?;
                    Some(bytes)
                })
                .and_then(|bytes| theme::artwork_colour(&bytes));
            Event::ArtworkColour { url, colour }
        });
    }

    /// How much of the page the rail and the player hide: less under a Glow theme, whose light should show.
    pub fn panel_opacity(&self) -> f32 {
        if self.glow_colour.is_some() { 0.55 } else { 0.86 }
    }

    /// The song as the listener has it: their own title and artist where they edited them.
    pub fn edited(&self, track: &Track) -> Track {
        match self.track_edits.get(&track.queue_key()) {
            Some(edit) => edit.apply(track),
            None => track.clone(),
        }
    }

    /// The listener's edits on songs about to be played or queued, so the queue, the lyrics and the
    /// scrobbles all use them.
    fn with_edits(&self, action: Action) -> Action {
        if self.track_edits.is_empty() {
            return action;
        }
        let all = |tracks: Vec<Track>| tracks.iter().map(|t| self.edited(t)).collect::<Vec<_>>();
        match action {
            Action::PlayList { tracks, index } => Action::PlayList { tracks: all(tracks), index },
            Action::PlayPicked { tracks, index } => Action::PlayPicked { tracks: all(tracks), index },
            Action::PlayShuffled(tracks) => Action::PlayShuffled(all(tracks)),
            Action::AddAllToQueue(tracks) => Action::AddAllToQueue(all(tracks)),
            Action::PlayNext(track) => Action::PlayNext(self.edited(&track)),
            Action::AddToQueue(track) => Action::AddToQueue(self.edited(&track)),
            Action::StartRadio(track) => Action::StartRadio(self.edited(&track)),
            other => other,
        }
    }

    /// The colour sampled from the cover playing, once it has been.
    pub fn artwork_colour(&self) -> Option<Color32> {
        let track = self.loaded.as_ref().or(self.queue.current())?;
        self.artwork_colours.get(track.artwork_url.as_ref()?).copied().flatten()
    }

    /// The colour of the light the page is lit by now: the cover's for Afterglow, the theme's otherwise.
    pub fn glow_target(&self) -> Option<(Color32, theme::Glow)> {
        let glow = theme::preset(&self.settings.theme).glow?;
        let own = theme::rgb(glow.colour);
        if glow.follows_artwork {
            let cover = self
                .queue
                .current()
                .and_then(|t| t.artwork_url.as_ref())
                .and_then(|url| self.artwork_colours.get(url).copied().flatten());
            return Some((cover.unwrap_or(own), glow));
        }
        Some((own, glow))
    }

    // ---- events -----------------------------------------------------------------------------------

    fn receive(&mut self) {
        while let Ok(event) = self.receiver.try_recv() {
            self.handle(event);
        }
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Search { id, provider, result } => {
                if id == self.search.id {
                    if let Some(slot) = self.search.results.iter_mut().find(|(p, _)| *p == provider) {
                        slot.1 = Some(result);
                    }
                }
            }
            Event::Home { id, slot, row } => {
                if id == self.home_id && slot < self.home.len() {
                    self.home[slot] = Some(row);
                }
            }
            Event::Resolved { request, track, start_ms, paused, result } => {
                if request != self.play_request {
                    return;
                }
                self.resolving = None;
                match result {
                    Ok(resolved) => {
                        self.generation = self.engine.play(resolved.url, resolved.user_agent, start_ms, paused);
                        self.started(track);
                    }
                    Err(why) => self.toast(format!("{}: {why}", track.title), true),
                }
            }
            Event::Engine(EngineEvent::Ended { generation, error }) => self.song_ended(generation, error),
            Event::Remote { url, result } => {
                let loaded = match result {
                    Ok(playlist) => Loadable::Ready(playlist),
                    Err(why) => Loadable::Failed(why),
                };
                // A followed playlist keeps what it was last read as, for its card.
                if let Loadable::Ready(playlist) = &loaded {
                    if let Some(followed) = self.library.followed.iter_mut().find(|p| p.source_url.as_deref() == Some(&url)) {
                        followed.title = playlist.title.clone();
                        followed.artwork_url = playlist.artwork_url.clone();
                        followed.track_count = playlist.track_count;
                        store::save_followed(&self.places, &self.library);
                    }
                }
                self.remote.insert(url, loaded);
            }
            Event::Link { result } => match result {
                Ok(track) => self.play_tracks(vec![track], 0),
                Err(why) => self.toast(why, true),
            },
            Event::Radio { seed, result } => {
                if self.queue.suggestions_seed.as_deref() != Some(seed.as_str()) {
                    return;
                }
                match result {
                    Ok(mut tracks) => {
                        if self.settings.autoplay_avoid_recent {
                            // What was played lately stays out, unless that would leave nothing.
                            let recent: std::collections::HashSet<String> = self.library.recent.iter().take(60).map(Track::queue_key).collect();
                            let fresh: Vec<Track> = tracks.iter().filter(|t| !recent.contains(&t.queue_key())).cloned().collect();
                            if !fresh.is_empty() {
                                tracks = fresh;
                            }
                        }
                        self.queue.set_suggestions(seed, tracks);
                        self.prefetch_upcoming();
                    }
                    Err(why) => eprintln!("Autoplay found nothing: {why}"),
                }
            }
            Event::Lyrics { key, result } => {
                let loaded = match result {
                    Ok(found) => Loadable::Ready(found),
                    Err(why) => Loadable::Failed(why),
                };
                self.lyrics.insert(key, loaded);
            }
            Event::Enriched(track) => {
                let key = track.queue_key();
                for slot in self.queue.tracks.iter_mut().chain(self.library.recent.iter_mut()) {
                    if slot.queue_key() == key {
                        *slot = track.clone();
                    }
                }
                if self.loaded.as_ref().is_some_and(|t| t.queue_key() == key) {
                    self.loaded = Some(track);
                }
                store::save_recent(&self.places, &self.library);
            }
            Event::ArtworkColour { url, colour } => {
                self.artwork_colours.insert(url, colour);
            }
            Event::ToolProgress(tool, progress) => self.tool_mut(tool).installing = Some(progress),
            Event::ToolDone { tool, result } => {
                self.tool_mut(tool).installing = None;
                match result {
                    Ok(message) => {
                        self.toast(message, false);
                        let path = tools::find(tool, &self.places);
                        match tool {
                            Tool::YtDlp => {
                                self.backend.set_path(path.clone());
                                self.yt_dlp.path = path;
                                self.load_home();
                            }
                            Tool::Mpv => {
                                self.engine.set_path(path.clone());
                                self.mpv.path = path;
                            }
                        }
                        self.read_tool_versions();
                    }
                    Err(why) => self.toast(why, true),
                }
            }
            Event::ShowWindow => {
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            Event::Update(newer) => {
                self.checking_update = false;
                match &newer {
                    // Fetched, checked and put in place by itself, where it can be.
                    Some(found) if self.settings.auto_update && found.package.is_some() && self.update_ready.as_deref() != Some(found.version.as_str()) => {
                        if !self.installing_update {
                            self.installing_update = true;
                            let package = found.package.clone().unwrap();
                            let version = found.version.clone();
                            self.spawn(move || Event::UpdateStaged(crate::update::stage(&package).map(|()| version)));
                        }
                    }
                    Some(found) if self.update_ready.as_deref() != Some(found.version.as_str()) => {
                        self.toast(format!("Noctorium {} is out. Settings › Updates installs it.", found.version), false);
                    }
                    _ => {}
                }
                self.newer = newer;
            }
            Event::UpdateStaged(result) => {
                self.installing_update = false;
                match result {
                    Ok(version) => {
                        self.toast(format!("Noctorium {version} is installed. It opens next time, or restart from Settings › Updates."), false);
                        self.update_ready = Some(version);
                    }
                    Err(why) => self.toast(format!("The update could not be installed: {why}"), true),
                }
            }
            Event::Scrobbled { site, failed } => match failed {
                None => {
                    // Sent: whatever waited for this site goes now.
                    let waiting: Vec<_> = self.pending_scrobbles.iter().filter(|p| p.site == site).cloned().collect();
                    if !waiting.is_empty() {
                        self.pending_scrobbles.retain(|p| p.site != site);
                        self.save_pending_scrobbles();
                        for pending in waiting {
                            self.send_scrobble(site, pending.track, Some(pending.started_at));
                        }
                    }
                }
                Some((pending, why)) => {
                    if why.contains("connecting again") {
                        self.scrobblers.remove(&site);
                        crate::secrets::forget(&self.places, site.secret_name());
                        self.toast(why, true);
                    } else if !self.pending_scrobbles.contains(&pending) {
                        self.pending_scrobbles.push(pending);
                        self.pending_scrobbles.truncate(500);
                        self.save_pending_scrobbles();
                    }
                }
            },
            Event::AuthorizationBegun { site, result } => {
                let Some(auth) = self.authorizing.as_mut().filter(|a| a.site == site) else { return };
                match result {
                    Ok((token, url)) => {
                        open_externally(&url);
                        auth.token = Some(token);
                        auth.url = Some(url);
                        auth.asked_at = Instant::now();
                    }
                    Err(why) => {
                        self.authorizing = None;
                        self.toast(why, true);
                    }
                }
            }
            Event::AuthorizationAnswered { site, result } => {
                let Some(auth) = self.authorizing.as_mut().filter(|a| a.site == site) else { return };
                auth.asking = false;
                if let Ok((username, key)) = result {
                    let account = crate::scrobble::Account { username: username.clone(), secret: key };
                    if let Ok(text) = serde_json::to_string(&account) {
                        crate::secrets::save(&self.places, site.secret_name(), &text);
                    }
                    self.scrobblers.insert(site, account);
                    self.authorizing = None;
                    self.toast(format!("{} connected as {username}", site.name()), false);
                }
            }
            Event::ListenBrainzChecked { token, result } => match result {
                Ok(username) => {
                    let account = crate::scrobble::Account { username: username.clone(), secret: token };
                    if let Ok(text) = serde_json::to_string(&account) {
                        crate::secrets::save(&self.places, crate::scrobble::Site::ListenBrainz.secret_name(), &text);
                    }
                    self.scrobblers.insert(crate::scrobble::Site::ListenBrainz, account);
                    self.toast(format!("ListenBrainz connected as {username}"), false);
                }
                Err(why) => self.toast(why, true),
            },
            Event::SignedIn { provider, result } => {
                if self.signing_in == Some(provider) {
                    self.signing_in = None;
                }
                match result {
                    Ok(SignedIn::YouTube(session, name)) => {
                        crate::secrets::save(&self.places, "youtube.cookies", &session.cookies);
                        crate::secrets::save(&self.places, "youtube.name", &name);
                        self.toast(format!("Signed in to YouTube Music as {name}"), false);
                        self.youtube = Some(session);
                        self.youtube_name = Some(name);
                        self.load_service_library();
                        self.load_home();
                    }
                    Ok(SignedIn::SoundCloud(session, profile)) => {
                        let fresh = self.soundcloud.is_none() && crate::secrets::load(&self.places, "soundcloud.token").as_deref() != Some(session.token.as_str());
                        crate::secrets::save(&self.places, "soundcloud.token", &session.token);
                        if fresh {
                            self.toast(format!("Signed in to SoundCloud as {}", profile.name), false);
                        }
                        self.soundcloud = Some((session, profile));
                        self.load_service_library();
                    }
                    Err(why) => self.toast(why, true),
                }
            }
            Event::ServiceLibrary(piece) => match piece {
                ServicePiece::YouTubePlaylists(p) => self.service_library.youtube_playlists = p,
                ServicePiece::YouTubeLikes(l) => self.service_library.youtube_likes = Some(l),
                ServicePiece::SoundCloudPlaylists(p) => self.service_library.soundcloud_playlists = p,
                ServicePiece::SoundCloudLikes(l) => self.service_library.soundcloud_likes = Some(l),
                ServicePiece::YouTubeHome(Ok(shelves)) if !shelves.is_empty() => {
                    // The account's own home page takes the place of the fixed rows.
                    let keep: Vec<Option<HomeRow>> = self.home.iter().take(1).cloned().collect();
                    let mine: Vec<Option<HomeRow>> = shelves
                        .into_iter()
                        .take(8)
                        .map(|(title, tracks)| Some(HomeRow { provider: ProviderType::YoutubeMusic, title, subtitle: "On your YouTube Music".into(), tracks: Ok(tracks) }))
                        .collect();
                    self.home = mine.into_iter().chain(keep.into_iter().filter(|r| r.as_ref().is_some_and(|r| r.title.starts_with("More from")))).collect();
                    self.home_id += 1;
                }
                ServicePiece::YouTubeHome(_) => {}
            },
            Event::AccountSignedIn { result, quietly } => {
                self.account_busy = false;
                match result {
                    Ok((user, token)) => {
                        // Connect's key comes with the account: every device of the listener's has the same.
                        let for_key = token.clone();
                        self.spawn(move || Event::ConnectKey(crate::account::connect_key(&for_key)));
                        crate::secrets::save(&self.places, "noctorium.token", &token);
                        if !quietly {
                            self.toast(format!("Signed in to Noctorium as {}", user.display_name), false);
                        }
                        self.account = Some((user, token));
                        self.refresh_account_stats();
                        self.report_plays();
                    }
                    Err(why) => {
                        if why.contains("signing in again") {
                            crate::secrets::forget(&self.places, "noctorium.token");
                        }
                        if !quietly || why.contains("signing in again") {
                            self.toast(why, true);
                        }
                    }
                }
            }
            Event::DeezerChart(result) => self.services.deezer_chart = Some(result),
            Event::AppleCharts(result) => self.services.apple_charts = Some(result),
            Event::VkSignedIn(result) => {
                self.services.vk_signing_in = false;
                match result {
                    Ok((cookies, name)) => {
                        let fresh = self.services.vk_name.is_none();
                        let kept = serde_json::json!({"p": cookies.p, "remixsid": cookies.remixsid}).to_string();
                        crate::secrets::save(&self.places, "vk.cookies", &kept);
                        crate::vk::set_current(Some(crate::vk::Session::new(cookies)));
                        if fresh {
                            self.toast(format!("Signed in to VK as {name}"), false);
                        }
                        self.services.vk_name = Some(name);
                        self.load_vk();
                    }
                    Err(why) => self.toast(why, true),
                }
            }
            Event::VkLibrary { music, playlists, recommended } => {
                self.services.vk_music = Some(music);
                self.services.vk_playlists = playlists;
                self.services.vk_recommended = recommended;
            }
            Event::AppleSignedIn(result) => match result {
                Ok((token, library)) => {
                    crate::secrets::save(&self.places, "applemusic.token", &token);
                    {
                        let mut sessions = crate::services::sessions();
                        sessions.apple_token = Some(token);
                        sessions.apple_storefront = library.storefront.clone();
                    }
                    if !self.services.apple_signed_in {
                        self.toast("Apple Music library read", false);
                    }
                    self.services.apple_signed_in = true;
                    self.settings.apple_storefront = library.storefront;
                    self.save_settings();
                    self.services.apple_library = Some(Ok((library.playlists, library.songs)));
                }
                Err(why) => {
                    if self.services.apple_signed_in || crate::services::sessions().apple_token.is_none() {
                        self.toast(why.clone(), true);
                    }
                    self.services.apple_library = Some(Err(why));
                }
            },
            Event::TidalReady { result, signed_in } => {
                self.services.tidal_signing_in = false;
                match result {
                    Ok(tokens) => {
                        if signed_in {
                            if let Ok(text) = serde_json::to_string(&tokens) {
                                crate::secrets::save(&self.places, "tidal.tokens", &text);
                            }
                        }
                        let session = crate::tidal::Session {
                            app: self.tidal_app(),
                            tokens: std::sync::Arc::new(std::sync::Mutex::new(tokens)),
                            country: "US".into(),
                        };
                        crate::services::sessions().tidal = Some(session.clone());
                        self.services.tidal_ready = true;
                        if signed_in {
                            self.spawn(move || {
                                Event::TidalCollection(session.me().and_then(|(id, name, country)| {
                                    let session = crate::tidal::Session { country, ..session };
                                    crate::services::sessions().tidal = Some(session.clone());
                                    session.collection(&id).map(|(tracks, playlists)| (name, tracks, playlists))
                                }))
                            });
                        }
                    }
                    Err(why) => self.toast(why, true),
                }
            }
            Event::TidalCollection(result) => match result {
                Ok((name, tracks, playlists)) => {
                    self.services.tidal_name = Some(name);
                    self.services.tidal_collection = Some(Ok((tracks, playlists)));
                }
                Err(why) => {
                    self.toast(why.clone(), true);
                    self.services.tidal_collection = Some(Err(why));
                }
            },
            Event::SpotifySignedIn(result) => {
                self.services.spotify_signing_in = false;
                match result {
                    Ok((tokens, name)) => {
                        crate::spotify::set_current(Some(crate::spotify::Account::new(tokens)));
                        self.save_spotify_tokens();
                        self.toast(format!("Signed in to Spotify as {name}"), false);
                        self.services.spotify_name = Some(name);
                        self.load_spotify();
                    }
                    Err(why) => self.toast(why, true),
                }
            }
            Event::SpotifyLibrary(piece) => {
                match piece {
                    SpotifyPiece::Name(name) => self.services.spotify_name = Some(name),
                    SpotifyPiece::Playlists(p) => self.services.spotify_playlists = p,
                    SpotifyPiece::Likes(l) => {
                        if let Err(why) = &l {
                            if why.contains("signing in again") {
                                self.toast(why.clone(), true);
                            }
                        }
                        self.services.spotify_likes = Some(l);
                    }
                    SpotifyPiece::Top(t) => self.services.spotify_top = t,
                    SpotifyPiece::Recent(t) => self.services.spotify_recent = t,
                }
                // A refresh along the way may have brought new tokens; Spotify retires the old ones.
                self.save_spotify_tokens();
            }
            Event::SpotifyImported(result) => match result {
                Ok(export) => {
                    let (made, updated) = crate::spotify::import_into(export, &mut self.library.playlists, store::now_seconds());
                    store::save_playlists(&self.places, &self.library);
                    self.toast(format!("Spotify's data imported: {made} playlists made, {updated} brought up to date."), false);
                }
                Err(why) => self.toast(why, true),
            },
            Event::BandcampDiscover { newest, result } => {
                if newest {
                    self.services.bandcamp_new = Some(result);
                } else {
                    self.services.bandcamp_top = Some(result);
                }
            }
            Event::DeezerProfile(result) => {
                if let Err(why) = &result {
                    self.toast(why.clone(), true);
                }
                self.services.deezer_profile = Some(result);
            }
            Event::AccountStats(result) => {
                if let Ok(stats) = result {
                    self.account_stats = Some(stats);
                }
            }
            Event::PlaysReported { sent, ok } => {
                if ok {
                    self.refresh_account_stats();
                } else {
                    // Kept for the next attempt, newest last, and not without end.
                    let mut kept = sent;
                    kept.append(&mut self.unreported_plays);
                    self.unreported_plays = kept;
                    let excess = self.unreported_plays.len().saturating_sub(1000);
                    self.unreported_plays.drain(..excess);
                }
                self.save_unreported_plays();
            }
            Event::YouTubeChanged { message, url, gone } => {
                let worked = message.is_ok();
                match message {
                    Ok(text) => self.toast(text, false),
                    Err(why) => self.toast(why, true),
                }
                if worked {
                    if let Some(url) = url {
                        self.remote.remove(&url);
                        let showing = self.page == Page::Remote(url.clone());
                        if gone && showing {
                            self.perform(Action::Back);
                        } else if showing {
                            self.load_remote(&url);
                        }
                    }
                    self.load_service_library();
                }
            }
            Event::Quiet => {}
            Event::Artist { video_id, result } => {
                self.artists.insert(video_id, match result {
                    Ok(artist) => Loadable::Ready(artist),
                    Err(why) => Loadable::Failed(why),
                });
            }
            Event::ConnectAdded { address, result, quiet } => match result {
                Ok(peer) if !quiet => self.toast(format!("Found {} at {address}", peer.name), false),
                Ok(_) => {}
                Err(why) if !quiet => self.toast(why, true),
                Err(_) => {}
            },
            Event::Diagnostics(checks) => self.diagnostics = Some(Loadable::Ready(checks)),
            Event::UpdateStarted(result) => {
                self.installing_update = false;
                match result {
                    Ok(()) => {
                        self.toast("The installer has started. Noctorium closes so it can be replaced.", false);
                        self.quitting = true;
                        self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(why) => self.toast(why, true),
                }
            }
            Event::SpotifyDevices(result) => self.spotify_devices = Some(result),
            Event::Related { key, result } => {
                self.related.insert(key, match result {
                    Ok(tracks) => Loadable::Ready(tracks),
                    Err(why) => Loadable::Failed(why),
                });
            }
            Event::ServiceDone { message } => match message {
                Ok(text) => self.toast(text, false),
                Err(why) => self.toast(why, true),
            },
            Event::QuickResults { id, result } => {
                if let Some(quick) = self.quick.as_mut().filter(|q| q.id == id) {
                    quick.results = Some(result);
                    quick.selected = 0;
                }
            }
            Event::JobProgress { key, progress } => {
                if let Some(job) = self.jobs.get_mut(&key) {
                    job.1 = progress;
                }
            }
            Event::Downloaded { key, result } => {
                self.jobs.remove(&key);
                match result {
                    Ok(entry) => {
                        let title = entry.track.title.clone();
                        self.downloads.add(entry);
                        self.toast(format!("“{title}” is downloaded and plays offline"), false);
                    }
                    Err(why) => self.toast(format!("The download failed: {why}"), true),
                }
            }
            Event::Exported { key, title, result } => {
                self.jobs.remove(&key);
                match result {
                    Ok(path) => self.toast(format!("“{title}” saved as {}", path.display()), false),
                    Err(why) => self.toast(format!("“{title}” could not be saved: {why}"), true),
                }
            }
            Event::ConnectKey(result) => match result {
                Ok(key) => self.start_connect(key),
                Err(why) => self.connect_problem = Some(why),
            },
            Event::ConnectAsked(asked) => {
                let reply = self.answer_connect(asked.command);
                let _ = asked.reply.send(reply);
            }
            Event::ConnectDone { message, handover } => {
                if let Some(handover) = handover {
                    let tracks: Vec<Track> = handover.tracks.iter().map(|t| t.track()).collect();
                    if !tracks.is_empty() {
                        let index = (handover.index.max(0) as usize).min(tracks.len() - 1);
                        self.queue.play_queue(tracks, index);
                        self.queue.clear_suggestions();
                        self.start_current(handover.position_ms.max(0), false);
                        self.queue_changed();
                    }
                }
                match message {
                    Ok(text) => self.toast(text, false),
                    Err(why) => self.toast(why, true),
                }
            }
            Event::Tray(command) => match command {
                crate::tray::Command::Open => {
                    self.window_minimised = false;
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                crate::tray::Command::PlayPause => self.toggle_play(),
                crate::tray::Command::Next => self.next(),
                crate::tray::Command::Previous => self.previous(),
                crate::tray::Command::Quit => {
                    self.quitting = true;
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            },
            Event::HotKey(pressed) => match pressed {
                crate::platform::Pressed::Media(crate::platform::MediaKey::PlayPause) => self.toggle_play(),
                crate::platform::Pressed::Media(crate::platform::MediaKey::Next) => self.next(),
                crate::platform::Pressed::Media(crate::platform::MediaKey::Previous) => self.previous(),
                crate::platform::Pressed::Global(index) => {
                    if let Some(action) = self.global_actions.get(index).map(|i| &crate::shortcuts::ACTIONS[*i]) {
                        let mut actions = Vec::new();
                        self.shortcut(action.does, &mut actions);
                        for action in actions {
                            self.perform(action);
                        }
                    }
                }
            },
            Event::Segments { key, segments } => {
                if self.loaded.as_ref().is_some_and(|t| t.queue_key() == key) {
                    self.skipper = crate::sponsorblock::Skipper::new(segments);
                }
            }
            Event::ToolVersions { yt_dlp, mpv } => {
                if let Some(days) = yt_dlp.as_deref().and_then(tools::yt_dlp_age_days) {
                    if days > tools::YT_DLP_STALE_DAYS {
                        self.toast(
                            format!("yt-dlp is {days} days old, and YouTube outgrows it. Settings › Playback tools › Update."),
                            true,
                        );
                    }
                }
                self.yt_dlp.version = yt_dlp;
                self.mpv.version = mpv;
            }
        }
    }

    fn tool_mut(&mut self, tool: Tool) -> &mut ToolInfo {
        match tool {
            Tool::YtDlp => &mut self.yt_dlp,
            Tool::Mpv => &mut self.mpv,
        }
    }

    // ---- actions ----------------------------------------------------------------------------------

    fn perform(&mut self, action: Action) {
        let action = self.with_edits(action);
        match action {
            Action::PlayList { tracks, index } => self.play_tracks(tracks, index),
            Action::PlayShuffled(tracks) => {
                if tracks.is_empty() {
                    return;
                }
                let start = rand::random_range(0..tracks.len());
                self.queue.play_queue(tracks, start);
                self.queue.set_shuffle(true);
                self.settings.shuffle = true;
                self.save_settings();
                self.queue.clear_suggestions();
                self.start_current(0, false);
                self.queue_changed();
            }
            Action::PlayNext(track) => {
                let empty = self.queue.tracks.is_empty();
                self.queue.play_next(track.clone());
                if empty {
                    self.queue.current_index = 0;
                    self.start_current(0, false);
                }
                self.toast(format!("“{}” plays next", track.title), false);
                self.queue_changed();
                self.prefetched.clear();
                self.prefetch_upcoming();
            }
            Action::AddToQueue(track) => {
                let empty = self.queue.tracks.is_empty();
                self.queue.add(track.clone());
                if empty {
                    self.queue.current_index = 0;
                    self.start_current(0, false);
                } else {
                    self.toast(format!("“{}” added to the queue", track.title), false);
                }
                self.queue_changed();
            }
            Action::AddAllToQueue(tracks) => {
                let count = tracks.len();
                let empty = self.queue.tracks.is_empty();
                for track in tracks {
                    self.queue.add(track);
                }
                if empty && count > 0 {
                    self.queue.current_index = 0;
                    self.start_current(0, false);
                }
                self.toast(format!("{count} songs added to the queue"), false);
                self.queue_changed();
            }
            Action::ToggleLike(track) => {
                let liked = self.library.toggle_like(&track);
                store::save_liked(&self.places, &self.library);
                self.toast(if liked { "Added to Liked songs" } else { "Removed from Liked songs" }, false);
                // And on the service the song is from, where the listener is signed in to it.
                match track.provider {
                    ProviderType::YoutubeMusic | ProviderType::YoutubeVideo => {
                        if let Some(session) = self.youtube.clone() {
                            let id = track.id.clone();
                            self.spawn(move || Event::ServiceDone {
                                message: session.set_liked(&id, liked).map(|_| {
                                    if liked { "Liked on YouTube Music too".into() } else { "Removed from your YouTube Music likes too".into() }
                                }),
                            });
                        }
                    }
                    ProviderType::Spotify => {
                        if let Some(account) = crate::spotify::current() {
                            let id = track.id.clone();
                            self.spawn(move || Event::ServiceDone {
                                message: account.set_saved(&id, liked).map(|_| {
                                    if liked { "Saved in your Spotify library too".into() } else { "Removed from your Spotify library too".into() }
                                }),
                            });
                        }
                    }
                    ProviderType::Soundcloud => {
                        if let Some((session, profile)) = self.soundcloud.clone() {
                            let id = track.id.clone();
                            self.spawn(move || Event::ServiceDone {
                                message: session.set_liked(profile.id, &id, liked).map(|_| {
                                    if liked { "Liked on SoundCloud too".into() } else { "Removed from your SoundCloud likes too".into() }
                                }),
                            });
                        }
                    }
                    _ => {}
                }
            }
            Action::StartRadio(track) => {
                self.queue.play_queue(vec![track.clone()], 0);
                self.queue.clear_suggestions();
                self.start_current(0, false);
                let seed = track.queue_key();
                self.queue.suggestions_seed = Some(seed.clone());
                let backend = self.backend.clone();
                self.spawn(move || Event::Radio { seed, result: backend.radio(&track, 25) });
                self.toast("Radio started", false);
                self.queue_changed();
            }
            Action::AddToPlaylist { id, track } => {
                if let Some(playlist) = self.library.playlist_mut(&id) {
                    playlist.tracks.push(track.clone());
                    playlist.updated_at_epoch_seconds = store::now_seconds();
                    let title = playlist.title.clone();
                    store::save_playlists(&self.places, &self.library);
                    self.toast(format!("Added to {title}"), false);
                }
            }
            Action::AddToNewPlaylist(track) => {
                let title = format!("My playlist #{}", self.library.playlists.len() + 1);
                let mut playlist = LocalPlaylist::create(&title, store::now_seconds());
                playlist.tracks.push(track);
                self.library.playlists.push(playlist);
                store::save_playlists(&self.places, &self.library);
                self.toast(format!("Added to {title}"), false);
            }
            Action::RemoveFromPlaylist { id, index } => {
                if let Some(playlist) = self.library.playlist_mut(&id) {
                    if index < playlist.tracks.len() {
                        playlist.tracks.remove(index);
                        playlist.updated_at_epoch_seconds = store::now_seconds();
                    }
                }
                store::save_playlists(&self.places, &self.library);
            }
            Action::MoveInPlaylist { id, from, to } => {
                if let Some(playlist) = self.library.playlist_mut(&id) {
                    if from < playlist.tracks.len() && to < playlist.tracks.len() {
                        let track = playlist.tracks.remove(from);
                        playlist.tracks.insert(to, track);
                    }
                }
                store::save_playlists(&self.places, &self.library);
            }
            Action::CreatePlaylist(title) => {
                let title = title.trim();
                if title.is_empty() {
                    return;
                }
                let playlist = LocalPlaylist::create(title, store::now_seconds());
                let id = playlist.id.clone();
                self.library.playlists.push(playlist);
                store::save_playlists(&self.places, &self.library);
                self.open(Page::Playlist(id));
            }
            Action::RenamePlaylist { id, title } => {
                if let Some(playlist) = self.library.playlist_mut(&id).filter(|_| !title.trim().is_empty()) {
                    playlist.title = title.trim().to_string();
                    playlist.updated_at_epoch_seconds = store::now_seconds();
                }
                store::save_playlists(&self.places, &self.library);
            }
            Action::DeletePlaylist(id) => {
                self.library.playlists.retain(|p| p.id != id);
                store::save_playlists(&self.places, &self.library);
                if self.page == Page::Playlist(id) {
                    self.page = Page::Library;
                }
                self.toast("Playlist deleted", false);
            }
            Action::SaveAsLocal(playlist) => {
                let mut local = LocalPlaylist::create(&playlist.title, store::now_seconds());
                local.tracks = playlist.tracks;
                let id = local.id.clone();
                self.library.playlists.push(local);
                store::save_playlists(&self.places, &self.library);
                self.toast("Saved to your playlists", false);
                self.open(Page::Playlist(id));
            }
            Action::Open(page) => self.open(page),
            Action::Back => {
                if let Some(previous) = self.history.pop() {
                    let metering = self.page == Page::NowPlaying;
                    self.page = previous;
                    if metering != (self.page == Page::NowPlaying) {
                        self.engine.set_levels(self.levels());
                    }
                }
            }
            Action::Search => self.run_search(),
            Action::OpenLink(url) => self.open_link(url),
            Action::Follow(playlist) => {
                let url = playlist.source_url.clone().unwrap_or_default();
                if !self.library.followed.iter().any(|p| p.source_url.as_deref() == Some(url.as_str())) {
                    self.library.followed.push(Playlist { tracks: vec![], ..playlist });
                    store::save_followed(&self.places, &self.library);
                    self.toast("Following this playlist", false);
                }
            }
            Action::Unfollow(url) => {
                self.library.followed.retain(|p| p.source_url.as_deref() != Some(url.as_str()));
                store::save_followed(&self.places, &self.library);
            }
            Action::TogglePlay => self.toggle_play(),
            Action::Next => self.next(),
            Action::Previous => self.previous(),
            Action::Seek(ms) => {
                if self.loaded.is_some() {
                    self.engine.seek(ms);
                } else {
                    self.resume_at = ms;
                }
            }
            Action::SetVolume(volume) => {
                self.settings.volume = volume.clamp(0.0, 1.0);
                self.settings.muted = false;
                self.engine.set_levels(self.levels());
                self.save_settings();
            }
            Action::ToggleMute => {
                self.settings.muted = !self.settings.muted;
                self.engine.set_levels(self.levels());
                self.save_settings();
            }
            Action::ToggleShuffle => {
                self.queue.toggle_shuffle();
                self.settings.shuffle = self.queue.shuffle;
                self.save_settings();
                self.prefetched.clear();
                self.prefetch_upcoming();
                self.queue_changed();
            }
            Action::CycleRepeat => {
                self.queue.repeat = self.queue.repeat.next();
                self.settings.repeat = self.queue.repeat;
                self.engine.set_levels(self.levels());
                self.save_settings();
                self.maybe_fetch_radio();
            }
            Action::Stop => self.stop(),
            Action::QueueJump(index) => {
                if self.queue.jump_to(index).is_some() {
                    self.start_current(0, false);
                    self.queue_changed();
                }
            }
            Action::QueueRemove(index) => {
                let was_current = index as isize == self.queue.current_index;
                self.queue.remove_at(index);
                if was_current {
                    if self.queue.current().is_some() {
                        self.start_current(0, false);
                    } else {
                        self.stop();
                    }
                }
                self.queue_changed();
            }
            Action::QueueMove { from, to } => {
                self.queue.move_track(from, to);
                self.queue_changed();
            }
            Action::QueueShuffleUpcoming => {
                self.queue.shuffle_upcoming();
                self.queue_changed();
            }
            Action::QueueClearUpcoming => {
                self.queue.clear_upcoming();
                self.queue_changed();
            }
            Action::QueueClear => {
                self.stop();
                self.queue.clear();
                self.queue_changed();
            }
            Action::PlaySuggestion(index) => {
                if self.queue.play_suggestion(index).is_some() {
                    self.start_current(0, false);
                    self.queue_changed();
                }
            }
            Action::KeepSuggestion(index) => {
                self.queue.keep_suggestion(index);
                self.queue_changed();
            }
            Action::SetTheme(key) => {
                self.settings.theme = key.to_string();
                self.save_settings();
                if let Some(url) = self.queue.current().and_then(|t| t.artwork_url.clone()) {
                    self.fetch_artwork_colour(url);
                }
            }
            Action::SetAutoplay(on) => {
                self.settings.autoplay = on;
                if !on {
                    self.queue.clear_suggestions();
                }
                self.save_settings();
                self.maybe_fetch_radio();
            }
            Action::SetRestoreQueue(on) => {
                self.settings.restore_queue = on;
                self.save_settings();
            }
            Action::SetSpeed(speed) => {
                self.settings.speed = speed.clamp(0.5, 2.0);
                self.engine.set_levels(self.levels());
                self.save_settings();
            }
            Action::SetCookies(browser) => {
                self.settings.cookies_browser = browser.clone();
                self.backend.set_cookies_browser(browser);
                self.save_settings();
            }
            Action::SetSearchMode(mode) => {
                self.settings.search_mode = mode;
                self.save_settings();
                if !self.search.query.is_empty() && self.inputs.search.trim() == self.search.query {
                    self.run_search();
                }
            }
            Action::InstallTool(tool) => {
                if self.tool_mut(tool).installing.is_some() {
                    return;
                }
                self.tool_mut(tool).installing = Some(0.0);
                let places = self.places.clone();
                let (events, ctx) = (self.events.clone(), self.ctx.clone());
                self.spawn(move || {
                    let last = std::sync::Mutex::new(Instant::now());
                    let progress = |fraction: f32| {
                        let mut last = last.lock().unwrap();
                        if last.elapsed() > Duration::from_millis(150) {
                            *last = Instant::now();
                            let _ = events.send(Event::ToolProgress(tool, fraction));
                            ctx.request_repaint();
                        }
                    };
                    Event::ToolDone { tool, result: tools::install(tool, &places, &progress) }
                });
            }
            Action::ImportNeighbour => {
                let brought = store::import_neighbour(&self.places, &mut self.library);
                store::save_playlists(&self.places, &self.library);
                store::save_liked(&self.places, &self.library);
                store::save_recent(&self.places, &self.library);
                self.toast(
                    if brought == 0 {
                        "Nothing new to bring over from Noctorium.".to_string()
                    } else {
                        format!("{brought} things brought over from Noctorium.")
                    },
                    false,
                );
            }
            Action::OpenFolder => {
                if let Some(root) = &self.places.root {
                    let _ = std::fs::create_dir_all(root);
                    open_externally(&root.to_string_lossy());
                }
            }
            Action::OpenUrl(url) => open_externally(&url),
            Action::Copy(text) => {
                self.ctx.copy_text(text);
                self.toast("Link copied", false);
            }
            Action::ToggleLyrics => {
                self.settings.lyrics_open = !self.settings.lyrics_open;
                self.save_settings();
                self.fetch_lyrics_if_open();
            }
            Action::RefreshHome => self.load_home(),
            Action::ShowShortcuts(on) => self.show_shortcuts = on,
            Action::SetSleep(minutes) => {
                self.sleep = minutes.map(|m| Sleep::At(Instant::now() + Duration::from_secs(m * 60)));
                if let Some(m) = minutes {
                    self.toast(format!("Sleep timer: pausing in {m} minutes"), false);
                }
            }
            Action::SleepAtEndOfSong => {
                self.sleep = Some(Sleep::EndOfSong);
                self.toast("Sleep timer: stopping at the end of this song", false);
            }
            Action::SetEqualizer(equalizer) => {
                self.settings.equalizer = equalizer;
                self.apply_levels();
            }
            Action::SetBoost(percent) => {
                self.settings.boost_percent = crate::audio::boost::clamp(percent);
                self.apply_levels();
            }
            Action::SetEvenLoudness(on) => {
                self.settings.even_loudness = on;
                self.apply_levels();
            }
            Action::CaptureSettingsQuery(words) => {
                self.inputs.settings_query = words;
                self.open(Page::Settings);
            }
            Action::SetSettings(settings) => {
                let old = std::mem::replace(&mut self.settings, *settings);
                self.save_settings();
                let connect_changed = old.connect_enabled != self.settings.connect_enabled
                    || old.connect_device_name.trim() != self.settings.connect_device_name.trim();
                if connect_changed {
                    match (self.settings.connect_enabled, self.connect_key.clone()) {
                        (true, Some(key)) => self.start_connect(key),
                        (false, _) => {
                            self.connect = None;
                            self.connect_problem = Some("Connect is off. Switch it on under Settings › Noctorium account.".into());
                        }
                        _ => {}
                    }
                }
                if old.sleep_fade_seconds != self.settings.sleep_fade_seconds && self.sleep_fade < 1.0 {
                    self.sleep_fade = 1.0;
                    self.engine.set_levels(self.levels());
                }
                if old.bandcamp_genres != self.settings.bandcamp_genres || old.looks.hidden_home != self.settings.looks.hidden_home {
                    self.load_home();
                }
            }
            Action::PlayPicked { tracks, index } => match self.settings.home_play {
                store::HomePlay::Shelf => self.play_tracks(tracks, index),
                store::HomePlay::Autoplay => {
                    if let Some(track) = tracks.get(index).cloned() {
                        self.perform(Action::StartRadio(track));
                    }
                }
            },
            Action::EditTrack { track, title, artist } => {
                let key = track.queue_key();
                // The service's own details, from the first edit on: what "Use the service's" goes back to.
                let original = self
                    .track_edits
                    .get(&key)
                    .and_then(|e| e.original.clone())
                    .map(|t| *t)
                    .unwrap_or_else(|| track.clone());
                let edit = store::TrackEdit { title, artist, original: Some(Box::new(original.clone())) };
                if edit.is_empty() {
                    self.track_edits.remove(&key);
                } else {
                    self.track_edits.insert(key.clone(), edit);
                }
                store::save_track_edits(&self.places, &self.track_edits);
                // The song as it now is, wherever it already sits: the queue, and what is playing.
                let now = self.edited(&original);
                for queued in self.queue.tracks.iter_mut().filter(|t| t.queue_key() == key) {
                    *queued = now.clone();
                }
                if self.loaded.as_ref().is_some_and(|l| l.queue_key() == key) {
                    self.loaded = Some(now.clone());
                    // The lyrics are looked for again under the new name.
                    self.lyrics.remove(&key);
                    self.fetch_lyrics_if_open();
                }
                self.queue_changed();
                self.toast(if self.track_edits.contains_key(&key) { "Details saved" } else { "Back to the service's details" }, false);
            }
            Action::EditDetails(track, title, artist) => self.inputs.editing = Some((track, title, artist)),
            Action::ToggleStopAfter => {
                if self.sleep == Some(Sleep::EndOfSong) {
                    self.sleep = None;
                    self.toast("Carries on after this song", false);
                } else {
                    self.sleep = Some(Sleep::EndOfSong);
                    self.toast("Stops after this song", false);
                }
            }
            Action::ClearHistory => {
                self.play_history.clear();
                store::save_history(&self.places, &self.play_history);
                self.toast("History cleared", false);
            }
            Action::AddConnectAddress(address) => {
                let address = address.trim().to_string();
                if address.is_empty() {
                    return;
                }
                if !self.settings.connect_addresses.contains(&address) {
                    self.settings.connect_addresses.push(address.clone());
                    self.save_settings();
                }
                if let Some(connect) = self.connect.clone() {
                    self.spawn(move || Event::ConnectAdded { result: connect.add_address(&address), address, quiet: false });
                }
            }
            Action::RemoveConnectAddress(address) => {
                self.settings.connect_addresses.retain(|a| *a != address);
                self.save_settings();
            }
            Action::SetFollowing { video_id, channel_id, follow } => {
                if let Some(session) = self.youtube.clone() {
                    if let Some(Loadable::Ready(artist)) = self.artists.get_mut(&video_id) {
                        artist.following = follow;
                    }
                    let name = match self.artists.get(&video_id) {
                        Some(Loadable::Ready(a)) => a.name.clone(),
                        _ => "the artist".into(),
                    };
                    self.spawn(move || Event::ServiceDone {
                        message: session
                            .set_following(&channel_id, follow)
                            .map(|_| if follow { format!("Following {name} on YouTube Music") } else { format!("No longer following {name}") }),
                    });
                }
            }
            Action::RunDiagnostics => {
                self.diagnostics = Some(Loadable::Loading);
                let checks = Diagnose {
                    yt_dlp: self.yt_dlp.path.clone(),
                    mpv: self.mpv.path.clone(),
                    folder: self.places.root.clone(),
                    connect: self.connect.as_ref().map(|c| c.port).ok_or_else(|| self.connect_problem.clone().unwrap_or_else(|| "Not signed in".into())),
                    widgets: self.widget_server.as_ref().map(|w| w.address.clone()),
                };
                self.spawn(move || Event::Diagnostics(checks.run()));
            }
            Action::InstallUpdate => {
                if let Some(newer) = self.newer.clone() {
                    self.installing_update = true;
                    match newer.package.clone() {
                        // Swapped in place: the program carries on, and the new one opens next time.
                        Some(package) => {
                            let version = newer.version.clone();
                            self.spawn(move || Event::UpdateStaged(crate::update::stage(&package).map(|()| version)));
                        }
                        None => self.spawn(move || Event::UpdateStarted(crate::update::install(&newer))),
                    }
                }
            }
            Action::CheckUpdates => {
                self.update_checked = Instant::now();
                if !self.checking_update {
                    self.checking_update = true;
                    self.spawn(|| Event::Update(crate::update::check()));
                }
            }
            Action::RestartToUpdate => match crate::update::relaunch() {
                Ok(()) => {
                    self.quitting = true;
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Err(why) => self.toast(why, true),
            },
            Action::LoadArtist(video_id) => {
                if let (Some(session), false) = (self.youtube.clone(), self.artists.contains_key(&video_id)) {
                    self.artists.insert(video_id.clone(), Loadable::Loading);
                    self.spawn(move || Event::Artist { result: session.artist_of(&video_id), video_id });
                }
            }
            Action::LoadRelated(track) => {
                let key = track.queue_key();
                if !self.related.contains_key(&key) {
                    self.related.insert(key.clone(), Loadable::Loading);
                    let backend = self.backend.clone();
                    self.spawn(move || Event::Related { key, result: backend.radio(&track, 20) });
                }
            }
            Action::LoadSpotifyDevices => {
                if let Some(account) = crate::spotify::current() {
                    self.spawn(move || Event::SpotifyDevices(account.devices()));
                }
            }
            Action::ChooseLyrics(provider) => {
                let Some(key) = self.queue.current().map(Track::queue_key) else { return };
                if let Some(Loadable::Ready(Some(shown))) = self.lyrics.get(&key) {
                    let other = shown.outcomes.iter().find(|o| o.provider == provider).and_then(|o| o.lyrics.clone());
                    if let Some(mut other) = other {
                        other.outcomes = shown.outcomes.clone();
                        self.lyrics.insert(key, Loadable::Ready(Some(other)));
                    }
                }
            }
            Action::SetLyricsSource(provider) => {
                self.settings.lyrics_source = provider;
                self.save_settings();
            }
            Action::SetLooks(looks) => {
                self.settings.looks = *looks;
                self.save_settings();
                if let Some(url) = self.queue.current().and_then(|t| t.artwork_url.clone()) {
                    self.fetch_artwork_colour(url);
                }
            }
            Action::SetVisualiser(on) => {
                self.settings.visualiser = on;
                self.apply_levels();
            }
            Action::SetSponsorBlock(on) => {
                self.settings.sponsorblock = on;
                if !on {
                    self.skipper = crate::sponsorblock::Skipper::default();
                }
                self.save_settings();
            }
            Action::Download(track) => self.start_download(track),
            Action::DownloadAll(tracks) => {
                let count = tracks.iter().filter(|t| self.downloads.file_for(t).is_none()).count();
                for track in tracks {
                    self.start_download(track);
                }
                self.toast(format!("Downloading {count} songs"), false);
            }
            Action::RemoveDownload(track) => {
                self.downloads.remove(&track);
                self.toast("Download removed", false);
            }
            Action::Export(track, format) => {
                let key = format!("export:{}", track.queue_key());
                if self.jobs.contains_key(&key) {
                    return;
                }
                let folder = self
                    .settings
                    .export_folder
                    .clone()
                    .map(std::path::PathBuf::from)
                    .or_else(crate::downloads::default_export_folder);
                let Some(folder) = folder else {
                    self.toast("There is no folder to save into. Choose one in Settings › Downloads.", true);
                    return;
                };
                self.jobs.insert(key.clone(), (format!("Saving “{}”", track.title), 0.0));
                let (backend, mpv) = (self.backend.clone(), self.mpv.path.clone());
                let (events, ctx) = (self.events.clone(), self.ctx.clone());
                self.spawn(move || {
                    let progress_key = key.clone();
                    let progress = |p: f32| {
                        let _ = events.send(Event::JobProgress { key: progress_key.clone(), progress: p });
                        ctx.request_repaint();
                    };
                    let result = crate::downloads::export(&backend, mpv.as_deref(), &track, &folder, format, &progress);
                    Event::Exported { key, title: track.title.clone(), result }
                });
            }
            Action::SetExportFolder(folder) => {
                self.settings.export_folder = folder.filter(|f| !f.trim().is_empty());
                self.save_settings();
            }
            Action::SetExportFormat(format) => {
                self.settings.export_format = format;
                self.save_settings();
            }
            Action::SetDiscord(discord) => {
                let was = self.settings.discord.enabled;
                self.settings.discord = discord;
                if was && !self.settings.discord.enabled {
                    self.presence.off();
                }
                self.presence_sent = None;
                self.save_settings();
            }
            Action::OpenQuickSearch => {
                self.quick = Some(QuickSearch {
                    text: String::new(),
                    searched: String::new(),
                    id: 0,
                    typed_at: Instant::now(),
                    results: None,
                    selected: 0,
                    focus: true,
                });
            }
            Action::CloseQuickSearch => self.quick = None,
            Action::ConnectScrobbler(site) => {
                self.authorizing = Some(Authorizing {
                    site,
                    token: None,
                    url: None,
                    asked_at: Instant::now(),
                    attempts: 0,
                    asking: false,
                });
                self.spawn(move || Event::AuthorizationBegun { site, result: crate::scrobble::begin_authorization(site) });
            }
            Action::CancelAuthorization => self.authorizing = None,
            Action::SignIn(provider) => {
                let Some(browser) = self.settings.cookies_browser.clone() else {
                    self.toast("Choose the browser you are signed in with first, just above.", true);
                    return;
                };
                let Some(yt_dlp) = self.yt_dlp.path.clone() else {
                    self.toast("yt-dlp is needed to read the browser's sign-in. Settings › Playback tools.", true);
                    return;
                };
                self.signing_in = Some(provider);
                self.spawn(move || {
                    let result = match provider {
                        ProviderType::Soundcloud => {
                            crate::accounts::export_browser_cookies(&yt_dlp, &browser, "scsearch1:noctorium session")
                                .and_then(|jar| crate::accounts::SoundCloud::from_cookies(&jar))
                                .and_then(sign_in_soundcloud)
                        }
                        _ => crate::accounts::export_browser_cookies(&yt_dlp, &browser, "https://music.youtube.com/watch?v=jNQXAC9IVRw")
                            .and_then(|jar| crate::accounts::YouTube::from_cookies(&jar))
                            .and_then(|session| session.account_name().map(|name| SignedIn::YouTube(session, name))),
                    };
                    Event::SignedIn { provider, result }
                });
            }
            Action::AccountLogIn { email, password } => {
                self.account_busy = true;
                self.spawn(move || Event::AccountSignedIn { result: crate::account::log_in(&email, &password), quietly: false });
            }
            Action::AccountSignUp { email, password, name } => {
                self.account_busy = true;
                self.spawn(move || Event::AccountSignedIn {
                    result: crate::account::sign_up(&email, &password, &name),
                    quietly: false,
                });
            }
            Action::AppleSignIn => {
                let Some(browser) = self.settings.cookies_browser.clone() else {
                    self.toast("Choose the browser you are signed in with first: Settings › Accounts.", true);
                    return;
                };
                let Some(yt_dlp) = self.yt_dlp.path.clone() else { return };
                self.services.apple_signed_in = true;
                self.spawn(move || {
                    Event::AppleSignedIn(
                        crate::accounts::export_browser_cookies(&yt_dlp, &browser, "https://music.youtube.com/watch?v=jNQXAC9IVRw")
                            .and_then(|jar| crate::applemusic::user_token_from(&jar))
                            .and_then(|token| crate::applemusic::library(&token).map(|library| (token, library))),
                    )
                });
            }
            Action::VkSignIn => {
                let Some(browser) = self.settings.cookies_browser.clone() else {
                    self.toast("Choose the browser you are signed in with first: Settings › Accounts.", true);
                    return;
                };
                let Some(yt_dlp) = self.yt_dlp.path.clone() else { return };
                self.services.vk_signing_in = true;
                self.spawn(move || {
                    Event::VkSignedIn(
                        crate::accounts::export_browser_cookies(&yt_dlp, &browser, "https://music.youtube.com/watch?v=jNQXAC9IVRw")
                            .and_then(|jar| crate::vk::Cookies::from_jar(&jar))
                            .and_then(|cookies| crate::vk::Session::new(cookies.clone()).name().map(|name| (cookies, name))),
                    )
                });
            }
            Action::VkSignOut => {
                crate::vk::set_current(None);
                crate::secrets::forget(&self.places, "vk.cookies");
                self.services.vk_name = None;
                self.services.vk_music = None;
                self.services.vk_playlists.clear();
                self.services.vk_recommended.clear();
                self.toast("Signed out of VK", false);
            }
            Action::AppleSignOut => {
                crate::secrets::forget(&self.places, "applemusic.token");
                crate::services::sessions().apple_token = None;
                self.services.apple_signed_in = false;
                self.services.apple_library = None;
                self.toast("Signed out of Apple Music", false);
            }
            Action::SetTidalApp { client_id, client_secret } => {
                self.settings.tidal_client_id = client_id.trim().to_string();
                self.save_settings();
                crate::secrets::save(&self.places, "tidal.secret", client_secret.trim());
                self.start_tidal_catalogue();
            }
            Action::TidalSignIn => {
                let app = self.tidal_app();
                if app.client_id.is_empty() {
                    self.toast("Enter your Tidal app's client id first.", true);
                    return;
                }
                self.services.tidal_signing_in = true;
                self.spawn(move || Event::TidalReady { result: crate::tidal::sign_in(&app, open_externally), signed_in: true });
            }
            Action::TidalSignOut => {
                crate::secrets::forget(&self.places, "tidal.tokens");
                self.services.tidal_name = None;
                self.services.tidal_collection = None;
                self.start_tidal_catalogue();
                self.toast("Signed out of Tidal", false);
            }
            Action::SetShortcut { id, global, chord } => {
                let map = if global { &mut self.settings.shortcuts.global } else { &mut self.settings.shortcuts.in_app };
                match chord {
                    Some(chord) => map.insert(id.to_string(), chord),
                    None => map.insert(id.to_string(), String::new()),
                };
                self.capturing = None;
                self.save_settings();
                if global {
                    self.start_hot_keys();
                }
            }
            Action::ResetShortcut(id) => {
                self.settings.shortcuts.in_app.remove(id);
                self.save_settings();
            }
            Action::SetGlobalShortcuts(on) => {
                self.settings.shortcuts.global_enabled = on;
                if on {
                    // The Kotlin desktop's suggestions, where nothing is chosen yet.
                    for action in crate::shortcuts::ACTIONS.iter().filter(|a| a.global) {
                        if let Some(suggestion) = action.global_suggestion {
                            self.settings.shortcuts.global.entry(action.id.to_string()).or_insert_with(|| suggestion.to_string());
                        }
                    }
                }
                self.save_settings();
                self.start_hot_keys();
            }
            Action::Capture(what) => self.capturing = what,
            Action::FocusSearch => {
                self.inputs.focus_search = true;
                self.open(Page::Search);
            }
            Action::StopAtStart => {
                if self.loaded.is_some() {
                    self.engine.set_paused(true);
                    self.engine.seek(0);
                }
            }
            Action::QueueClearAllButCurrent => {
                self.queue.clear_all_but_current();
                self.queue_changed();
            }
            Action::SetBoostBy(step) => {
                let wanted = (self.settings.boost_percent as i32 + step).max(0) as u32;
                self.perform(Action::SetBoost(wanted));
                self.toast(
                    if crate::audio::boost::is_on(self.settings.boost_percent) {
                        format!("Volume booster {}", crate::audio::boost::times_label(self.settings.boost_percent))
                    } else {
                        "Volume booster off".to_string()
                    },
                    false,
                );
            }
            Action::ConnectSend(peer) => {
                let Some(connect) = self.connect.clone() else { return };
                if self.queue.tracks.is_empty() {
                    self.toast("There is nothing playing here to send.", true);
                    return;
                }
                let mut command = crate::connect::Command::new(crate::connect::CommandType::TakeOver);
                command.tracks = self.queue.tracks.iter().map(crate::connect::WireTrack::of).collect();
                command.index = self.queue.current_index.max(0) as i32;
                command.position_ms = Some(if self.loaded.is_some() { self.engine.state().position_ms } else { self.resume_at });
                // The music goes; here it stops.
                self.stop();
                self.driving = Some((peer.id.clone(), peer.name.clone()));
                self.spawn(move || Event::ConnectDone {
                    message: connect.command(&peer, command).and_then(|r| if r.ok { Ok(format!("Playing on {} now", peer.name)) } else { Err(r.message.unwrap_or_else(|| format!("{} would not take it.", peer.name))) }),
                    handover: None,
                });
            }
            Action::ConnectBringBack(peer) => {
                let Some(connect) = self.connect.clone() else { return };
                self.driving = None;
                self.spawn(move || match connect.command(&peer, crate::connect::Command::new(crate::connect::CommandType::HandBack)) {
                    Ok(reply) if reply.ok => Event::ConnectDone { message: Ok(format!("Brought back from {}", peer.name)), handover: reply.handover },
                    Ok(reply) => Event::ConnectDone { message: Err(reply.message.unwrap_or_else(|| format!("{} would not hand it back.", peer.name))), handover: None },
                    Err(why) => Event::ConnectDone { message: Err(why), handover: None },
                });
            }
            Action::ConnectCommand(peer, kind) => {
                let Some(connect) = self.connect.clone() else { return };
                self.spawn(move || Event::ConnectDone {
                    message: connect
                        .command(&peer, crate::connect::Command::new(kind))
                        .and_then(|r| if r.ok { Ok(String::new()) } else { Err(r.message.unwrap_or_default()) })
                        .map(|_| String::new()),
                    handover: None,
                });
            }
            Action::SetWidgets(preferences) => {
                let before = self.settings.widgets.clone();
                self.settings.widgets = preferences.clone();
                self.save_settings();
                let restart = before.enabled != preferences.enabled || before.port != preferences.port || before.allow_network != preferences.allow_network;
                if restart {
                    self.widget_server = None;
                    if preferences.enabled {
                        self.start_widgets();
                    }
                } else if let Some(server) = &self.widget_server {
                    server.set_preferences(preferences);
                }
            }
            Action::SetCloseToTray(on) => {
                self.settings.close_to_tray = on;
                self.save_settings();
            }
            Action::SetStartup(mode) => {
                if crate::startup::set(mode) {
                    self.startup_mode = mode;
                } else {
                    self.toast("Noctorium could not change how it opens at sign-in.", true);
                    self.startup_mode = crate::startup::mode();
                }
            }
            Action::ToggleWindow => {
                self.window_minimised = !self.window_minimised;
                self.ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(self.window_minimised));
                if !self.window_minimised {
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
            Action::SpotifySignIn => {
                self.services.spotify_signing_in = true;
                self.spawn(|| {
                    let result = crate::spotify::sign_in(open_externally).and_then(|tokens| {
                        let account = crate::spotify::Account::new(tokens);
                        let name = account.display_name()?;
                        let tokens = account.tokens.lock().unwrap().clone();
                        Ok((tokens, name))
                    });
                    Event::SpotifySignedIn(result)
                });
            }
            Action::SpotifySignOut => {
                crate::spotify::set_current(None);
                crate::secrets::forget(&self.places, "spotify.tokens");
                let keep_bandcamp = (self.services.bandcamp_top.take(), self.services.bandcamp_new.take());
                self.services.spotify_name = None;
                self.services.spotify_playlists.clear();
                self.services.spotify_likes = None;
                self.services.spotify_top.clear();
                self.services.spotify_recent.clear();
                (self.services.bandcamp_top, self.services.bandcamp_new) = keep_bandcamp;
                self.toast("Signed out of Spotify", false);
            }
            Action::ImportSpotifyData(path) => {
                let path = std::path::PathBuf::from(path.trim().trim_matches('"'));
                self.toast("Reading Spotify's data…", false);
                self.spawn(move || Event::SpotifyImported(crate::spotify::read_export(&path)));
            }
            Action::SetDeezerProfile(profile) => {
                self.settings.deezer_profile = profile.filter(|p| !p.trim().is_empty());
                self.save_settings();
                self.services.deezer_profile = None;
                self.load_deezer_profile();
            }
            Action::AccountSignOut => {
                self.connect = None;
                self.account = None;
                self.account_stats = None;
                crate::secrets::forget(&self.places, "noctorium.token");
                self.toast("Signed out of Noctorium", false);
            }
            Action::SignOut(provider) => {
                match provider {
                    ProviderType::Soundcloud => {
                        self.soundcloud = None;
                        crate::secrets::forget(&self.places, "soundcloud.token");
                        self.service_library.soundcloud_playlists.clear();
                        self.service_library.soundcloud_likes = None;
                    }
                    _ => {
                        self.youtube = None;
                        self.youtube_name = None;
                        crate::secrets::forget(&self.places, "youtube.cookies");
                        crate::secrets::forget(&self.places, "youtube.name");
                        self.service_library.youtube_playlists.clear();
                        self.service_library.youtube_likes = None;
                        self.load_home();
                    }
                }
                self.toast(format!("Signed out of {}", provider.display_name()), false);
            }
            Action::AddToServicePlaylist { playlist_id, title, track } => {
                if let Some(session) = self.youtube.clone() {
                    self.spawn(move || Event::ServiceDone {
                        message: session.add_to_playlist(&playlist_id, &track.id).map(|_| format!("Added to {title} on YouTube Music")),
                    });
                }
            }
            Action::YouTubeCreate { title, tracks } => {
                let Some(session) = self.youtube.clone() else {
                    self.toast("Sign in to YouTube Music in Settings › Accounts first.", true);
                    return;
                };
                let ids: Vec<String> = tracks
                    .iter()
                    .filter(|t| matches!(t.provider, ProviderType::YoutubeMusic | ProviderType::YoutubeVideo))
                    .map(|t| t.id.clone())
                    .collect();
                let left_out = tracks.len() - ids.len();
                self.spawn(move || Event::YouTubeChanged {
                    message: session.create_playlist(&title, &ids, crate::accounts::Privacy::Private).map(|_| {
                        if left_out == 0 {
                            format!("Made \"{title}\" on YouTube Music")
                        } else {
                            format!("Made \"{title}\" on YouTube Music, without the {left_out} songs from other services")
                        }
                    }),
                    url: None,
                    gone: false,
                });
            }
            Action::YouTubeEdit { id, url, edit } => {
                if let Some(session) = self.youtube.clone() {
                    self.spawn(move || Event::YouTubeChanged {
                        message: session.edit_playlist(&id, &edit).map(|_| "Saved on YouTube Music".to_string()),
                        url: Some(url),
                        gone: false,
                    });
                }
            }
            Action::YouTubeRemove { id, url, track } => {
                if let Some(session) = self.youtube.clone() {
                    self.spawn(move || Event::YouTubeChanged {
                        message: session.remove_from_playlist(&id, &track.id).map(|_| format!("Removed {} from the playlist", track.title)),
                        url: Some(url),
                        gone: false,
                    });
                }
            }
            Action::YouTubeDelete { id, url } => {
                if let Some(session) = self.youtube.clone() {
                    self.library.followed.retain(|p| p.source_url.as_deref() != Some(url.as_str()));
                    store::save_followed(&self.places, &self.library);
                    self.spawn(move || Event::YouTubeChanged {
                        message: session.delete_playlist(&id).map(|_| "Playlist deleted from YouTube Music".to_string()),
                        url: Some(url),
                        gone: true,
                    });
                }
            }
            Action::ConnectListenBrainz(token) => {
                let token = token.trim().to_string();
                if !token.is_empty() {
                    self.spawn(move || Event::ListenBrainzChecked {
                        result: crate::scrobble::validate_listenbrainz(&token),
                        token,
                    });
                }
            }
            Action::DisconnectScrobbler(site) => {
                self.scrobblers.remove(&site);
                crate::secrets::forget(&self.places, site.secret_name());
                self.pending_scrobbles.retain(|p| p.site != site);
                self.save_pending_scrobbles();
                self.toast(format!("{} disconnected", site.name()), false);
            }
            Action::QuickSearchType(text) => {
                self.perform(Action::OpenQuickSearch);
                if let Some(quick) = self.quick.as_mut() {
                    quick.text = text;
                    quick.typed_at = Instant::now() - Duration::from_secs(1);
                }
            }
            Action::QuickSearchAll(text) => {
                self.quick = None;
                self.inputs.search = text;
                self.run_search();
            }
            Action::OpenDownloadsFolder => {
                if let Some(folder) = self.downloads.folder() {
                    let _ = std::fs::create_dir_all(folder);
                    open_externally(&folder.to_string_lossy());
                }
            }
            Action::CaptureSearch(words) => {
                self.inputs.search = words;
                self.run_search();
            }
        }
    }

    // ---- keyboard ---------------------------------------------------------------------------------

    fn start_connect(&mut self, key: String) {
        self.connect_key = Some(key.clone());
        if !self.settings.connect_enabled {
            self.connect = None;
            self.connect_problem = Some("Connect is off. Switch it on under Settings › Noctorium account.".into());
            return;
        }
        if self.settings.connect_device_id.is_empty() {
            self.settings.connect_device_id = crate::domain::new_id();
            self.save_settings();
        }
        let chosen = self.settings.connect_device_name.trim().to_string();
        let name = Some(chosen)
            .filter(|n| !n.is_empty())
            .ok_or(())
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .or_else(|_| std::env::var("HOSTNAME"))
            .map(|n| n.trim().to_string())
            .ok()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "This computer".into());
        let identity = crate::connect::Identity { id: self.settings.connect_device_id.clone(), name };
        let (events, ctx) = (self.events.clone(), self.ctx.clone());
        self.connect = None;
        match crate::connect::Connect::start(identity, key, move |asked| {
            let _ = events.send(Event::ConnectAsked(asked));
            ctx.request_repaint();
        }) {
            Ok(connect) => {
                self.connect = Some(std::sync::Arc::new(connect));
                self.connect_problem = None;
            }
            Err(why) => self.connect_problem = Some(why),
        }
    }

    /// What this device says of itself to the others, twice a second.
    fn feed_connect(&mut self) {
        // The devices added by their address are asked again every few seconds, as the others announce themselves.
        if self.connect.is_some() && !self.settings.connect_addresses.is_empty() && self.last_connect_probe.elapsed() > Duration::from_secs(8) {
            self.last_connect_probe = Instant::now();
            for address in self.settings.connect_addresses.clone() {
                let connect = self.connect.clone().unwrap();
                self.spawn(move || Event::ConnectAdded { result: connect.add_address(&address), address, quiet: true });
            }
        }
        let Some(connect) = &self.connect else { return };
        let state = self.engine.state();
        let track = self.loaded.clone().or_else(|| self.queue.current().cloned());
        let playing = state.status == Status::Playing;
        let wire = track.as_ref().map(crate::connect::WireTrack::of);
        connect.show(crate::connect::Here {
            now_playing: wire.as_ref().map(|t| crate::connect::NowPlaying {
                title: t.title.clone(),
                artist: t.artists.iter().map(|a| a.name.clone()).collect::<Vec<_>>().join(", "),
                provider: Some(t.provider.key_name().into()),
                artwork_url: t.artwork_url.clone(),
                playing,
                position_ms: state.position_ms,
                duration_ms: state.duration_ms,
            }),
            state: Some(crate::connect::RemoteState {
                device_id: connect.identity.id.clone(),
                device_name: connect.identity.name.clone(),
                kind: crate::connect::DeviceKind::Desktop,
                playing,
                track: wire,
                position_ms: state.position_ms,
                duration_ms: state.duration_ms,
                volume: self.settings.volume,
                queue_size: self.queue.tracks.len() as i32,
                queue_index: self.queue.current_index as i32,
                shuffle: self.queue.shuffle,
                repeat: match self.queue.repeat {
                    RepeatMode::Off => "OFF".into(),
                    RepeatMode::All => "ALL".into(),
                    RepeatMode::One => "ONE".into(),
                },
                controlled_by: self.driven_by.clone(),
                controlled_by_id: None,
                driving: self.driving.as_ref().map(|(id, name)| crate::connect::DeviceRef { id: id.clone(), name: name.clone() }),
                terminal: false,
                queue_key: Some(crate::connect::queue_key(&self.queue.tracks)),
                tells_whom_it_drives: true,
            }),
            queue: Some(crate::connect::RemoteQueue {
                tracks: self.queue.tracks.iter().map(crate::connect::WireTrack::of).collect(),
                index: self.queue.current_index as i32,
                key: crate::connect::queue_key(&self.queue.tracks),
            }),
        });
    }

    /// Carries out what another device asked of this one, and says how it went.
    fn answer_connect(&mut self, command: crate::connect::Command) -> crate::connect::CommandReply {
        use crate::connect::CommandType as C;
        let from = command.from_device_name.clone();
        let mut handover = None;
        match command.kind {
            C::TakeOver => {
                let tracks: Vec<Track> = command.tracks.iter().map(|t| t.track()).collect();
                if tracks.is_empty() {
                    return crate::connect::CommandReply { ok: false, message: Some("There was nothing to play.".into()), state: None, handover: None };
                }
                let index = (command.index.max(0) as usize).min(tracks.len() - 1);
                self.driving = None;
                self.queue.play_queue(tracks, index);
                self.queue.clear_suggestions();
                self.start_current(command.position_ms.unwrap_or(0).max(0), false);
                self.queue_changed();
                if let Some(name) = &from {
                    self.toast(format!("{name} sent its music here"), false);
                }
            }
            C::HandBack => {
                handover = Some(crate::connect::Handover {
                    tracks: self.queue.tracks.iter().map(crate::connect::WireTrack::of).collect(),
                    index: self.queue.current_index.max(0) as i32,
                    position_ms: if self.loaded.is_some() { self.engine.state().position_ms } else { self.resume_at },
                });
                self.stop();
            }
            C::Play => {
                if self.engine.state().status != Status::Playing {
                    self.toggle_play();
                }
            }
            C::Pause => {
                if self.engine.state().status == Status::Playing {
                    self.engine.set_paused(true);
                }
            }
            C::Next => self.next(),
            C::Previous => self.previous(),
            C::Seek => self.engine.seek(command.position_ms.unwrap_or(0)),
            C::Volume => self.perform(Action::SetVolume(command.volume.unwrap_or(self.settings.volume))),
            C::Shuffle => {
                self.queue.set_shuffle(command.enabled.unwrap_or(false));
                self.queue_changed();
            }
            C::Repeat => {
                self.queue.repeat = match command.repeat.as_deref() {
                    Some("ALL") => RepeatMode::All,
                    Some("ONE") => RepeatMode::One,
                    _ => RepeatMode::Off,
                };
                self.engine.set_levels(self.levels());
            }
            C::Jump => {
                if command.index >= 0 {
                    self.perform(Action::QueueJump(command.index as usize));
                }
            }
            C::Add => {
                for wire in command.tracks.iter().rev() {
                    if command.enabled == Some(true) {
                        self.queue.play_next(wire.track());
                    } else {
                        self.queue.add(wire.track());
                    }
                }
                self.queue_changed();
            }
            C::Remove => {
                if command.index >= 0 {
                    self.perform(Action::QueueRemove(command.index as usize));
                }
            }
            C::Move => {
                if let (Ok(from), Some(Ok(to))) = (usize::try_from(command.index), command.to_index.map(usize::try_from)) {
                    self.perform(Action::QueueMove { from, to });
                }
            }
        }
        if !matches!(command.kind, C::HandBack | C::TakeOver) {
            self.driven_by = from;
        }
        self.feed_connect();
        crate::connect::CommandReply { ok: true, message: None, state: None, handover }
    }

    fn start_widgets(&mut self) {
        match crate::widgets::Server::start(self.settings.widgets.clone()) {
            Ok(server) => self.widget_server = Some(server),
            Err(why) => {
                self.toast(why, true);
                self.settings.widgets.enabled = false;
            }
        }
    }

    /// What the stream widgets show, twice a second.
    fn feed_widgets(&mut self) {
        let Some(server) = &self.widget_server else { return };
        if self.widgets_fed_at.is_some_and(|at| at.elapsed() < Duration::from_millis(500)) {
            return;
        }
        self.widgets_fed_at = Some(Instant::now());
        let state = self.engine.state();
        let (track, kind) = match (&self.loaded, &self.resolving) {
            (Some(t), _) => (Some(t.clone()), if state.status == Status::Paused { "paused" } else { "playing" }),
            (None, Some(t)) => (Some(t.clone()), "loading"),
            _ => (None, "stopped"),
        };
        let accent = self.glow_colour.unwrap_or(self.palette.accent);
        server.show(crate::widgets::Playing {
            track,
            state: kind,
            position_ms: state.position_ms,
            duration_ms: state.duration_ms,
            accent: format!("#{:02X}{:02X}{:02X}", accent.r(), accent.g(), accent.b()),
            accent_from_artwork: self.glow_colour.is_some(),
        });
    }

    fn keep_tray_in_step(&mut self) {
        let Some(tray) = &self.tray else { return };
        let song = self.loaded.as_ref().map(|t| format!("{} · {}", t.title, t.artist_line()));
        let playing = self.engine.state().status == Status::Playing;
        if self.tray_shown != (song.clone(), playing) {
            tray.show(song.as_deref(), playing);
            self.tray_shown = (song, playing);
        }
    }

    /// (Re)registers the media keys and the listener's global chords with the system.
    fn start_hot_keys(&mut self) {
        self.hot_keys = None;
        let mut chords = Vec::new();
        self.global_actions.clear();
        for (index, action) in crate::shortcuts::ACTIONS.iter().enumerate() {
            if let Some(hot) = self.settings.shortcuts.global_chord(action).and_then(|c| crate::shortcuts::windows_hot_key(&c)) {
                chords.push(hot);
                self.global_actions.push(index);
            }
        }
        let (events, ctx) = (self.events.clone(), self.ctx.clone());
        let keys = crate::platform::listen(chords, move |pressed| {
            let _ = events.send(Event::HotKey(pressed));
            ctx.request_repaint();
        });
        self.media_keys = keys.media.clone();
        self.global_granted = keys.global.clone();
        self.hot_keys = Some(keys);
    }

    /// What one shortcut does.
    fn shortcut(&self, does: crate::shortcuts::Does, actions: &mut Vec<Action>) {
        use crate::shortcuts::Does;
        let state = self.engine.state();
        actions.push(match does {
            Does::PlayPause => Action::TogglePlay,
            Does::Stop => Action::StopAtStart,
            Does::StopAfterCurrent => Action::SleepAtEndOfSong,
            Does::Next => Action::Next,
            Does::Previous => Action::Previous,
            Does::Seek(by) => {
                if self.loaded.is_none() {
                    return;
                }
                Action::Seek((state.position_ms + by).max(0))
            }
            Does::Shuffle => Action::ToggleShuffle,
            Does::Repeat => Action::CycleRepeat,
            Does::Like => match self.loaded.clone().or_else(|| self.queue.current().cloned()) {
                Some(track) => Action::ToggleLike(track),
                None => return,
            },
            Does::Volume(by) => Action::SetVolume(self.settings.volume + by as f32 / 100.0),
            Does::Mute => Action::ToggleMute,
            Does::Boost(by) => Action::SetBoostBy(by),
            Does::ClearAllButCurrent => Action::QueueClearAllButCurrent,
            Does::QuickSearch => {
                if self.quick.is_some() {
                    Action::CloseQuickSearch
                } else {
                    Action::OpenQuickSearch
                }
            }
            Does::Search => Action::FocusSearch,
            Does::GoHome => Action::Open(Page::Home),
            Does::GoSearch => Action::Open(Page::Search),
            Does::GoLibrary => Action::Open(Page::Library),
            Does::GoDownloads => Action::Open(Page::Downloads),
            Does::GoNowPlaying => Action::Open(Page::NowPlaying),
            Does::GoQueue => Action::Open(Page::Queue),
            Does::GoSettings => Action::Open(Page::Settings),
            Does::GoServices => Action::Open(Page::Services),
            Does::Lyrics => Action::ToggleLyrics,
            Does::PasteLink => return,
            Does::Help => Action::ShowShortcuts(!self.show_shortcuts),
            Does::ToggleWindow => Action::ToggleWindow,
        });
    }

    fn shortcuts(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let typing = ctx.wants_keyboard_input();
        let presses: Vec<(Key, Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, repeat, modifiers, .. } if !*repeat || matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown) => {
                        Some((*key, *modifiers))
                    }
                    _ => None,
                })
                .collect()
        });
        // Changing a shortcut: the next chord pressed is the new one. Escape leaves it as it was; Backspace
        // leaves it with none.
        if let Some((id, global)) = self.capturing {
            for (key, modifiers) in presses {
                match key {
                    Key::Escape => actions.push(Action::Capture(None)),
                    Key::Backspace | Key::Delete if !modifiers.any() => {
                        actions.push(Action::SetShortcut { id, global, chord: None })
                    }
                    _ => {
                        if let Some(chord) = crate::shortcuts::chord_of(key, modifiers) {
                            // A global chord without a modifier would take the key from every program.
                            if global && !chord.has_modifier() {
                                self.toast("A global shortcut needs Ctrl, Alt, Shift or Win with it.", true);
                                continue;
                            }
                            actions.push(Action::SetShortcut { id, global, chord: Some(chord.to_string()) });
                        }
                    }
                }
                break;
            }
            return;
        }
        // A link pasted anywhere but into a text box is played.
        if !typing {
            let pasted: Option<String> = ctx.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Paste(text) => Some(text.trim().to_string()),
                    _ => None,
                })
            });
            if let Some(text) = pasted.filter(|t| t.starts_with("http://") || t.starts_with("https://") || crate::sharelink::is_link(t)) {
                actions.push(Action::OpenLink(text));
            }
        }
        for (key, modifiers) in presses {
            let Some(chord) = crate::shortcuts::chord_of(key, modifiers) else { continue };
            // In a text box, keys without a modifier are the box's, and so are its own: copy, paste, undo.
            if typing && (!chord.has_modifier() || ["A", "C", "V", "X", "Z", "Y"].contains(&chord.key.as_str()) || matches!(key, Key::ArrowLeft | Key::ArrowRight)) {
                continue;
            }
            if !typing && ((key == Key::Backspace && !modifiers.any()) || (key == Key::ArrowLeft && modifiers.alt)) {
                actions.push(Action::Back);
                continue;
            }
            let found = crate::shortcuts::ACTIONS
                .iter()
                .find(|action| self.settings.shortcuts.in_app_chords(action).contains(&chord));
            if let Some(action) = found {
                self.shortcut(action.does, actions);
            }
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.loaded.as_ref().or(self.resolving.as_ref()) {
            Some(track) => format!("{} · {} — Noctorium", track.title, track.artist_line()),
            None => TITLE.to_string(),
        };
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive();
        // Every six hours, for a Noctorium left playing in the tray.
        if self.settings.check_updates && !self.checking_update && self.update_ready.is_none() && self.update_checked.elapsed() > std::time::Duration::from_secs(6 * 3600) {
            self.update_checked = Instant::now();
            self.checking_update = true;
            self.spawn(|| Event::Update(crate::update::check()));
        }
        // The palette is worked out every frame -- the accent may follow the cover -- and egui dressed in it
        // only when it changed.
        let artwork = self.artwork_colour();
        let palette = theme::palette_for(&self.settings.theme, &self.settings.looks, artwork);
        let look = (palette, self.settings.looks.corners, theme::skin_of(&self.settings.theme));
        if self.applied_look != Some(look) {
            self.palette = palette;
            theme::apply(ctx, &palette, look.1, look.2);
            self.applied_look = Some(look);
        }
        widgets::BADGE_MARKS.store(
            self.settings.looks.badges == crate::looks::ServiceBadgeStyle::Marks,
            std::sync::atomic::Ordering::Relaxed,
        );
        icons::set_icon_set(self.settings.looks.icons);
        widgets::set_browsing(&self.settings.looks);
        let text_size = self.settings.looks.text_size.scale();
        if self.applied_text_size != Some(text_size) {
            ctx.set_zoom_factor(text_size);
            self.applied_text_size = Some(text_size);
        }
        let animations = self.settings.looks.animations;
        if self.applied_animations != Some(animations) {
            ctx.all_styles_mut(|style| style.animation_time = if animations { 1.0 / 12.0 } else { 0.0 });
            self.applied_animations = Some(animations);
        }
        if self.applied_font != Some(self.settings.looks.font) {
            theme::install_fonts(ctx, self.settings.looks.font);
            self.applied_font = Some(self.settings.looks.font);
        }
        // The light eases from one cover's colour to the next rather than jumping.
        let target = self.glow_target();
        self.glow_colour = match (self.glow_colour, target) {
            (Some(now), Some((wanted, _))) if now != wanted && self.settings.looks.animations => {
                let dt = ctx.input(|i| i.stable_dt).min(0.1);
                ctx.request_repaint();
                Some(theme::mix(now, wanted, (dt * 3.0).min(1.0)).max_towards(wanted))
            }
            (_, Some((wanted, _))) => Some(self.glow_colour.unwrap_or(wanted)),
            (_, None) => None,
        };

        let mut actions: Vec<Action> = Vec::new();
        self.shortcuts(ctx, &mut actions);
        if let Some(mut capture) = self.capture.take() {
            capture.step(self, ctx, &mut actions);
            self.capture = Some(capture);
        }
        let mut inputs = std::mem::take(&mut self.inputs);
        widgets::paint_background(ctx, &self.palette, self.glow_colour.zip(target.map(|(_, g)| g)));
        pages::skin_title_bar(self, ctx);
        player::player_bar(self, ctx, &mut inputs, &mut actions);
        pages::rail(self, ctx, &mut actions);
        if self.settings.lyrics_open {
            player::lyrics_panel(self, ctx, &mut actions);
        }
        pages::central(self, ctx, &mut inputs, &mut actions);
        widgets::toasts(self, ctx);
        let mut quick = self.quick.take();
        pages::quick_search(self, ctx, quick.as_mut(), &mut actions);
        self.quick = quick;
        if self.show_shortcuts {
            pages::shortcuts_window(self, ctx, &mut actions);
        }
        self.inputs = inputs;
        if self.inputs.editing.is_some() {
            pages::edit_details_window(self, ctx, &mut actions);
        }
        for action in actions {
            self.perform(action);
        }

        if self.settings.lyrics_open {
            self.fetch_lyrics_if_open();
        }
        self.watch();
        self.update_presence();
        self.keep_tray_in_step();
        self.feed_widgets();
        self.feed_connect();
        // Closing with "close to the tray" on leaves Noctorium playing there.
        if ctx.input(|i| i.viewport().close_requested()) && self.settings.close_to_tray && self.tray.is_some() && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            self.window_minimised = true;
            if !self.told_about_tray {
                self.told_about_tray = true;
                self.toast("Noctorium is still playing, in the tray. Quit it from there.", false);
            }
        }
        self.track_listening();
        self.poll_authorization();
        self.run_quick_search();
        if self.sleep.is_some() || self.authorizing.is_some() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        self.update_title(ctx);
        if self.queue_dirty.is_some_and(|at| at.elapsed() > Duration::from_secs(2)) {
            self.save_queue();
        }
        self.toasts.retain(|t| t.at.elapsed() < Duration::from_secs(if t.error { 7 } else { 3 }));
        let state = self.engine.state();
        if matches!(state.status, Status::Playing | Status::Loading) || self.resolving.is_some() || !self.toasts.is_empty() {
            let pace = if self.page == Page::NowPlaying && self.settings.visualiser && state.status == Status::Playing { 33 } else { 200 };
            ctx.request_repaint_after(Duration::from_millis(pace));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_spotify_tokens();
        self.save_queue();
        self.save_settings();
        self.engine.stop();
        // A moment for the mpv thread to let go of its player before the process ends.
        std::thread::sleep(Duration::from_millis(150));
    }
}

trait Towards {
    fn max_towards(self, target: Color32) -> Color32;
}

impl Towards for Color32 {
    /// Snaps the last step, so easing ends rather than creeping a unit at a time for ever.
    fn max_towards(self, target: Color32) -> Color32 {
        let close = |a: u8, b: u8| (a as i16 - b as i16).abs() <= 2;
        if close(self.r(), target.r()) && close(self.g(), target.g()) && close(self.b(), target.b()) {
            target
        } else {
            self
        }
    }
}

/// Opens a link in the browser, or a folder in the file manager.
fn open_externally(target: &str) {
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(target);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(target);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(target);
        c
    };
    let _ = command.spawn();
}
