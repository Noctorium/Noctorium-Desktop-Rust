//! How Noctorium looks beyond its theme, as Noctorium-Base's `Personalisation`, `Looks` and `Appearance` put
//! it: the accent, a theme of the listener's own, the typeface, the corners, the lyrics, the cover and what
//! is behind it, the Now playing layout, the seek bar, the player bar, the visualiser and the badges.
//! Saved under the names the Kotlin enums have, so each means the same on every player.

use serde::{Deserialize, Serialize};

/// An enum the settings offer as a row of choices: every value, and what each is called.
pub trait Choice: Copy + PartialEq + 'static {
    const ALL: &'static [Self];
    fn label(self) -> &'static str;
    fn description(self) -> &'static str {
        ""
    }
}

macro_rules! choice {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => ($label:expr, $description:expr)),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl Choice for $name {
            const ALL: &'static [Self] = &[$($name::$variant),+];
            fn label(self) -> &'static str {
                match self { $($name::$variant => $label),+ }
            }
            fn description(self) -> &'static str {
                match self { $($name::$variant => $description),+ }
            }
        }
    };
}

choice!(
    /// The accent: the theme's, one of Noctorium's, the cover's colour, or the listener's own.
    Accent {
        #[default]
        Theme => ("Theme's own", "The accent the theme comes with."),
        Violet => ("Violet", ""),
        Magenta => ("Magenta", ""),
        Ember => ("Ember", ""),
        Azure => ("Azure", ""),
        Mint => ("Mint", ""),
        Artwork => ("Match the artwork", "The colour of the cover playing."),
        Custom => ("Your own", "Any colour, by its hex code."),
    }
);

impl Accent {
    pub fn rgb(self) -> Option<u32> {
        match self {
            Accent::Violet => Some(0xB47CFF),
            Accent::Magenta => Some(0xFF6EC7),
            Accent::Ember => Some(0xFF9757),
            Accent::Azure => Some(0x5AB2FF),
            Accent::Mint => Some(0x5FE3B0),
            _ => None,
        }
    }
}

choice!(
    /// The typeface the whole window is set in: the three every computer has without a download.
    FontChoice {
        #[default]
        Default => ("Default", "The system's own typeface."),
        Serif => ("Serif", "A book face, like the notes in a record sleeve."),
        Mono => ("Monospace", "Every letter the same width, like a terminal."),
    }
);

choice!(
    CornerStyle {
        Sharp => ("Sharp", "Square corners everywhere."),
        #[default]
        Soft => ("Soft", "Gently rounded, as Noctorium has always been."),
        Round => ("Round", "Twice as round."),
    }
);

impl CornerStyle {
    pub fn scale(self) -> f32 {
        match self {
            CornerStyle::Sharp => 0.0,
            CornerStyle::Soft => 1.0,
            CornerStyle::Round => 2.0,
        }
    }
}

choice!(
    LyricsSize {
        Small => ("Small", ""),
        #[default]
        Default => ("Default", ""),
        Large => ("Large", ""),
        Huge => ("Huge", ""),
    }
);

impl LyricsSize {
    pub fn scale(self) -> f32 {
        match self {
            LyricsSize::Small => 0.85,
            LyricsSize::Default => 1.0,
            LyricsSize::Large => 1.2,
            LyricsSize::Huge => 1.45,
        }
    }
}

choice!(
    LyricsAlignment {
        #[default]
        Auto => ("As laid out", "Left beside the music, centred when the lyrics have the page."),
        Start => ("Left", ""),
        Centre => ("Centred", ""),
    }
);

choice!(
    /// How the cover is cut on Now playing.
    CoverStyle {
        #[default]
        Rounded => ("Rounded", "Softened corners, as it has always been."),
        Square => ("Square", "Sharp corners, the way the sleeve was printed."),
        Circle => ("Circle", "Cut round, like a badge."),
        Record => ("Record", "Set into a vinyl disc that turns while the music plays."),
    }
);

choice!(
    /// What is behind Now playing.
    NowPlayingBackdrop {
        #[default]
        Wash => ("Colour wash", "A glow in the colour of the cover."),
        Cover => ("Cover", "The cover itself, enlarged and dimmed behind everything."),
        Plain => ("Plain", "The theme's own background, and nothing else."),
    }
);

