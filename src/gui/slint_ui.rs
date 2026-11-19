//! Noctorium Rust's window drawn by Slint, beside the egui one until it has everything egui has.
//!
//! The app underneath is the same [`App`]: its state, its threads and every [`Action`]. Slint is declarative where
//! egui draws every frame, so this window works the other way round from `pages.rs`: a timer and the threads wake
//! [`Ui::refresh`], which runs the app's [`App::tick`] and [`App::settle`] and hands the `.slint` pages new models
//! when what they show has changed, and the pages call back through `Bridge` with what the listener did, which
//! becomes the same Actions the egui window pushes.
//!
//! Started with `--ui slint`, or `NOCTORIUM_UI=slint`. `NOCTORIUM_CAPTURE` takes pictures of it as it does of the
//! egui window, see [`Capture`].

slint::include_modules!();

use super::host::{Host, WindowCommand, WindowHost};
use super::{Action, App, Loadable, Page, SearchKind};
use crate::domain::{format_duration, Playlist, ProviderType, Track};
use crate::mpv::Status;
use crate::queue::RepeatMode;
use crate::store::Places;
use slint::{ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Whether the listener asked for the Slint window: `--ui slint`, or `NOCTORIUM_UI=slint`.
pub fn wanted(arguments: &[String]) -> bool {
    let said = arguments.windows(2).any(|w| w[0] == "--ui" && w[1].eq_ignore_ascii_case("slint"))
        || arguments.iter().any(|a| a.eq_ignore_ascii_case("--ui=slint"));
    said || std::env::var("NOCTORIUM_UI").is_ok_and(|v| v.eq_ignore_ascii_case("slint"))
}

thread_local! {
    /// The window and the app, on the thread that draws: the timer and the threads' wake-ups find them here.
    static UI: RefCell<Option<Rc<RefCell<Ui>>>> = const { RefCell::new(None) };
}

/// Opens the Slint window, and returns when it is closed.
pub fn run(listener: Option<std::net::TcpListener>, in_tray: bool) -> Result<(), String> {
    let window = AppWindow::new().map_err(|e| e.to_string())?;
    let minimised = Arc::new(AtomicBool::new(false));
    let host: Host = Arc::new(SlintHost { window: window.as_weak(), minimised: minimised.clone(), waking: Arc::new(AtomicBool::new(false)) });
    let places = Places::system();
    let mut app = App::new(host.clone(), places);
    if in_tray {
        window.window().set_minimized(true);
        app.set_window_minimised(true);
        minimised.store(true, Ordering::Relaxed);
    }
    if let Some(listener) = listener {
        let (events, host) = (app.events_sender(), host.clone());
        crate::instance::listen(listener, move || {
            let _ = events.send(super::Event::ShowWindow);
            host.wake();
        });
    }
    let icon = super::widgets::mark_rgba(64);
    window.set_mark(Image::from_rgba8(SharedPixelBuffer::clone_from_slice(&icon, 64, 64)));

    let ui = Rc::new(RefCell::new(Ui::new(app, window.as_weak(), host.clone())));
    UI.with(|slot| *slot.borrow_mut() = Some(ui.clone()));
    wire(&window, &ui);
    ui.borrow_mut().refresh(true);

    // The app looks at its threads' news and the player at least ten times a second, as egui's repaints did.
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), refresh_now);
    let capture = Capture::from_environment();
    let capture_timer = slint::Timer::default();
    if let Some(capture) = capture {
        let capture = RefCell::new(capture);
        capture_timer.start(slint::TimerMode::Repeated, Duration::from_millis(250), move || capture.borrow_mut().step());
    }

    {
        let ui = ui.clone();
        window.window().on_close_requested(move || {
            if ui.borrow_mut().app.close_to_tray() {
                slint::CloseRequestResponse::KeepWindowShown
            } else {
                let _ = slint::quit_event_loop();
                slint::CloseRequestResponse::HideWindow
            }
        });
    }
    window.show().map_err(|e| e.to_string())?;
    slint::run_event_loop_until_quit().map_err(|e| e.to_string())?;
    timer.stop();
    ui.borrow_mut().app.shut_down();
    UI.with(|slot| *slot.borrow_mut() = None);
    Ok(())
}

fn refresh_now() {
    let ui = UI.with(|slot| slot.borrow().clone());
    if let Some(ui) = ui {
        // A callback from the window may hold the app: then this wake-up's work is done by that callback's own refresh.
        if let Ok(mut ui) = ui.try_borrow_mut() {
            ui.refresh(false);
        }
    }
}

/// The window, for the app: threads wake it through Slint's event loop, which is safe from anywhere.
struct SlintHost {
    window: slint::Weak<AppWindow>,
    minimised: Arc<AtomicBool>,
    /// A wake-up is already on its way; another is not needed.
    waking: Arc<AtomicBool>,
}

impl WindowHost for SlintHost {
    fn wake(&self) {
        if self.waking.swap(true, Ordering::AcqRel) {
            return;
        }
        let waking = self.waking.clone();
        let _ = slint::invoke_from_event_loop(move || {
            waking.store(false, Ordering::Release);
            refresh_now();
        });
    }

    fn wake_after(&self, _after: Duration) {
        // The window's own timer comes round every tenth of a second, which is soon enough for anything asked
        // for later.
    }

    fn command(&self, command: WindowCommand) {
        let minimised = self.minimised.clone();
        let _ = self.window.upgrade_in_event_loop(move |window| match command {
            WindowCommand::Show => {
                let _ = window.show();
                window.window().set_minimized(false);
                minimised.store(false, Ordering::Relaxed);
            }
            WindowCommand::Minimise(yes) => {
                window.window().set_minimized(yes);
                minimised.store(yes, Ordering::Relaxed);
            }
            WindowCommand::Focus => {
                let _ = window.show();
            }
            WindowCommand::Close => {
                let _ = window.hide();
                let _ = slint::quit_event_loop();
            }
            WindowCommand::CancelClose => {}
            WindowCommand::Title(title) => window.set_window_title(title.into()),
        });
    }

    fn copy_text(&self, text: String) {
        let _ = self.window.upgrade_in_event_loop(move |window| window.invoke_copy(text.into()));
    }

    fn focused(&self) -> bool {
        !self.minimised.load(Ordering::Relaxed)
    }
}

/// The app, the window, and what the window was last given.
struct Ui {
    app: App,
    window: slint::Weak<AppWindow>,
    covers: Covers,
    /// The songs behind each list the window shows, by the name Bridge.play is told.
    lists: HashMap<String, Vec<Track>>,
    /// The artists Search listed last, opened by their place in it.
    found_artists: Vec<crate::artists::ArtistRef>,
    /// Playlists shown as rows -- a Playlists search's, an artist's albums and playlists -- by list, opened by place.
    shown_lists: HashMap<String, Vec<Playlist>>,
    /// What the models were last built from, so they are built again only when it changes.
    built: u64,
    /// The page the window last showed, so a new one starts at its top.
    shown_page: Option<Page>,
}

impl Ui {
    fn new(app: App, window: slint::Weak<AppWindow>, host: Host) -> Ui {
        Ui { covers: Covers::new(host), app, window, lists: HashMap::new(), found_artists: Vec::new(), shown_lists: HashMap::new(), built: 0, shown_page: None }
    }

    /// The app's turn, then the window brought up to date.
    fn refresh(&mut self, force: bool) {
        self.app.tick();
        self.app.settle();
        let arrived = self.covers.receive();
        let Some(window) = self.window.upgrade() else { return };
        self.theme(&window);
        self.player(&window);
        let signature = self.signature();
        if force || arrived || signature != self.built {
            self.built = signature;
            self.build(&window);
        }
    }

