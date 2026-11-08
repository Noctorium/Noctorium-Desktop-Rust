//! Settings, laid out as the Kotlin desktop lays them out: a home with every page on it, grouped into what each
//! is about, and a search box above them that finds any one setting by name, by what it does or by what people
//! call it, and goes straight to it. Each page is a column of cards, one card to a group of settings.

use super::pages;
use super::widgets::{notice, primary, secondary};
use super::{Action, App, Inputs, Page};
use crate::domain::ProviderType;
use crate::looks::Choice;
use crate::theme::{self, Palette, PRESETS};
use crate::tools::Tool;
use eframe::egui::{self, pos2, vec2, Align, Align2, RichText, Sense, Stroke, StrokeKind, Ui};

/// The groups the pages of Settings come in, in the order the home shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    You,
    Look,
    Listening,
    Services,
    Device,
    About,
}

impl Section {
    pub const ALL: [Section; 6] = [Section::You, Section::Look, Section::Listening, Section::Services, Section::Device, Section::About];

    pub fn title(self) -> &'static str {
        match self {
            Section::You => "You",
            Section::Look => "Look",
            Section::Listening => "Listening",
            Section::Services => "Services",
            Section::Device => "This device",
            Section::About => "About",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Section::You => "Your account and your profile",
            Section::Look => "Colours, type, the player bar, now playing and Home",
            Section::Listening => "How the music sounds, and how it goes on",
            Section::Services => "Where the music comes from, and who hears about it",
            Section::Device => "This computer: starting up, the keyboard, files and the tools that play",
            Section::About => "Noctorium itself",
        }
    }
}

/// Every page of Settings, in its section, in the order the home shows them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsPage {
    Account,
    Profile,
    NoctoriumPlaylists,
    Theme,
    Text,
    PlayerBar,
    NowPlaying,
    Browsing,
    Sound,
    PlaybackQueue,
    Lyrics,
    YouTube,
    SoundCloud,
    Spotify,
    Bandcamp,
    Vk,
    AppleMusic,
    Deezer,
    Tidal,
    Scrobbling,
    Discord,
    StreamWidgets,
    Startup,
    Shortcuts,
    Downloads,
    PlaybackTools,
    Updates,
    Experimental,
    Diagnostics,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 29] = [
        SettingsPage::Account,
        SettingsPage::Profile,
        SettingsPage::NoctoriumPlaylists,
        SettingsPage::Theme,
        SettingsPage::Text,
        SettingsPage::PlayerBar,
        SettingsPage::NowPlaying,
        SettingsPage::Browsing,
        SettingsPage::Sound,
        SettingsPage::PlaybackQueue,
        SettingsPage::Lyrics,
        SettingsPage::YouTube,
        SettingsPage::SoundCloud,
        SettingsPage::Spotify,
        SettingsPage::Bandcamp,
        SettingsPage::Vk,
        SettingsPage::AppleMusic,
        SettingsPage::Deezer,
        SettingsPage::Tidal,
        SettingsPage::Scrobbling,
        SettingsPage::Discord,
        SettingsPage::StreamWidgets,
        SettingsPage::Startup,
        SettingsPage::Shortcuts,
        SettingsPage::Downloads,
        SettingsPage::PlaybackTools,
        SettingsPage::Updates,
        SettingsPage::Experimental,
        SettingsPage::Diagnostics,
    ];

    pub fn title(self) -> &'static str {
        match self {
            SettingsPage::Account => "Noctorium account",
            SettingsPage::Profile => "Your Noctorium profile",
            SettingsPage::NoctoriumPlaylists => "Noctorium playlists",
            SettingsPage::Theme => "Theme and colours",
            SettingsPage::Text => "Typeface and text",
            SettingsPage::PlayerBar => "Player bar",
            SettingsPage::NowPlaying => "Now playing",
            SettingsPage::Browsing => "Home and browsing",
            SettingsPage::Sound => "Sound",
            SettingsPage::PlaybackQueue => "Playback and queue",
            SettingsPage::Lyrics => "Lyrics providers",
            SettingsPage::YouTube => "YouTube Music account",
            SettingsPage::SoundCloud => "SoundCloud account",
            SettingsPage::Spotify => "Spotify library",
            SettingsPage::Bandcamp => "Bandcamp",
            SettingsPage::Vk => "VK Music",
            SettingsPage::AppleMusic => "Apple Music",
            SettingsPage::Deezer => "Deezer",
            SettingsPage::Tidal => "Tidal",
            SettingsPage::Scrobbling => "Scrobbling",
            SettingsPage::Discord => "Discord Rich Presence",
            SettingsPage::StreamWidgets => "Stream widgets",
            SettingsPage::Startup => "Startup and tray",
            SettingsPage::Shortcuts => "Keyboard shortcuts",
            SettingsPage::Downloads => "Saving music",
            SettingsPage::PlaybackTools => "Playback tools",
            SettingsPage::Updates => "Updates",
            SettingsPage::Experimental => "Experimental",
            SettingsPage::Diagnostics => "Diagnostics",
        }
    }

    pub fn section(self) -> Section {
        use SettingsPage::*;
        match self {
            Account | Profile | NoctoriumPlaylists => Section::You,
            Theme | Text | PlayerBar | NowPlaying | Browsing => Section::Look,
            Sound | PlaybackQueue | Lyrics => Section::Listening,
            YouTube | SoundCloud | Spotify | Bandcamp | Vk | AppleMusic | Deezer | Tidal | Scrobbling | Discord | StreamWidgets => Section::Services,
            Startup | Shortcuts | Downloads | PlaybackTools => Section::Device,
            Updates | Experimental | Diagnostics => Section::About,
        }
    }

    /// Its picture on its tile and beside its settings in the search's results.
    fn icon(self) -> &'static str {
        use SettingsPage::*;
        match self {
            Account => "📊",
            Profile => "👤",
            NoctoriumPlaylists => "☁",
            Theme => "🎨",
            Text => "🔤",
            PlayerBar => "🎛",
            NowPlaying => "💿",
            Browsing => "🏠",
            Sound => "🎚",
            PlaybackQueue => "☰",
            Lyrics => "🎤",
            YouTube => "▶",
            SoundCloud => "☁",
            Spotify => "🎧",
            Bandcamp => "💽",
            Vk => "🎶",
            AppleMusic => "🎵",
            Deezer => "〰",
            Tidal => "🌊",
            Scrobbling => "📜",
            Discord => "🎮",
            StreamWidgets => "📺",
            Startup => "⏻",
            Shortcuts => "⌨",
            Downloads => "💾",
            PlaybackTools => "🔧",
            Updates => "⬆",
            Experimental => "🧪",
            Diagnostics => "🩺",
        }
    }

    /// The services whose pages are in beta, and say so above everything else on them.
    fn in_beta(self) -> bool {
        matches!(self, SettingsPage::Spotify | SettingsPage::Bandcamp | SettingsPage::Vk | SettingsPage::AppleMusic | SettingsPage::Deezer | SettingsPage::Tidal)
    }

    /// The pages that show what the Services page reads, its charts and best-sellers, and so want it read.
    pub fn reads_services(self) -> bool {
        matches!(self, SettingsPage::Bandcamp | SettingsPage::AppleMusic | SettingsPage::Deezer | SettingsPage::Spotify | SettingsPage::Vk | SettingsPage::Tidal)
    }
}

// ---------------------------------------------------------------------------------------------------------
// The home

/// The home of Settings: its title, the search box, and every page under the section it belongs to -- or,
/// while something is typed in the box, the settings that match it.
pub fn home(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.label(RichText::new("Settings").font(theme::bold(30.0)).color(palette.text));
    ui.label(RichText::new("Accounts, services and how Noctorium behaves.").color(palette.subtext));
    ui.add_space(12.0);

    let width = ui.available_width().min(640.0);
    let field = ui.add(
        egui::TextEdit::singleline(&mut inputs.settings_query)
            .hint_text("🔍  Search settings: afterglow, booster, last.fm, shortcuts…")
            .font(theme::regular(15.0))
            .margin(vec2(12.0, 9.0))
            .desired_width(width),
    );
    let (enter, escape) = ui.input(|i| (i.key_pressed(egui::Key::Enter), i.key_pressed(egui::Key::Escape)));
    if field.has_focus() && escape {
        inputs.settings_query.clear();
    }
    ui.add_space(4.0);

    if !inputs.settings_query.trim().is_empty() {
        let matches = search(&inputs.settings_query);
        if field.lost_focus() && enter {
            if let Some(first) = matches.first() {
                open_entry(ui, first, actions);
            }
        }
        results(ui, app, &matches, actions);
        return;
    }

    for section in Section::ALL {
        section_heading(ui, &palette, section);
        let pages: Vec<SettingsPage> = SettingsPage::ALL.iter().copied().filter(|p| p.section() == section).collect();
        let gap = 10.0;
        let available = ui.available_width();
        let columns = if available >= 720.0 { 2 } else { 1 };
        let tile_width = (available - gap * (columns as f32 - 1.0)) / columns as f32;
        for row in pages.chunks(columns) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for &page in row {
                    let text = tile_text(app, page);
                    if tile(ui, &palette, tile_width, page.icon(), &text.title, &text.subtitle, text.active).clicked() {
                        actions.push(Action::Open(Page::SettingsAt(page)));
                    }
                }
            });
            ui.add_space(gap - ui.spacing().item_spacing.y);
        }
    }
}

/// A section's name over its pages: in small capitals in the accent, with what is in it beside.
fn section_heading(ui: &mut Ui, palette: &Palette, section: Section) {
    ui.add_space(14.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(section.title().to_uppercase()).font(theme::bold(11.5)).color(palette.accent).extra_letter_spacing(1.0));
        ui.add_space(6.0);
        ui.add(egui::Label::new(RichText::new(section.description()).font(theme::regular(12.5)).color(palette.subtext)).truncate());
    });
    ui.add_space(6.0);
}

/// What a page's tile says: its name -- the account's and the profile's are the listener's own -- what it
/// holds, and whether it is on.
struct TileText {
    title: String,
    subtitle: String,
    active: bool,
}

