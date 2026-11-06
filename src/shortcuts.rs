//! Keyboard shortcuts: every action Noctorium has a key for, its defaults, and the listener's own changes --
//! Noctorium-Base's `ShortcutPreferences` and `KeyChord`, and the Kotlin desktop's `ShortcutAction` table.
//!
//! In-app chords work while the window has focus. Global ones work from any program; they are off until
//! switched on, and registered with the system's own hot-key service, never a keyboard hook.

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    /// In-app chords the listener changed, by action id. An empty string turns an action's keys off.
    pub in_app: BTreeMap<String, String>,
    /// Global chords, by action id.
    pub global: BTreeMap<String, String>,
    pub global_enabled: bool,
}

impl Preferences {
    /// The chords an action answers to in the window: the listener's one, or its defaults.
    pub fn in_app_chords(&self, action: &Action) -> Vec<Chord> {
        match self.in_app.get(action.id) {
            Some(own) if own.trim().is_empty() => vec![],
            Some(own) => Chord::parse(own).into_iter().collect(),
            None => action.defaults.iter().filter_map(|d| Chord::parse(d)).collect(),
        }
    }

    pub fn global_chord(&self, action: &Action) -> Option<Chord> {
        if !self.global_enabled || !action.global {
            return None;
        }
        self.global.get(action.id).and_then(|c| Chord::parse(c))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

const NAMED: [&str; 33] = [
    "Space", "Enter", "Escape", "Tab", "Backspace", "Delete", "Insert", "Home", "End", "PageUp", "PageDown", "Up",
    "Down", "Left", "Right", "Comma", "Period", "Slash", "Semicolon", "Quote", "BracketLeft", "BracketRight",
    "Backslash", "Minus", "Equals", "Backquote", "MediaPlayPause", "MediaNext", "MediaPrevious", "MediaStop",
    "VolumeUp", "VolumeDown", "VolumeMute",
];

fn alias(word: &str) -> &str {
    match word {
        "control" | "ctl" => "ctrl",
        "option" | "opt" => "alt",
        "cmd" | "command" | "super" | "win" | "windows" => "meta",
        // "Mod" is the platform's own: Ctrl, and ⌘ on a Mac.
        "mod" => if cfg!(target_os = "macos") { "meta" } else { "ctrl" },
        "esc" => "escape",
        "return" => "enter",
        "del" => "delete",
        "ins" => "insert",
        "pgup" => "pageup",
        "pgdn" | "pagedn" => "pagedown",
        "arrowup" => "up",
        "arrowdown" => "down",
        "arrowleft" => "left",
        "arrowright" => "right",
        "spacebar" => "space",
        "," => "comma",
        "." => "period",
        "/" => "slash",
        ";" => "semicolon",
        "'" => "quote",
        "[" => "bracketleft",
        "]" => "bracketright",
        "\\" => "backslash",
        "-" => "minus",
        "=" => "equals",
        "`" => "backquote",
        "playpause" | "mediaplay" => "mediaplaypause",
        "nexttrack" => "medianext",
        "previoustrack" | "prevtrack" => "mediaprevious",
        "mute" => "volumemute",
        other => other,
    }
}

impl Chord {
    pub fn parse(text: &str) -> Option<Chord> {
        let (mut ctrl, mut alt, mut shift, mut meta) = (false, false, false, false);
        let mut key = None;
        let parts: Vec<&str> = text.split('+').map(str::trim).filter(|p| !p.is_empty()).collect();
        if parts.is_empty() {
            return None;
        }
        for part in parts {
            let lower = part.to_lowercase();
            match alias(&lower) {
                "ctrl" => ctrl = true,
                "alt" => alt = true,
                "shift" => shift = true,
                "meta" => meta = true,
                name => {
                    if key.is_some() {
                        return None;
                    }
                    key = Some(key_name(name)?);
                }
            }
        }
        Some(Chord { key: key?, ctrl, alt, shift, meta })
    }

    pub fn has_modifier(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.meta
    }
}

fn key_name(lower: &str) -> Option<String> {
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() == 1 && chars[0].is_ascii_alphanumeric() {
        return Some(lower.to_uppercase());
    }
    if let Some(n) = lower.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
        if (1..=24).contains(&n) {
            return Some(format!("F{n}"));
        }
    }
    NAMED.iter().find(|n| n.to_lowercase() == lower).map(|n| n.to_string())
}

impl std::fmt::Display for Chord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.meta {
            parts.push(if cfg!(target_os = "macos") { "⌘".into() } else { "Win".into() });
        }
        parts.push(self.key.clone());
        f.write_str(&parts.join("+"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Playing,
    Sound,
    Queue,
    Places,
    Window,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::Playing => "Playing",
            Group::Sound => "Sound",
            Group::Queue => "The queue",
            Group::Places => "Going places",
            Group::Window => "The window",
        }
    }
}