    /// Does what the listener did, and shows what came of it.
    fn act(&mut self, action: Action) {
        self.app.act(action);
        self.refresh(false);
    }

    fn theme(&self, window: &AppWindow) {
        let p = self.app.palette;
        let theme = window.global::<Theme>();
        let wanted = Palette {
            page: colour(p.page),
            panel: colour(p.panel),
            card: colour(p.card),
            raised: colour(p.raised),
            line: colour(p.line),
            text: colour(p.text),
            subtext: colour(p.subtext),
            faint: colour(p.faint),
            accent: colour(p.accent),
            on_accent: colour(p.on_accent),
            error: colour(p.error),
            light: p.light,
        };
        if theme.get_p() != wanted {
            theme.set_p(wanted);
        }
        let looks = &self.app.settings.looks;
        let corners = looks.corners.scale();
        if (theme.get_corners() - corners).abs() > f32::EPSILON {
            theme.set_corners(corners);
        }
        let font: SharedString = match looks.font {
            crate::looks::FontChoice::Serif if cfg!(windows) => "Cambria",
            crate::looks::FontChoice::Serif if cfg!(target_os = "macos") => "Georgia",
            crate::looks::FontChoice::Serif => "DejaVu Serif",
            crate::looks::FontChoice::Mono if cfg!(windows) => "Consolas",
            crate::looks::FontChoice::Mono if cfg!(target_os = "macos") => "Menlo",
            crate::looks::FontChoice::Mono => "DejaVu Sans Mono",
            // The system's own, as egui has it on Windows: Segoe UI. Elsewhere Slint asks the system.
            _ if cfg!(windows) => "Segoe UI",
            _ => "",
        }
        .into();
        if theme.get_font() != font {
            theme.set_font(font);
        }
        if theme.get_animations() != looks.animations {
            theme.set_animations(looks.animations);
        }
    }

    /// The player bar and the lyric being sung: every refresh, as the song moves on.
    fn player(&mut self, window: &AppWindow) {
        let app = &self.app;
        let state = app.engine_state();
        let track = app.loaded.clone().or_else(|| app.resolving.clone()).or_else(|| app.queue.current().cloned()).map(|t| app.edited(&t));
        let live = track.as_ref().is_some_and(|t| t.provider.is_radio());
        let duration = if state.duration_ms > 0 { state.duration_ms } else { track.as_ref().and_then(|t| t.duration_ms).unwrap_or(0) };
        let position = match app.inputs.scrubbing {
            Some(fraction) => (fraction as f64 * duration as f64) as i64,
            None => state.position_ms,
        };
        let art = track.as_ref().and_then(|t| t.artwork_url.clone()).and_then(|url| self.covers.get(&url, false));
        let speed = format!("{:.2}×", app.settings.speed).replace(".00×", "×").replace("0×", "×");
        let wanted = PlayerState {
            has_track: track.is_some(),
            title: track.as_ref().map(|t| t.title.clone()).unwrap_or_default().into(),
            artist: track.as_ref().map(|t| t.artist_line()).unwrap_or_default().into(),
            has_art: art.is_some(),
            art: art.unwrap_or_default(),
            liked: track.as_ref().is_some_and(|t| app.library.is_liked(t)),
            playing: state.status == Status::Playing,
            busy: state.status == Status::Loading || app.resolving.is_some(),
            live,
            progress: if duration > 0 { (position as f64 / duration as f64) as f32 } else { 0.0 },
            position: format_duration(position).into(),
            duration: if duration > 0 { format_duration(duration).into() } else { "-:--".into() },
            volume: app.settings.volume,
            muted: app.settings.muted,
            shuffle: app.queue.shuffle,
            repeat: match app.queue.repeat {
                RepeatMode::Off => 0,
                RepeatMode::All => 1,
                RepeatMode::One => 2,
            },
            speed: speed.into(),
            lyrics_open: app.settings.lyrics_open,
        };
        if window.get_player() != wanted {
            window.set_player(wanted);
        }
        // The line being sung, from where the song is now.
        if let Some(lyrics) = self.current_lyrics() {
            if lyrics.synced {
                let current = lyrics.lines.iter().rposition(|l| l.start_ms.is_some_and(|at| at <= position + 250)).map(|i| i as i32).unwrap_or(-1);
                if window.get_lyric_current() != current {
                    window.set_lyric_current(current);
                }
            }
        }
    }

    fn current_lyrics(&self) -> Option<&crate::lyrics::Lyrics> {
        let key = self.app.queue.current()?.queue_key();
        match self.app.lyrics.get(&key) {
            Some(Loadable::Ready(Some(lyrics))) => Some(lyrics),
            _ => None,
        }
    }

    /// Everything the models are made of, folded into a number: when it moves, they are built again.
    fn signature(&self) -> u64 {
        let app = &self.app;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        format!("{:?}", app.page).hash(&mut h);
        app.settings.theme.hash(&mut h);
        // The first-time setup: whether it is shown, its step, and what its steps choose.
        (app.settings.setup_done, format!("{:?}", app.setup_step), app.settings.autoplay, format!("{:?}", app.settings.search_mode)).hash(&mut h);
        app.settings.profile_name.hash(&mut h);
        format!("{:?}", app.settings.looks.hidden_pages).hash(&mut h);
        app.queue.current_index.hash(&mut h);
        app.queue.tracks.iter().for_each(|t| t.queue_key().hash(&mut h));
        app.queue.suggestions.iter().for_each(|t| t.queue_key().hash(&mut h));
        app.loaded.as_ref().or(app.resolving.as_ref()).map(|t| t.queue_key()).hash(&mut h);
        app.library.liked.len().hash(&mut h);
        app.library.liked.first().map(|t| t.queue_key()).hash(&mut h);
        app.library.recent.len().hash(&mut h);
        app.library.recent.first().map(|t| t.queue_key()).hash(&mut h);
        for p in &app.library.playlists {
            (&p.id, &p.title, p.tracks.len()).hash(&mut h);
        }
        for p in &app.library.followed {
            (&p.title, &p.source_url).hash(&mut h);
        }
        for row in &app.home {
            match row {
                None => 0usize.hash(&mut h),
                Some(row) => (&row.title, row.tracks.as_ref().map(|t| t.len()).map_err(|e| e.len())).hash(&mut h),
            }
        }
        app.search.id.hash(&mut h);
        format!("{:?}", app.search_kind).hash(&mut h);
        app.search.playlists.as_ref().map(|r| r.as_ref().map(|p| p.len()).map_err(|e| e.len())).hash(&mut h);
        for (provider, result) in &app.search.results {
            (format!("{provider:?}"), result.as_ref().map(|r| r.as_ref().map(|t| t.len()).map_err(|e| e.len()))).hash(&mut h);
        }
        if let Page::Remote(url) = &app.page {
            match app.remote.get(url) {
                Some(Loadable::Ready(p)) => p.tracks.len().hash(&mut h),
                Some(Loadable::Failed(e)) => e.hash(&mut h),
                _ => 0.hash(&mut h),
            }
            // An artist's albums and playlists, which come after their songs.
            app.artist_extras.get(url).map(|e| (e.popular.len(), e.rows.len())).hash(&mut h);
        }
        if let Some(track) = app.queue.current() {
            match app.lyrics.get(&track.queue_key()) {
                Some(Loadable::Ready(Some(l))) => (1, l.lines.len(), &l.source).hash(&mut h),
                Some(Loadable::Ready(None)) => 2.hash(&mut h),
                Some(Loadable::Failed(e)) => e.hash(&mut h),
                Some(Loadable::Loading) => 3.hash(&mut h),
                None => 4.hash(&mut h),
            }
        }
        app.toasts.iter().for_each(|t| t.text.hash(&mut h));
        h.finish()
    }