fn tile_text(app: &App, page: SettingsPage) -> TileText {
    let s = &app.settings;
    let looks = &s.looks;
    let plain = |subtitle: String| TileText { title: page.title().to_string(), subtitle, active: false };
    let on = |subtitle: String, active: bool| TileText { title: page.title().to_string(), subtitle, active };
    match page {
        SettingsPage::Account => match &app.account {
            Some((user, _)) => TileText {
                title: user.display_name.clone(),
                subtitle: match &app.account_stats {
                    Some(stats) => format!("{} streamed · {} different · {:.0} hours listened", stats.streams, stats.unique_tracks, stats.hours),
                    None => "Signed in to Noctorium".into(),
                },
                active: true,
            },
            None => plain("Sign in to count what you listen to".into()),
        },
        SettingsPage::Profile => TileText { title: s.profile_name.clone(), subtitle: "Your local profile and history".into(), active: false },
        SettingsPage::NoctoriumPlaylists => {
            use crate::noctplaylists::CloudKind;
            match (&app.playlist_sync.problem, s.playlist_cloud.kind) {
                (_, CloudKind::None) => plain("Your playlists on every device: choose where to keep them".into()),
                (Some(problem), _) => plain(problem.clone()),
                (None, CloudKind::Folder) => on(format!("In {}", s.playlist_cloud.folder), true),
                (None, CloudKind::GoogleDrive) => on("In your Google Drive".into(), app.google_signed_in()),
            }
        }
        SettingsPage::Theme => {
            let theme = if s.theme == theme::CUSTOM {
                "Your own"
            } else {
                PRESETS.iter().find(|p| p.key == s.theme).map(|p| p.name).unwrap_or("Night")
            };
            plain(format!("{theme} · accent: {}", looks.accent.label().to_lowercase()))
        }
        SettingsPage::Text => plain(format!("{} typeface · {} size", looks.font.label(), looks.text_size.label().to_lowercase())),
        SettingsPage::PlayerBar => plain(format!(
            "{}, {} · {} seek bar",
            looks.player_bar.label(),
            looks.player_bar_position.label().to_lowercase(),
            looks.progress.label()
        )),
        SettingsPage::NowPlaying => plain(format!("{} · {} backdrop · lyrics", looks.now_playing.label(), looks.backdrop.label().to_lowercase())),
        SettingsPage::Browsing => plain("Cards, badges, what Home shows and the sidebar".into()),
        SettingsPage::Sound => {
            if s.equalizer.enabled {
                on(format!("Equaliser on · {}", s.equalizer.preset.name()), true)
            } else {
                plain("Equaliser off".into())
            }
        }
        SettingsPage::PlaybackQueue => plain(format!(
            "{:.2}× · autoplay {} · queue {}",
            s.speed,
            if s.autoplay { "on" } else { "off" },
            if s.restore_queue { "kept" } else { "not kept" }
        )),
        SettingsPage::Lyrics => plain(format!("Opens on {} · 8 sources", s.lyrics_source.display_name())),
        SettingsPage::YouTube => match &app.youtube {
            Some(_) => on(format!("Signed in as {}", app.youtube_name.clone().unwrap_or_else(|| "your account".into())), true),
            None => plain("Not signed in".into()),
        },
        SettingsPage::SoundCloud => match &app.soundcloud {
            Some((_, profile)) => on(format!("Signed in as {}", profile.name), true),
            None => plain("Not signed in".into()),
        },
        SettingsPage::Spotify => match &app.services.spotify_name {
            Some(name) => on(format!("Signed in as {name}"), true),
            None => plain("Public playlists by link, or your library signed in".into()),
        },
        SettingsPage::Bandcamp => {
            let genres = s.bandcamp_genres.len();
            plain(if genres == 0 { "Search, albums and songs".into() } else { format!("{genres} genres on Home") })
        }
        SettingsPage::Vk => match &app.services.vk_name {
            Some(name) => on(format!("Signed in as {name}"), true),
            None => plain("Sign in from your browser".into()),
        },
        SettingsPage::AppleMusic => {
            if matches!(app.services.apple_library, Some(Ok(_))) {
                on("Your library is read".into(), true)
            } else {
                plain("The catalogue, and your library from the browser".into())
            }
        }
        SettingsPage::Deezer => match &s.deezer_profile {
            Some(_) => on("Your public profile is shown".into(), true),
            None => plain("Charts, playlists and your public profile".into()),
        },
        SettingsPage::Tidal => match &app.services.tidal_name {
            Some(name) => on(format!("Signed in as {name}"), true),
            None if !s.tidal_client_id.is_empty() => plain("Your Tidal app is set up".into()),
            None => plain("Needs a Tidal app of your own".into()),
        },
        SettingsPage::Scrobbling => {
            let connected: Vec<&str> = crate::scrobble::Site::ALL.iter().filter(|site| app.scrobblers.contains_key(site)).map(|site| site.name()).collect();
            if connected.is_empty() {
                plain("Connect Last.fm, Libre.fm or ListenBrainz".into())
            } else {
                on(format!("Connected: {}", connected.join(", ")), true)
            }
        }
        SettingsPage::Discord => {
            if s.discord.enabled {
                on(format!("Enabled · {}", s.discord.timestamps.name()), true)
            } else {
                plain("Disabled".into())
            }
        }
        SettingsPage::StreamWidgets => {
            if s.widgets.enabled {
                let running = app.widget_server.is_some();
                on(if running { format!("Serving {}", s.widgets.theme) } else { "On, but not serving".into() }, running)
            } else {
                plain("For OBS or Streamlabs, off".into())
            }
        }
        SettingsPage::Startup => plain(format!(
            "{} · closing {}",
            match app.startup_mode {
                crate::startup::Mode::Off => "Starts when opened".to_string(),
                mode => format!("Starts with the computer: {}", mode.name().to_lowercase()),
            },
            if s.close_to_tray { "keeps it in the tray" } else { "quits" }
        )),
        SettingsPage::Shortcuts => on(
            if s.shortcuts.global_enabled { "In the window, and global ones from any program".into() } else { "In the window; global ones off".into() },
            s.shortcuts.global_enabled,
        ),
        SettingsPage::Downloads => plain(match &s.export_folder {
            Some(folder) => format!("Into {folder}"),
            None => format!("Onto your desktop, as {}", s.export_format.name()),
        }),
        SettingsPage::PlaybackTools => {
            let missing: Vec<&str> = [(Tool::YtDlp, &app.yt_dlp), (Tool::Mpv, &app.mpv)]
                .iter()
                .filter(|(_, info)| info.path.is_none())
                .map(|(tool, _)| tool.name())
                .collect();
            if missing.is_empty() {
                on("yt-dlp and mpv are ready".into(), true)
            } else {
                plain(format!("{} is missing", missing.join(" and ")))
            }
        }
        SettingsPage::Experimental => plain("Codes, for things that are not settings yet".into()),
        SettingsPage::Updates => match (&app.update_ready, &app.newer) {
            (Some(version), _) => on(format!("{version} is installed: restart to use it"), true),
            (None, Some(newer)) => plain(format!("Noctorium {} is available", newer.version)),
            (None, None) => plain(format!(
                "You have {} · {}",
                crate::version(),
                if !s.check_updates { "not looking" } else if s.auto_update && cfg!(windows) { "installs by itself" } else { "tells you" }
            )),
        },
        SettingsPage::Diagnostics => plain("Check yt-dlp, mpv, storage and the network".into()),
    }
}

/// A page's tile on the home: its picture, its name and what it holds, a tick where it is on or signed in,
/// and the chevron that says it opens.
fn tile(ui: &mut Ui, palette: &Palette, width: f32, icon: &str, title: &str, subtitle: &str, active: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 68.0), Sense::click());
    let radius = egui::CornerRadius::same(theme::corner(14));
    let fill = if response.hovered() { palette.card.lerp_to_gamma(palette.accent, 0.08) } else { palette.card };
    ui.painter().rect_filled(rect, radius, fill);
    if response.hovered() {
        ui.painter().rect_stroke(rect, radius, Stroke::new(1.0_f32, palette.accent.gamma_multiply(0.45)), StrokeKind::Inside);
    }
    let painter = ui.painter_at(rect);
    painter.text(rect.left_center() + vec2(24.0, 0.0), Align2::CENTER_CENTER, icon, theme::regular(20.0), palette.accent);
    let right = rect.right() - if active { 52.0 } else { 30.0 };
    let text_left = rect.left() + 50.0;
    let text_width = (right - text_left).max(40.0);
    let title_galley = layout(ui, title, theme::bold(14.5), palette.text, text_width, 1);
    let subtitle_galley = layout(ui, subtitle, theme::regular(12.5), palette.subtext, text_width, 2);
    let block = title_galley.size().y + 2.0 + subtitle_galley.size().y;
    let top = rect.center().y - block / 2.0;
    painter.galley(pos2(text_left, top), title_galley.clone(), palette.text);
    painter.galley(pos2(text_left, top + title_galley.size().y + 2.0), subtitle_galley, palette.subtext);
    if active {
        painter.text(pos2(rect.right() - 40.0, rect.center().y), Align2::CENTER_CENTER, "✔", theme::regular(14.0), palette.accent);
    }
    painter.text(pos2(rect.right() - 16.0, rect.center().y), Align2::CENTER_CENTER, "›", theme::bold(20.0), palette.subtext);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// `text` set within `width` on at most `rows` lines, an ellipsis where it runs over.
fn layout(ui: &Ui, text: &str, font: egui::FontId, colour: egui::Color32, width: f32, rows: usize) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), egui::TextFormat::simple(font, colour));
    job.wrap = egui::text::TextWrapping { max_width: width, max_rows: rows, break_anywhere: false, overflow_character: Some('…') };
    ui.fonts(|f| f.layout_job(job))
}

// ---------------------------------------------------------------------------------------------------------
// The search

/// One setting as the search knows it: the page it is on, the card it is in there -- what the page scrolls to
/// and lights up -- and the words it is found by.
struct Entry {
    page: SettingsPage,
    title: &'static str,
    anchor: Option<&'static str>,
    about: &'static str,
    words: &'static str,
}

const fn e(page: SettingsPage, title: &'static str, anchor: Option<&'static str>, about: &'static str, words: &'static str) -> Entry {
    Entry { page, title, anchor, about, words }
}

use SettingsPage as P;

