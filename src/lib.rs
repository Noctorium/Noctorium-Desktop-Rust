//! Noctorium for the desktop, in Rust: a music player for YouTube Music and SoundCloud.
//!
//! The same player as the Kotlin desktop -- one home, one search, one library, one queue -- on the same two
//! programs underneath, yt-dlp to read the services and mpv to make the sound, with the window drawn by egui.
//! Everything is in this library, so the tests reach all of it; `main.rs` only opens the window.

pub mod account;
pub mod accounts;
pub mod applemusic;
pub mod audio;
pub mod bandcamp;
pub mod bangs;
pub mod closematch;
pub mod connect;
pub mod deezer;
pub mod discord;
pub mod domain;
pub mod downloads;
pub mod gui;
pub mod hlsrelay;
pub mod instance;
pub mod looks;
pub mod lyrics;
pub mod marks;
pub mod matching;
pub mod mpv;
pub mod noctplaylists;
pub mod oauth;
pub mod platform;
pub mod queue;
pub mod radio;
pub mod sponsorblock;
pub mod scrobble;
pub mod services;
pub mod sharelink;
pub mod secrets;
pub mod shortcuts;
pub mod soundcloud;
pub mod spotify;
pub mod startup;
pub mod store;
pub mod theme;
pub mod tidal;
pub mod tools;
pub mod tray;
pub mod update;
pub mod vk;
pub mod widgets;
pub mod ytdlp;
pub mod ytmusic;

/// This build's version: the release tag's, which the release build passes in as NOCTORIUM_VERSION, or
/// Cargo's for a build of one's own -- as Noctorium Stats has it.
pub fn version() -> &'static str {
    const FROM_TAG: Option<&str> = option_env!("NOCTORIUM_VERSION");
    match FROM_TAG.map(|v| v.trim().trim_start_matches('v')).filter(|v| !v.is_empty()) {
        Some(version) => version,
        None => env!("CARGO_PKG_VERSION"),
    }
}