choice!(
    NowPlayingLayout {
        #[default]
        SideBySide => ("Side by side", "The track, the controls and the cover on the left, the lyrics or what is next beside them."),
        PanelLeft => ("Panel left", "The same, turned round: the lyrics or what is next on the left, the cover on the right."),
        Stage => ("Centre stage", "A large cover in the middle with the track and the controls beside it, and nothing else."),
        Focus => ("Focus", "The cover, the track and the controls stacked down the middle, like a record on a shelf."),
        Banner => ("Banner", "The track in one band across the top, the lyrics and what is next the full width beneath."),
        SingAlong => ("Sing along", "The lyrics large across most of the page, the cover and the controls in a column beside them."),
        Immersive => ("Full cover", "The cover fills the page, and the track and the controls sit over its foot."),
        Split => ("Split", "The cover fills the left half from top to bottom; the track, the controls and the lyrics share the right."),
        CoverFlow => ("Cover flow", "The queue as a row of covers turned towards the one playing, the track and the controls beneath."),
        Turntable => ("Turntable", "The record on a turntable whose arm crosses it as the song plays, the lyrics beside it."),
        Poster => ("Big type", "No cover: the title and the artist set large, like a poster, with the controls under them."),
        Playlist => ("Playlist", "The cover in a panel on the left, the queue as a table with columns, and the title large above it."),
    }
);

choice!(
    /// How far through the song is drawn.
    ProgressBarStyle {
        #[default]
        Minimal => ("Minimal", "A hairline track with a small dot, and the times sitting quietly at each end."),
        Material => ("Material", "The standard slider, with a larger handle and a thicker track."),
        Wave => ("Wave", "The part that has played is a wave, and it travels while the music does."),
        Segments => ("Segments", "The track in even blocks, filling one at a time."),
        Capsule => ("Capsule", "One thick rounded bar, filled from the left. No handle, nothing else."),
        Bars => ("Bars", "A row of thin bars of different heights, lit as the song plays."),
        Beads => ("Beads", "A string of dots: the ones played are filled in, and the one the song has reached is larger."),
        Neon => ("Neon", "A bright line with a glow round it and a spark at its head, and what is still to come in faint dashes."),
        Ruler => ("Ruler", "A tick every few seconds and a longer one each minute, with a pointer riding above the song's place."),
        Classic => ("Classic", "The old Windows kind: a sunken well filling with square blocks, and a raised slab for a handle."),
        Luna => ("Luna", "Windows XP's: green blocks filling a rounded white well, and its pointed handle."),
        Slider => ("Slider", "A thin groove that fills with the accent as the song plays, and a small round handle to drag."),
    }
);

choice!(
    PlayerBarStyle {
        #[default]
        Inline => ("Inline", "One row: the controls on the left, then the track, the seek bar and the tools."),
        Stacked => ("Stacked", "The seek bar across the top, the track on the left and the controls centred beneath."),
        Centered => ("Centred", "The track on the left, the controls in the middle with the seek bar under them, the tools on the right."),
        Slim => ("Slim", "One short row with every control and a hairline of progress, so the page gets the most room."),
        SlimLeft => ("Slim left", "Slim with the controls on the left and the track after them."),
        Spotlight => ("Spotlight", "A taller bar with a large cover, tinted with the colour of the artwork."),
        Floating => ("Floating", "The bar lifted off the edge of the window and rounded, with a gap all round it, like a dock."),
        Island => ("Island", "Only a small pill in the middle: the cover, the track, previous, play and next, the progress along its edge."),
        Display => ("Display", "The track in a display at the centre, like a stereo's, the controls to its left and the volume to its right."),
        Taskbar => ("Taskbar", "A desktop's taskbar: a start button that opens Now playing, the song as a pressed button, a tray with the volume and the clock."),
        Toolbar => ("Toolbar", "One row of flat buttons: add to a playlist, previous, play, pause, stop and next, a slider and the time, repeat, shuffle and the volume."),
        #[serde(rename = "TOOLBAR_2")]
        Toolbar2 => ("Toolbar 2", "The Toolbar's flat buttons with play and pause as one, and more: like, lyrics, the queue, repeat, shuffle, the sleep timer, Connect, the speed and the volume."),
    }
);

choice!(
    PlayerBarPosition {
        #[default]
        Bottom => ("Bottom", "Along the foot of the window, under whatever you are browsing."),
        Top => ("Top", "Across the head of the window, above whatever you are browsing."),
    }
);