const INDEX: &[Entry] = &[
    e(P::Account, "Noctorium account", Some("Your account"), "One sign-in for the players and the website, and the statistics of what you listen to.", "sign in login log in email password statistics stats plays hours make an account sign up"),
    e(P::Account, "Noctorium Connect", Some("Noctorium Connect"), "Whether this computer finds your other devices and shows up on theirs, and what they call it.", "connect devices device name network phone remote cast"),
    e(P::NoctoriumPlaylists, "Noctorium playlists", Some("Noctorium playlists"), "Playlists on every device you use Noctorium on, kept in a synced folder or your Google Drive.", "sync cloud google drive onedrive dropbox backup phone every device cross platform"),
    e(P::NoctoriumPlaylists, "Keep them in", Some("Keep them in"), "A folder Google Drive, OneDrive or Dropbox syncs, or your Google Drive signed in to.", "folder google drive sign in client id oauth"),
    e(P::Profile, "Profile name", Some("Profile name"), "The name this computer's Noctorium goes by.", "name profile greeting"),
    e(P::Profile, "History", Some("History"), "Keeping a history of what you play, and clearing it.", "history recently played clear forget keep"),
    e(P::Profile, "Bring over from Noctorium", Some("Bring over from Noctorium"), "The playlists, pinned songs and history of the Kotlin desktop on this computer.", "import kotlin library data folder migrate"),
    e(P::Theme, "Theme", Some("Theme"), "Noctorium's own palettes, the Glow themes' lit black, your editor's and terminal's favourites, and Windows 98 and XP.", "dark mode light mode night dusk day colour scheme color scheme colours colors palette skin appearance catppuccin dracula nord gruvbox tokyo night rose pine solarized windows 98 xp retro glow afterglow cinder velvet smoke crimson"),
    e(P::Theme, "Accent", Some("Accent"), "The colour buttons, sliders and highlights are drawn in: the theme's own, a named one, your own, or the cover's.", "accent colour color highlight tint match the artwork cover colours"),
    e(P::Theme, "Your own colours", Some("Your own colours"), "Six colours of your own, for a theme of your own.", "custom theme custom colors hex make my own theme"),
    e(P::Theme, "Animations", Some("Finish"), "Pages fading in, the light easing, records turning. Off makes every change instant.", "motion reduce motion transitions effects animation instant"),
    e(P::Theme, "Surfaces", Some("Finish"), "Solid panels, or glass.", "glass liquid glass blur transparency translucent acrylic frosted"),
    e(P::Theme, "Corners", Some("Finish"), "How round the corners of cards, buttons and covers are.", "rounded corners square roundness radius"),
    e(P::Theme, "Icons", Some("Finish"), "How the controls draw their icons.", "icon set icon theme icon style glyphs symbols outlined rounded sharp two-tone line"),
    e(P::Theme, "Service badge style", Some("Finish"), "Each service's own mark on the badges, or the letters, YTM, SC or SP.", "logo logos service logos icons brand badge badges marks letters"),
    e(P::Text, "Text size", Some("Text"), "Everything written, bigger or smaller at once.", "font size bigger text larger text smaller text zoom scale accessibility readability"),
    e(P::Text, "Typeface", Some("Text"), "The typeface everything is written in.", "font font family serif monospace typography"),
    e(P::PlayerBar, "Player bar layout", Some("Player bar"), "Inline, Stacked, Centred, Slim, Spotlight, Floating, the Island, Display, Taskbar, Toolbar and more.", "player style bar style controls layout toolbar island floating taskbar compact"),
    e(P::PlayerBar, "Player bar position", Some("Player bar"), "Along the foot of the window, or the top.", "top bottom move the player bar"),
    e(P::PlayerBar, "Buttons on the player bar", Some("Player bar"), "Which of shuffle, repeat, like, lyrics, the queue, the sleep timer, volume and more it carries.", "hide buttons show buttons player buttons stop shuffle repeat like"),
    e(P::PlayerBar, "Show the clock", Some("Player bar"), "The time in the Taskbar layout's tray.", "clock time tray clock"),
    e(P::PlayerBar, "Seek bar style", Some("Seek bar"), "Material, Minimal, Wave, Segments, Capsule, Classic, Bars, Beads, Neon, Ruler, Slider and more.", "progress bar scrubber slider wave waveform timeline"),
    e(P::PlayerBar, "Right-hand figure", Some("Seek bar"), "The time on the right of the seek bar: the song's length, or the time left.", "time remaining time left duration countdown elapsed"),
    e(P::NowPlaying, "Now playing layout", Some("Now playing"), "Side by side, stage, focus, banner, sing along, full cover, split, cover flow, turntable, big type, playlist.", "now playing layout arrangement turntable vinyl poster immersive sing along karaoke full screen player"),
    e(P::NowPlaying, "Cover", Some("Now playing"), "How the cover is drawn: square, rounded, round, or as a record.", "album art artwork cover art record vinyl cover size"),
    e(P::NowPlaying, "Backdrop", Some("Now playing"), "What is behind the now playing screen: a wash of the cover's colours, the cover itself, or plain.", "background wallpaper blur ambient wash"),
    e(P::NowPlaying, "Panel", Some("Now playing"), "How wide the panel beside the cover is, and whether it opens on Up next or the lyrics.", "panel width up next lyrics tab default tab sidebar width"),
    e(P::NowPlaying, "Under the cover", Some("Now playing"), "The line being sung or the album, and a button to follow the artist.", "lyric line current lyric album name follow artist subscribe"),
    e(P::NowPlaying, "Visualiser", Some("Visualiser and playlist layout"), "Bars or a level meter drawn from the sound itself.", "visualizer spectrum analyser analyzer audio bars vu meter level meter sound bars"),
    e(P::NowPlaying, "Playlist layout columns", Some("Visualiser and playlist layout"), "Which columns the Playlist layout's table has, and its status bar.", "queue columns table columns playlist columns status bar codec bitrate"),
    e(P::NowPlaying, "Lyrics look", Some("Lyrics"), "How big the lyrics are, how they are aligned, and whether the other lines dim.", "lyrics size lyric font size alignment centre center left dim fade karaoke"),
    e(P::Browsing, "Card size", Some("Browsing"), "How big the covers on Home and in the library are.", "tile size thumbnail size grid size covers"),
    e(P::Browsing, "Service badges", Some("Browsing"), "Whether a cover carries the mark of the service it comes from.", "badges icons on covers logos"),
    e(P::Browsing, "Play and menu buttons", Some("Browsing"), "Whether a card's buttons are always there or only when pointed at.", "hover buttons card buttons"),
    e(P::Browsing, "Parts of Home", Some("On Home"), "Which rows Home shows: the greeting, pinned songs, played lately and each service's shelves.", "home page home screen hide home rows shelves greeting recently played pinned"),
    e(P::Browsing, "Search", Some("Search"), "Where search goes when no bang says, and which services a search of all of them asks.", "search default bang hybrid search all services !all"),
    e(P::Browsing, "Pages in the sidebar", Some("In the sidebar"), "Which pages the sidebar offers. Home and Settings always stay.", "sidebar navigation menu hide pages rail"),
    e(P::Sound, "Equaliser", Some("Equaliser"), "Ten bands from the deepest bass to the air above the cymbals, and presets to start from.", "equalizer eq bass treble bass boost tone frequencies preset preamp rock pop jazz"),
    e(P::Sound, "Volume booster", Some("Volume booster"), "Louder than full volume, up to 1000%, behind a limiter so it gets loud, not broken.", "louder boost volume boost amplify amplifier gain quiet songs too quiet 1000%"),
    e(P::Sound, "Even out loudness", Some("Loudness"), "Songs play at about the same volume.", "normalise normalize loudness replaygain volume level"),
    e(P::PlaybackQueue, "Speed", Some("Speed"), "Faster or slower without changing the pitch, from half to double.", "playback speed tempo faster slower podcast rate"),
    e(P::PlaybackQueue, "Autoplay", Some("Autoplay"), "When the queue runs out, songs like the last one carry on.", "radio endless keep playing continue infinite play recommendations home autoplay shelf"),
    e(P::PlaybackQueue, "Keep the queue between launches", Some("The queue"), "Opens with the queue as you left it, and picks the song up where it was.", "remember queue restore queue resume save queue restore session"),
    e(P::PlaybackQueue, "Sleep timer", Some("Sleep timer"), "The music grows quieter over the sleep timer's last moments instead of stopping dead.", "sleep timer bedtime fade fade out"),
    e(P::PlaybackQueue, "SponsorBlock", Some("Skipping"), "Skip the parts of a YouTube video that are not the music: intros, outros, sponsor reads.", "sponsorblock sponsor block skip sponsors skip intro skip outro adverts ads"),
    e(P::Lyrics, "Warn when lyrics are a close match", Some("Close matches"), "The note above lyrics found for a song under another name, that they may not be its own.", "close match wrong lyrics lyrics warning approximate fuzzy reupload uploaded"),
    e(P::Lyrics, "Open lyrics on", Some("Open lyrics on"), "Which source the lyrics open on. Every source is still asked.", "lyrics provider lyrics source lrclib genius musixmatch better lyrics karalyr synclrc happi synced lyrics"),
    e(P::YouTube, "YouTube Music account", Some("Sign-in"), "Your YouTube Music, signed in from a browser you already use.", "youtube ytm google sign in login cookies browser firefox chrome edge"),
    e(P::YouTube, "YouTube Music history", Some("History on YouTube Music"), "Add what you play to your YouTube Music history, for its recommendations.", "watch history youtube history recommendations"),
    e(P::SoundCloud, "SoundCloud account", Some("Sign-in"), "Your SoundCloud, signed in from a browser you already use.", "soundcloud sc sign in login cookies browser"),
    e(P::Spotify, "Spotify", Some("Spotify"), "Public playlists and albums by link, your library signed in, or Spotify's data download.", "spotify sp sign in liked songs import data download"),
    e(P::Spotify, "Spotify's songs", Some("Spotify's songs"), "Matched on YouTube Music and played here, or played on Spotify (Premium).", "spotify premium spotify connect device play on spotify"),
    e(P::Bandcamp, "Bandcamp", Some("Bandcamp"), "Search, albums and songs, played straight from Bandcamp.", "bandcamp bc"),
    e(P::Bandcamp, "Genres on Home", Some("Genres on Home"), "Each genre picked adds a row of its best-sellers to Home.", "bandcamp genres genres best sellers"),
    e(P::Vk, "VK Music", Some("VK Music"), "Your music, playlists and recommendations, signed in from the browser.", "vk vkontakte sign in"),
    e(P::AppleMusic, "Apple Music", Some("Apple Music"), "Its catalogue and charts, and your library signed in from the browser.", "apple music itunes am storefront"),
    e(P::Deezer, "Deezer", Some("Deezer"), "Its charts, playlists and albums by link, and your public profile.", "deezer dz profile flow"),
    e(P::Tidal, "Tidal", Some("Tidal"), "Signed in with your own Tidal app: its catalogue and your collection.", "tidal td client id client secret redirect developer"),
    e(P::Scrobbling, "Scrobbling", Some("Scrobbling"), "What you listen to, on Last.fm, Libre.fm and ListenBrainz.", "scrobble scrobbling last.fm lastfm libre.fm librefm listenbrainz token"),
    e(P::Discord, "Discord Rich Presence", Some("Discord"), "What is playing, on your Discord profile, while Discord runs on this computer.", "discord presence status rich profile activity listening playing card buttons privacy"),
    e(P::StreamWidgets, "Stream widgets", Some("Stream widgets"), "What is playing, over your stream in OBS or Streamlabs.", "obs streamlabs overlay browser source twitch widget port"),
    e(P::Startup, "Closing the window", Some("Closing the window"), "Whether closing the window leaves Noctorium playing in the tray.", "close exit quit tray minimise minimize background"),
    e(P::Startup, "Start with the computer", Some("Start with the computer"), "Whether Noctorium opens when you sign in to the computer.", "startup autostart launch at login boot"),
    e(P::Startup, "Announcing tracks", Some("Announcing tracks"), "A note from the system when a song starts.", "notifications notify toast announce"),
    e(P::Startup, "Open Noctorium on", Some("Opening"), "Which page Noctorium opens on.", "start page home page default page launch page first page"),
    e(P::Shortcuts, "Keyboard shortcuts", Some("Shortcuts"), "Every shortcut, to change, and global ones that work from any program.", "keyboard keys hotkeys hot keys global shortcuts"),
    e(P::Shortcuts, "Media keys", Some("Media keys"), "The keyboard's play, pause, next and previous keys.", "media keys play pause next previous"),
    e(P::Downloads, "Saving music", Some("Saving music"), "Songs saved as files: a tagged 320 kbps MP3, or the original audio, and the folder.", "download downloads offline mp3 save file export folder desktop original quality"),
    e(P::PlaybackTools, "Playback tools", Some("Playback tools"), "yt-dlp reads the services; mpv makes the sound.", "yt-dlp ytdlp mpv install update version tools"),
    e(P::Updates, "Updates", Some("Updates"), "Whether a newer Noctorium is out, and installing it by itself.", "update upgrade new version check for updates release auto update automatic updates install restart"),
    e(P::Experimental, "Codes", Some("Codes"), "Things that are not settings yet: type a code and press Enter.", "experimental code codes easter egg secret cheat"),
    e(P::Updates, "About Noctorium", Some("About Noctorium"), "The version, the licence and the source.", "about version licence license github source credits"),
    e(P::Diagnostics, "Diagnostics", Some("Diagnostics"), "Checks that yt-dlp, mpv and storage are working, for when something is not.", "troubleshoot problems debug logs help not working broken test"),
];