/// What a shortcut does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Does {
    PlayPause,
    Stop,
    StopAfterCurrent,
    Next,
    Previous,
    Seek(i64),
    Shuffle,
    Repeat,
    Like,
    Volume(i32),
    Mute,
    Boost(i32),
    ClearAllButCurrent,
    QuickSearch,
    Search,
    GoHome,
    GoSearch,
    GoLibrary,
    GoDownloads,
    GoNowPlaying,
    GoQueue,
    GoSettings,
    GoServices,
    Lyrics,
    PasteLink,
    Help,
    ToggleWindow,
}

pub struct Action {
    pub id: &'static str,
    pub title: &'static str,
    pub group: Group,
    pub does: Does,
    pub defaults: &'static [&'static str],
    pub global: bool,
    pub global_suggestion: Option<&'static str>,
}

macro_rules! action {
    ($id:expr, $title:expr, $group:ident, $does:expr, [$($d:expr),*]) => {
        Action { id: $id, title: $title, group: Group::$group, does: $does, defaults: &[$($d),*], global: false, global_suggestion: None }
    };
    ($id:expr, $title:expr, $group:ident, $does:expr, [$($d:expr),*], global $suggestion:expr) => {
        Action { id: $id, title: $title, group: Group::$group, does: $does, defaults: &[$($d),*], global: true, global_suggestion: $suggestion }
    };
}

/// Every action, with the Kotlin desktop's ids and defaults.
pub const ACTIONS: &[Action] = &[
    action!("playPause", "Play or pause", Playing, Does::PlayPause, ["Space", "K"], global Some("Ctrl+Alt+Home")),
    action!("stop", "Stop", Playing, Does::Stop, ["Mod+Period"], global Some("Ctrl+Alt+End")),
    action!("stopAfterCurrent", "Stop after this song", Playing, Does::StopAfterCurrent, ["Mod+Shift+Period"]),
    action!("next", "Next song", Playing, Does::Next, ["N", "Mod+Right"], global Some("Ctrl+Alt+PageDown")),
    action!("previous", "Previous song", Playing, Does::Previous, ["P", "Mod+Left"], global Some("Ctrl+Alt+PageUp")),
    action!("seekForward", "Forward 5 seconds", Playing, Does::Seek(5_000), ["Right"]),
    action!("seekBack", "Back 5 seconds", Playing, Does::Seek(-5_000), ["Left"]),
    action!("shuffle", "Shuffle", Playing, Does::Shuffle, ["S", "Mod+S"]),
    action!("repeat", "Repeat", Playing, Does::Repeat, ["R", "Mod+R"]),
    action!("like", "Like the song playing", Playing, Does::Like, ["F"], global None),
    action!("volumeUp", "Volume up", Sound, Does::Volume(5), ["Up", "Mod+Up"], global Some("Ctrl+Alt+Up")),
    action!("volumeDown", "Volume down", Sound, Does::Volume(-5), ["Down", "Mod+Down"], global Some("Ctrl+Alt+Down")),
    action!("mute", "Mute", Sound, Does::Mute, ["M"], global None),
    action!("boostUp", "Volume booster up", Sound, Does::Boost(10), ["Shift+Up"]),
    action!("boostDown", "Volume booster down", Sound, Does::Boost(-10), ["Shift+Down"]),
    action!("clearAllButCurrent", "Clear all but this song", Queue, Does::ClearAllButCurrent, []),
    action!("quickSearch", "Quick search", Places, Does::QuickSearch, ["Mod+F"], global Some("Ctrl+Alt+Space")),
    action!("search", "Search, with the caret in the box", Places, Does::Search, ["Slash"]),
    action!("goHome", "Go to Home", Places, Does::GoHome, ["1"]),
    action!("goSearch", "Go to Search", Places, Does::GoSearch, ["2"]),
    action!("goLibrary", "Go to the library", Places, Does::GoLibrary, ["4"]),
    action!("goDownloads", "Go to Downloads", Places, Does::GoDownloads, ["5"]),
    action!("goNowPlaying", "Go to Now playing", Places, Does::GoNowPlaying, ["6"]),
    action!("goQueue", "Go to the queue", Places, Does::GoQueue, ["7"]),
    action!("goSettings", "Go to Settings", Places, Does::GoSettings, ["8"]),
    action!("goServices", "Go to Services", Places, Does::GoServices, ["9"]),
    action!("lyrics", "Show or hide the lyrics", Places, Does::Lyrics, ["Mod+L"]),
    action!("pasteLink", "Play the link you copied", Places, Does::PasteLink, ["Mod+V"]),
    action!("help", "The list of shortcuts", Places, Does::Help, ["Shift+Slash"]),
    action!("toggleWindow", "Show or hide Noctorium", Window, Does::ToggleWindow, [], global None),
];

