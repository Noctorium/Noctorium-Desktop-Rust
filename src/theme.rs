//! The themes: Noctorium-Base's presets, colour for colour, and how egui is dressed in one.
//!
//! A theme is six colours and a switch -- where the page sits, a panel against it, a card against that,
//! writing on all three, and one accent -- and everything else is worked out from them. The Glow themes add
//! a light laid across the page: Afterglow's in the colour of the cover playing.

use crate::looks::{CornerStyle, FontChoice, Looks};
use eframe::egui::{self, Color32, CornerRadius, Stroke, Visuals};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colours {
    pub background: u32,
    pub panel: u32,
    pub card: u32,
    pub text: u32,
    pub subtext: u32,
    pub accent: u32,
    pub light: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlowPlacement {
    /// Rising from the cover in the corner, and behind the now playing view.
    Artwork,
    /// Across the top of the page, gone before the middle.
    Top,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    pub colour: u32,
    pub follows_artwork: bool,
    pub placement: GlowPlacement,
    pub strength: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemePreset {
    /// The name settings save it under, Kotlin's enum name.
    pub key: &'static str,
    pub name: &'static str,
    pub family: &'static str,
    pub colours: Colours,
    pub glow: Option<Glow>,
}

const fn c(background: u32, panel: u32, card: u32, text: u32, subtext: u32, accent: u32, light: bool) -> Colours {
    Colours { background, panel, card, text, subtext, accent, light }
}

const fn plain(key: &'static str, name: &'static str, family: &'static str, colours: Colours) -> ThemePreset {
    ThemePreset { key, name, family, colours, glow: None }
}

pub const PRESETS: &[ThemePreset] = &[
    plain("NOCTORIUM_NIGHT", "Night", "Noctorium", c(0x000000, 0x07050A, 0x15101C, 0xF8F4FF, 0xCFC5DA, 0xB47CFF, false)),
    plain("NOCTORIUM_DUSK", "Dusk", "Noctorium", c(0x0D0B12, 0x141019, 0x1D1826, 0xF3F1F8, 0xC9C2D6, 0xB47CFF, false)),
    plain("NOCTORIUM_DAY", "Day", "Noctorium", c(0xFAF7FF, 0xFFFFFF, 0xECE6F6, 0x1A1425, 0x5B5470, 0x7C3AED, true)),
    plain("CRIMSON", "Crimson", "Crimson", c(0x000000, 0x080203, 0x170609, 0xFFF1F3, 0xD6B4BA, 0xE0243F, false)),
    plain("CRIMSON_SCARLET", "Scarlet", "Crimson", c(0x000000, 0x110404, 0x230A0B, 0xFFF4F1, 0xE0BBB5, 0xFF3B36, false)),
    plain("CRIMSON_GARNET", "Garnet", "Crimson", c(0x000000, 0x0F0307, 0x2A0B15, 0xFAEAEE, 0xCFA7B1, 0xC81E45, false)),
    plain("CATPPUCCIN_LATTE", "Latte", "Catppuccin", c(0xEFF1F5, 0xE6E9EF, 0xCCD0DA, 0x4C4F69, 0x6C6F85, 0x8839EF, true)),
    plain("CATPPUCCIN_FRAPPE", "Frappé", "Catppuccin", c(0x303446, 0x292C3C, 0x414559, 0xC6D0F5, 0xA5ADCE, 0xCA9EE6, false)),
    plain("CATPPUCCIN_MACCHIATO", "Macchiato", "Catppuccin", c(0x24273A, 0x1E2030, 0x363A4F, 0xCAD3F5, 0xA5ADCB, 0xC6A0F6, false)),
    plain("CATPPUCCIN_MOCHA", "Mocha", "Catppuccin", c(0x1E1E2E, 0x181825, 0x313244, 0xCDD6F4, 0xA6ADC8, 0xCBA6F7, false)),
    plain("NORD", "Nord", "Nord", c(0x2E3440, 0x3B4252, 0x434C5E, 0xECEFF4, 0xD8DEE9, 0x88C0D0, false)),
    plain("DRACULA", "Dracula", "Dracula", c(0x282A36, 0x21222C, 0x44475A, 0xF8F8F2, 0xBFBFBF, 0xBD93F9, false)),
    plain("GRUVBOX", "Gruvbox", "Gruvbox", c(0x282828, 0x1D2021, 0x3C3836, 0xEBDBB2, 0xA89984, 0xFE8019, false)),
    plain("ROSE_PINE", "Rosé Pine", "Rosé Pine", c(0x191724, 0x1F1D2E, 0x26233A, 0xE0DEF4, 0x908CAA, 0xC4A7E7, false)),
    plain("TOKYO_NIGHT", "Tokyo Night", "Tokyo Night", c(0x1A1B26, 0x16161E, 0x24283B, 0xC0CAF5, 0xA9B1D6, 0x7AA2F7, false)),
    plain("SOLARIZED_DARK", "Solarized Dark", "Solarized", c(0x002B36, 0x073642, 0x0D3D49, 0xEEE8D5, 0x93A1A1, 0x268BD2, false)),
    plain("SOLARIZED_LIGHT", "Solarized Light", "Solarized", c(0xFDF6E3, 0xEEE8D5, 0xE7E0C9, 0x073642, 0x586E75, 0x268BD2, true)),
    ThemePreset {
        key: "AFTERGLOW",
        name: "Afterglow",
        family: "Glow",
        colours: c(0x0B0A0A, 0x121111, 0x1A1A1A, 0xF6F2F1, 0xA8A2A0, 0xFFF7F4, false),
        glow: Some(Glow { colour: 0xC41E0C, follows_artwork: true, placement: GlowPlacement::Artwork, strength: 0.34 }),
    },
    ThemePreset {
        key: "CINDER",
        name: "Cinder",
        family: "Glow",
        colours: c(0x0B0A0A, 0x121111, 0x1A1A1A, 0xF6F2F1, 0xA8A2A0, 0xD93826, false),
        glow: Some(Glow { colour: 0xC41E0C, follows_artwork: false, placement: GlowPlacement::Artwork, strength: 0.28 }),
    },
    ThemePreset {
        key: "VELVET",
        name: "Velvet",
        family: "Glow",
        colours: c(0x0A0A0A, 0x0F0F0F, 0x141414, 0xF7F7F7, 0xA7A7A7, 0xFFFFFF, false),
        glow: Some(Glow { colour: 0x7A2450, follows_artwork: false, placement: GlowPlacement::Top, strength: 0.2 }),
    },
    ThemePreset {
        key: "SMOKE",
        name: "Smoke",
        family: "Glow",
        colours: c(0x0A0A0A, 0x0F0F0F, 0x141414, 0xF7F7F7, 0xA7A7A7, 0xFFFFFF, false),
        glow: Some(Glow { colour: 0x8C8C8C, follows_artwork: false, placement: GlowPlacement::Top, strength: 0.1 }),
    },
    // The Windows themes' colours. Their bevels and title bars are the Compose desktop's skins, not drawn here.
    plain("WINDOWS_98", "98", "Windows", c(0xC0C0C0, 0xB4B4B4, 0xFFFFFF, 0x000000, 0x3A3A3A, 0x000080, true)),
    plain("WINDOWS_98_NOCTORIUM", "Noctorium 98", "Windows", c(0x231B2E, 0x1B1524, 0x0B0810, 0xF8F4FF, 0xCFC5DA, 0xB47CFF, false)),
    plain("WINDOWS_XP", "XP", "Windows", c(0xECE9D8, 0xD6DFF7, 0xFFFFFF, 0x000000, 0x4D4D4D, 0x245EDC, true)),
];

/// The theme made of the listener's own colours, saved as `CUSTOM` as Kotlin saves it.
pub const CUSTOM: &str = "CUSTOM";

pub fn preset(key: &str) -> &'static ThemePreset {
    PRESETS.iter().find(|p| p.key == key).unwrap_or(&PRESETS[0])
}

/// The colours in force: the preset's, or the listener's own for `CUSTOM`.
pub fn colours_of(key: &str, looks: &Looks) -> Colours {
    if key == CUSTOM { looks.custom_theme.colours() } else { preset(key).colours }
}

/// The palette in force: the theme's colours with the accent the listener chose, the cover's colour
/// standing in for it when the accent follows the artwork.
pub fn palette_for(key: &str, looks: &Looks, artwork: Option<Color32>) -> Palette {
    let mut colours = colours_of(key, looks);
    let artwork = artwork.map(|c| u32::from_be_bytes([0, c.r(), c.g(), c.b()]));
    colours.accent = looks.accent_rgb(colours.accent, artwork);
    Palette::from_colours(colours)
}

/// How a theme is drawn beyond its colours: the Windows themes are bevels and title bars too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skin {
    Standard,
    Windows98,
    Windows98Noctorium,
    WindowsXp,
}