    /// The window's models, built again from the app.
    fn build(&mut self, window: &AppWindow) {
        self.lists.clear();
        let page = self.app.page.clone();
        window.set_page(page_name(&page).into());
        window.set_rail_page(rail_name(&page).into());
        if self.shown_page.as_ref() != Some(&page) {
            self.shown_page = Some(page.clone());
            if page == Page::Search {
                window.invoke_focus_search();
            }
        }
        self.build_setup(window);
        self.build_rail(window);
        self.build_toasts(window);
        match &page {
            Page::Home => self.build_home(window),
            Page::Search => self.build_search(window),
            Page::Library => self.build_library(window),
            Page::Liked | Page::Recent | Page::Playlist(_) | Page::Remote(_) | Page::ServiceLikes(_) => self.build_songs(window, &page),
            Page::Queue => self.build_queue(window),
            Page::NowPlaying => self.build_now_playing(window),
            Page::Settings | Page::SettingsAt(_) => self.build_settings(window),
            other => window.set_not_yet(not_yet_name(other).into()),
        }
    }

    fn build_rail(&mut self, window: &AppWindow) {
        use crate::looks::SidebarPage as S;
        let looks = &self.app.settings.looks;
        let entries = [
            (Icons::get(window).get_home(), "Home", "home", None),
            (Icons::get(window).get_now_playing(), "Now playing", "nowplaying", Some(S::NowPlaying)),
            (Icons::get(window).get_search(), "Search", "search", Some(S::Search)),
            (Icons::get(window).get_library(), "Library", "library", Some(S::Library)),
            (Icons::get(window).get_queue(), "Queue", "queue", Some(S::Queue)),
            (Icons::get(window).get_services(), "Services", "services", Some(S::Services)),
            (Icons::get(window).get_connect(), "Connect", "connect", Some(S::Connect)),
            (Icons::get(window).get_downloads(), "Downloads", "downloads", Some(S::Downloads)),
            (Icons::get(window).get_profile(), "Profile", "profile", Some(S::Profile)),
            (Icons::get(window).get_settings(), "Settings", "settings", None),
        ];
        let nav: Vec<NavEntry> = entries
            .into_iter()
            .filter(|(_, _, _, part)| part.is_none_or(|p| looks.shows_page(p)))
            .map(|(icon, text, page, _)| NavEntry { icon, text: text.into(), page: page.into() })
            .collect();
        window.set_nav(model(nav));
        let mut items = Vec::new();
        let liked_art = self.app.library.liked.first().and_then(|t| t.artwork_url.clone());
        items.push(self.library_item(&format!("Liked songs · {}", self.app.library.liked.len()), "", liked_art, "liked".into()));
        let playlists: Vec<(String, Option<String>, String)> = self
            .app
            .library
            .playlists
            .iter()
            .map(|p| (p.title.clone(), p.tracks.first().and_then(|t| t.artwork_url.clone()), format!("playlist:{}", p.id)))
            .chain(self.app.library.followed.iter().filter_map(|p| Some((p.title.clone(), p.artwork_url.clone(), format!("remote:{}", p.source_url.as_ref()?)))))
            .collect();
        for (title, art, page) in playlists {
            items.push(self.library_item(&title, "", art, page));
        }
        window.set_rail_playlists(model(items));
    }

    fn library_item(&mut self, title: &str, subtitle: &str, art: Option<String>, page: String) -> LibraryItem {
        let image = art.and_then(|url| self.covers.get(&url, false));
        LibraryItem { title: title.into(), subtitle: subtitle.into(), has_art: image.is_some(), art: image.unwrap_or_default(), page: page.into() }
    }

    fn build_toasts(&self, window: &AppWindow) {
        let toasts: Vec<ToastItem> = self.app.toasts.iter().map(|t| ToastItem { text: t.text.clone().into(), error: t.error }).collect();
        window.set_toasts(model(toasts));
    }

    /// A song as a row or a card shows it.
    fn item(&mut self, track: &Track, current: bool) -> TrackItem {
        let shown = self.app.edited(track);
        let playing_key = self.app.loaded.as_ref().or(self.app.resolving.as_ref()).map(|t| t.queue_key());
        let art = shown.artwork_url.as_deref().and_then(|url| self.covers.get(url, false));
        TrackItem {
            title: shown.title.clone().into(),
            subtitle: shown.artist_line().into(),
            duration: shown.duration_ms.filter(|_| !shown.provider.is_radio()).map(format_duration).unwrap_or_default().into(),
            has_art: art.is_some(),
            art: art.unwrap_or_default(),
            playing: playing_key.is_some_and(|k| k == track.queue_key()),
            liked: self.app.library.is_liked(track),
            current,
        }
    }

    fn items(&mut self, list: &str, tracks: &[Track], limit: usize) -> ModelRc<TrackItem> {
        let shown: Vec<Track> = tracks.iter().take(limit).cloned().collect();
        let items: Vec<TrackItem> = shown.iter().map(|t| self.item(t, false)).collect();
        self.lists.insert(list.to_string(), tracks.to_vec());
        model(items)
    }

    /// Playlists as rows of a list -- their cover, their name, whose and where from -- kept to be opened by place.
    fn list_rows(&mut self, list: &str, lists: &[Playlist]) -> ModelRc<TrackItem> {
        let rows: Vec<Track> = lists
            .iter()
            .map(|p| Track {
                provider: p.provider,
                id: p.source_url.clone().unwrap_or_default(),
                title: p.title.clone(),
                artists: vec![crate::domain::Artist {
                    id: String::new(),
                    name: [p.owner_name.clone(), Some(p.provider.display_name().to_string())].into_iter().flatten().collect::<Vec<_>>().join(" · "),
                    provider: p.provider,
                }],
                album: None,
                duration_ms: None,
                artwork_url: p.artwork_url.clone(),
                source_url: String::new(),
            })
            .collect();
        self.shown_lists.insert(list.to_string(), lists.to_vec());
        self.items(list, &rows, usize::MAX)
    }

    fn shelf(&mut self, list: &str, title: &str, subtitle: &str, tracks: &[Track]) -> Shelf {
        Shelf { title: title.into(), subtitle: subtitle.into(), list: list.into(), tracks: self.items(list, tracks, 20), loading: false, error: "".into() }
    }