choice!(
    /// How the controls' symbols are drawn.
    IconSet {
        #[default]
        Standard => ("Standard", "The symbols this player has always shown."),
        Rounded => ("Rounded", "Solid shapes with softened corners and round ends."),
        Outlined => ("Outlined", "The same shapes drawn as outlines: lighter on the eye."),
        Sharp => ("Sharp", "Square corners and straight ends, for a crisper look."),
        Filled => ("Filled", "Solid shapes, bold at a glance."),
        TwoTone => ("Two-tone", "An outline over a faint fill of the same colour."),
        Line => ("Line", "Thin single strokes in one colour, like a desktop's own icon theme."),
    }
);

choice!(
    SurfaceStyle {
        #[default]
        Solid => ("Solid", "Panels and cards in the theme's own colours."),
        Glass => ("Liquid glass", "The player and the rail float as glass over whatever is beneath them."),
    }
);

choice!(
    VisualiserStyle {
        #[default]
        Bars => ("Bars", "Ten bars, an octave each."),
        LevelMeter => ("Level meter", "Left and right, as a mixing desk shows them."),
    }
);

choice!(
    /// How a song's service is shown beside it.
    ServiceBadgeStyle {
        #[default]
        Marks => ("Marks", "Each service's own mark."),
        Letters => ("Letters", "The service's short name: YTM, SC, SP."),
    }
);

choice!(
    TextSize {
        Small => ("Small", ""),
        #[default]
        Default => ("Default", ""),
        Large => ("Large", ""),
        Largest => ("Largest", ""),
    }
);

impl TextSize {
    pub fn scale(self) -> f32 {
        match self {
            TextSize::Small => 0.9,
            TextSize::Default => 1.0,
            TextSize::Large => 1.15,
            TextSize::Largest => 1.3,
        }
    }
}

choice!(
    /// How big the covers on Home and in the library are.
    CardSize {
        Compact => ("Compact", ""),
        #[default]
        Comfortable => ("Comfortable", ""),
        Large => ("Large", ""),
    }
);

impl CardSize {
    pub fn width(self) -> f32 {
        match self {
            CardSize::Compact => 140.0,
            CardSize::Comfortable => 172.0,
            CardSize::Large => 208.0,
        }
    }
}

choice!(
    /// When a song or a card names the service it comes from.
    BadgePolicy {
        #[default]
        Auto => ("Only when mixed", "Shown when a row holds more than one service."),
        Always => ("Always", "Every card names its service."),
        Never => ("Never", "No service badges anywhere."),
    }
);

choice!(
    HoverControls {
        #[default]
        OnHover => ("On hover", "Play and menu buttons appear when you point at something."),
        Always => ("Always visible", "Buttons stay put, easier to find and to hit."),
    }
);

choice!(
    /// The time on the right of the seek bar.
    TimeDisplay {
        #[default]
        Total => ("Total length", ""),
        Remaining => ("Time remaining", ""),
    }
);

choice!(
    CoverSize {
        Smaller => ("Smaller", ""),
        #[default]
        Standard => ("Standard", ""),
        Larger => ("Larger", ""),
    }
);

impl CoverSize {
    pub fn scale(self) -> f32 {
        match self {
            CoverSize::Smaller => 0.8,
            CoverSize::Standard => 1.0,
            CoverSize::Larger => 1.18,
        }
    }
}

choice!(
    PanelWidth {
        Narrow => ("Narrow", ""),
        #[default]
        Normal => ("Normal", ""),
        Wide => ("Wide", ""),
    }
);

impl PanelWidth {
    pub fn width(self) -> f32 {
        match self {
            PanelWidth::Narrow => 360.0,
            PanelWidth::Normal => 430.0,
            PanelWidth::Wide => 520.0,
        }
    }
}

choice!(
    /// What the panel beside the cover opens on.
    NowPlayingTab {
        #[default]
        UpNext => ("Up next", ""),
        Lyrics => ("Lyrics", ""),
        Related => ("Related", ""),
    }
);

/// The buttons the player bar can carry; Stop only when asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlayerButton {
    Shuffle,
    Repeat,
    Like,
    Lyrics,
    Queue,
    SleepTimer,
    Volume,
    Speed,
    Devices,
    Stop,
}

impl PlayerButton {
    pub const ALL: [PlayerButton; 10] = [
        PlayerButton::Shuffle,
        PlayerButton::Repeat,
        PlayerButton::Like,
        PlayerButton::Lyrics,
        PlayerButton::Queue,
        PlayerButton::SleepTimer,
        PlayerButton::Volume,
        PlayerButton::Speed,
        PlayerButton::Devices,
        PlayerButton::Stop,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PlayerButton::Shuffle => "Shuffle",
            PlayerButton::Repeat => "Repeat",
            PlayerButton::Like => "Like",
            PlayerButton::Lyrics => "Lyrics",
            PlayerButton::Queue => "Queue",
            PlayerButton::SleepTimer => "Sleep timer",
            PlayerButton::Volume => "Volume",
            PlayerButton::Speed => "Speed",
            PlayerButton::Devices => "Devices and Connect",
            PlayerButton::Stop => "Stop",
        }
    }
}