/// The settings matching what is typed, best first: every word typed must be found in the setting's name, its
/// page's, what it does or the words people use for it.
fn search(query: &str) -> Vec<&'static Entry> {
    let words: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
    let mut found: Vec<(u32, usize, &'static Entry)> = Vec::new();
    for (index, entry) in INDEX.iter().enumerate() {
        let title = entry.title.to_lowercase();
        let page = entry.page.title().to_lowercase();
        let about = entry.about.to_lowercase();
        let mut score = 0;
        let mut all = true;
        for word in &words {
            let word = word.as_str();
            let s = if title.starts_with(word) || title.split_whitespace().any(|w| w.starts_with(word)) {
                8
            } else if title.contains(word) {
                6
            } else if entry.words.split_whitespace().any(|w| w.starts_with(word)) {
                4
            } else if page.contains(word) {
                3
            } else if entry.words.contains(word) || about.contains(word) {
                1
            } else {
                0
            };
            if s == 0 {
                all = false;
                break;
            }
            score += s;
        }
        if all {
            found.push((score, index, entry));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    found.into_iter().map(|(_, _, entry)| entry).take(40).collect()
}

/// The id the card to light up is kept under, while its page opens.
fn anchor_id() -> egui::Id {
    egui::Id::new("settings-anchor")
}

fn open_entry(ui: &Ui, entry: &Entry, actions: &mut Vec<Action>) {
    if let Some(anchor) = entry.anchor {
        let now = ui.input(|i| i.time);
        ui.data_mut(|d| d.insert_temp(anchor_id(), (anchor.to_string(), now, false)));
    }
    actions.push(Action::Open(Page::SettingsAt(entry.page)));
}

fn results(ui: &mut Ui, app: &App, matches: &[&'static Entry], actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.add_space(8.0);
    if matches.is_empty() {
        ui.label(RichText::new("No setting goes by that name.").color(palette.faint));
        return;
    }
    ui.label(RichText::new(format!("{} found · Enter opens the first", matches.len())).font(theme::regular(12.0)).color(palette.faint));
    ui.add_space(4.0);
    let width = ui.available_width();
    for (index, entry) in matches.iter().enumerate() {
        let (rect, response) = ui.allocate_exact_size(vec2(width, 54.0), Sense::click());
        let radius = egui::CornerRadius::same(theme::corner(10));
        if response.hovered() || index == 0 {
            ui.painter().rect_filled(rect, radius, if response.hovered() { palette.wash(0.14) } else { palette.card });
        }
        let painter = ui.painter_at(rect);
        painter.text(rect.left_center() + vec2(22.0, 0.0), Align2::CENTER_CENTER, entry.page.icon(), theme::regular(18.0), palette.accent);
        let left = rect.left() + 46.0;
        let text_width = rect.width() - 56.0;
        let where_ = format!("{}  ·  {} › {}", entry.title, entry.page.section().title(), entry.page.title());
        let first = layout(ui, &where_, theme::bold(14.0), palette.text, text_width, 1);
        let second = layout(ui, entry.about, theme::regular(12.5), palette.subtext, text_width, 1);
        painter.galley(pos2(left, rect.top() + 9.0), first, palette.text);
        painter.galley(pos2(left, rect.top() + 30.0), second, palette.subtext);
        if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
            open_entry(ui, entry, actions);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// A page

/// A page of Settings: the way back, the section it is in, its name, and its cards.
pub fn page(ui: &mut Ui, app: &App, page: SettingsPage, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    ui.horizontal(|ui| {
        if ui
            .add(egui::Button::new(RichText::new("←").font(theme::bold(20.0)).color(palette.text)).frame(false).min_size(vec2(32.0, 32.0)))
            .on_hover_text("Back to Settings")
            .clicked()
        {
            actions.push(Action::Open(Page::Settings));
        }
        ui.vertical(|ui| {
            let crumb = ui.add(
                egui::Label::new(RichText::new(format!("Settings  ›  {}", page.section().title())).font(theme::regular(12.0)).color(palette.subtext))
                    .sense(Sense::click()),
            );
            if crumb.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                actions.push(Action::Open(Page::Settings));
            }
            ui.label(RichText::new(page.title()).font(theme::bold(26.0)).color(palette.text));
        });
    });
    ui.add_space(12.0);
    if page.in_beta() {
        notice(ui, &palette, crate::services::BETA_WARNING, false);
        ui.add_space(8.0);
    }
    ui.set_max_width(ui.available_width().min(980.0));
    match page {
        SettingsPage::Account => account(ui, app, inputs, actions),
        SettingsPage::Profile => profile(ui, app, inputs, actions),
        SettingsPage::NoctoriumPlaylists => noctorium_playlists(ui, app, inputs, actions),
        SettingsPage::Theme => theme_page(ui, app, actions),
        SettingsPage::Text => text_page(ui, app, actions),
        SettingsPage::PlayerBar => player_bar(ui, app, actions),
        SettingsPage::NowPlaying => now_playing(ui, app, actions),
        SettingsPage::Browsing => browsing(ui, app, actions),
        SettingsPage::Sound => sound(ui, app, actions),
        SettingsPage::PlaybackQueue => playback_queue(ui, app, actions),
        SettingsPage::Lyrics => lyrics(ui, app, actions),
        SettingsPage::YouTube => service_account(ui, app, ProviderType::YoutubeMusic, actions),
        SettingsPage::SoundCloud => service_account(ui, app, ProviderType::Soundcloud, actions),
        SettingsPage::Spotify => spotify(ui, app, inputs, actions),
        SettingsPage::Bandcamp => bandcamp(ui, app, inputs, actions),
        SettingsPage::Vk => group(ui, app, "VK Music", Some(pages::VK_ABOUT), |ui| pages::vk_section(ui, app, inputs, actions)),
        SettingsPage::AppleMusic => group(ui, app, "Apple Music", Some(pages::APPLE_MUSIC_ABOUT), |ui| pages::apple_music_section(ui, app, inputs, actions)),
        SettingsPage::Deezer => group(ui, app, "Deezer", Some(pages::DEEZER_ABOUT), |ui| pages::deezer_section(ui, app, inputs, actions)),
        SettingsPage::Tidal => group(ui, app, "Tidal", Some(pages::TIDAL_ABOUT), |ui| pages::tidal_section(ui, app, inputs, actions)),
        SettingsPage::Scrobbling => group(
            ui,
            app,
            "Scrobbling",
            Some("What you listen to, on Last.fm, Libre.fm and ListenBrainz: a song counts once half of it, or four minutes, has played."),
            |ui| pages::scrobble_settings(ui, app, inputs, actions),
        ),
        SettingsPage::Discord => group(ui, app, "Discord", Some("What is playing, on your Discord profile, while Discord runs on this computer."), |ui| {
            pages::discord_settings(ui, app, actions)
        }),
        SettingsPage::StreamWidgets => group(
            ui,
            app,
            "Stream widgets",
            Some("What is playing, over your stream in OBS or Streamlabs: add the address as a browser source."),
            |ui| pages::widget_settings(ui, app, actions),
        ),
        SettingsPage::Startup => startup(ui, app, actions),
        SettingsPage::Shortcuts => shortcuts(ui, app, actions),
        SettingsPage::Downloads => group(ui, app, "Saving music", Some("Songs downloaded play offline; songs saved as files are yours to keep."), |ui| {
            pages::download_settings(ui, app, actions)
        }),
        SettingsPage::PlaybackTools => group(ui, app, "Playback tools", Some("yt-dlp reads the services; mpv makes the sound."), |ui| {
            pages::tool_row(ui, app, Tool::YtDlp, &app.yt_dlp, actions);
            pages::tool_row(ui, app, Tool::Mpv, &app.mpv, actions);
        }),
        SettingsPage::Updates => updates(ui, app, actions),
        SettingsPage::Experimental => experimental(ui, app, inputs, actions),
        SettingsPage::Diagnostics => group(
            ui,
            app,
            "Diagnostics",
            Some("Checks that yt-dlp, mpv, storage and the services can be reached, for when something is not working."),
            |ui| pages::diagnostics_settings(ui, app, actions),
        ),
    }
}

/// A card of settings under its name, and what they are for. The search's chosen setting scrolls its card
/// into view and rims it in the accent for a moment.
fn group(ui: &mut Ui, app: &App, title: &str, about: Option<&str>, add: impl FnOnce(&mut Ui)) {
    let palette = app.palette;
    let frame = egui::Frame::new()
        .fill(palette.card)
        .corner_radius(egui::CornerRadius::same(theme::corner(14)))
        .inner_margin(egui::Margin { left: 18, right: 18, top: 14, bottom: 16 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Boxes, tracks and fields are drawn in the card's own colour by default, which on a card is no
            // colour at all: an unticked box and a slider's track vanished. A step off the card, and a rim.
            let widgets = &mut ui.visuals_mut().widgets;
            for state in [&mut widgets.inactive, &mut widgets.hovered] {
                state.bg_fill = palette.page;
                state.bg_stroke = Stroke::new(1.0_f32, palette.line);
            }
            ui.visuals_mut().extreme_bg_color = palette.page;
            ui.label(RichText::new(title).font(theme::bold(16.0)).color(palette.text));
            if let Some(about) = about {
                ui.label(RichText::new(about).font(theme::regular(12.5)).color(palette.subtext));
            }
            ui.add_space(10.0);
            add(ui);
        });
    let rect = frame.response.rect;
    let lit: Option<(String, f64, bool)> = ui.data(|d| d.get_temp(anchor_id()));
    if let Some((anchor, since, scrolled)) = lit {
        if anchor == title {
            let now = ui.input(|i| i.time);
            if !scrolled {
                ui.scroll_to_rect(rect, Some(Align::TOP));
                ui.data_mut(|d| d.insert_temp(anchor_id(), (anchor.clone(), now, true)));
            }
            let strength = 1.0 - ((now - since) / 1.6).clamp(0.0, 1.0) as f32;
            if strength > 0.0 {
                ui.painter().rect_stroke(
                    rect,
                    egui::CornerRadius::same(theme::corner(14)),
                    Stroke::new(2.0_f32, palette.accent.gamma_multiply(strength)),
                    StrokeKind::Inside,
                );
                ui.ctx().request_repaint();
            } else {
                ui.data_mut(|d| d.remove::<(String, f64, bool)>(anchor_id()));
            }
        }
    }
    ui.add_space(12.0);
}

/// The width the names of a card's settings are set in, so their controls line up.
const LABEL: f32 = 170.0;

/// One setting in a card: its name on the left, its control beside it.
fn labelled<R>(ui: &mut Ui, palette: &Palette, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal_wrapped(|ui| {
        ui.allocate_ui_with_layout(vec2(LABEL, 22.0), egui::Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(LABEL);
            ui.add(egui::Label::new(RichText::new(label).color(palette.subtext)).truncate());
        });
        add(ui)
    })
    .inner
}

/// A row of choices, each with what it does on hover. True when changed.
fn choices<T: Choice>(ui: &mut Ui, palette: &Palette, label: &str, value: &mut T) -> bool {
    labelled(ui, palette, label, |ui| {
        let mut changed = false;
        for &choice in T::ALL {
            let response = ui.selectable_value(value, choice, choice.label());
            let response = if choice.description().is_empty() { response } else { response.on_hover_text(choice.description()) };
            changed |= response.changed();
        }
        changed
    })
}

/// A switch with its name, and what it does underneath in quieter writing. True when changed.
fn switch(ui: &mut Ui, palette: &Palette, value: &mut bool, label: &str, about: &str) -> bool {
    let changed = ui.checkbox(value, RichText::new(label).color(palette.text)).changed();
    if !about.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(26.0);
            ui.label(RichText::new(about).font(theme::regular(12.0)).color(palette.faint));
        });
    }
    ui.add_space(4.0);
    changed
}