    fn build_home(&mut self, window: &AppWindow) {
        use crate::looks::HomePart;
        let looks = self.app.settings.looks.clone();
        let name = self.app.settings.profile_name.split_whitespace().next().unwrap_or("").to_string();
        let greeting = if !looks.shows_home(HomePart::Greeting) {
            "Home".to_string()
        } else if name.is_empty() || self.app.settings.profile_name == "Noctorium Listener" {
            super::pages::greeting().to_string()
        } else {
            format!("{}, {name}", super::pages::greeting())
        };
        window.set_greeting(greeting.into());
        window.set_card_side(self.app.settings.looks.card_size.width());
        window.set_home_notice(if self.app.yt_dlp.path.is_none() {
            "yt-dlp is not installed, so there is nothing to search or play yet. Settings › Playback tools installs it.".into()
        } else {
            "".into()
        });
        let mut shelves = Vec::new();
        let recent = self.app.library.recent.clone();
        if !recent.is_empty() && looks.shows_home(HomePart::Recent) {
            shelves.push(self.shelf("home:recent", "Jump back in", "What you played last", &recent));
        }
        let liked = self.app.library.liked.clone();
        if !liked.is_empty() && looks.shows_home(HomePart::Pinned) {
            shelves.push(self.shelf("home:liked", "Liked songs", "", &liked));
        }
        let rows: Vec<Option<super::HomeRow>> = self.app.home.clone();
        for (slot, row) in rows.iter().enumerate() {
            let list = format!("home:{slot}");
            match row {
                None if self.app.yt_dlp.path.is_some() => shelves.push(Shelf { title: "".into(), subtitle: "".into(), list: list.into(), tracks: model(vec![]), loading: true, error: "".into() }),
                None => {}
                Some(row) => match &row.tracks {
                    Ok(tracks) if !tracks.is_empty() => shelves.push(self.shelf(&list, &row.title, &row.subtitle, tracks)),
                    Ok(_) => {}
                    Err(why) => shelves.push(Shelf { title: row.title.clone().into(), subtitle: row.subtitle.clone().into(), list: list.into(), tracks: model(vec![]), loading: false, error: why.clone().into() }),
                },
            }
        }
        window.set_home_shelves(model(shelves));
    }