/// The Playlist layout's table columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueColumn {
    Number,
    Title,
    Artist,
    Album,
    Duration,
    Service,
}

impl QueueColumn {
    pub const ALL: [QueueColumn; 6] =
        [QueueColumn::Number, QueueColumn::Title, QueueColumn::Artist, QueueColumn::Album, QueueColumn::Duration, QueueColumn::Service];

    pub fn heading(self) -> &'static str {
        match self {
            QueueColumn::Number => "#",
            QueueColumn::Title => "Title",
            QueueColumn::Artist => "Artist",
            QueueColumn::Album => "Album",
            QueueColumn::Duration => "Duration",
            QueueColumn::Service => "Service",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            QueueColumn::Number => "Number",
            other => other.heading(),
        }
    }
}

/// The rows Home can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HomePart {
    Greeting,
    Pinned,
    Recent,
    YoutubeMusic,
    Soundcloud,
    Bandcamp,
    Spotify,
    Vk,
    Deezer,
    Tidal,
    AppleMusic,
}

impl HomePart {
    pub const ALL: [HomePart; 11] = [
        HomePart::Greeting,
        HomePart::Pinned,
        HomePart::Recent,
        HomePart::YoutubeMusic,
        HomePart::Soundcloud,
        HomePart::Bandcamp,
        HomePart::Spotify,
        HomePart::Vk,
        HomePart::Deezer,
        HomePart::Tidal,
        HomePart::AppleMusic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HomePart::Greeting => "Greeting",
            HomePart::Pinned => "Pinned",
            HomePart::Recent => "Played lately",
            HomePart::YoutubeMusic => "From YouTube Music",
            HomePart::Soundcloud => "From SoundCloud",
            HomePart::Bandcamp => "From Bandcamp",
            HomePart::Spotify => "From Spotify",
            HomePart::Vk => "From VK Music",
            HomePart::Deezer => "From Deezer",
            HomePart::Tidal => "From Tidal",
            HomePart::AppleMusic => "From Apple Music",
        }
    }

    /// The part a service's rows belong to.
    pub fn of(provider: crate::domain::ProviderType) -> Option<HomePart> {
        use crate::domain::ProviderType as P;
        Some(match provider {
            P::YoutubeMusic | P::YoutubeVideo => HomePart::YoutubeMusic,
            P::Soundcloud => HomePart::Soundcloud,
            P::Bandcamp => HomePart::Bandcamp,
            P::Spotify => HomePart::Spotify,
            P::Vk => HomePart::Vk,
            P::Deezer => HomePart::Deezer,
            P::Tidal => HomePart::Tidal,
            P::AppleMusic => HomePart::AppleMusic,
            P::Local | P::Radio => return None,
        })
    }
}

/// The pages the sidebar can leave out. Home and Settings always stay.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SidebarPage {
    NowPlaying,
    Search,
    Library,
    Queue,
    Services,
    Connect,
    Downloads,
    Profile,
}

impl SidebarPage {
    pub const ALL: [SidebarPage; 8] = [
        SidebarPage::NowPlaying,
        SidebarPage::Search,
        SidebarPage::Library,
        SidebarPage::Queue,
        SidebarPage::Services,
        SidebarPage::Connect,
        SidebarPage::Downloads,
        SidebarPage::Profile,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SidebarPage::NowPlaying => "Now playing",
            SidebarPage::Search => "Search",
            SidebarPage::Library => "Library",
            SidebarPage::Queue => "Queue",
            SidebarPage::Services => "Services",
            SidebarPage::Connect => "Connect",
            SidebarPage::Downloads => "Downloads",
            SidebarPage::Profile => "Profile",
        }
    }
}

/// A theme of the listener's own: the six colours every theme is made of, as hex codes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomTheme {
    pub background: String,
    pub panel: String,
    pub card: String,
    pub text: String,
    pub subtext: String,
    pub accent: String,
    pub light: bool,
}

