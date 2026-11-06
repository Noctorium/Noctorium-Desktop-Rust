//! The queue: what is playing, what comes after it, and what autoplay has lined up beyond that.
//!
//! A port of Noctorium-Base's `QueueManager`, rule for rule. A manual queue may mix services; shuffle keeps
//! the order it came in so switching it off puts everything back; and autoplay's suggestions wait outside the
//! queue until the queue runs out, when they join it.

use crate::domain::Track;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Queue {
    pub tracks: Vec<Track>,
    /// -1 when nothing is chosen.
    pub current_index: isize,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    /// The order before shuffling, to go back to.
    original: Vec<Track>,
    /// Autoplay's songs, waiting for the queue to run out.
    pub suggestions: Vec<Track>,
    /// The song the suggestions were found for, so they are not looked for twice.
    pub suggestions_seed: Option<String>,
}

impl Queue {
    pub fn current(&self) -> Option<&Track> {
        usize::try_from(self.current_index)
            .ok()
            .and_then(|i| self.tracks.get(i))
    }

    pub fn up_next(&self) -> &[Track] {
        let start = (self.current_index + 1).max(0) as usize;
        self.tracks.get(start..).unwrap_or(&[])
    }

    pub fn has_next(&self) -> bool {
        !self.tracks.is_empty()
            && ((self.current_index as usize) < self.tracks.len() - 1
                || self.repeat == RepeatMode::All
                || !self.suggestions.is_empty())
    }

    pub fn has_previous(&self) -> bool {
        !self.tracks.is_empty() && (self.current_index > 0 || self.repeat == RepeatMode::All)
    }

    /// What will play when this song ends, for looking its address up ahead of time.
    pub fn upcoming(&self) -> Option<&Track> {
        if self.tracks.is_empty() {
            return None;
        }
        let last = self.tracks.len() as isize - 1;
        match () {
            _ if self.repeat == RepeatMode::One => self.current(),
            _ if self.current_index < last => self.tracks.get((self.current_index + 1) as usize),
            _ if self.repeat == RepeatMode::All => self.tracks.first(),
            _ => self.suggestions.first(),
        }
    }

    pub fn total_duration_ms(&self) -> i64 {
        self.tracks
            .iter()
            .map(|t| t.duration_ms.unwrap_or(0).max(0))
            .sum()
    }

    pub fn play_queue(&mut self, tracks: Vec<Track>, start: usize) {
        if start >= tracks.len() {
            return;
        }
        let (shuffle, repeat) = (self.shuffle, self.repeat);
        *self = Queue {
            original: tracks.clone(),
            tracks,
            current_index: start as isize,
            repeat,
            ..Default::default()
        };
        if shuffle {
            self.set_shuffle(true);
        }
    }

    pub fn add(&mut self, track: Track) {
        self.tracks.push(track.clone());
        self.original.push(track);
    }

    pub fn play_next(&mut self, track: Track) {
        let at = ((self.current_index + 1).max(0) as usize).min(self.tracks.len());
        self.tracks.insert(at, track.clone());
        let original_at = self
            .current()
            .and_then(|c| self.original.iter().position(|t| t == c))
            .map(|i| i + 1)
            .unwrap_or(0)
            .min(self.original.len());
        self.original.insert(original_at, track);
    }

    pub fn remove_at(&mut self, index: usize) {
        if index >= self.tracks.len() {
            return;
        }
        let removed = self.tracks.remove(index);
        if let Some(i) = self.original.iter().position(|t| *t == removed) {
            self.original.remove(i);
        }
        let len = self.tracks.len() as isize;
        self.current_index = if len == 0 {
            -1
        } else if (index as isize) < self.current_index {
            self.current_index - 1
        } else if self.current_index >= len {
            len - 1
        } else {
            self.current_index
        };
    }

    pub fn move_track(&mut self, from: usize, to: usize) {
        let len = self.tracks.len();
        if from >= len || to >= len || from == to {
            return;
        }
        let playing = self.current().cloned();
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        self.current_index = playing
            .and_then(|p| self.tracks.iter().position(|t| *t == p))
            .map(|i| i as isize)
            .unwrap_or(-1);
        if !self.shuffle {
            self.original = self.tracks.clone();
        }
    }

    pub fn jump_to(&mut self, index: usize) -> Option<Track> {
        if index >= self.tracks.len() {
            return None;
        }
        self.current_index = index as isize;
        self.current().cloned()
    }