/// Ticks for the members of a list kept in `list`: ticked is in it. `inverted` for lists of what is hidden.
fn ticks<T: Copy + PartialEq>(ui: &mut Ui, list: &mut Vec<T>, all: &[T], label: impl Fn(T) -> &'static str, inverted: bool) -> bool {
    let mut changed = false;
    for &item in all {
        let mut on = list.contains(&item) != inverted;
        if ui.checkbox(&mut on, label(item)).changed() {
            changed = true;
            if on != inverted {
                list.push(item);
            } else {
                list.retain(|x| *x != item);
            }
        }
    }
    changed
}

/// Changes to the looks, made on a copy and sent once at the end of the page.
fn with_looks(app: &App, actions: &mut Vec<Action>, edit: impl FnOnce(&mut crate::looks::Looks) -> bool) {
    let mut looks = app.settings.looks.clone();
    if edit(&mut looks) {
        actions.push(Action::SetLooks(Box::new(looks)));
    }
}

/// Changes to the settings, made on a copy and sent once.
fn with_settings(app: &App, actions: &mut Vec<Action>, edit: impl FnOnce(&mut crate::store::Settings) -> bool) {
    let mut settings = app.settings.clone();
    if edit(&mut settings) {
        actions.push(Action::SetSettings(Box::new(settings)));
    }
}

// ---------------------------------------------------------------------------------------------------------
// You

fn account(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    group(ui, app, "Your account", Some("One sign-in for the players and the website, and the statistics of what you listen to."), |ui| {
        pages::account_settings(ui, app, inputs, actions)
    });
    group(ui, app, "Noctorium Connect", Some("Whether this computer finds your other devices and shows up on theirs, and what they call it."), |ui| {
        pages::connect_settings(ui, app, inputs, actions)
    });
}

fn profile(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Profile name", Some("The name this computer's Noctorium goes by, and greets you with. It stays on this computer."), |ui| {
        ui.horizontal(|ui| {
            let name = inputs.profile_name.get_or_insert_with(|| app.settings.profile_name.clone());
            let field = ui.add(egui::TextEdit::singleline(name).desired_width(260.0));
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (secondary(ui, &palette, "Save").clicked() || enter) && !name.trim().is_empty() && name.trim() != app.settings.profile_name {
                let mut settings = app.settings.clone();
                settings.profile_name = name.trim().to_string();
                actions.push(Action::SetSettings(Box::new(settings)));
            }
        });
    });
    group(ui, app, "History", Some(&format!("{} songs played here. They are on your profile, by day.", app.play_history.len())), |ui| {
        with_settings(app, actions, |s| {
            switch(ui, &palette, &mut s.keep_history, "Keep a history of what I play", "A song goes in once half a minute of it has been heard.")
        });
        ui.horizontal(|ui| {
            if secondary(ui, &palette, "See it on your profile").clicked() {
                actions.push(Action::Open(Page::Profile));
            }
            if app.play_history.is_empty() {
                return;
            }
            if inputs.confirm_delete.as_deref() == Some("history") {
                ui.label(RichText::new("Forget every song in it?").color(palette.error));
                if ui.button(RichText::new("Clear").color(palette.error)).clicked() {
                    actions.push(Action::ClearHistory);
                    inputs.confirm_delete = None;
                }
                if ui.button("Keep").clicked() {
                    inputs.confirm_delete = None;
                }
            } else if secondary(ui, &palette, "Clear history").clicked() {
                inputs.confirm_delete = Some("history".into());
            }
        });
    });
    group(ui, app, "Bring over from Noctorium", Some("The playlists, pinned songs and history of the Kotlin desktop on this computer."), |ui| {
        ui.horizontal(|ui| {
            if secondary(ui, &palette, "Bring them over").clicked() {
                actions.push(Action::ImportNeighbour);
            }
            if secondary(ui, &palette, "Open the data folder").clicked() {
                actions.push(Action::OpenFolder);
            }
        });
        if let Some(root) = &app.places.root {
            ui.label(RichText::new(root.display().to_string()).font(theme::regular(12.0)).color(palette.faint));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
// Look

fn theme_page(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::looks::{parse_hex, to_hex, Accent};
    let palette = app.palette;
    group(ui, app, "Theme", Some("Noctorium's own, and palettes from editors and terminals. Afterglow's light takes the colour of the cover playing."), |ui| {
        // Families of one -- Nord, Dracula and the rest -- share a row rather than each taking one.
        let family_of = |preset: &theme::ThemePreset| match preset.family {
            "Noctorium" | "Crimson" | "Catppuccin" | "Glow" | "Windows" | "Yours" => preset.family,
            _ => "Editors and terminals",
        };
        let yours = theme::ThemePreset { key: theme::CUSTOM, name: "Your own", family: "Yours", colours: app.settings.looks.custom_theme.colours(), glow: None };
        let all: Vec<&theme::ThemePreset> = PRESETS.iter().filter(|p| !theme::hidden(p.key)).chain(std::iter::once(&yours)).collect();
        let mut families: Vec<&str> = Vec::new();
        for preset in all.iter().copied() {
            if !families.contains(&family_of(preset)) {
                families.push(family_of(preset));
            }
        }
        for family in families {
            ui.label(RichText::new(family).font(theme::bold(12.5)).color(palette.subtext));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(10.0, 10.0);
                for preset in all.iter().copied().filter(|p| family_of(p) == family) {
                    if super::widgets::theme_swatch(ui, &palette, preset, app.settings.theme == preset.key).clicked() {
                        actions.push(Action::SetTheme(preset.key));
                    }
                }
            });
            ui.add_space(6.0);
        }
    });
    group(ui, app, "Accent", Some("The colour buttons, sliders and highlights are drawn in: the theme's own, one of Noctorium's, the cover's, or your own."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Accent", &mut looks.accent);
            if looks.accent == Accent::Custom {
                labelled(ui, &palette, "Your colour", |ui| {
                    changed |= ui.add_sized(vec2(96.0, 22.0), egui::TextEdit::singleline(&mut looks.custom_accent).hint_text("#B47CFF")).changed();
                    match parse_hex(&looks.custom_accent) {
                        Some(rgb) => {
                            let (swatch, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                            ui.painter().rect_filled(swatch, 4, theme::rgb(rgb));
                        }
                        None => {
                            ui.label(RichText::new("A colour as six hex digits, #RRGGBB.").color(palette.faint));
                        }
                    }
                });
            }
            changed
        });
    });
    group(ui, app, "Finish", Some("How everything is drawn: in motion or still, solid or glass, its corners, icons and badges."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = labelled(ui, &palette, "Animations", |ui| {
                ui.checkbox(&mut looks.animations, "Pages fading in, the light easing, records turning; off makes every change instant").changed()
            });
            changed |= choices(ui, &palette, "Surfaces", &mut looks.surface);
            changed |= choices(ui, &palette, "Corners", &mut looks.corners);
            changed |= choices(ui, &palette, "Icons", &mut looks.icons);
            changed |= choices(ui, &palette, "Service badge style", &mut looks.badges);
            changed
        });
    });
    group(ui, app, "Your own colours", Some("Six colours, as hex codes, for a theme of your own: pick \"Your own\" under Theme to use them."), |ui| {
        let mut also: Option<Action> = None;
        with_looks(app, actions, |looks| {
            let mut changed = false;
            let custom = &mut looks.custom_theme;
            egui::Grid::new("custom-theme").num_columns(3).spacing(vec2(10.0, 6.0)).show(ui, |ui| {
                for (name, value) in [
                    ("Page", &mut custom.background),
                    ("Panels", &mut custom.panel),
                    ("Cards", &mut custom.card),
                    ("Writing", &mut custom.text),
                    ("Quieter writing", &mut custom.subtext),
                    ("Accent", &mut custom.accent),
                ] {
                    ui.label(RichText::new(name).color(palette.subtext));
                    changed |= ui.add_sized(vec2(96.0, 22.0), egui::TextEdit::singleline(value)).changed();
                    match parse_hex(value) {
                        Some(rgb) => {
                            let (swatch, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                            ui.painter().rect_filled(swatch, 4, theme::rgb(rgb));
                            ui.painter().rect_stroke(swatch, 4, Stroke::new(1.0_f32, palette.line), StrokeKind::Inside);
                        }
                        None => {
                            ui.label(RichText::new("not a colour").color(palette.error));
                        }
                    }
                    ui.end_row();
                }
            });
            changed |= ui.checkbox(&mut custom.light, "It is a light theme (dark writing on a pale page)").changed();
            let colours = custom.colours();
            let contrast = crate::looks::contrast_ratio(colours.text, colours.background);
            if contrast < 4.5 {
                ui.label(RichText::new(format!("The writing stands out only {contrast:.1}:1 from the page; 4.5:1 or more reads comfortably.")).color(palette.error));
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if app.settings.theme != theme::CUSTOM && primary(ui, &palette, "Use my theme").clicked() {
                    also = Some(Action::SetTheme(theme::CUSTOM));
                }
                if secondary(ui, &palette, "Start from the theme in use").on_hover_text("Copy the current theme's colours into these").clicked() {
                    let now = theme::colours_of(&app.settings.theme, &app.settings.looks);
                    *custom = crate::looks::CustomTheme {
                        background: to_hex(now.background),
                        panel: to_hex(now.panel),
                        card: to_hex(now.card),
                        text: to_hex(now.text),
                        subtext: to_hex(now.subtext),
                        accent: to_hex(now.accent),
                        light: now.light,
                    };
                    changed = true;
                }
                if secondary(ui, &palette, "Reset every look").on_hover_text("Every setting under Look back as it came").clicked() {
                    also = Some(Action::SetLooks(Box::default()));
                }
            });
            changed
        });
        actions.extend(also);
    });
}

fn text_page(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Text", Some("How big everything is written, and in what typeface."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Text size", &mut looks.text_size);
            changed |= choices(ui, &palette, "Typeface", &mut looks.font);
            changed
        });
    });
}

fn player_bar(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Player bar", Some("The bar with the controls on it: its layout, where it sits and its buttons."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Layout", &mut looks.player_bar);
            changed |= choices(ui, &palette, "Position", &mut looks.player_bar_position);
            changed |= labelled(ui, &palette, "Its buttons", |ui| {
                ticks(ui, &mut looks.player_buttons, &crate::looks::PlayerButton::ALL, |b| b.label(), false)
            });
            changed |= labelled(ui, &palette, "Clock", |ui| ui.checkbox(&mut looks.taskbar_clock, "The time in the Taskbar layout's tray").changed());
            changed
        });
    });
    group(ui, app, "Seek bar", Some("The bar a song is sought along, in the player bar and on now playing."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Style", &mut looks.progress);
            changed |= choices(ui, &palette, "Right-hand figure", &mut looks.time_display);
            changed
        });
    });
}