/// The chord egui's key press makes, for matching against the table.
pub fn chord_of(key: egui::Key, modifiers: egui::Modifiers) -> Option<Chord> {
    use egui::Key as K;
    let name = match key {
        K::Space => "Space",
        K::Enter => "Enter",
        K::Escape => "Escape",
        K::Tab => "Tab",
        K::Backspace => "Backspace",
        K::Delete => "Delete",
        K::Insert => "Insert",
        K::Home => "Home",
        K::End => "End",
        K::PageUp => "PageUp",
        K::PageDown => "PageDown",
        K::ArrowUp => "Up",
        K::ArrowDown => "Down",
        K::ArrowLeft => "Left",
        K::ArrowRight => "Right",
        K::Comma => "Comma",
        K::Period => "Period",
        K::Slash | K::Questionmark => "Slash",
        K::Semicolon => "Semicolon",
        K::Quote => "Quote",
        K::OpenBracket => "BracketLeft",
        K::CloseBracket => "BracketRight",
        K::Backslash => "Backslash",
        K::Minus => "Minus",
        K::Equals | K::Plus => "Equals",
        K::Backtick => "Backquote",
        other => {
            let text = other.name();
            return key_name(&text.to_lowercase()).map(|key| Chord {
                key,
                ctrl: modifiers.ctrl,
                alt: modifiers.alt,
                shift: modifiers.shift,
                meta: modifiers.mac_cmd || (cfg!(target_os = "macos") && modifiers.command),
            });
        }
    };
    // "?" is Shift+Slash on most layouts and arrives as its own key.
    let shift = modifiers.shift || key == K::Questionmark;
    Some(Chord {
        key: name.to_string(),
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
        shift,
        meta: modifiers.mac_cmd || (cfg!(target_os = "macos") && modifiers.command),
    })
}

/// The Windows virtual-key code and RegisterHotKey modifiers of a chord, for a global hot key.
pub fn windows_hot_key(chord: &Chord) -> Option<(u32, u32)> {
    let key = chord.key.as_str();
    let vk = if key.len() == 1 {
        key.chars().next()? as u32
    } else if let Some(n) = key.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        0x6F + n
    } else {
        match key {
            "Space" => 0x20,
            "Enter" => 0x0D,
            "Escape" => 0x1B,
            "Tab" => 0x09,
            "Backspace" => 0x08,
            "Delete" => 0x2E,
            "Insert" => 0x2D,
            "Home" => 0x24,
            "End" => 0x23,
            "PageUp" => 0x21,
            "PageDown" => 0x22,
            "Up" => 0x26,
            "Down" => 0x28,
            "Left" => 0x25,
            "Right" => 0x27,
            "Comma" => 0xBC,
            "Period" => 0xBE,
            "Slash" => 0xBF,
            "Semicolon" => 0xBA,
            "Quote" => 0xDE,
            "BracketLeft" => 0xDB,
            "BracketRight" => 0xDD,
            "Backslash" => 0xDC,
            "Minus" => 0xBD,
            "Equals" => 0xBB,
            "Backquote" => 0xC0,
            "MediaPlayPause" => 0xB3,
            "MediaNext" => 0xB0,
            "MediaPrevious" => 0xB1,
            "MediaStop" => 0xB2,
            "VolumeUp" => 0xAF,
            "VolumeDown" => 0xAE,
            "VolumeMute" => 0xAD,
            _ => return None,
        }
    };
    let modifiers = (chord.alt as u32) | ((chord.ctrl as u32) << 1) | ((chord.shift as u32) << 2) | ((chord.meta as u32) << 3);
    Some((vk, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_are_read_in_any_spelling() {
        let chord = Chord::parse("control + alt + pgdn").unwrap();
        assert_eq!(chord.to_string(), "Ctrl+Alt+PageDown");
        assert_eq!(Chord::parse("shift+/").unwrap().key, "Slash");
        assert_eq!(Chord::parse("f12").unwrap().key, "F12");
        assert!(Chord::parse("ctrl+a+b").is_none());
        assert!(Chord::parse("ctrl").is_none());
    }

    #[test]
    fn the_listeners_chord_replaces_the_defaults_and_empty_turns_it_off() {
        let play = ACTIONS.iter().find(|a| a.id == "playPause").unwrap();
        let mut preferences = Preferences::default();
        assert_eq!(preferences.in_app_chords(play).len(), 2);
        preferences.in_app.insert("playPause".into(), "Ctrl+P".into());
        assert_eq!(preferences.in_app_chords(play)[0].to_string(), "Ctrl+P");
        preferences.in_app.insert("playPause".into(), "".into());
        assert!(preferences.in_app_chords(play).is_empty());
    }

    #[test]
    fn global_chords_only_when_switched_on() {
        let next = ACTIONS.iter().find(|a| a.id == "next").unwrap();
        let mut preferences = Preferences::default();
        preferences.global.insert("next".into(), "Ctrl+Alt+PageDown".into());
        assert!(preferences.global_chord(next).is_none());
        preferences.global_enabled = true;
        assert_eq!(windows_hot_key(&preferences.global_chord(next).unwrap()), Some((0x22, 0b011)));
    }

    #[test]
    fn every_action_has_a_distinct_id() {
        let mut ids: Vec<_> = ACTIONS.iter().map(|a| a.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), ACTIONS.len());
    }
}