    /// Moves on. `respect_repeat_one` is for a song that ended by itself; the Next button skips anyway.
    pub fn next(&mut self, respect_repeat_one: bool) -> Option<Track> {
        if self.tracks.is_empty() {
            return None;
        }
        let last = self.tracks.len() as isize - 1;
        if respect_repeat_one && self.repeat == RepeatMode::One {
            return self.current().cloned();
        }
        if self.current_index < last {
            self.current_index += 1;
        } else if self.repeat == RepeatMode::All {
            self.current_index = 0;
        } else if !self.suggestions.is_empty() {
            // The end of the queue, with autoplay's songs waiting: they join it, and the first plays.
            let joining = std::mem::take(&mut self.suggestions);
            self.current_index = self.tracks.len() as isize;
            self.tracks.extend(joining.iter().cloned());
            self.original.extend(joining);
        } else {
            return None;
        }
        self.current().cloned()
    }

    pub fn previous(&mut self) -> Option<Track> {
        if self.tracks.is_empty() {
            return None;
        }
        if self.current_index > 0 {
            self.current_index -= 1;
        } else if self.repeat == RepeatMode::All {
            self.current_index = self.tracks.len() as isize - 1;
        } else {
            return None;
        }
        self.current().cloned()
    }

    /// Autoplay's songs for `seed`, less anything already queued.
    pub fn set_suggestions(&mut self, seed: String, tracks: Vec<Track>) {
        let mut seen: HashSet<String> = self.tracks.iter().map(Track::queue_key).collect();
        self.suggestions = tracks
            .into_iter()
            .filter(|t| seen.insert(t.queue_key()))
            .collect();
        self.suggestions_seed = Some(seed);
    }

