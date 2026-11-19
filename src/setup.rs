//! The first-time setup, as Noctorium-Base's `SetupStep` has it: the same steps in the same words on every Noctorium.
//! Shown once -- on a first start, until it is finished or skipped -- by `Settings::setup_done`.

use crate::bangs::SearchMode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Look,
    Search,
    Listening,
    Accounts,
    Done,
}

impl Step {
    pub const ALL: [Step; 6] = [Step::Welcome, Step::Look, Step::Search, Step::Listening, Step::Accounts, Step::Done];

    pub fn title(self) -> &'static str {
        match self {
            Step::Welcome => "Welcome to Noctorium",
            Step::Look => "Pick a look",
            Step::Search => "Where to search",
            Step::Listening => "When the queue runs out",
            Step::Accounts => "Your accounts",
            Step::Done => "You're all set",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Step::Welcome => "YouTube Music, SoundCloud, Spotify, Bandcamp and more, in one player. A few quick choices and you're listening \u{2014} or skip them, and choose later in Settings.",
            Step::Look => "A theme to start with. There are many more in Settings, and an accent of your own.",
            Step::Search => "Which services a search asks. A bang at the start or end \u{2014} !ytm, !sc, !sp \u{2014} sends one search to one service.",
            Step::Listening => "Keep playing songs like the last one, or stop when the queue ends.",
            Step::Accounts => "Sign in to bring your playlists and likes. Noctorium plays without signing in, and you can sign in any time.",
            Step::Done => "Search for something, or paste a link from YouTube, SoundCloud or Spotify to play it.",
        }
    }

    pub fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn next(self) -> Option<Step> {
        Step::ALL.get(self.index() + 1).copied()
    }

    pub fn previous(self) -> Option<Step> {
        self.index().checked_sub(1).map(|i| Step::ALL[i])
    }

    /// "Step 2 of 6".
    pub fn progress(self) -> String {
        format!("Step {} of {}", self.index() + 1, Step::ALL.len())
    }
}

/// The themes the setup offers, by the names settings keep them under.
pub const THEMES: [&str; 8] = ["NOCTORIUM_NIGHT", "NOCTORIUM_DUSK", "NOCTORIUM_DAY", "CATPPUCCIN_MOCHA", "TOKYO_NIGHT", "ROSE_PINE", "NORD", "DRACULA"];

/// Where searches go: every service at once, or one of the two that need nothing set up.
pub const SEARCH_MODES: [SearchMode; 3] = [SearchMode::Hybrid, SearchMode::YoutubeMusic, SearchMode::Soundcloud];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{load_settings, save_settings, Places};

    #[test]
    fn steps_go_forward_and_back() {
        assert_eq!(Step::Welcome.next(), Some(Step::Look));
        assert_eq!(Step::Done.next(), None);
        assert_eq!(Step::Welcome.previous(), None);
        assert_eq!(Step::Look.progress(), "Step 2 of 6");
    }

    #[test]
    fn the_setup_is_offered_on_a_first_start_only() {
        let folder = std::env::temp_dir().join(format!("noctorium-setup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let places = Places { root: Some(folder.clone()), neighbour: None };
        assert!(!load_settings(&places).setup_done, "no settings at all: a first start");
        // Something saved before finishing it: a restart offers it again.
        save_settings(&places, &load_settings(&places));
        assert!(!load_settings(&places).setup_done);
        // Finished: never again, whatever else is saved.
        let mut done = load_settings(&places);
        done.setup_done = true;
        save_settings(&places, &done);
        assert!(load_settings(&places).setup_done);
        // Settings from before there was a setup read as done, so an update does not show it.
        std::fs::write(folder.join("settings.json"), r#"{ "volume": 0.5 }"#).unwrap();
        assert!(load_settings(&places).setup_done);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