impl Default for CustomTheme {
    /// Starts as Night, to be changed from there.
    fn default() -> Self {
        CustomTheme {
            background: "#000000".into(),
            panel: "#07050A".into(),
            card: "#15101C".into(),
            text: "#F8F4FF".into(),
            subtext: "#CFC5DA".into(),
            accent: "#B47CFF".into(),
            light: false,
        }
    }
}

impl CustomTheme {
    /// The colours, each one that does not read as a colour taken from Night.
    pub fn colours(&self) -> crate::theme::Colours {
        let night = CustomTheme::default();
        let pick = |mine: &str, fallback: &str| parse_hex(mine).or_else(|| parse_hex(fallback)).unwrap_or(0);
        crate::theme::Colours {
            background: pick(&self.background, &night.background),
            panel: pick(&self.panel, &night.panel),
            card: pick(&self.card, &night.card),
            text: pick(&self.text, &night.text),
            subtext: pick(&self.subtext, &night.subtext),
            accent: pick(&self.accent, &night.accent),
            light: self.light,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Looks {
    pub accent: Accent,
    pub custom_accent: String,
    pub custom_theme: CustomTheme,
    pub font: FontChoice,
    pub corners: CornerStyle,
    pub lyrics_size: LyricsSize,
    pub lyrics_alignment: LyricsAlignment,
    /// Dims the lines not being sung; off keeps every line at full strength.
    pub dim_lyrics: bool,
    pub cover: CoverStyle,
    pub backdrop: NowPlayingBackdrop,
    pub now_playing: NowPlayingLayout,
    pub progress: ProgressBarStyle,
    pub player_bar: PlayerBarStyle,
    pub player_bar_position: PlayerBarPosition,
    pub icons: IconSet,
    pub surface: SurfaceStyle,
    pub visualiser: VisualiserStyle,
    pub badges: ServiceBadgeStyle,
    /// Pages fading in, a new song's name rising into place, the light easing; off makes every change instant.
    pub animations: bool,
    pub text_size: TextSize,
    pub card_size: CardSize,
    pub badge_policy: BadgePolicy,
    pub hover_controls: HoverControls,
    pub time_display: TimeDisplay,
    /// The time in the Taskbar layout's tray.
    pub taskbar_clock: bool,
    pub player_buttons: Vec<PlayerButton>,
    pub cover_size: CoverSize,
    pub panel_width: PanelWidth,
    pub open_on: NowPlayingTab,
    /// The line being sung, under the cover; off shows the album there instead.
    pub lyric_line: bool,
    /// The button for following the artist, under their name on Now playing.
    pub follow_button: bool,
    pub queue_columns: Vec<QueueColumn>,
    /// The Playlist layout's status bar: the stream's format, and how far into the whole queue.
    pub status_bar: bool,
    pub hidden_home: Vec<HomePart>,
    pub hidden_pages: Vec<SidebarPage>,
    /// The listener's own picture behind the secret Shadow theme, as this computer's copy of it; empty for none.
    pub shadow_picture: String,
}

impl Default for Looks {
    fn default() -> Self {
        Looks {
            accent: Accent::default(),
            custom_accent: String::new(),
            custom_theme: CustomTheme::default(),
            font: FontChoice::default(),
            corners: CornerStyle::default(),
            lyrics_size: LyricsSize::default(),
            lyrics_alignment: LyricsAlignment::default(),
            dim_lyrics: true,
            cover: CoverStyle::default(),
            backdrop: NowPlayingBackdrop::default(),
            now_playing: NowPlayingLayout::default(),
            progress: ProgressBarStyle::default(),
            player_bar: PlayerBarStyle::default(),
            player_bar_position: PlayerBarPosition::default(),
            icons: IconSet::default(),
            surface: SurfaceStyle::default(),
            visualiser: VisualiserStyle::default(),
            badges: ServiceBadgeStyle::default(),
            animations: true,
            text_size: TextSize::default(),
            card_size: CardSize::default(),
            badge_policy: BadgePolicy::default(),
            hover_controls: HoverControls::default(),
            time_display: TimeDisplay::default(),
            taskbar_clock: true,
            player_buttons: PlayerButton::ALL.iter().copied().filter(|b| *b != PlayerButton::Stop).collect(),
            cover_size: CoverSize::default(),
            panel_width: PanelWidth::default(),
            open_on: NowPlayingTab::default(),
            lyric_line: true,
            follow_button: true,
            queue_columns: vec![QueueColumn::Number, QueueColumn::Title, QueueColumn::Artist, QueueColumn::Album, QueueColumn::Duration],
            status_bar: true,
            hidden_home: vec![],
            hidden_pages: vec![],
            shadow_picture: String::new(),
        }
    }
}

impl Looks {
    pub fn shows(&self, button: PlayerButton) -> bool {
        self.player_buttons.contains(&button)
    }

    pub fn shows_page(&self, page: SidebarPage) -> bool {
        !self.hidden_pages.contains(&page)
    }

    pub fn shows_home(&self, part: HomePart) -> bool {
        !self.hidden_home.contains(&part)
    }
}

impl Looks {
    /// The accent in force, given the theme's own and the cover's colour if one was found.
    pub fn accent_rgb(&self, theme_accent: u32, artwork: Option<u32>) -> u32 {
        match self.accent {
            Accent::Theme => theme_accent,
            Accent::Artwork => artwork.unwrap_or(theme_accent),
            Accent::Custom => parse_hex(&self.custom_accent).unwrap_or(theme_accent),
            named => named.rgb().unwrap_or(theme_accent),
        }
    }

    /// Whether anything needs the colour of the cover playing.
    pub fn wants_artwork_colour(&self) -> bool {
        self.accent == Accent::Artwork || self.backdrop == NowPlayingBackdrop::Wash
    }
}

/// "#RRGGBB" or "RRGGBB", also with an alpha in front, as an RGB value; None for anything else. Forgiving
/// about the hash and the case, strict about the length, as Kotlin's `parseHexColour` is.
pub fn parse_hex(text: &str) -> Option<u32> {
    let clean = text.trim().trim_start_matches('#');
    let rgb = match clean.len() {
        6 => clean,
        8 => &clean[2..],
        _ => return None,
    };
    if !clean.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(rgb, 16).ok()
}

/// The contrast between two colours as WCAG counts it: 1 for the same colour, 21 for black on white.
pub fn contrast_ratio(a: u32, b: u32) -> f32 {
    fn luminance(rgb: u32) -> f32 {
        let channel = |shift: u32| {
            let c = ((rgb >> shift) & 0xFF) as f32 / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

pub fn to_hex(rgb: u32) -> String {
    format!("#{:06X}", rgb & 0xFF_FFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colours_are_read_as_kotlin_reads_them() {
        assert_eq!(parse_hex("#b47cff"), Some(0xB47CFF));
        assert_eq!(parse_hex("FFB47CFF"), Some(0xB47CFF));
        assert_eq!(parse_hex(" B47CF "), None);
        assert_eq!(parse_hex("#GGGGGG"), None);
        assert_eq!(to_hex(0xB47CFF), "#B47CFF");
        assert!((contrast_ratio(0x000000, 0xFFFFFF) - 21.0).abs() < 0.01);
        assert!((contrast_ratio(0x777777, 0x777777) - 1.0).abs() < 0.01);
    }

    #[test]
    fn the_accent_follows_the_choice() {
        let mut looks = Looks::default();
        assert_eq!(looks.accent_rgb(0x123456, Some(0xABCDEF)), 0x123456);
        looks.accent = Accent::Artwork;
        assert_eq!(looks.accent_rgb(0x123456, Some(0xABCDEF)), 0xABCDEF);
        assert_eq!(looks.accent_rgb(0x123456, None), 0x123456);
        looks.accent = Accent::Mint;
        assert_eq!(looks.accent_rgb(0x123456, None), 0x5FE3B0);
        looks.accent = Accent::Custom;
        looks.custom_accent = "nonsense".into();
        assert_eq!(looks.accent_rgb(0x123456, None), 0x123456);
    }

    #[test]
    fn looks_are_saved_under_kotlins_names() {
        let looks = Looks { now_playing: NowPlayingLayout::SingAlong, progress: ProgressBarStyle::Wave, ..Looks::default() };
        let json = serde_json::to_value(&looks).unwrap();
        assert_eq!(json["nowPlaying"], "SING_ALONG");
        assert_eq!(json["progress"], "WAVE");
        let toolbar: Looks = serde_json::from_str(r#"{"playerBar":"TOOLBAR_2","icons":"TWO_TONE"}"#).unwrap();
        assert_eq!(toolbar.player_bar, PlayerBarStyle::Toolbar2);
        assert_eq!(toolbar.icons, IconSet::TwoTone);
        let back: Looks = serde_json::from_str(r#"{"cover":"RECORD","fromTheFuture":true}"#).unwrap();
        assert_eq!(back.cover, CoverStyle::Record);
        assert_eq!(back.font, FontChoice::Default);
    }
}