pub fn skin_of(key: &str) -> Skin {
    match key {
        "WINDOWS_98" => Skin::Windows98,
        "WINDOWS_98_NOCTORIUM" => Skin::Windows98Noctorium,
        "WINDOWS_XP" => Skin::WindowsXp,
        _ => Skin::Standard,
    }
}

/// Windows 98's system colours, as its Windows Standard scheme set them, or the same look at night for
/// Noctorium 98: the roles every bevel and title bar of the time was drawn from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Windows98 {
    pub face: u32,
    pub highlight: u32,
    pub light: u32,
    pub shadow: u32,
    pub dark_shadow: u32,
    pub window: u32,
    pub text: u32,
    pub title: u32,
    pub title_end: u32,
    pub title_text: u32,
    pub desktop: u32,
}

pub const WINDOWS_98: Windows98 = Windows98 {
    face: 0xC0C0C0,
    highlight: 0xFFFFFF,
    light: 0xDFDFDF,
    shadow: 0x808080,
    dark_shadow: 0x000000,
    window: 0xFFFFFF,
    text: 0x000000,
    title: 0x000080,
    title_end: 0x1084D0,
    title_text: 0xFFFFFF,
    desktop: 0x008080,
};

pub const WINDOWS_98_NOCTORIUM: Windows98 = Windows98 {
    face: 0x231B2E,
    highlight: 0x8F7BB8,
    light: 0x45395A,
    shadow: 0x120D18,
    dark_shadow: 0x000000,
    window: 0x0B0810,
    text: 0xF8F4FF,
    title: 0x2B0E5C,
    title_end: 0x8B5CF6,
    title_text: 0xFFFFFF,
    desktop: 0x140A26,
};

