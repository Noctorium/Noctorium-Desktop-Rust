//! Pictures of the window, for working on it: with `NOCTORIUM_CAPTURE` set to a folder, Noctorium opens
//! each page named in `NOCTORIUM_CAPTURE_PAGES` in turn, saves a PNG of it there, and closes.
//!
//! ```text
//! NOCTORIUM_CAPTURE=shots NOCTORIUM_CAPTURE_PAGES="home,search:daft punk,settings" noctorium
//! ```
//!
//! A page is `home`, `search:<words>`, `library`, `queue` or `settings`. Three steps take no picture of
//! their own and change what the pages after them show: `theme:<KEY>` switches the theme, `play` plays the
//! search's first song, and `lyrics` opens the lyrics. `looks:<name>=<VALUE>;...` changes Settings ›
//! Personalisation by the names it is saved under, `looks:nowPlaying=STAGE;cover=RECORD`, and
//! `settings:<words>` opens Settings searched for those words, and `settingspage:<Name>` one page of
//! Settings by its name in the code, `settingspage:Theme`. Each waits `NOCTORIUM_CAPTURE_WAIT` seconds (eight by default) for the
//! services to answer and the covers to arrive.

use super::{Action, App, Page};
use eframe::egui;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Capture {
    folder: PathBuf,
    steps: Vec<String>,
    next: usize,
    started: Instant,
    wait: Duration,
    requested: bool,
    /// Whether this step has done what it does, which it does once.
    acted: bool,
}

impl Capture {
    pub fn from_environment() -> Option<Capture> {
        let folder = PathBuf::from(std::env::var_os("NOCTORIUM_CAPTURE").filter(|v| !v.is_empty())?);
        let steps = std::env::var("NOCTORIUM_CAPTURE_PAGES")
            .unwrap_or_else(|_| "home".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let wait = std::env::var("NOCTORIUM_CAPTURE_WAIT")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(8.0);
        let _ = std::fs::create_dir_all(&folder);
        Some(Capture {
            folder,
            steps,
            next: 0,
            started: Instant::now(),
            wait: Duration::from_secs_f32(wait),
            requested: false,
            acted: false,
        })
    }

    /// Moves to the next page, takes its picture once it has had time to fill in, and closes at the end.
    pub fn step(&mut self, app: &App, ctx: &egui::Context, actions: &mut Vec<Action>) {
        ctx.request_repaint_after(Duration::from_millis(100));
        let images: Vec<egui::ColorImage> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some((**image).clone()),
                    _ => None,
                })
                .collect()
        });
        if let Some(image) = images.into_iter().next() {
            let name = sanitise(&self.steps[self.next]);
            let path = self.folder.join(format!("{:02}-{name}.png", self.next + 1));
            let [w, h] = image.size;
            let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            if let Some(buffer) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                let _ = buffer.save(&path);
                println!("Saved {}", path.display());
            }
            self.next += 1;
            self.requested = false;
            self.acted = false;
            self.started = Instant::now();
        }
        if self.next >= self.steps.len() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        let step = self.steps[self.next].clone();
        let search_pending = app.search.results.iter().any(|(_, r)| r.is_none());
        if step == "play" && (app.search.results.is_empty() || search_pending) {
            return;
        }
        if !self.acted || app_page_differs(app, &step) {
            println!("Step {}: {step}", self.next + 1);
            open(&step, app, actions);
            self.acted = true;
        }
        if step.starts_with("theme:") || step.starts_with("looks:") || step == "play" || step == "lyrics" || step == "diagnose" || step == "edit" {
            // Nothing to picture; the theme is for the pages after it.
            if self.started.elapsed() > Duration::from_millis(300) {
                self.next += 1;
                self.acted = false;
                self.started = Instant::now();
            }
            return;
        }
        if !self.requested && self.started.elapsed() > self.wait {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.requested = true;
        }
    }
}