    pub fn clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.suggestions_seed = None;
    }

    pub fn play_suggestion(&mut self, index: usize) -> Option<Track> {
        if index >= self.suggestions.len() {
            return None;
        }
        let joining: Vec<Track> = self.suggestions.drain(..=index).collect();
        self.current_index = (self.tracks.len() + index) as isize;
        self.tracks.extend(joining.iter().cloned());
        self.original.extend(joining);
        self.current().cloned()
    }

    pub fn keep_suggestion(&mut self, index: usize) {
        if index < self.suggestions.len() {
            let kept = self.suggestions.remove(index);
            self.add(kept);
        }
    }

    pub fn shuffle_upcoming(&mut self) {
        if self.up_next().len() < 2 {
            return;
        }
        let keep = (self.current_index + 1).max(0) as usize;
        self.tracks[keep..].shuffle(&mut rand::rng());
        if !self.shuffle {
            self.original = self.tracks.clone();
        }
    }

    pub fn clear_upcoming(&mut self) {
        let keep = (self.current_index + 1).max(0) as usize;
        if keep >= self.tracks.len() {
            return;
        }
        self.tracks.truncate(keep);
        let kept: HashSet<String> = self.tracks.iter().map(Track::queue_key).collect();
        self.original.retain(|t| kept.contains(&t.queue_key()));
    }

    /// Everything but the song playing goes, and the suggestions that followed something else with it.
    pub fn clear_all_but_current(&mut self) {
        let Some(playing) = self.current().cloned() else { return };
        let follows = self.suggestions_seed.as_deref() == Some(playing.queue_key().as_str());
        self.tracks = vec![playing.clone()];
        self.original = vec![playing];
        self.current_index = 0;
        if !follows {
            self.clear_suggestions();
        }
    }

    pub fn toggle_shuffle(&mut self) {
        let wanted = !self.shuffle;
        self.set_shuffle(wanted);
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.tracks.is_empty() || self.shuffle == enabled {
            self.shuffle = enabled;
            return;
        }
        let playing = self.current().cloned();
        if enabled {
            let current = self.current_index;
            let mut others: Vec<Track> = self
                .tracks
                .iter()
                .enumerate()
                .filter(|(i, _)| *i as isize != current)
                .map(|(_, t)| t.clone())
                .collect();
            others.shuffle(&mut rand::rng());
            self.tracks = playing.into_iter().chain(others).collect();
            self.current_index = 0;
        } else {
            self.tracks = self.original.clone();
            self.current_index = playing
                .and_then(|p| self.tracks.iter().position(|t| *t == p))
                .unwrap_or(0) as isize;
        }
        self.shuffle = enabled;
    }

    /// Puts back a queue saved when Noctorium last closed.
    pub fn restore(&mut self, tracks: Vec<Track>, index: isize, repeat: RepeatMode) {
        if tracks.is_empty() || !self.tracks.is_empty() {
            return;
        }
        let index = index.clamp(0, tracks.len() as isize - 1);
        *self = Queue {
            original: tracks.clone(),
            tracks,
            current_index: index,
            repeat,
            ..Default::default()
        };
    }

    pub fn clear(&mut self) {
        let (shuffle, repeat) = (self.shuffle, self.repeat);
        *self = Queue {
            shuffle,
            repeat,
            ..Default::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProviderType;

    fn track(id: &str) -> Track {
        Track {
            provider: ProviderType::YoutubeMusic,
            id: id.into(),
            title: id.into(),
            artists: vec![],
            album: None,
            duration_ms: Some(1000),
            artwork_url: None,
            source_url: format!("https://music.youtube.com/watch?v={id}"),
        }
    }

    fn ids(queue: &Queue) -> Vec<String> {
        queue.tracks.iter().map(|t| t.id.clone()).collect()
    }

    #[test]
    fn next_and_previous_walk_the_queue() {
        let mut q = Queue::default();
        q.play_queue(vec![track("a"), track("b"), track("c")], 0);
        assert_eq!(q.next(false).unwrap().id, "b");
        assert_eq!(q.next(false).unwrap().id, "c");
        assert!(q.next(false).is_none());
        assert_eq!(q.previous().unwrap().id, "b");
    }

    #[test]
    fn repeat_all_wraps_and_repeat_one_stays() {
        let mut q = Queue::default();
        q.play_queue(vec![track("a"), track("b")], 1);
        q.repeat = RepeatMode::All;
        assert_eq!(q.next(false).unwrap().id, "a");
        q.repeat = RepeatMode::One;
        assert_eq!(q.next(true).unwrap().id, "a");
        assert_eq!(q.next(false).unwrap().id, "b");
    }

    #[test]
    fn suggestions_join_when_the_queue_runs_out() {
        let mut q = Queue::default();
        q.play_queue(vec![track("a")], 0);
        q.set_suggestions("a".into(), vec![track("a"), track("x"), track("y")]);
        assert_eq!(q.suggestions.len(), 2, "what is queued is not suggested");
        assert!(q.has_next());
        assert_eq!(q.next(false).unwrap().id, "x");
        assert_eq!(ids(&q), ["a", "x", "y"]);
        assert!(q.suggestions.is_empty());
    }

    #[test]
    fn shuffle_keeps_the_song_playing_first_and_unshuffle_puts_it_back() {
        let mut q = Queue::default();
        let tracks: Vec<Track> = (0..20).map(|i| track(&i.to_string())).collect();
        q.play_queue(tracks.clone(), 5);
        q.set_shuffle(true);
        assert_eq!(q.current().unwrap().id, "5");
        assert_eq!(q.current_index, 0);
        assert_eq!(q.tracks.len(), 20);
        q.set_shuffle(false);
        assert_eq!(q.tracks, tracks);
        assert_eq!(q.current().unwrap().id, "5");
    }

    #[test]
    fn play_next_goes_straight_after_the_current_song() {
        let mut q = Queue::default();
        q.play_queue(vec![track("a"), track("b")], 0);
        q.play_next(track("n"));
        assert_eq!(ids(&q), ["a", "n", "b"]);
    }

    #[test]
    fn removing_before_the_current_song_keeps_it_current() {
        let mut q = Queue::default();
        q.play_queue(vec![track("a"), track("b"), track("c")], 2);
        q.remove_at(0);
        assert_eq!(q.current().unwrap().id, "c");
        q.remove_at(1);
        assert_eq!(q.current().unwrap().id, "b");
    }

    #[test]
    fn moving_follows_the_song_playing() {
        let mut q = Queue::default();
        q.play_queue(vec![track("a"), track("b"), track("c")], 0);
        q.move_track(0, 2);
        assert_eq!(ids(&q), ["b", "c", "a"]);
        assert_eq!(q.current().unwrap().id, "a");
    }

    #[test]
    fn clearing_upcoming_keeps_what_has_played() {
        let mut q = Queue::default();
        q.play_queue(vec![track("a"), track("b"), track("c")], 1);
        q.clear_upcoming();
        assert_eq!(ids(&q), ["a", "b"]);
    }
}