/// Luna, XP's look in its default blue: the stops of its title bar and the edges of its buttons.
pub mod xp {
    pub const TITLE_TOP: u32 = 0x0997FF;
    pub const TITLE: u32 = 0x0053EE;
    pub const TITLE_FOOT: u32 = 0x003DD7;
    pub const TITLE_TEXT: u32 = 0xFFFFFF;
    pub const BUTTON_EDGE: u32 = 0x003C74;
    pub const BUTTON_FOOT: u32 = 0xD6D0C5;
    pub const HOT: u32 = 0xF8B636;
    pub const SELECTION: u32 = 0x316AC5;
    pub const CLOSE: u32 = 0xE0532F;
    pub const CAPTION_BUTTON: u32 = 0x2A6CF0;
}

impl Skin {
    pub fn windows98(self) -> Option<&'static Windows98> {
        match self {
            Skin::Windows98 => Some(&WINDOWS_98),
            Skin::Windows98Noctorium => Some(&WINDOWS_98_NOCTORIUM),
            _ => None,
        }
    }
}

static CORNER_SCALE: AtomicU8 = AtomicU8::new(2);
static SKIN: AtomicU8 = AtomicU8::new(0);

/// A corner radius as the listener's corner style has it: none for Sharp, doubled for Round, and square
/// under the 98 skin whatever was chosen.
pub fn corner(radius: u8) -> u8 {
    let halves = CORNER_SCALE.load(Ordering::Relaxed) as u16;
    ((radius as u16 * halves) / 2).min(255) as u8
}

/// The skin in force, as last applied.
pub fn skin() -> Skin {
    match SKIN.load(Ordering::Relaxed) {
        1 => Skin::Windows98,
        2 => Skin::Windows98Noctorium,
        3 => Skin::WindowsXp,
        _ => Skin::Standard,
    }
}

/// A 98 bevel round `rect`: lit along the top and the left and shadowed along the bottom and the right when
/// raised, the other way round when sunken -- two lines each side, outermost first.
pub fn bevel(painter: &egui::Painter, rect: egui::Rect, raised: bool, colours: &Windows98) {
    let (outer_lit, inner_lit, outer_dark, inner_dark) = if raised {
        (colours.highlight, colours.light, colours.dark_shadow, colours.shadow)
    } else {
        (colours.shadow, colours.dark_shadow, colours.highlight, colours.light)
    };
    let r = rect.shrink(0.5);
    let line = |a: egui::Pos2, b: egui::Pos2, c: u32| painter.line_segment([a, b], Stroke::new(1.0_f32, rgb(c)));
    line(r.left_bottom(), r.left_top(), outer_lit);
    line(r.left_top(), r.right_top(), outer_lit);
    line(r.right_top(), r.right_bottom(), outer_dark);
    line(r.left_bottom(), r.right_bottom(), outer_dark);
    let r = r.shrink(1.0);
    line(r.left_bottom(), r.left_top(), inner_lit);
    line(r.left_top(), r.right_top(), inner_lit);
    line(r.right_top(), r.right_bottom(), inner_dark);
    line(r.left_bottom(), r.right_bottom(), inner_dark);
}