fn now_playing(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Now playing", Some("The screen the player bar opens: its layout, the cover, the backdrop and the panel."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Layout", &mut looks.now_playing);
            changed |= choices(ui, &palette, "Cover", &mut looks.cover);
            changed |= choices(ui, &palette, "Cover size", &mut looks.cover_size);
            changed |= choices(ui, &palette, "Backdrop", &mut looks.backdrop);
            changed |= choices(ui, &palette, "Panel width", &mut looks.panel_width);
            changed |= choices(ui, &palette, "Open the panel on", &mut looks.open_on);
            changed |= labelled(ui, &palette, "Under the cover", |ui| {
                let mut changed = ui.checkbox(&mut looks.lyric_line, "The line being sung (off shows the album)").changed();
                changed |= ui.checkbox(&mut looks.follow_button, "A button to follow the artist").changed();
                changed
            });
            changed
        });
    });
    group(ui, app, "Visualiser and playlist layout", Some("Bars measured from the sound itself, and the Playlist layout's table."), |ui| {
        let mut bars = app.settings.visualiser;
        if switch(ui, &palette, &mut bars, "Visualiser", "Bars under the cover on Now playing, measured from the sound.") {
            actions.push(Action::SetVisualiser(bars));
        }
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Drawn as", &mut looks.visualiser);
            changed |= labelled(ui, &palette, "Playlist layout columns", |ui| {
                let mut changed = false;
                for column in crate::looks::QueueColumn::ALL {
                    let mut on = looks.queue_columns.contains(&column);
                    if ui.checkbox(&mut on, column.label()).changed() {
                        changed = true;
                        if on {
                            // Back in its usual place among the others.
                            looks.queue_columns.push(column);
                            let order = |c: &crate::looks::QueueColumn| crate::looks::QueueColumn::ALL.iter().position(|x| x == c);
                            looks.queue_columns.sort_by_key(order);
                        } else {
                            looks.queue_columns.retain(|c| *c != column);
                        }
                    }
                }
                changed |= ui.checkbox(&mut looks.status_bar, "Status bar").changed();
                changed
            });
            changed
        });
    });
    group(ui, app, "Lyrics", Some("How the words are set on the now playing screen."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Size", &mut looks.lyrics_size);
            changed |= choices(ui, &palette, "Alignment", &mut looks.lyrics_alignment);
            changed |= labelled(ui, &palette, "The other lines", |ui| ui.checkbox(&mut looks.dim_lyrics, "Dim the lines not being sung").changed());
            changed
        });
    });
}

fn browsing(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::bangs::SearchMode;
    let palette = app.palette;
    group(ui, app, "Browsing", Some("Cards, the badges on them and their buttons."), |ui| {
        with_looks(app, actions, |looks| {
            let mut changed = choices(ui, &palette, "Card size", &mut looks.card_size);
            changed |= choices(ui, &palette, "Service badges", &mut looks.badge_policy);
            changed |= choices(ui, &palette, "Play and menu buttons", &mut looks.hover_controls);
            changed
        });
    });
    group(ui, app, "On Home", Some("Which rows Home shows: the greeting, pinned songs, played lately and each service's shelves."), |ui| {
        with_looks(app, actions, |looks| {
            ui.horizontal_wrapped(|ui| ticks(ui, &mut looks.hidden_home, &crate::looks::HomePart::ALL, |p| p.label(), true)).inner
        });
    });
    group(ui, app, "Search", Some("Where a search goes when no bang says, and which services a search of all of them asks."), |ui| {
        labelled(ui, &palette, "Search goes to", |ui| {
            egui::ComboBox::from_id_salt("search-mode").selected_text(app.settings.search_mode.display_name()).show_ui(ui, |ui| {
                for mode in SearchMode::OFFERED {
                    if ui.selectable_label(app.settings.search_mode == mode, mode.display_name()).clicked() {
                        actions.push(Action::SetSearchMode(mode));
                    }
                }
            });
            ui.label(RichText::new("unless a bang says otherwise: !ytm, !sc, !yt, !all…").color(palette.faint));
        });
        with_settings(app, actions, |s| {
            labelled(ui, &palette, "Everywhere (!all) asks", |ui| {
                ticks(
                    ui,
                    &mut s.hybrid_search,
                    &[
                        ProviderType::YoutubeMusic,
                        ProviderType::YoutubeVideo,
                        ProviderType::Soundcloud,
                        ProviderType::Bandcamp,
                        ProviderType::Spotify,
                        ProviderType::Vk,
                        ProviderType::Deezer,
                        ProviderType::AppleMusic,
                        ProviderType::Tidal,
                    ],
                    |p| p.display_name(),
                    false,
                )
            })
        });
    });
    group(ui, app, "In the sidebar", Some("Which pages the sidebar offers. Home and Settings always stay."), |ui| {
        with_looks(app, actions, |looks| {
            ui.horizontal_wrapped(|ui| ticks(ui, &mut looks.hidden_pages, &crate::looks::SidebarPage::ALL, |p| p.label(), true)).inner
        });
    });
}

// ---------------------------------------------------------------------------------------------------------
// Listening

fn sound(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::audio::{band_label, boost, EqualizerPreset, BANDS_HZ, MAX_GAIN_DB};
    let palette = app.palette;
    group(ui, app, "Equaliser", Some("Ten bands from the deepest bass to the air above the cymbals, shaped by mpv for every song from every service."), |ui| {
        let mut eq = app.settings.equalizer.clone();
        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut eq.enabled, "On").changed();
            ui.add_space(12.0);
            ui.add_enabled_ui(eq.enabled, |ui| {
                ui.label(RichText::new("Preset").color(palette.subtext));
                egui::ComboBox::from_id_salt("eq-preset").selected_text(eq.preset.name()).show_ui(ui, |ui| {
                    for preset in EqualizerPreset::ALL {
                        if ui.selectable_label(eq.preset == preset, preset.name()).clicked() {
                            if preset == EqualizerPreset::Custom && eq.preset != EqualizerPreset::Custom {
                                // Your own starts from the curve you were hearing.
                                eq.custom_gains = eq.gains();
                            }
                            eq.preset = preset;
                            changed = true;
                        }
                    }
                });
                if ui.button("Flat").on_hover_text("Every band back to nothing").clicked() {
                    eq.preset = EqualizerPreset::Flat;
                    eq.preamp_db = 0.0;
                    changed = true;
                }
            });
        });
        ui.add_space(8.0);
        ui.add_enabled_ui(eq.enabled, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 14.0;
                let mut gains = eq.gains();
                let mut released = false;
                ui.vertical(|ui| {
                    let slider = ui.add(egui::Slider::new(&mut eq.preamp_db, -12.0..=12.0).vertical().step_by(0.5).show_value(false));
                    released |= slider.drag_stopped() || (slider.changed() && !slider.dragged());
                    slider.on_hover_text(format!("Preamp, before the bands: {:+.1} dB", eq.preamp_db));
                    ui.label(RichText::new("Pre").font(theme::regular(11.0)).color(palette.subtext));
                });
                ui.separator();
                for (index, hz) in BANDS_HZ.iter().enumerate() {
                    ui.vertical(|ui| {
                        let slider = ui.add(egui::Slider::new(&mut gains[index], -MAX_GAIN_DB..=MAX_GAIN_DB).vertical().step_by(0.5).show_value(false));
                        if slider.changed() {
                            eq.preset = EqualizerPreset::Custom;
                            eq.custom_gains = gains;
                        }
                        released |= slider.drag_stopped() || (slider.changed() && !slider.dragged());
                        slider.on_hover_text(format!("{:+.1} dB", gains[index]));
                        ui.label(RichText::new(band_label(*hz)).font(theme::regular(11.0)).color(palette.subtext));
                    });
                }
                // Sent once a band is let go of: rewriting mpv's filters on every step of a drag stutters.
                changed |= released;
            });
        });
        if changed {
            actions.push(Action::SetEqualizer(eq));
        }
    });
    group(ui, app, "Volume booster", Some("Louder than the computer's own volume goes, up to 1000%, behind a limiter that keeps it from breaking up."), |ui| {
        ui.horizontal(|ui| {
            let mut percent = app.settings.boost_percent as f32;
            let slider = ui.add(
                egui::Slider::new(&mut percent, boost::OFF as f32..=boost::MAX as f32)
                    .step_by(boost::STEP as f64)
                    .custom_formatter(|v, _| if v as u32 <= boost::OFF { "Off".into() } else { format!("{v:.0}%") }),
            );
            if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
                actions.push(Action::SetBoost(percent as u32));
            }
            if boost::is_on(app.settings.boost_percent) && ui.button("Off").clicked() {
                actions.push(Action::SetBoost(boost::OFF));
            }
        });
        if boost::is_loud(app.settings.boost_percent) {
            ui.label(RichText::new(boost::LOUD_WARNING).color(palette.error));
        }
    });
    group(ui, app, "Loudness", None, |ui| {
        let mut even = app.settings.even_loudness;
        if switch(ui, &palette, &mut even, "Even out loudness", "Songs play at about the same volume, however loud each was made.") {
            actions.push(Action::SetEvenLoudness(even));
        }
    });
}

fn playback_queue(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    use crate::store::{AutoplaySource, HomePlay};
    let palette = app.palette;
    group(ui, app, "Speed", Some("Faster or slower without changing the pitch, from half to double."), |ui| {
        ui.horizontal(|ui| {
            let mut speed = app.settings.speed;
            let slider = ui.add(egui::Slider::new(&mut speed, 0.5..=2.0).step_by(0.05).suffix("×"));
            if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
                actions.push(Action::SetSpeed(speed));
            }
            if (app.settings.speed - 1.0).abs() > 0.001 && ui.button("1×").clicked() {
                actions.push(Action::SetSpeed(1.0));
            }
        });
    });
    group(ui, app, "Autoplay", Some("What plays when the queue runs out, and after a song picked on its own."), |ui| {
        let mut autoplay = app.settings.autoplay;
        if switch(ui, &palette, &mut autoplay, "Autoplay", "When the queue runs out, carry on with songs like the last one.") {
            actions.push(Action::SetAutoplay(autoplay));
        }
        with_settings(app, actions, |s| {
            let mut changed = labelled(ui, &palette, "A song from Home or a search", |ui| {
                let mut changed = ui.selectable_value(&mut s.home_play, HomePlay::Autoplay, "Autoplay after it").on_hover_text("The song, then songs like it, as a radio started from it").changed();
                changed |= ui.selectable_value(&mut s.home_play, HomePlay::Shelf, "The songs around it").on_hover_text("The song, then the rest of the shelf or the results it was picked from").changed();
                changed
            });
            changed |= labelled(ui, &palette, "Its songs come from", |ui| {
                let mut changed = ui.selectable_value(&mut s.autoplay_from, AutoplaySource::SameService, "The same service").changed();
                changed |= ui
                    .selectable_value(&mut s.autoplay_from, AutoplaySource::YoutubeMusic, "YouTube Music radio")
                    .on_hover_text("YouTube Music's radio after every song, whatever service it came from")
                    .changed();
                changed
            });
            ui.add_space(4.0);
            changed |= switch(ui, &palette, &mut s.autoplay_avoid_recent, "Skip songs played lately", "So the same few do not keep coming round.");
            changed
        });
    });
    group(ui, app, "The queue", Some("What happens to the queue between launches."), |ui| {
        let mut restore = app.settings.restore_queue;
        if switch(ui, &palette, &mut restore, "Keep the queue between launches", "Opens with the queue as you left it, and picks the song up where it was.") {
            actions.push(Action::SetRestoreQueue(restore));
        }
    });
    group(ui, app, "Sleep timer", Some("How the sleep timer ends the music."), |ui| {
        with_settings(app, actions, |s| {
            labelled(ui, &palette, "Fade out over", |ui| {
                let mut changed = false;
                for seconds in [0u32, 15, 30, 60] {
                    let label = if seconds == 0 { "No fade".to_string() } else { format!("{seconds} seconds") };
                    changed |= ui.selectable_value(&mut s.sleep_fade_seconds, seconds, label).changed();
                }
                changed
            })
        });
    });
    group(ui, app, "Skipping", Some("Leaving out the parts of a YouTube video that are not the music."), |ui| {
        let mut sponsor = app.settings.sponsorblock;
        if switch(ui, &palette, &mut sponsor, "SponsorBlock", "Skip intros, outros, sponsor reads and talking, as SponsorBlock's contributors mark them.") {
            actions.push(Action::SetSponsorBlock(sponsor));
        }
    });
}