fn app_page_differs(app: &App, step: &str) -> bool {
    match step {
        "home" => app.page != Page::Home,
        "library" => app.page != Page::Library,
        "queue" => app.page != Page::Queue,
        "settings" => app.page != Page::Settings,
        s if s.starts_with("settings:") => app.page != Page::Settings,
        s if s.starts_with("settingspage:") => !matches!(app.page, Page::SettingsAt(_)),
        "nowplaying" => app.page != Page::NowPlaying,
        "shortcuts" => app.page != Page::Shortcuts,
        "services" => app.page != Page::Services,
        "connect" => app.page != Page::Connect,
        "profile" => app.page != Page::Profile,
        _ => false,
    }
}

fn open(step: &str, app: &App, actions: &mut Vec<Action>) {
    if let Some(words) = step.strip_prefix("search:") {
        if app.search.query != words.trim() || app.page != Page::Search {
            actions.push(Action::CaptureSearch(words.to_string()));
        }
        return;
    }
    if step == "play" {
        for (provider, result) in &app.search.results {
            match result {
                Some(Ok(tracks)) => println!("  {}: {} songs", provider.display_name(), tracks.len()),
                Some(Err(why)) => println!("  {}: {why}", provider.display_name()),
                None => println!("  {}: still searching", provider.display_name()),
            }
        }
        let first = app.search.results.iter().find_map(|(_, r)| r.as_ref()?.as_ref().ok()?.first().cloned());
        if let (Some(track), true) = (first, app.loaded.is_none() && app.resolving.is_none()) {
            actions.push(Action::PlayList { tracks: vec![track], index: 0 });
        }
        return;
    }
    if let Some(words) = step.strip_prefix("quick:") {
        actions.push(Action::QuickSearchType(words.to_string()));
        return;
    }
    if step == "lyrics" {
        if !app.settings.lyrics_open {
            actions.push(Action::ToggleLyrics);
        }
        return;
    }
    if step == "edit" {
        if let Some(track) = app.queue.current() {
            actions.push(Action::EditDetails(track.clone(), track.title.clone(), track.artist_line()));
        }
        return;
    }
    if step == "diagnose" {
        actions.push(Action::RunDiagnostics);
        return;
    }
    if let Some(name) = step.strip_prefix("settingspage:") {
        match super::settings::SettingsPage::ALL.iter().find(|p| format!("{p:?}").eq_ignore_ascii_case(name)) {
            Some(page) => actions.push(Action::Open(Page::SettingsAt(*page))),
            None => println!("  no settings page {name}"),
        }
        return;
    }
    if let Some(words) = step.strip_prefix("settings:") {
        actions.push(Action::CaptureSettingsQuery(words.to_string()));
        return;
    }
    if let Some(pairs) = step.strip_prefix("looks:") {
        let mut looks = serde_json::to_value(&app.settings.looks).unwrap_or_default();
        for pair in pairs.split(';') {
            if let Some((name, value)) = pair.split_once('=') {
                looks[name.trim()] = match value.trim() {
                    "true" => serde_json::Value::Bool(true),
                    "false" => serde_json::Value::Bool(false),
                    // A list, its items between bars: playerButtons=SHUFFLE|STOP, or [] for none.
                    "[]" => serde_json::json!([]),
                    list if list.contains('|') => serde_json::json!(list.split('|').collect::<Vec<_>>()),
                    other => serde_json::Value::String(other.to_string()),
                };
            }
        }
        match serde_json::from_value::<crate::looks::Looks>(looks) {
            Ok(looks) => actions.push(Action::SetLooks(Box::new(looks))),
            Err(why) => println!("  looks: {why}"),
        }
        return;
    }
    if let Some(key) = step.strip_prefix("theme:") {
        if let Some(preset) = crate::theme::PRESETS.iter().find(|p| p.key == key) {
            if app.settings.theme != preset.key {
                actions.push(Action::SetTheme(preset.key));
            }
        }
        return;
    }
    let page = match step {
        "library" => Page::Library,
        "queue" => Page::Queue,
        "settings" => Page::Settings,
        "nowplaying" => Page::NowPlaying,
        "shortcuts" => Page::Shortcuts,
        "services" => Page::Services,
        "connect" => Page::Connect,
        "profile" => Page::Profile,
        _ => Page::Home,
    };
    if app.page != page {
        actions.push(Action::Open(page));
    }
}

fn sanitise(step: &str) -> String {
    step.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect()
}