/// A rectangle shaded from one colour to another, left to right or top to bottom.
pub fn gradient(painter: &egui::Painter, rect: egui::Rect, from: Color32, to: Color32, across: bool) {
    let mut mesh = egui::Mesh::default();
    let (a, b, c, d) = if across { (from, to, to, from) } else { (from, from, to, to) };
    mesh.colored_vertex(rect.left_top(), a);
    mesh.colored_vertex(rect.right_top(), b);
    mesh.colored_vertex(rect.right_bottom(), c);
    mesh.colored_vertex(rect.left_bottom(), d);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

pub fn rgb(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// The colours in force, worked out once per frame from the preset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub page: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub raised: Color32,
    pub line: Color32,
    pub text: Color32,
    pub subtext: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub error: Color32,
    pub light: bool,
}

impl Palette {
    pub fn of(preset: &ThemePreset) -> Palette {
        Palette::from_colours(preset.colours)
    }

    pub fn from_colours(c: Colours) -> Palette {
        let (page, panel, card, text, subtext, accent) =
            (rgb(c.background), rgb(c.panel), rgb(c.card), rgb(c.text), rgb(c.subtext), rgb(c.accent));
        Palette {
            page,
            panel,
            card,
            raised: mix(card, text, 0.07),
            line: mix(card, text, 0.12),
            text,
            subtext,
            faint: mix(subtext, page, 0.35),
            accent,
            on_accent: on(accent),
            error: if c.light { Color32::from_rgb(0xC0, 0x1C, 0x3A) } else { Color32::from_rgb(0xFF, 0x7A, 0x8A) },
            light: c.light,
        }
    }

    /// The accent at a fraction of its strength, for washes behind things.
    pub fn wash(&self, alpha: f32) -> Color32 {
        self.accent.gamma_multiply(alpha)
    }
}

/// Writing that reads on `colour`: dark on a pale colour, white on a deep one.
pub fn on(colour: Color32) -> Color32 {
    let luminance = 0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b());
    if luminance > 0.4 {
        Color32::from_rgb(0x14, 0x10, 0x18)
    } else {
        Color32::WHITE
    }
}

fn channel(value: u8) -> f32 {
    let v = value as f32 / 255.0;
    if v <= 0.03928 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let lerp = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * amount).round() as u8;
    Color32::from_rgb(lerp(a.r(), b.r()), lerp(a.g(), b.g()), lerp(a.b(), b.b()))
}

/// egui dressed in the palette, with corners as the listener likes them and the skin's edges.
pub fn apply(ctx: &egui::Context, palette: &Palette, corners: CornerStyle, skin: Skin) {
    let halves = match skin {
        Skin::Windows98 | Skin::Windows98Noctorium => 0,
        Skin::WindowsXp => 1,
        Skin::Standard => (corners.scale() * 2.0) as u8,
    };
    CORNER_SCALE.store(halves, Ordering::Relaxed);
    SKIN.store(
        match skin {
            Skin::Standard => 0,
            Skin::Windows98 => 1,
            Skin::Windows98Noctorium => 2,
            Skin::WindowsXp => 3,
        },
        Ordering::Relaxed,
    );
    apply_visuals(ctx, palette, skin);
}