    /// The first-time setup, while it is shown: see [`crate::setup`].
    fn build_setup(&mut self, window: &AppWindow) {
        use crate::setup::{Step, THEMES};
        let step = self.app.setup_step;
        let colour = |v: u32| slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8);
        let themes: Vec<SetupTheme> = THEMES
            .iter()
            .map(|key| {
                let preset = crate::theme::preset(key);
                SetupTheme {
                    key: preset.key.into(),
                    name: preset.name.into(),
                    chosen: self.app.settings.theme == *key,
                    background: colour(preset.colours.background),
                    card: colour(preset.colours.card),
                    accent: colour(preset.colours.accent),
                }
            })
            .collect();
        window.set_setup(SetupState {
            shown: !self.app.settings.setup_done,
            step: format!("{step:?}").to_lowercase().into(),
            progress: step.progress().to_uppercase().into(),
            title: step.title().into(),
            about: step.about().into(),
            index: step.index() as i32,
            count: Step::ALL.len() as i32,
            themes: model(themes),
            search_mode: match self.app.settings.search_mode {
                crate::bangs::SearchMode::Hybrid => "hybrid",
                crate::bangs::SearchMode::Soundcloud => "soundcloud",
                _ => "youtube-music",
            }
            .into(),
            autoplay: self.app.settings.autoplay,
        });
    }

    fn build_search(&mut self, window: &AppWindow) {
        window.set_search_kind(match self.app.search_kind {
            SearchKind::Music => "music",
            SearchKind::Radio => "radio",
            SearchKind::Artists => "artists",
            SearchKind::Playlists => "playlists",
        }.into());
        if self.app.search_kind == SearchKind::Playlists {
            let (loading, error, found) = match &self.app.search.playlists {
                None => (!self.app.search.query.is_empty(), String::new(), vec![]),
                Some(Err(why)) => (false, why.clone(), vec![]),
                Some(Ok(found)) => (false, String::new(), found.clone()),
            };
            let tracks = self.list_rows("search:playlists", &found);
            let group = Shelf {
                title: "Playlists".into(),
                subtitle: if self.app.search.query.is_empty() { "".into() } else { format!("“{}” on YouTube Music and SoundCloud", self.app.search.query).into() },
                list: "search:playlists".into(),
                tracks,
                loading,
                error: error.into(),
            };
            let none = !loading && found.is_empty() && self.app.search.playlists.is_some();
            window.set_search_hint(if self.app.search.query.is_empty() { "Press Enter to search.".into() } else if none { "No playlists by that name.".into() } else { "".into() });
            window.set_search_groups(model(vec![group]));
            return;
        }
        if self.app.search_kind == SearchKind::Artists {
            // Who was searched for, from what each service found, shown as rows that open their pages.
            let found: Vec<(crate::domain::ProviderType, Vec<Track>)> = self
                .app
                .search
                .results
                .iter()
                .filter_map(|(p, r)| r.as_ref().and_then(|r| r.as_ref().ok()).map(|t| (*p, t.clone())))
                .collect();
            self.found_artists = crate::artists::found(&self.app.search.query, &found);
            let shown: Vec<Track> = self
                .found_artists
                .iter()
                .map(|a| Track {
                    provider: a.provider,
                    id: crate::artists::key(a),
                    title: a.name.clone(),
                    artists: vec![crate::domain::Artist { id: String::new(), name: format!("Artist · {}", a.provider.display_name()), provider: a.provider }],
                    album: None,
                    duration_ms: None,
                    artwork_url: a.artwork.clone(),
                    source_url: String::new(),
                })
                .collect();
            let waiting = self.app.search.results.iter().any(|(_, r)| r.is_none());
            let group = Shelf {
                title: "Artists".into(),
                subtitle: if self.app.search.query.is_empty() { "".into() } else { format!("“{}”", self.app.search.query).into() },
                list: "search:artists".into(),
                tracks: self.items("search:artists", &shown, 30),
                loading: waiting && shown.is_empty(),
                error: "".into(),
            };
            window.set_search_hint(if self.app.search.results.is_empty() { "Press Enter to search.".into() } else if !waiting && shown.is_empty() { "No artist by that name.".into() } else { "".into() });
            window.set_search_groups(model(vec![group]));
            return;
        }
        if self.app.search_kind == SearchKind::Radio {
            let stations = self.app.radio.stations.clone();
            let group = Shelf {
                title: "Radio".into(),
                subtitle: "Stations from Radio Browser".into(),
                list: "search:radio".into(),
                tracks: self.items("search:radio", &stations, 50),
                loading: self.app.radio.loading,
                error: self.app.radio.error.clone().unwrap_or_default().into(),
            };
            window.set_search_groups(model(vec![group]));
            window.set_search_hint("".into());
            return;
        }
        let results = self.app.search.results.iter().map(|(p, r)| (*p, r.clone())).collect::<Vec<_>>();
        let mut groups = Vec::new();
        for (n, (provider, result)) in results.iter().enumerate() {
            let list = format!("search:{n}");
            let title = provider.display_name();
            groups.push(match result {
                None => Shelf { title: title.into(), subtitle: "".into(), list: list.into(), tracks: model(vec![]), loading: true, error: "".into() },
                Some(Err(why)) => Shelf { title: title.into(), subtitle: "".into(), list: list.into(), tracks: model(vec![]), loading: false, error: why.clone().into() },
                Some(Ok(tracks)) => Shelf { title: title.into(), subtitle: "".into(), list: list.clone().into(), tracks: self.items(&list, tracks, 12), loading: false, error: "".into() },
            });
        }
        window.set_search_hint(if results.is_empty() { "Press Enter to search.".into() } else { "".into() });
        window.set_search_groups(model(groups));
    }

    fn build_library(&mut self, window: &AppWindow) {
        window.set_liked_count(self.app.library.liked.len() as i32);
        window.set_recent_count(self.app.library.recent.len() as i32);
        let playlists: Vec<(String, String, Option<String>, String)> = self
            .app
            .library
            .playlists
            .iter()
            .map(|p| {
                let count = p.tracks.len();
                let detail = format!("{} {}{}", count, if count == 1 { "song" } else { "songs" }, if p.synced { " · Noctorium playlist" } else { "" });
                (p.title.clone(), detail, p.tracks.first().and_then(|t| t.artwork_url.clone()), format!("playlist:{}", p.id))
            })
            .collect();
        let items: Vec<LibraryItem> = playlists.into_iter().map(|(t, d, a, p)| self.library_item(&t, &d, a, p)).collect();
        window.set_library_playlists(model(items));
        let followed: Vec<(String, String, Option<String>, String)> = self
            .app
            .library
            .followed
            .iter()
            .filter_map(|p| Some((p.title.clone(), p.provider.display_name().to_string(), p.artwork_url.clone(), format!("remote:{}", p.source_url.as_ref()?))))
            .collect();
        let items: Vec<LibraryItem> = followed.into_iter().map(|(t, d, a, p)| self.library_item(&t, &d, a, p)).collect();
        window.set_library_followed(model(items));
    }

    fn build_songs(&mut self, window: &AppWindow, page: &Page) {
        let summary = |tracks: &[Track]| {
            let total: i64 = tracks.iter().filter_map(|t| t.duration_ms).sum();
            let count = tracks.len();
            format!("{count} {} · {}", if count == 1 { "song" } else { "songs" }, format_duration(total))
        };
        let (kind, title, detail, art, tracks, loading, error): (&str, String, String, Option<String>, Vec<Track>, bool, String) = match page {
            Page::Liked => ("LIKED", "Liked songs".into(), summary(&self.app.library.liked), self.app.library.liked.first().and_then(|t| t.artwork_url.clone()), self.app.library.liked.clone(), false, String::new()),
            Page::Recent => ("HISTORY", "Recently played".into(), summary(&self.app.library.recent), self.app.library.recent.first().and_then(|t| t.artwork_url.clone()), self.app.library.recent.clone(), false, String::new()),
            Page::Playlist(id) => match self.app.library.playlists.iter().find(|p| &p.id == id) {
                Some(p) => (if p.synced { "NOCTORIUM PLAYLIST" } else { "PLAYLIST" }, p.title.clone(), summary(&p.tracks), p.tracks.first().and_then(|t| t.artwork_url.clone()), p.tracks.clone(), false, String::new()),
                None => ("PLAYLIST", "Playlist".into(), String::new(), None, vec![], false, "This playlist is not in your library any more.".into()),
            },
            Page::Remote(url) => match self.app.remote.get(url) {
                Some(Loadable::Ready(p)) => (if crate::artists::is_key(url) { "ARTIST" } else { "PLAYLIST" }, p.title.clone(), format!("{} · {}", p.provider.display_name(), summary(&p.tracks)), p.artwork_url.clone().or_else(|| p.tracks.first().and_then(|t| t.artwork_url.clone())), p.tracks.clone(), false, String::new()),
                Some(Loadable::Failed(why)) => ("PLAYLIST", "Playlist".into(), String::new(), None, vec![], false, why.clone()),
                _ => ("PLAYLIST", "Playlist".into(), String::new(), None, vec![], true, String::new()),
            },
            Page::ServiceLikes(provider) => {
                let likes = match provider {
                    ProviderType::Soundcloud => self.app.service_library.soundcloud_likes.clone(),
                    ProviderType::Spotify => self.app.services.spotify_likes.clone(),
                    _ => self.app.service_library.youtube_likes.clone(),
                };
                match likes {
                    Some(Ok(tracks)) => ("LIKES", format!("Your {} likes", provider.display_name()), summary(&tracks), tracks.first().and_then(|t| t.artwork_url.clone()), tracks, false, String::new()),
                    Some(Err(why)) => ("LIKES", "Likes".into(), String::new(), None, vec![], false, why),
                    None => ("LIKES", "Likes".into(), String::new(), None, vec![], true, String::new()),
                }
            }
            _ => unreachable!("only pages of songs come here"),
        };
        // An artist's page: their most played, and their albums and playlists, above their songs.
        let mut groups = Vec::new();
        if let Page::Remote(url) = page {
            if let Some(extras) = self.app.artist_extras.get(url).cloned() {
                if !extras.popular.is_empty() {
                    let tracks = self.items("artist:popular", &extras.popular, usize::MAX);
                    groups.push(Shelf { title: "Popular".into(), subtitle: "".into(), list: "artist:popular".into(), tracks, loading: false, error: "".into() });
                }
                for (i, (title, lists)) in extras.rows.iter().enumerate() {
                    let list = format!("artist:row:{i}");
                    let tracks = self.list_rows(&list, lists);
                    groups.push(Shelf { title: title.clone().into(), subtitle: "".into(), list: list.into(), tracks, loading: false, error: "".into() });
                }
            }
        }
        let image = art.and_then(|url| self.covers.get(&url, true));
        let songs = SongsPage {
            groups: model(groups),
            kind: kind.into(),
            title: title.into(),
            detail: detail.into(),
            has_art: image.is_some(),
            art: image.unwrap_or_default(),
            list: "songs".into(),
            tracks: self.items("songs", &tracks, usize::MAX),
            loading,
            error: error.into(),
        };
        window.set_songs(songs);
    }

    fn build_queue(&mut self, window: &AppWindow) {
        let tracks = self.app.queue.tracks.clone();
        let current = self.app.queue.current_index;
        let items: Vec<TrackItem> = tracks.iter().enumerate().map(|(i, t)| self.item(t, i as isize == current)).collect();
        self.lists.insert("queue".into(), tracks.clone());
        window.set_queue_tracks(model(items));
        let suggestions = self.app.queue.suggestions.clone();
        window.set_queue_suggestions(self.items("suggestions", &suggestions, usize::MAX));
        window.set_queue_suggestions_title(if suggestions.is_empty() { "".into() } else { "What plays when the queue runs out.".into() });
        let total = self.app.queue.total_duration_ms();
        let count = tracks.len();
        window.set_queue_summary(if count == 0 { "".into() } else { format!("{count} {} · {}", if count == 1 { "song" } else { "songs" }, format_duration(total)).into() });
    }

    fn build_now_playing(&mut self, window: &AppWindow) {
        let track = self.app.loaded.clone().or_else(|| self.app.resolving.clone()).or_else(|| self.app.queue.current().cloned());
        let now = match &track {
            None => NowPlaying::default(),
            Some(track) => {
                let shown = self.app.edited(track);
                let art = shown.artwork_url.as_deref().and_then(|url| self.covers.get(url, true));
                NowPlaying {
                    has_track: true,
                    title: shown.title.clone().into(),
                    artist: shown.artist_line().into(),
                    album: shown.album.as_ref().map(|a| a.title.clone()).unwrap_or_default().into(),
                    has_art: art.is_some(),
                    art: art.unwrap_or_default(),
                    liked: self.app.library.is_liked(track),
                }
            }
        };
        window.set_now(now);
        // What plays after this song: the rest of the queue, then autoplay's songs, as egui's Up next has them.
        let mut up_next = self.app.queue.up_next().to_vec();
        up_next.extend(self.app.queue.suggestions.iter().cloned());
        window.set_up_next(self.items("upnext", &up_next, 100));
        // The lyrics, as they stand.
        let (lines, synced, note) = match track.as_ref().map(|t| (t.provider.is_radio(), self.app.lyrics.get(&t.queue_key()))) {
            None => (vec![], false, String::new()),
            Some((true, _)) => (vec![], false, "A radio station has no lyrics.".into()),
            Some((false, None)) | Some((false, Some(Loadable::Loading))) => (vec![], false, "Looking for the lyrics…".into()),
            Some((false, Some(Loadable::Failed(why)))) => (vec![], false, why.clone()),
            Some((false, Some(Loadable::Ready(None)))) => (vec![], false, "No lyrics were found for this song.".into()),
            Some((false, Some(Loadable::Ready(Some(lyrics))))) => (
                lyrics.lines.iter().map(|l| LyricLine { text: l.text.clone().into(), time_ms: l.start_ms.map(|m| m as i32).unwrap_or(-1) }).collect(),
                lyrics.synced,
                lyrics.source.clone(),
            ),
        };
        window.set_lyrics(model(lines));
        window.set_lyrics_synced(synced);
        window.set_lyrics_note(note.into());
        if window.get_now_side() == "lyrics" {
            self.app.fetch_lyrics();
        }
    }

    fn build_settings(&mut self, window: &AppWindow) {
        let selected = self.app.settings.theme.clone();
        let themes: Vec<ThemeItem> = crate::theme::PRESETS
            .iter()
            .filter(|p| !crate::theme::hidden(p.key))
            .map(|p| {
                let c = p.colours;
                let rgb = |v: u32| slint::Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8);
                ThemeItem {
                    key: p.key.into(),
                    name: p.name.into(),
                    family: p.family.into(),
                    page: rgb(c.background),
                    panel: rgb(c.panel),
                    card: rgb(c.card),
                    text: rgb(c.text),
                    accent: rgb(c.accent),
                    selected: p.key == selected,
                }
            })
            .collect();
        window.set_themes(model(themes));
    }

    // ---- what the window asks for ------------------------------------------------------------------

    fn track_at(&self, list: &str, index: i32) -> Option<Track> {
        let list = self.lists.get(list)?;
        list.get(usize::try_from(index).ok()?).cloned()
    }

    fn play(&mut self, list: &str, index: i32) {
        let Ok(index) = usize::try_from(index) else { return };
        let action = match list {
            "queue" => Action::QueueJump(index),
            "suggestions" => Action::PlaySuggestion(index),
            "upnext" => {
                let queued = self.app.queue.up_next().len();
                if index < queued {
                    Action::QueueJump((self.app.queue.current_index.max(-1) + 1) as usize + index)
                } else {
                    Action::PlaySuggestion(index - queued)
                }
            }
            "search:radio" => match self.track_at(list, index as i32) {
                Some(station) => Action::PlayStation(station),
                None => return,
            },
            // An artist found: their page, not a song.
            "search:artists" => match self.found_artists.get(index) {
                Some(artist) => Action::OpenArtist(artist.clone()),
                None => return,
            },
            // A playlist or an album shown as a row: it opens, as a link to it does.
            list if self.shown_lists.contains_key(list) => match self.shown_lists.get(list).and_then(|l| l.get(index)).and_then(|p| p.source_url.clone()) {
                Some(link) => Action::OpenLink(link),
                None => return,
            },
            "songs" => match self.lists.get(list) {
                Some(tracks) => Action::PlayList { tracks: tracks.clone(), index },
                None => return,
            },
            _ => match self.lists.get(list) {
                Some(tracks) => Action::PlayPicked { tracks: tracks.clone(), index },
                None => return,
            },
        };
        self.act(action);
    }

    fn track_menu(&mut self, list: &str, index: i32, what: &str) {
        let Some(track) = self.track_at(list, index) else { return };
        let at = index as usize;
        let action = match what {
            "next" => Action::PlayNext(track),
            "queue" => Action::AddToQueue(track),
            "radio" => Action::StartRadio(track),
            "artist" => match crate::artists::of_track(&track) {
                Some(artist) => Action::OpenArtist(artist),
                None => return,
            },
            "like" => Action::ToggleLike(track),
            "up" if list == "queue" && at > 0 => Action::QueueMove { from: at, to: at - 1 },
            "down" if list == "queue" && at + 1 < self.app.queue.tracks.len() => Action::QueueMove { from: at, to: at + 1 },
            "remove" if list == "queue" => Action::QueueRemove(at),
            _ => return,
        };
        self.act(action);
    }

    fn open(&mut self, target: &str, window: &AppWindow) {
        let page = match target {
            "back" => return self.act(Action::Back),
            "home" => Page::Home,
            "search" => Page::Search,
            "library" => Page::Library,
            "liked" => Page::Liked,
            "recent" => Page::Recent,
            "queue" => Page::Queue,
            "nowplaying" => Page::NowPlaying,
            "nowplaying-lyrics" => {
                window.set_now_side("lyrics".into());
                self.app.fetch_lyrics();
                Page::NowPlaying
            }
            "settings" => Page::Settings,
            "services" => Page::Services,
            "connect" => Page::Connect,
            "downloads" => Page::Downloads,
            "profile" => Page::Profile,
            other => match (other.strip_prefix("playlist:"), other.strip_prefix("remote:")) {
                (Some(id), _) => Page::Playlist(id.to_string()),
                (_, Some(url)) => Page::Remote(url.to_string()),
                _ => return,
            },
        };
        self.act(Action::Open(page));
    }

    fn bridge(&mut self, what: &str, argument: &str, window: &AppWindow) {
        let current = || self.app.loaded.clone().or_else(|| self.app.queue.current().cloned());
        let action = match what {
            "open" => return self.open(argument, window),
            "toggle-play" => Action::TogglePlay,
            "next" => Action::Next,
            "previous" => Action::Previous,
            "toggle-shuffle" => Action::ToggleShuffle,
            "cycle-repeat" => Action::CycleRepeat,
            "toggle-mute" => Action::ToggleMute,
            "refresh-home" => Action::RefreshHome,
            "queue-shuffle-upcoming" => Action::QueueShuffleUpcoming,
            "queue-clear-upcoming" => Action::QueueClearUpcoming,
            "queue-clear" => Action::QueueClear,
            "artist-current" => match current().as_ref().and_then(crate::artists::of_track) {
                Some(artist) => Action::OpenArtist(artist),
                None => return,
            },
            "search-kind" => Action::SetSearchKind(match argument {
                "radio" => SearchKind::Radio,
                "artists" => SearchKind::Artists,
                "playlists" => SearchKind::Playlists,
                _ => SearchKind::Music,
            }),
            "like-current" => match current() {
                Some(track) => Action::ToggleLike(track),
                None => return,
            },
            "next-speed" => {
                let speeds = [0.75_f32, 1.0, 1.25, 1.5, 2.0];
                let now = self.app.settings.speed;
                Action::SetSpeed(speeds.iter().copied().find(|s| *s > now + 0.01).unwrap_or(speeds[0]))
            }
            "seek-ms" => match argument.parse::<i64>() {
                Ok(ms) if ms >= 0 => Action::Seek(ms),
                _ => return,
            },
            "seek-by" => match argument.parse::<i64>() {
                Ok(by) => Action::Seek((self.app.engine_state().position_ms + by).max(0)),
                _ => return,
            },
            "want-lyrics" => {
                self.app.fetch_lyrics();
                return self.refresh(false);
            }
            "setup" => {
                let step = self.app.setup_step;
                match argument {
                    "skip" => Action::FinishSetup(None),
                    "accounts" => Action::FinishSetup(Some(Page::Services)),
                    "back" => match step.previous() {
                        Some(back) => Action::SetupStep(back),
                        None => return,
                    },
                    _ => match step.next() {
                        Some(next) => Action::SetupStep(next),
                        None => Action::FinishSetup(None),
                    },
                }
            }
            "setup-search" => Action::SetSearchMode(match argument {
                "hybrid" => crate::bangs::SearchMode::Hybrid,
                "soundcloud" => crate::bangs::SearchMode::Soundcloud,
                _ => crate::bangs::SearchMode::YoutubeMusic,
            }),
            "setup-autoplay" => Action::SetAutoplay(argument == "on"),
            "theme" => match crate::theme::PRESETS.iter().find(|p| p.key == argument) {
                Some(preset) => Action::SetTheme(preset.key),
                None => return,
            },
            "new-playlist" => Action::CreatePlaylist { title: argument.to_string(), noctorium: true },
            "play-all" | "shuffle-all" => match self.lists.get(argument).cloned() {
                Some(tracks) if !tracks.is_empty() => {
                    if what == "play-all" {
                        Action::PlayList { tracks, index: 0 }
                    } else {
                        Action::PlayShuffled(tracks)
                    }
                }
                _ => return,
            },
            _ => return,
        };
        self.act(action);
    }

    fn search(&mut self, text: &str) {
        self.app.inputs.search = text.to_string();
        self.act(Action::Search);
    }

    fn seek(&mut self, fraction: f32) {
        let state = self.app.engine_state();
        let duration = if state.duration_ms > 0 { state.duration_ms } else { self.app.queue.current().and_then(|t| t.duration_ms).unwrap_or(0) };
        if duration > 0 {
            self.act(Action::Seek((fraction.clamp(0.0, 1.0) as f64 * duration as f64) as i64));
        }
    }
}

