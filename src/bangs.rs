//! Bangs: a word starting with "!" that sends one search to one service -- `!ytm daft punk`, or
//! `daft punk !sc`. The same words, and the same rules, as Noctorium-Base's `SearchBangs`.
//!
//! At the start or at the end of what was typed, in any case. Only a bang this knows counts: anything else
//! that starts with "!" stays in the words, since a band can be called "!!!". A bang at the start is taken
//! over one at the end.

use serde::{Deserialize, Serialize};

/// Where a search goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SearchMode {
    YoutubeMusic,
    YoutubeVideo,
    Soundcloud,
    /// YouTube Music and SoundCloud at once, the results side by side.
    Hybrid,
    Spotify,
    Bandcamp,
    Vk,
    Deezer,
    Tidal,
    AppleMusic,
}

impl SearchMode {
    pub fn display_name(self) -> &'static str {
        match self {
            SearchMode::YoutubeMusic => "YouTube Music",
            SearchMode::YoutubeVideo => "YouTube",
            SearchMode::Soundcloud => "SoundCloud",
            SearchMode::Hybrid => "Everywhere",
            SearchMode::Spotify => "Spotify",
            SearchMode::Bandcamp => "Bandcamp",
            SearchMode::Vk => "VK Music",
            SearchMode::Deezer => "Deezer",
            SearchMode::Tidal => "Tidal",
            SearchMode::AppleMusic => "Apple Music",
        }
    }

    /// The searches this player can make itself. The others need the services' own sign-ins, which the
    /// Kotlin players have and this one does not yet.
    pub const OFFERED: [SearchMode; 10] = [
        SearchMode::YoutubeMusic,
        SearchMode::Soundcloud,
        SearchMode::YoutubeVideo,
        SearchMode::Deezer,
        SearchMode::Bandcamp,
        SearchMode::Spotify,
        SearchMode::AppleMusic,
        SearchMode::Tidal,
        SearchMode::Vk,
        SearchMode::Hybrid,
    ];

    pub fn offered_here(self) -> bool {
        Self::OFFERED.contains(&self)
    }
}

/// Every bang, without its "!", and the search it chooses. In the order Kotlin lists them.
pub const ALL: [(&str, SearchMode); 18] = [
    ("ytm", SearchMode::YoutubeMusic),
    ("yt", SearchMode::YoutubeMusic),
    ("youtube", SearchMode::YoutubeMusic),
    ("ytv", SearchMode::YoutubeVideo),
    ("sc", SearchMode::Soundcloud),
    ("soundcloud", SearchMode::Soundcloud),
    ("sp", SearchMode::Spotify),
    ("spotify", SearchMode::Spotify),
    ("bc", SearchMode::Bandcamp),
    ("bandcamp", SearchMode::Bandcamp),
    ("vk", SearchMode::Vk),
    ("dz", SearchMode::Deezer),
    ("deezer", SearchMode::Deezer),
    ("td", SearchMode::Tidal),
    ("tidal", SearchMode::Tidal),
    ("am", SearchMode::AppleMusic),
    ("applemusic", SearchMode::AppleMusic),
    ("all", SearchMode::Hybrid),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BangQuery {
    /// The words to look for.
    pub query: String,
    /// The bang as typed, "!sc", and the search it means; None for the search page's own choice.
    pub bang: Option<(String, SearchMode)>,
}

impl BangQuery {
    pub fn mode(&self) -> Option<SearchMode> {
        self.bang.as_ref().map(|(_, mode)| *mode)
    }
}

/// `text` read for a bang. Without one, the query is the text exactly as it was.
pub fn parse(text: &str) -> BangQuery {
    let trimmed = text.trim();
    let first = trimmed.split([' ', '\t']).next().unwrap_or("");
    if let Some(mode) = bang_in(first) {
        return BangQuery {
            query: trimmed[first.len()..].trim().to_string(),
            bang: Some((first.to_string(), mode)),
        };
    }
    let last = trimmed.rsplit([' ', '\t']).next().unwrap_or("");
    if let Some(mode) = bang_in(last) {
        return BangQuery {
            query: trimmed[..trimmed.len() - last.len()].trim().to_string(),
            bang: Some((last.to_string(), mode)),
        };
    }
    BangQuery {
        query: text.to_string(),
        bang: None,
    }
}

/// The shortest bang that means `mode`: "!sc" for SoundCloud.
pub fn bang_for(mode: SearchMode) -> String {
    let word = ALL
        .iter()
        .filter(|(_, m)| *m == mode)
        .map(|(word, _)| *word)
        .min_by_key(|word| word.len())
        .unwrap_or("all");
    format!("!{word}")
}

fn bang_in(word: &str) -> Option<SearchMode> {
    let name = word.strip_prefix('!')?;
    if name.is_empty() {
        return None;
    }
    let lower = name.to_lowercase();
    ALL.iter()
        .find(|(word, _)| *word == lower)
        .map(|(_, mode)| *mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bang_at_the_start() {
        let parsed = parse("!sc daft punk");
        assert_eq!(parsed.query, "daft punk");
        assert_eq!(parsed.mode(), Some(SearchMode::Soundcloud));
    }

    #[test]
    fn a_bang_at_the_end_in_any_case() {
        let parsed = parse("daft punk !YTM");
        assert_eq!(parsed.query, "daft punk");
        assert_eq!(parsed.mode(), Some(SearchMode::YoutubeMusic));
    }

    #[test]
    fn the_start_wins_over_the_end() {
        assert_eq!(parse("!sc a !ytm").mode(), Some(SearchMode::Soundcloud));
    }

    #[test]
    fn unknown_bangs_are_words() {
        let parsed = parse("!!! !Action Pact!");
        assert_eq!(parsed.mode(), None);
        assert_eq!(parsed.query, "!!! !Action Pact!");
    }

    #[test]
    fn the_shortest_bang_is_suggested() {
        assert_eq!(bang_for(SearchMode::Soundcloud), "!sc");
        assert_eq!(bang_for(SearchMode::YoutubeMusic), "!yt");
        assert_eq!(bang_for(SearchMode::Hybrid), "!all");
    }
}
