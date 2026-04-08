use std::collections::VecDeque;

use rand::seq::SliceRandom;
use rand::rng;

use crate::types::RepeatMode;

/// A playback queue backed by VecDeque.
/// Supports shuffle and repeat modes.
#[derive(Clone)]
pub struct Tracklist {
    tracks: VecDeque<String>,
    current_index: Option<usize>,
    shuffle: bool,
    repeat: RepeatMode,
    shuffle_order: Vec<usize>,
    shuffle_position: Option<usize>,
}

impl Tracklist {
    pub fn new() -> Self {
        Self {
            tracks: VecDeque::new(),
            current_index: None,
            shuffle: false,
            repeat: RepeatMode::Off,
            shuffle_order: Vec::new(),
            shuffle_position: None,
        }
    }

    pub fn add(&mut self, uri: String) {
        self.tracks.push_back(uri);
    }

    pub fn load(&mut self, uris: Vec<String>) {
        self.tracks.clear();
        self.tracks.extend(uris);
        self.current_index = if self.tracks.is_empty() {
            None
        } else {
            Some(0)
        };
        if self.shuffle {
            self.generate_shuffle_order();
        }
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
        self.shuffle_order.clear();
        self.shuffle_position = None;
    }

    pub fn current(&self) -> Option<&str> {
        self.current_index
            .and_then(|i| self.tracks.get(i))
            .map(String::as_str)
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<&str> {
        let _idx = self.current_index?;
        if self.repeat == RepeatMode::One {
            return self.current();
        }
        if self.shuffle {
            return self.next_shuffled();
        }
        self.next_sequential()
    }

    pub fn previous(&mut self) -> Option<&str> {
        let _idx = self.current_index?;
        if self.shuffle {
            return self.previous_shuffled();
        }
        self.previous_sequential()
    }

    fn next_sequential(&mut self) -> Option<&str> {
        let idx = self.current_index?;
        if idx + 1 < self.tracks.len() {
            self.current_index = Some(idx + 1);
            self.current()
        } else if self.repeat == RepeatMode::All && !self.tracks.is_empty() {
            self.current_index = Some(0);
            self.current()
        } else {
            None
        }
    }

    fn previous_sequential(&mut self) -> Option<&str> {
        let idx = self.current_index?;
        if idx > 0 {
            self.current_index = Some(idx - 1);
            self.current()
        } else {
            None
        }
    }

    fn next_shuffled(&mut self) -> Option<&str> {
        let pos = self.shuffle_position?;
        if pos + 1 < self.shuffle_order.len() {
            self.shuffle_position = Some(pos + 1);
            self.current_index = Some(self.shuffle_order[pos + 1]);
            self.current()
        } else if self.repeat == RepeatMode::All && !self.shuffle_order.is_empty() {
            self.generate_shuffle_order();
            self.current()
        } else {
            None
        }
    }

    fn previous_shuffled(&mut self) -> Option<&str> {
        let pos = self.shuffle_position?;
        if pos > 0 {
            self.shuffle_position = Some(pos - 1);
            self.current_index = Some(self.shuffle_order[pos - 1]);
            self.current()
        } else {
            None
        }
    }

    pub fn set_shuffle(&mut self, on: bool) {
        if on && !self.shuffle {
            self.shuffle = true;
            self.generate_shuffle_order();
        } else if !on && self.shuffle {
            self.shuffle = false;
            self.shuffle_order.clear();
            self.shuffle_position = None;
        }
    }

    pub fn get_shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    pub fn get_repeat(&self) -> RepeatMode {
        self.repeat
    }

    fn generate_shuffle_order(&mut self) {
        let len = self.tracks.len();
        if len == 0 {
            self.shuffle_order.clear();
            self.shuffle_position = None;
            return;
        }
        let current = self.current_index.unwrap_or(0);
        let mut others: Vec<usize> = (0..len).filter(|&i| i != current).collect();
        others.shuffle(&mut rng());
        self.shuffle_order = Vec::with_capacity(len);
        self.shuffle_order.push(current);
        self.shuffle_order.extend(others);
        self.shuffle_position = Some(0);
    }

    pub fn index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }
}