fn apply_visuals(ctx: &egui::Context, palette: &Palette, skin: Skin) {
    let mut visuals = if palette.light { Visuals::light() } else { Visuals::dark() };
    visuals.panel_fill = palette.panel;
    visuals.window_fill = palette.card;
    visuals.extreme_bg_color = mix(palette.page, palette.card, 0.5);
    visuals.faint_bg_color = palette.card;
    visuals.code_bg_color = palette.card;
    visuals.override_text_color = Some(palette.text);
    visuals.hyperlink_color = palette.accent;
    visuals.selection.bg_fill = palette.wash(0.45);
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.window_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.window_corner_radius = CornerRadius::same(corner(14));
    visuals.menu_corner_radius = CornerRadius::same(corner(10));
    visuals.slider_trailing_fill = true;
    let radius = CornerRadius::same(corner(8));
    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = palette.card;
    widgets.noninteractive.weak_bg_fill = palette.card;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.line);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.subtext);
    widgets.noninteractive.corner_radius = radius;
    for (state, fill) in [
        (&mut widgets.inactive, palette.card),
        (&mut widgets.hovered, palette.raised),
        (&mut widgets.active, mix(palette.raised, palette.accent, 0.25)),
        (&mut widgets.open, palette.raised),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.corner_radius = radius;
        state.bg_stroke = Stroke::NONE;
        state.fg_stroke = Stroke::new(1.5_f32, palette.text);
    }
    widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.line);
    widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    if let Some(w) = skin.windows98() {
        // 98's controls are grey slabs edged in black, its selection navy.
        for state in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.open] {
            state.bg_fill = rgb(w.face);
            state.weak_bg_fill = rgb(w.face);
            state.bg_stroke = Stroke::new(1.0_f32, rgb(w.shadow));
        }
        widgets.active.bg_fill = rgb(w.light);
        widgets.active.weak_bg_fill = rgb(w.light);
        widgets.active.bg_stroke = Stroke::new(1.0_f32, rgb(w.dark_shadow));
        visuals.selection.bg_fill = rgb(w.title);
        visuals.selection.stroke = Stroke::new(1.0_f32, rgb(w.title_text));
        visuals.window_stroke = Stroke::new(1.0_f32, rgb(w.dark_shadow));
        visuals.window_shadow = egui::Shadow::NONE;
        visuals.popup_shadow = egui::Shadow::NONE;
    }
    if skin == Skin::WindowsXp {
        // Luna's buttons: white fading to beige inside a dark blue edge, glowing orange when pointed at.
        for state in [&mut widgets.inactive, &mut widgets.open] {
            state.bg_fill = Color32::WHITE;
            state.weak_bg_fill = mix(Color32::WHITE, rgb(xp::BUTTON_FOOT), 0.5);
            state.bg_stroke = Stroke::new(1.0_f32, rgb(xp::BUTTON_EDGE));
        }
        widgets.hovered.bg_stroke = Stroke::new(2.0_f32, rgb(xp::HOT));
        widgets.hovered.weak_bg_fill = Color32::WHITE;
        widgets.active.weak_bg_fill = rgb(xp::BUTTON_FOOT);
        widgets.active.bg_stroke = Stroke::new(1.0_f32, rgb(xp::BUTTON_EDGE));
        visuals.selection.bg_fill = rgb(xp::SELECTION);
        visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    }
    ctx.set_visuals(visuals);
}