/// Connects the window's Bridge to the app.
fn wire(window: &AppWindow, ui: &Rc<RefCell<Ui>>) {
    let bridge = window.global::<Bridge>();
    let weak = window.as_weak();
    {
        let (ui, weak) = (ui.clone(), weak.clone());
        bridge.on_act(move |what, argument| {
            let Some(window) = weak.upgrade() else { return };
            ui.borrow_mut().bridge(&what, &argument, &window);
        });
    }
    {
        let ui = ui.clone();
        bridge.on_play(move |list, index| ui.borrow_mut().play(&list, index));
    }
    {
        let ui = ui.clone();
        bridge.on_track_menu(move |list, index, what| ui.borrow_mut().track_menu(&list, index, &what));
    }
    {
        let ui = ui.clone();
        bridge.on_seek(move |fraction| ui.borrow_mut().seek(fraction));
    }
    {
        let ui = ui.clone();
        bridge.on_volume(move |volume| ui.borrow_mut().act(Action::SetVolume(volume)));
    }
    {
        let ui = ui.clone();
        bridge.on_search(move |text| ui.borrow_mut().search(&text));
    }
    {
        let ui = ui.clone();
        window.on_menu_liked_of(move |list, index| {
            let ui = ui.borrow();
            ui.track_at(&list, index).is_some_and(|t| ui.app.library.is_liked(&t))
        });
    }
}