fn lyrics(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Close matches", Some("For a song no source knows by its name -- uploaded by somebody else, or \"slowed + reverb\" -- the closest song that has lyrics is shown."), |ui| {
        with_settings(app, actions, |s| {
            switch(
                ui,
                &palette,
                &mut s.lyrics_close_match_warning,
                "Warn when lyrics are a close match",
                "A small note above them that they may not be the song's. Off hides the note; the close match stays.",
            )
        });
    });
    group(ui, app, "Open lyrics on", Some("Which source the lyrics open on when it has them. Every source is still asked, and you can switch above the lyrics."), |ui| {
        with_settings(app, actions, |s| {
            let mut changed = false;
            for provider in crate::lyrics::Provider::ALL {
                ui.horizontal(|ui| {
                    changed |= ui.radio_value(&mut s.lyrics_source, provider, provider.display_name()).changed();
                    match provider.key_environment() {
                        Some(key) if std::env::var(key).map(|k| k.trim().is_empty()).unwrap_or(true) => {
                            ui.label(RichText::new(format!("needs a key in {key}")).font(theme::regular(12.0)).color(palette.faint));
                        }
                        Some(_) => {
                            ui.label(RichText::new("key set").font(theme::regular(12.0)).color(palette.accent));
                        }
                        None => {}
                    }
                });
            }
            changed
        });
    });
}

// ---------------------------------------------------------------------------------------------------------
// Services

fn service_account(ui: &mut Ui, app: &App, provider: ProviderType, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Sign-in", Some(&format!("Your {}, signed in from a browser you already use.", provider.display_name())), |ui| {
        let who = match provider {
            ProviderType::Soundcloud => app.soundcloud.as_ref().map(|(_, p)| p.name.clone()),
            _ => app.youtube.as_ref().map(|_| app.youtube_name.clone().unwrap_or_else(|| "your account".into())),
        };
        labelled(ui, &palette, "Sign-ins from", |ui| {
            let current = app.settings.cookies_browser.clone();
            egui::ComboBox::from_id_salt("cookies").selected_text(current.clone().unwrap_or_else(|| "No browser".into())).show_ui(ui, |ui| {
                if ui.selectable_label(current.is_none(), "No browser").clicked() {
                    actions.push(Action::SetCookies(None));
                }
                for browser in ["firefox", "chrome", "edge", "brave", "chromium", "opera", "vivaldi", "safari"] {
                    if ui.selectable_label(current.as_deref() == Some(browser), browser).clicked() {
                        actions.push(Action::SetCookies(Some(browser.to_string())));
                    }
                }
            });
        });
        labelled(ui, &palette, provider.display_name(), |ui| match who {
            Some(name) => {
                ui.label(RichText::new(format!("Signed in as {name}")).color(palette.accent));
                if secondary(ui, &palette, "Sign out").clicked() {
                    actions.push(Action::SignOut(provider));
                }
            }
            None if app.signing_in == Some(provider) => super::widgets::loading(ui, &palette, "Reading the browser's sign-in…"),
            None => {
                let label = match &app.settings.cookies_browser {
                    Some(browser) => format!("Sign in from {browser}"),
                    None => "Sign in from your browser".to_string(),
                };
                if primary(ui, &palette, &label).clicked() {
                    actions.push(Action::SignIn(provider));
                }
            }
        });
        ui.add_space(4.0);
        ui.label(
            RichText::new("Only browsing uses it: streams are always fetched signed out, which the services refuse less. Chrome and Edge lock their cookies while they are open. Signed in, your playlists and likes are in the Library.")
                .font(theme::regular(12.0))
                .color(palette.faint),
        );
    });
    if provider == ProviderType::YoutubeMusic {
        group(ui, app, "History on YouTube Music", None, |ui| {
            with_settings(app, actions, |s| {
                switch(ui, &palette, &mut s.youtube_history, "Add what I play to my YouTube Music history", "And let its recommendations learn from it.")
            });
        });
    }
}

fn spotify(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    use crate::store::SpotifyPlayback;
    let palette = app.palette;
    group(ui, app, "Spotify", Some(pages::SPOTIFY_ABOUT), |ui| pages::spotify_section(ui, app, inputs, actions));
    group(ui, app, "Spotify's songs", Some("Spotify lets only its own players make the sound, so its songs are matched on YouTube Music, or played by Spotify itself."), |ui| {
        let mut load_devices = false;
        with_settings(app, actions, |s| {
            let mut changed = labelled(ui, &palette, "Played", |ui| {
                let mut changed = ui.selectable_value(&mut s.spotify_playback, SpotifyPlayback::Matched, "Matched and played here").changed();
                changed |= ui
                    .selectable_value(&mut s.spotify_playback, SpotifyPlayback::OnSpotify, "On Spotify (Premium)")
                    .on_hover_text("Spotify's songs play in your Spotify app, on the device you choose")
                    .changed();
                changed
            });
            if s.spotify_playback == SpotifyPlayback::OnSpotify {
                changed |= labelled(ui, &palette, "On", |ui| {
                    if crate::spotify::current().is_none() {
                        ui.label(RichText::new("sign in to Spotify above first").color(palette.faint));
                        return false;
                    }
                    let mut changed = ui.selectable_value(&mut s.spotify_device, String::new(), "Whichever is active").changed();
                    match &app.spotify_devices {
                        Some(Ok(devices)) => {
                            for (id, name, active) in devices {
                                let label = if *active { format!("{name} (active)") } else { name.clone() };
                                changed |= ui.selectable_value(&mut s.spotify_device, id.clone(), label).changed();
                            }
                        }
                        Some(Err(why)) => {
                            ui.label(RichText::new(why).color(palette.error));
                        }
                        None => {}
                    }
                    if ui.small_button("⟳ Devices").clicked() {
                        load_devices = true;
                    }
                    changed
                });
            }
            changed
        });
        if load_devices {
            actions.push(Action::LoadSpotifyDevices);
        }
    });
}

fn bandcamp(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    group(ui, app, "Bandcamp", Some(pages::BANDCAMP_ABOUT), |ui| pages::bandcamp_section(ui, app, inputs, actions));
    group(ui, app, "Genres on Home", Some("Each genre picked adds a row of its best-sellers to Home."), |ui| {
        with_settings(app, actions, |s| {
            ui.horizontal_wrapped(|ui| {
                let mut changed = false;
                for (tag, name) in crate::bandcamp::GENRES {
                    let mut on = s.bandcamp_genres.iter().any(|g| g == tag);
                    if ui.checkbox(&mut on, name).changed() {
                        changed = true;
                        if on {
                            s.bandcamp_genres.push(tag.to_string());
                        } else {
                            s.bandcamp_genres.retain(|g| g != tag);
                        }
                    }
                }
                changed
            })
            .inner
        });
    });
}

// ---------------------------------------------------------------------------------------------------------
// This device

fn startup(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Closing the window", Some("What the close button does."), |ui| {
        let mut tray = app.settings.close_to_tray;
        if switch(ui, &palette, &mut tray, "Keep playing in the tray", "Closing the window leaves Noctorium playing in the tray; quit it from the tray's menu.") {
            actions.push(Action::SetCloseToTray(tray));
        }
    });
    group(ui, app, "Start with the computer", Some("Whether Noctorium opens when you sign in to the computer, and how."), |ui| {
        ui.horizontal(|ui| {
            for mode in [crate::startup::Mode::Off, crate::startup::Mode::Window, crate::startup::Mode::Tray] {
                if ui.selectable_label(app.startup_mode == mode, mode.name()).clicked() && app.startup_mode != mode {
                    actions.push(Action::SetStartup(mode));
                }
            }
        });
    });
    group(ui, app, "Announcing tracks", None, |ui| {
        with_settings(app, actions, |s| {
            switch(ui, &palette, &mut s.announce_tracks, "Say what is playing when a song starts", "With a note from the system while the window is away.")
        });
    });
    group(ui, app, "Opening", Some("The page Noctorium opens on."), |ui| {
        ui.horizontal(|ui| {
            for page in crate::store::StartPage::ALL {
                if ui.selectable_label(app.settings.start_page == page, page.label()).clicked() && app.settings.start_page != page {
                    let mut settings = app.settings.clone();
                    settings.start_page = page;
                    actions.push(Action::SetSettings(Box::new(settings)));
                }
            }
        });
    });
}

fn shortcuts(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Media keys", None, |ui| {
        let media = if app.media_keys.is_empty() {
            if cfg!(windows) {
                "The keyboard's media keys are held by another program.".to_string()
            } else {
                "The keyboard's media keys are answered on Windows.".to_string()
            }
        } else {
            format!("Answered from any program: {}", app.media_keys.join(", "))
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(media).color(palette.subtext));
            if secondary(ui, &palette, "Show every shortcut  ?").clicked() {
                actions.push(Action::ShowShortcuts(true));
            }
        });
    });
    group(ui, app, "Shortcuts", Some("Click one and press the new keys. Esc leaves it as it was; Backspace leaves it with none."), |ui| {
        pages::shortcuts_editor(ui, app, actions)
    });
}

// ---------------------------------------------------------------------------------------------------------
// About