/// The fonts: the typeface chosen -- the system's own, a serif or a monospace -- with fallbacks for the
/// scripts song titles come in.
pub fn install_fonts(ctx: &egui::Context, choice: FontChoice) {
    use egui::{FontData, FontDefinitions, FontFamily};
    use std::sync::Arc;
    let mut fonts = FontDefinitions::default();
    let chosen: &[(&str, &str)] = match (choice, cfg!(windows), cfg!(target_os = "macos")) {
        // Cambria rather than Georgia: Georgia Bold's tall letters bleed in egui's glyph atlas at large sizes.
        (FontChoice::Serif, true, _) => &[
            ("ui", r"C:\Windows\Fonts\cambria.ttc"),
            ("bold", r"C:\Windows\Fonts\cambriab.ttf"),
            ("ui", r"C:\Windows\Fonts\georgia.ttf"),
            ("bold", r"C:\Windows\Fonts\georgiab.ttf"),
        ],
        (FontChoice::Mono, true, _) => &[("ui", r"C:\Windows\Fonts\consola.ttf"), ("bold", r"C:\Windows\Fonts\consolab.ttf")],
        (FontChoice::Serif, _, true) => &[("ui", "/System/Library/Fonts/Supplemental/Georgia.ttf"), ("bold", "/System/Library/Fonts/Supplemental/Georgia Bold.ttf")],
        (FontChoice::Mono, _, true) => &[("ui", "/System/Library/Fonts/Menlo.ttc")],
        (FontChoice::Serif, false, false) => &[
            ("ui", "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf"),
            ("bold", "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf"),
            ("ui", "/usr/share/fonts/TTF/DejaVuSerif.ttf"),
            ("bold", "/usr/share/fonts/TTF/DejaVuSerif-Bold.ttf"),
        ],
        (FontChoice::Mono, false, false) => &[
            ("ui", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
            ("bold", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"),
        ],
        _ => &[],
    };
    let system: &[(&str, &str)] = if cfg!(windows) {
        &[
            ("ui", r"C:\Windows\Fonts\segoeui.ttf"),
            ("bold", r"C:\Windows\Fonts\segoeuib.ttf"),
            ("symbols", r"C:\Windows\Fonts\seguisym.ttf"),
            ("cjk-jp", r"C:\Windows\Fonts\YuGothM.ttc"),
            ("cjk-sc", r"C:\Windows\Fonts\msyh.ttc"),
            ("cjk-kr", r"C:\Windows\Fonts\malgun.ttf"),
        ]
    } else if cfg!(target_os = "macos") {
        &[
            ("ui", "/System/Library/Fonts/SFNS.ttf"),
            ("cjk-jp", "/System/Library/Fonts/Hiragino Sans GB.ttc"),
            ("cjk-kr", "/System/Library/Fonts/AppleSDGothicNeo.ttc"),
        ]
    } else {
        &[
            ("ui", "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf"),
            ("bold", "/usr/share/fonts/truetype/noto/NotoSans-Bold.ttf"),
            ("ui", "/usr/share/fonts/noto/NotoSans-Regular.ttf"),
            ("bold", "/usr/share/fonts/noto/NotoSans-Bold.ttf"),
            ("cjk-jp", "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        ]
    };
    let mut loaded: Vec<&str> = Vec::new();
    for (name, path) in chosen.iter().chain(system.iter()) {
        if loaded.contains(name) {
            continue;
        }
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert((*name).to_string(), Arc::new(FontData::from_owned(bytes)));
            loaded.push(name);
        }
    }
    let monospace_built_in = fonts.families.get(&FontFamily::Monospace).and_then(|m| m.first().cloned());
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    if loaded.contains(&"ui") {
        proportional.insert(0, "ui".into());
    } else if let (FontChoice::Mono, Some(hack)) = (choice, monospace_built_in) {
        // egui's own monospace, where the system has none to offer.
        proportional.insert(0, hack);
    }
    for fallback in ["symbols", "cjk-jp", "cjk-sc", "cjk-kr"] {
        if loaded.contains(&fallback) {
            proportional.push(fallback.into());
        }
    }
    let regular_chain = proportional.clone();
    let mut bold_chain = regular_chain.clone();
    if loaded.contains(&"bold") {
        bold_chain.insert(0, "bold".into());
    }
    fonts.families.insert(FontFamily::Name("bold".into()), bold_chain);
    ctx.set_fonts(fonts);
}

pub fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("bold".into()))
}

pub fn regular(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Proportional)
}

/// A cover's colour for Afterglow's light: the average of its more colourful pixels, lifted to be a light.
pub fn artwork_colour(bytes: &[u8]) -> Option<Color32> {
    let image = image::load_from_memory(bytes).ok()?.thumbnail(32, 32).to_rgb8();
    let (mut r, mut g, mut b, mut weight) = (0f32, 0f32, 0f32, 0f32);
    for pixel in image.pixels() {
        let [pr, pg, pb] = pixel.0.map(|v| v as f32 / 255.0);
        let max = pr.max(pg).max(pb);
        let min = pr.min(pg).min(pb);
        let saturation = if max > 0.0 { (max - min) / max } else { 0.0 };
        // Colourful and not too dark counts most; grey and black barely at all.
        let w = 0.05 + saturation * saturation * max;
        r += pr * w;
        g += pg * w;
        b += pb * w;
        weight += w;
    }
    if weight <= 0.0 {
        return None;
    }
    let (r, g, b) = (r / weight, g / weight, b / weight);
    let max = r.max(g).max(b).max(0.001);
    // As bright as a light should be, whatever the cover's own brightness.
    let lift = (0.8 / max).min(3.0);
    Some(Color32::from_rgb(
        (r * lift * 255.0).min(255.0) as u8,
        (g * lift * 255.0).min(255.0) as u8,
        (b * lift * 255.0).min(255.0) as u8,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_a_distinct_key_and_unknown_keys_fall_back_to_night() {
        let mut keys: Vec<_> = PRESETS.iter().map(|p| p.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), PRESETS.len());
        assert_eq!(preset("SOMETHING_NEW").key, "NOCTORIUM_NIGHT");
        assert_eq!(preset("NORD").name, "Nord");
    }

    #[test]
    fn writing_on_the_accent_reads() {
        assert_eq!(on(Color32::WHITE), Color32::from_rgb(0x14, 0x10, 0x18));
        assert_eq!(on(Color32::from_rgb(0x00, 0x00, 0x80)), Color32::WHITE);
    }
}