impl Default for Tracklist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tracklist_is_empty() {
        let tl = Tracklist::new();
        assert!(tl.is_empty());
        assert_eq!(tl.len(), 0);
        assert!(tl.current().is_none());
        assert!(tl.index().is_none());
    }

    #[test]
    fn add_does_not_set_current() {
        let mut tl = Tracklist::new();
        tl.add("file:///a.mp3".into());
        assert_eq!(tl.len(), 1);
        assert!(tl.current().is_none());
    }

    #[test]
    fn load_sets_current_to_first() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into(), "file:///b.mp3".into()]);
        assert_eq!(tl.len(), 2);
        assert_eq!(tl.index(), Some(0));
        assert_eq!(tl.current(), Some("file:///a.mp3"));
    }

    #[test]
    fn load_empty_sets_none() {
        let mut tl = Tracklist::new();
        tl.load(vec![]);
        assert!(tl.is_empty());
        assert!(tl.current().is_none());
    }

    #[test]
    fn load_replaces_existing() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into()]);
        tl.load(vec!["file:///b.mp3".into(), "file:///c.mp3".into()]);
        assert_eq!(tl.len(), 2);
        assert_eq!(tl.current(), Some("file:///b.mp3"));
    }

    #[test]
    fn clear_resets_everything() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into()]);
        tl.clear();
        assert!(tl.is_empty());
        assert!(tl.current().is_none());
        assert!(tl.index().is_none());
    }

    #[test]
    fn next_advances() {
        let mut tl = Tracklist::new();
        tl.load(vec![
            "file:///a.mp3".into(),
            "file:///b.mp3".into(),
            "file:///c.mp3".into(),
        ]);
        assert_eq!(tl.next(), Some("file:///b.mp3"));
        assert_eq!(tl.index(), Some(1));
        assert_eq!(tl.next(), Some("file:///c.mp3"));
        assert_eq!(tl.index(), Some(2));
    }

    #[test]
    fn next_at_end_returns_none() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into()]);
        assert_eq!(tl.next(), None);
        assert_eq!(tl.index(), Some(0));
    }

    #[test]
    fn next_on_empty_returns_none() {
        let mut tl = Tracklist::new();
        assert_eq!(tl.next(), None);
    }

    #[test]
    fn previous_goes_back() {
        let mut tl = Tracklist::new();
        tl.load(vec![
            "file:///a.mp3".into(),
            "file:///b.mp3".into(),
            "file:///c.mp3".into(),
        ]);
        tl.next();
        tl.next();
        assert_eq!(tl.previous(), Some("file:///b.mp3"));
        assert_eq!(tl.index(), Some(1));
        assert_eq!(tl.previous(), Some("file:///a.mp3"));
        assert_eq!(tl.index(), Some(0));
    }

    #[test]
    fn previous_at_start_returns_none() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into()]);
        assert_eq!(tl.previous(), None);
        assert_eq!(tl.index(), Some(0));
    }

    #[test]
    fn previous_on_empty_returns_none() {
        let mut tl = Tracklist::new();
        assert_eq!(tl.previous(), None);
    }

    #[test]
    fn repeat_all_wraps_at_end() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into(), "file:///b.mp3".into()]);
        tl.set_repeat(RepeatMode::All);
        tl.next();
        assert_eq!(tl.next(), Some("file:///a.mp3"));
        assert_eq!(tl.index(), Some(0));
    }

    #[test]
    fn repeat_one_returns_same_track() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into(), "file:///b.mp3".into()]);
        tl.set_repeat(RepeatMode::One);
        assert_eq!(tl.next(), Some("file:///a.mp3"));
        assert_eq!(tl.next(), Some("file:///a.mp3"));
    }

    #[test]
    fn repeat_off_returns_none_at_end() {
        let mut tl = Tracklist::new();
        tl.load(vec!["file:///a.mp3".into()]);
        tl.set_repeat(RepeatMode::Off);
        assert_eq!(tl.next(), None);
    }

    #[test]
    fn repeat_mode_cycle() {
        assert_eq!(RepeatMode::Off.cycle(), RepeatMode::All);
        assert_eq!(RepeatMode::All.cycle(), RepeatMode::One);
        assert_eq!(RepeatMode::One.cycle(), RepeatMode::Off);
    }

    #[test]
    fn shuffle_produces_permutation() {
        let mut tl = Tracklist::new();
        tl.load(vec![
            "file:///a.mp3".into(),
            "file:///b.mp3".into(),
            "file:///c.mp3".into(),
            "file:///d.mp3".into(),
            "file:///e.mp3".into(),
        ]);
        tl.set_shuffle(true);
        let mut visited = vec![tl.current().unwrap().to_string()];
        for _ in 0..4 {
            visited.push(tl.next().unwrap().to_string());
        }
        visited.sort();
        assert_eq!(visited, vec![
            "file:///a.mp3",
            "file:///b.mp3",
            "file:///c.mp3",
            "file:///d.mp3",
            "file:///e.mp3",
        ]);
    }

    #[test]
    fn shuffle_off_restores_order() {
        let mut tl = Tracklist::new();
        tl.load(vec![
            "file:///a.mp3".into(),
            "file:///b.mp3".into(),
            "file:///c.mp3".into(),
        ]);
        tl.set_shuffle(true);
        tl.next();
        let current_uri = tl.current().unwrap().to_string();
        tl.set_shuffle(false);
        assert_eq!(tl.current().unwrap(), current_uri);
    }

    #[test]
    fn shuffle_previous_walks_backward() {
        let mut tl = Tracklist::new();
        tl.load(vec![
            "file:///a.mp3".into(),
            "file:///b.mp3".into(),
            "file:///c.mp3".into(),
        ]);
        tl.set_shuffle(true);
        let first = tl.current().unwrap().to_string();
        tl.next();
        let back = tl.previous().unwrap().to_string();
        assert_eq!(back, first);
    }
}
