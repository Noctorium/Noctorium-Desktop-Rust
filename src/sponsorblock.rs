//! SponsorBlock: the stretches of a YouTube video that its viewers marked as not the music -- a sponsor,
//! an intro, a "like and subscribe" -- skipped as they come, as Noctorium-Base's `SponsorBlockClient` does.
//!
//! Asked by the first four characters of the video id's SHA-256, never the id itself, so SponsorBlock does
//! not learn what is being listened to.

use sha2::{Digest, Sha256};
use std::time::Duration;

pub const BASE_URL: &str = "https://sponsor.ajay.app";
pub const CATEGORIES: [&str; 7] = ["sponsor", "selfpromo", "interaction", "intro", "outro", "preview", "music_offtopic"];
/// Shorter than this is a half-second of somebody's opinion, not a stretch worth a seek heard as a stutter.
const MIN_SEGMENT_MS: i64 = 1_000;

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start_ms: i64,
    pub end_ms: i64,
    pub category: String,
}

impl Segment {
    pub fn contains(&self, position_ms: i64) -> bool {
        position_ms >= self.start_ms && position_ms < self.end_ms
    }

    pub fn description(&self) -> &'static str {
        match self.category.as_str() {
            "sponsor" => "a sponsor",
            "selfpromo" => "self-promotion",
            "interaction" => "a reminder to subscribe",
            "intro" => "the intro",
            "outro" => "the outro",
            "preview" => "a preview",
            "music_offtopic" => "a part that is not the music",
            _ => "a marked part",
        }
    }
}

pub fn hash_prefix(video_id: &str) -> String {
    Sha256::digest(video_id.as_bytes()).iter().take(2).map(|b| format!("{b:02x}")).collect()
}

/// The segments for one video. Nothing marked, or SponsorBlock not answering, is an empty list: the song
/// plays as it is.
pub fn segments_for(video_id: &str) -> Vec<Segment> {
    let categories = format!("[{}]", CATEGORIES.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(","));
    let reply = ureq::get(&format!("{BASE_URL}/api/skipSegments/{}", hash_prefix(video_id)))
        .query("categories", &categories)
        .set("User-Agent", concat!("Noctorium-Rust/", env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(10))
        .call();
    match reply.ok().and_then(|r| r.into_string().ok()) {
        Some(body) => parse(&body, video_id),
        None => vec![],
    }
}

pub fn parse(body: &str, video_id: &str) -> Vec<Segment> {
    let Ok(root) = serde_json::from_str::<serde_json::Value>(body) else { return vec![] };
    let Some(entry) = root.as_array().into_iter().flatten().find(|v| v["videoID"].as_str() == Some(video_id)) else {
        return vec![];
    };
    let mut found: Vec<Segment> = entry["segments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| {
            let start = s["segment"][0].as_f64()?;
            let end = s["segment"][1].as_f64()?;
            let (start_ms, end_ms) = ((start * 1000.0) as i64, (end * 1000.0) as i64);
            (end_ms - start_ms >= MIN_SEGMENT_MS).then(|| Segment {
                start_ms,
                end_ms,
                category: s["category"].as_str().unwrap_or("unknown").to_string(),
            })
        })
        .collect();
    found.sort_by_key(|s| s.start_ms);
    found
}

/// The segments of the song playing not yet skipped: each is skipped once, so seeking back into one plays it.
#[derive(Default)]
pub struct Skipper {
    pending: Vec<Segment>,
}

impl Skipper {
    pub fn new(segments: Vec<Segment>) -> Self {
        Skipper { pending: segments }
    }

    pub fn skip_from(&mut self, position_ms: i64) -> Option<Segment> {
        let index = self.pending.iter().position(|s| s.contains(position_ms))?;
        Some(self.pending.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_id_is_never_sent_whole() {
        assert_eq!(hash_prefix("dQw4w9WgXcQ").len(), 4);
    }

    #[test]
    fn segments_are_read_for_this_video_only_and_short_ones_dropped() {
        let body = r#"[{"videoID":"other","segments":[{"segment":[0,30],"category":"sponsor"}]},
            {"videoID":"me","segments":[{"segment":[200.5,230],"category":"outro"},
            {"segment":[0,12],"category":"intro"},{"segment":[50,50.4],"category":"sponsor"}]}]"#;
        let found = parse(body, "me");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].category, "intro");
        assert_eq!(found[1].start_ms, 200_500);
    }

    #[test]
    fn each_segment_is_skipped_once() {
        let mut skipper = Skipper::new(vec![Segment { start_ms: 0, end_ms: 5000, category: "intro".into() }]);
        assert!(skipper.skip_from(1000).is_some());
        assert!(skipper.skip_from(1000).is_none());
    }
}