fn updates(ui: &mut Ui, app: &App, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Updates", Some("Whether a newer Noctorium is out, and installing it."), |ui| {
        ui.horizontal(|ui| match (&app.update_ready, &app.newer) {
            (Some(version), _) => {
                ui.label(RichText::new(format!("Noctorium {version} is installed and opens next time.")).font(theme::bold(14.0)).color(palette.accent));
                if primary(ui, &palette, "Restart now").clicked() {
                    actions.push(Action::RestartToUpdate);
                }
            }
            (None, Some(newer)) => {
                ui.label(RichText::new(format!("Noctorium {} is out.", newer.version)).font(theme::bold(14.0)).color(palette.accent));
                if ui.link("See what is new").clicked() {
                    actions.push(Action::OpenUrl(newer.url.clone()));
                }
                if app.installing_update {
                    super::widgets::loading(ui, &palette, "Downloading and checking it…");
                } else if (newer.package.is_some() || (!cfg!(windows) && newer.installer.is_some())) && primary(ui, &palette, "Install it").clicked() {
                    actions.push(Action::InstallUpdate);
                }
            }
            (None, None) => {
                ui.label(RichText::new(format!("You have Noctorium Rust {}, the newest we know of.", crate::version())).color(palette.text));
                if app.checking_update {
                    super::widgets::loading(ui, &palette, "Looking…");
                } else if secondary(ui, &palette, "Check now").clicked() {
                    actions.push(Action::CheckUpdates);
                }
            }
        });
        ui.add_space(8.0);
        with_settings(app, actions, |s| {
            let mut changed = switch(ui, &palette, &mut s.check_updates, "Look for updates", "When Noctorium starts and every six hours: one request to GitHub.");
            if cfg!(windows) {
                ui.add_enabled_ui(s.check_updates, |ui| {
                    changed |= switch(
                        ui,
                        &palette,
                        &mut s.auto_update,
                        "Install updates by themselves",
                        "Downloaded, checked against the checksum the release publishes, and put in place; the new version opens next time.",
                    );
                });
            }
            changed
        });
    });
    group(ui, app, "About Noctorium", None, |ui| {
        ui.label(format!("Noctorium Rust {}: Noctorium for the desktop, in Rust", crate::version()));
        ui.label(
            RichText::new("Noctorium is an independent third-party client. It is not affiliated with Google, YouTube, SoundCloud, Spotify, Bandcamp, VK, Apple, Deezer, TIDAL, Last.fm, ListenBrainz or Discord.")
                .color(palette.faint),
        );
        ui.label(RichText::new("GPL-3.0-only. Lyrics from LRCLIB, Better Lyrics, Karalyr, SyncLRC, lyrics.ovh, Musixmatch, Happi and Genius.").color(palette.faint));
        if ui.link("Source on GitHub").clicked() {
            actions.push(Action::OpenUrl("https://github.com/Noctorium/Noctorium-Desktop-Rust".into()));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
// You › Noctorium playlists

/// Where the playlists every Noctorium shares are kept: a folder Google Drive, OneDrive or Dropbox already syncs,
/// or the listener's Google Drive itself, signed in to. The Kotlin desktop's page, with the same choices.
fn noctorium_playlists(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    use crate::noctplaylists::{CloudKind, CloudSettings};
    let palette = app.palette;
    let cloud = app.settings.playlist_cloud.clone();
    group(
        ui,
        app,
        "Noctorium playlists",
        Some("A Noctorium playlist is on every device you use Noctorium on -- this computer, your phone, the Kotlin desktop -- kept in storage of your own. A local playlist stays on this computer. You choose which when you make one."),
        |ui| {
            let sync = &app.playlist_sync;
            let status = match (&sync.problem, cloud.kind) {
                (_, CloudKind::None) => "Not set up: Noctorium playlists stay here until you choose where to keep them.".to_string(),
                (Some(problem), _) => problem.clone(),
                _ if sync.syncing => "Syncing…".to_string(),
                _ => match sync.last_synced_ms {
                    Some(at) => format!("Kept and synced. Last synced {} seconds ago.", (crate::store::now_seconds() * 1000 - at).max(0) / 1000),
                    None => "Kept there; not synced yet.".to_string(),
                },
            };
            ui.horizontal(|ui| {
                let colour = if sync.problem.is_some() { palette.error } else { palette.subtext };
                ui.label(RichText::new(status).color(colour));
                if cloud.kind != CloudKind::None && secondary(ui, &palette, "⟳ Sync now").clicked() {
                    actions.push(Action::SyncPlaylists);
                }
            });
        },
    );
    group(ui, app, "Keep them in", None, |ui| {
        // A synced folder
        if ui.radio(cloud.kind == CloudKind::Folder, RichText::new("A synced folder").color(palette.text)).clicked() {
            actions.push(Action::SetPlaylistCloud(CloudSettings { kind: CloudKind::Folder, ..cloud.clone() }));
        }
        ui.label(RichText::new("A folder that Google Drive, OneDrive, Dropbox or Syncthing keeps in step. They go in a folder called \"Noctorium Playlists\" inside it.").font(theme::regular(12.0)).color(palette.faint));
        if cloud.kind == CloudKind::Folder || cloud.kind == CloudKind::None {
            ui.horizontal_wrapped(|ui| {
                for (name, place) in crate::noctplaylists::synced_folders() {
                    if secondary(ui, &palette, &format!("☁ {name}")).on_hover_text(place.display().to_string()).clicked() {
                        actions.push(Action::SetPlaylistCloud(CloudSettings { kind: CloudKind::Folder, folder: place.display().to_string(), ..cloud.clone() }));
                    }
                }
            });
            ui.horizontal(|ui| {
                let field = inputs.playlist_folder.get_or_insert_with(|| cloud.folder.clone());
                ui.add(egui::TextEdit::singleline(field).hint_text("The folder's path").desired_width(380.0));
                if !field.trim().is_empty() && field.trim() != cloud.folder && primary(ui, &palette, "Use it").clicked() {
                    actions.push(Action::SetPlaylistCloud(CloudSettings { kind: CloudKind::Folder, folder: field.trim().to_string(), ..cloud.clone() }));
                }
            });
        }
        ui.add_space(8.0);
        // Google Drive
        if ui.radio(cloud.kind == CloudKind::GoogleDrive, RichText::new("Google Drive").color(palette.text)).clicked() {
            actions.push(Action::SetPlaylistCloud(CloudSettings { kind: CloudKind::GoogleDrive, ..cloud.clone() }));
        }
        ui.label(RichText::new("Signed in to your Google Drive. Noctorium can see only the files it makes there, nothing else in your Drive.").font(theme::regular(12.0)).color(palette.faint));
        if cloud.kind == CloudKind::GoogleDrive {
            if app.google_signed_in() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Signed in to Google Drive").color(palette.accent));
                    if secondary(ui, &palette, "Sign out").clicked() {
                        actions.push(Action::GoogleSignOut);
                    }
                });
            } else {
                ui.label(RichText::new("Needs an OAuth client of your own, made once: in Google Cloud Console make a project, turn on the Google Drive API, and under Credentials make an OAuth client of the \"Desktop app\" type. The same one serves every Noctorium.").font(theme::regular(12.0)).color(palette.subtext));
                let (id, secret) = inputs.google_app.get_or_insert_with(|| (cloud.google_client_id.clone(), String::new()));
                labelled(ui, &palette, "Client id", |ui| ui.add(egui::TextEdit::singleline(id).desired_width(380.0)));
                labelled(ui, &palette, "Client secret", |ui| ui.add(egui::TextEdit::singleline(secret).password(true).desired_width(380.0)));
                ui.horizontal(|ui| {
                    if secondary(ui, &palette, "Save").clicked() {
                        actions.push(Action::SetGoogleApp { client_id: id.clone(), client_secret: std::mem::take(secret) });
                    }
                    if app.playlist_sync.signing_in {
                        super::widgets::loading(ui, &palette, "Finish signing in on the page that opened in your browser…");
                    } else if primary(ui, &palette, "Sign in with Google").clicked() {
                        actions.push(Action::GoogleSignIn);
                    }
                    if ui.link("Open Google Cloud Console").clicked() {
                        actions.push(Action::OpenUrl("https://console.cloud.google.com/apis/credentials".into()));
                    }
                });
            }
        }
        if cloud.kind != CloudKind::None {
            ui.add_space(8.0);
            if secondary(ui, &palette, "Stop syncing").clicked() {
                actions.push(Action::SetPlaylistCloud(CloudSettings { kind: CloudKind::None, ..cloud.clone() }));
            }
        }
    });
}

// ---------------------------------------------------------------------------------------------------------
// About › Experimental

/// The codes there are, by what is typed for them: the Kotlin players' `ExperimentalCode`. Typed in any case, with
/// spaces either side or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExperimentalCode {
    /// Closes Noctorium, as the tray's Quit does. Nothing actually crashes.
    Crash,
    /// The secret theme, kept from then on like any, until another is chosen.
    Shadow,
}

impl ExperimentalCode {
    pub fn find(typed: &str) -> Option<ExperimentalCode> {
        match typed.trim().to_lowercase().as_str() {
            "crash" => Some(ExperimentalCode::Crash),
            "sh@d0w" => Some(ExperimentalCode::Shadow),
            _ => None,
        }
    }
}

/// A box for codes, which do things not yet worth a setting of their own -- and, for now, one easter egg.
fn experimental(ui: &mut Ui, app: &App, inputs: &mut Inputs, actions: &mut Vec<Action>) {
    let palette = app.palette;
    group(ui, app, "Codes", Some("Things that are not settings yet. Type a code and press Enter."), |ui| {
        ui.horizontal(|ui| {
            let field = ui.add(egui::TextEdit::singleline(&mut inputs.code).hint_text("Code").desired_width(260.0));
            let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (primary(ui, &palette, "Enter").clicked() || enter) && !inputs.code.trim().is_empty() {
                inputs.code_answer = match ExperimentalCode::find(&inputs.code) {
                    Some(code) => {
                        actions.push(Action::Code(code));
                        None
                    }
                    None => Some("Nothing happens. Not yet, anyway.".into()),
                };
                inputs.code.clear();
            }
        });
        if let Some(answer) = &inputs.code_answer {
            ui.label(RichText::new(answer).color(palette.subtext));
        }
    });
    // Only for whoever has the secret theme on: their own picture behind it.
    if app.settings.theme == super::shadow::KEY {
        let picture = &app.settings.looks.shadow_picture;
        group(ui, app, "Your picture", Some("Put a picture of your own behind this theme: paste where it is on this computer. It is copied into Noctorium's folder and goes nowhere else."), |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut inputs.shadow_path).hint_text("Path to a PNG, JPEG or WebP").desired_width(320.0));
                let label = if picture.is_empty() { "Add a picture" } else { "Change picture" };
                if primary(ui, &palette, label).clicked() && !inputs.shadow_path.trim().is_empty() {
                    actions.push(Action::SetShadowPicture(inputs.shadow_path.clone()));
                }
                if !picture.is_empty() && ui.button("Remove").clicked() {
                    actions.push(Action::SetShadowPicture(String::new()));
                }
            });
            if let Some(answer) = &inputs.shadow_answer {
                ui.label(RichText::new(answer).color(palette.subtext));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::ExperimentalCode;

    #[test]
    fn a_code_is_found_however_it_is_typed_and_anything_else_is_none() {
        assert_eq!(ExperimentalCode::find("crash"), Some(ExperimentalCode::Crash));
        assert_eq!(ExperimentalCode::find("  CRASH "), Some(ExperimentalCode::Crash));
        assert_eq!(ExperimentalCode::find(""), None);
        assert_eq!(ExperimentalCode::find("crashes"), None);
        assert_eq!(ExperimentalCode::find("sh@d0w"), Some(ExperimentalCode::Shadow));
        assert_eq!(ExperimentalCode::find(" SH@D0W "), Some(ExperimentalCode::Shadow));
        assert_eq!(ExperimentalCode::find("shadow"), None);
    }
}