fn model<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(items))
}

fn colour(c: eframe::egui::Color32) -> slint::Color {
    slint::Color::from_argb_u8(c.a(), c.r(), c.g(), c.b())
}

/// The page as the window names it.
fn page_name(page: &Page) -> &'static str {
    match page {
        Page::Home => "home",
        Page::Search => "search",
        Page::Library => "library",
        Page::Liked | Page::Recent | Page::Playlist(_) | Page::Remote(_) | Page::ServiceLikes(_) => "songs",
        Page::Queue => "queue",
        Page::NowPlaying => "nowplaying",
        Page::Settings | Page::SettingsAt(_) => "settings",
        _ => "other",
    }
}

/// What the rail lights up for the page.
fn rail_name(page: &Page) -> String {
    match page {
        Page::Liked => "liked".into(),
        Page::Recent => "recent".into(),
        Page::Playlist(id) => format!("playlist:{id}"),
        Page::Remote(url) => format!("remote:{url}"),
        Page::Services | Page::ServiceLikes(_) => "services".into(),
        Page::Connect => "connect".into(),
        Page::Downloads => "downloads".into(),
        Page::Profile => "profile".into(),
        Page::Shortcuts => "settings".into(),
        other => page_name(other).into(),
    }
}

fn not_yet_name(page: &Page) -> &'static str {
    match page {
        Page::Services => "Services",
        Page::Connect => "Connect",
        Page::Downloads => "Downloads",
        Page::Profile => "Profile",
        Page::Shortcuts => "Keyboard shortcuts",
        _ => "This page",
    }
}

// ---- covers ------------------------------------------------------------------------------------------

type Pixels = SharedPixelBuffer<Rgba8Pixel>;

/// The covers, fetched and decoded on threads of their own and made into images on the window's thread.
struct Covers {
    images: HashMap<(String, bool), Option<Image>>,
    asked: HashSet<(String, bool)>,
    jobs: Sender<(String, bool)>,
    done: Receiver<((String, bool), Option<Pixels>)>,
}

impl Covers {
    fn new(host: Host) -> Covers {
        let (jobs, work) = channel::<(String, bool)>();
        let (finished, done) = channel();
        let work = Arc::new(Mutex::new(work));
        for _ in 0..6 {
            let (work, finished, host) = (work.clone(), finished.clone(), host.clone());
            std::thread::spawn(move || loop {
                let job = match work.lock().map(|w| w.recv()) {
                    Ok(Ok(job)) => job,
                    _ => return,
                };
                let pixels = fetch_cover(&job.0, if job.1 { 640 } else { 320 });
                if finished.send((job, pixels)).is_err() {
                    return;
                }
                host.wake();
            });
        }
        Covers { images: HashMap::new(), asked: HashSet::new(), jobs, done }
    }

    /// The cover if it is here; otherwise it is asked for, and the window is built again when it comes.
    fn get(&mut self, url: &str, large: bool) -> Option<Image> {
        let key = (url.to_string(), large);
        if let Some(image) = self.images.get(&key) {
            return image.clone();
        }
        // A large one not here yet: the small one, while it comes.
        if self.asked.insert(key.clone()) {
            let _ = self.jobs.send(key.clone());
        }
        if large {
            return self.images.get(&(url.to_string(), false)).cloned().flatten();
        }
        None
    }

    /// Takes in the covers that arrived. True if any did.
    fn receive(&mut self) -> bool {
        let mut any = false;
        while let Ok((key, pixels)) = self.done.try_recv() {
            self.images.insert(key, pixels.map(Image::from_rgba8));
            any = true;
        }
        any
    }
}

/// A cover from the network, squared and made small enough to keep many of.
fn fetch_cover(url: &str, side: u32) -> Option<Pixels> {
    let response = ureq::get(url).timeout(Duration::from_secs(15)).call().ok()?;
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut std::io::Read::take(response.into_reader(), 12 * 1024 * 1024), &mut bytes).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = image.dimensions();
    let (x, y, cw, ch) = square_crop(url, w, h);
    let cropped = image::imageops::crop_imm(&image, x, y, cw, ch).to_image();
    let side = side.min(cw.max(1));
    let small = image::imageops::resize(&cropped, side, side, image::imageops::FilterType::Triangle);
    Some(SharedPixelBuffer::clone_from_slice(small.as_raw(), small.width(), small.height()))
}

/// The square part of a cover: YouTube's letterboxed thumbnails as egui crops them (see `widgets::square_uv`),
/// anything else from its middle.
fn square_crop(url: &str, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let (fx, fy, fw, fh) = {
        let lower = url.to_lowercase();
        if lower.contains("ytimg.com/vi") && (lower.contains("hqdefault") || lower.contains("sddefault") || lower.ends_with("/default.jpg")) {
            (0.21875, 0.125, 0.5625, 0.75)
        } else if lower.contains("ytimg.com/vi") && (lower.contains("mqdefault") || lower.contains("maxresdefault") || lower.contains("hq720")) {
            (0.21875, 0.0, 0.5625, 1.0)
        } else {
            (0.0, 0.0, 1.0, 1.0)
        }
    };
    let (x, y, cw, ch) = ((w as f32 * fx) as u32, (h as f32 * fy) as u32, (w as f32 * fw) as u32, (h as f32 * fh) as u32);
    let side = cw.min(ch).max(1);
    (x + (cw - side.min(cw)) / 2, y + (ch - side.min(ch)) / 2, side, side)
}

// ---- pictures ----------------------------------------------------------------------------------------

/// Pictures of the Slint window, as `capture.rs` takes them of the egui one: `NOCTORIUM_CAPTURE` names a
/// folder and `NOCTORIUM_CAPTURE_PAGES` the steps. A page is `home`, `search:<words>`, `library`, `queue`,
/// `nowplaying`, `lyrics` (Now playing on its lyrics), `liked` or `settings`; `play` plays the search's first
/// song and `theme:<KEY>` switches the theme, without a picture of their own.
struct Capture {
    folder: std::path::PathBuf,
    steps: Vec<String>,
    next: usize,
    started: Instant,
    wait: Duration,
    acted: bool,
    /// This step's own wait, and whether it takes a picture at the end of it.
    step_wait: Duration,
    picture: bool,
}

impl Capture {
    fn from_environment() -> Option<Capture> {
        let folder = std::path::PathBuf::from(std::env::var_os("NOCTORIUM_CAPTURE").filter(|v| !v.is_empty())?);
        let steps = std::env::var("NOCTORIUM_CAPTURE_PAGES")
            .unwrap_or_else(|_| "home".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let wait = std::env::var("NOCTORIUM_CAPTURE_WAIT").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(8.0);
        let _ = std::fs::create_dir_all(&folder);
        Some(Capture { folder, steps, next: 0, started: Instant::now(), wait: Duration::from_secs_f32(wait), acted: false, step_wait: Duration::ZERO, picture: true })
    }

    fn step(&mut self) {
        let Some(ui) = UI.with(|slot| slot.borrow().clone()) else { return };
        let Ok(mut ui) = ui.try_borrow_mut() else { return };
        let Some(window) = ui.window.upgrade() else { return };
        let Some(step) = self.steps.get(self.next).cloned() else {
            let _ = slint::quit_event_loop();
            return;
        };
        if !self.acted {
            self.acted = true;
            self.started = Instant::now();
            println!("Step {}: {step}", self.next + 1);
            let silent = step == "play" || step.starts_with("theme:");
            match step.as_str() {
                "play" => ui.play("search:0", 0),
                "home" => ui.open("home", &window),
                "library" => ui.open("library", &window),
                "queue" => ui.open("queue", &window),
                "liked" => ui.open("liked", &window),
                "settings" => ui.open("settings", &window),
                "nowplaying" => {
                    window.set_now_side("next".into());
                    ui.open("nowplaying", &window)
                }
                "lyrics" => ui.open("nowplaying-lyrics", &window),
                s if s.starts_with("search:") => {
                    let words = s.trim_start_matches("search:").to_string();
                    ui.open("search", &window);
                    window.set_search_query(words.clone().into());
                    ui.search(&words);
                }
                s if s.starts_with("theme:") => ui.bridge("theme", s.trim_start_matches("theme:"), &window),
                kind @ ("music" | "radio" | "artists" | "playlists") => ui.bridge("search-kind", kind, &window),
                s if s.starts_with("setup:") => {
                    if let Some(action) = super::capture::setup_capture_action(s) {
                        ui.act(action);
                    }
                }
                // An artist's page, as egui's capture opens one: `artist:sc|<name>|<profile>` or `artist:ytm|<name>`.
                s if s.starts_with("artist:") => {
                    let mut parts = s.trim_start_matches("artist:").split('|');
                    let provider = if parts.next() == Some("sc") { ProviderType::Soundcloud } else { ProviderType::YoutubeMusic };
                    let name = parts.next().unwrap_or("").trim().to_string();
                    let profile = parts.next().map(|p| p.trim().to_string()).filter(|p| !p.is_empty());
                    ui.act(Action::OpenArtist(crate::artists::ArtistRef { provider, name, profile, artwork: None }));
                }
                _ => {}
            }
            self.picture = !silent;
            // A theme takes a moment; a song is looked up and started in about the time a page fills in.
            self.step_wait = if step.starts_with("theme:") { Duration::from_millis(600) } else { self.wait };
            return;
        }
        if self.started.elapsed() < self.step_wait {
            return;
        }
        if !self.picture {
            self.next += 1;
            self.acted = false;
            return;
        }
        match window.window().take_snapshot() {
            Ok(pixels) => {
                let name: String = step.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
                let path = self.folder.join(format!("{:02}-{name}.png", self.next + 1));
                let saved = image::RgbaImage::from_raw(pixels.width(), pixels.height(), pixels.as_bytes().to_vec())
                    .map(|img| img.save(&path));
                match saved {
                    Some(Ok(())) => println!("Saved {}", path.display()),
                    _ => println!("Could not save {}", path.display()),
                }
            }
            Err(why) => println!("No picture of {step}: {why}"),
        }
        self.next += 1;
        self.acted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_slint_window_is_asked_for_by_flag_or_environment() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(wanted(&args(&["--ui", "slint"])));
        assert!(wanted(&args(&["--ui=Slint"])));
        assert!(!wanted(&args(&["--ui", "egui"])) || std::env::var("NOCTORIUM_UI").is_ok());
        assert!(!wanted(&args(&["--tray"])) || std::env::var("NOCTORIUM_UI").is_ok());
    }

    #[test]
    fn covers_are_cut_square_as_egui_cuts_them() {
        // YouTube's 4:3 thumbnail around a 16:9 frame around the square cover: its middle square.
        assert_eq!(square_crop("https://i.ytimg.com/vi/abc/hqdefault.jpg", 480, 360), (105, 45, 270, 270));
        // A 16:9 one: the middle of it.
        assert_eq!(square_crop("https://i.ytimg.com/vi/abc/maxresdefault.jpg", 1280, 720), (280, 0, 720, 720));
        // Anything else wider than tall, from its middle; a square one whole.
        assert_eq!(square_crop("https://example.com/a.jpg", 400, 300), (50, 0, 300, 300));
        assert_eq!(square_crop("https://example.com/a.jpg", 500, 500), (0, 0, 500, 500));
    }

    #[test]
    fn every_page_has_a_name_the_window_knows() {
        assert_eq!(page_name(&Page::Home), "home");
        assert_eq!(page_name(&Page::Playlist("x".into())), "songs");
        assert_eq!(page_name(&Page::SettingsAt(super::super::settings::SettingsPage::PlaybackTools)), "settings");
        assert_eq!(page_name(&Page::Connect), "other");
        assert_eq!(rail_name(&Page::Playlist("x".into())), "playlist:x");
        assert_eq!(rail_name(&Page::Remote("https://a".into())), "remote:https://a");
        assert_eq!(not_yet_name(&Page::Services), "Services");
    }
}
