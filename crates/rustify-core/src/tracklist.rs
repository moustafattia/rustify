use std::collections::VecDeque;

/// A playback queue backed by VecDeque.
/// Stores track URIs and maintains a current position index.
pub struct Tracklist {
    tracks: VecDeque<String>,
    current_index: Option<usize>,
}

impl Tracklist {
    pub fn new() -> Self {
        Self {
            tracks: VecDeque::new(),
            current_index: None,
        }
    }

    /// Append a single track URI to the end of the queue.
    pub fn add(&mut self, uri: String) {
        self.tracks.push_back(uri);
    }

    /// Replace the entire tracklist with the given URIs.
    /// Sets the current position to the first track if non-empty.
    pub fn load(&mut self, uris: Vec<String>) {
        self.tracks.clear();
        self.tracks.extend(uris);
        self.current_index = if self.tracks.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// Remove all tracks and reset position.
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
    }

    /// Get the URI of the current track, if any.
    pub fn current(&self) -> Option<&str> {
        self.current_index
            .and_then(|i| self.tracks.get(i))
            .map(String::as_str)
    }

    /// Advance to the next track and return its URI.
    /// Returns None if already at the end.
    pub fn next(&mut self) -> Option<&str> {
        let idx = self.current_index?;
        if idx + 1 < self.tracks.len() {
            self.current_index = Some(idx + 1);
            self.current()
        } else {
            None
        }
    }

    /// Go back to the previous track and return its URI.
    /// Returns None if already at the beginning.
    pub fn previous(&mut self) -> Option<&str> {
        let idx = self.current_index?;
        if idx > 0 {
            self.current_index = Some(idx - 1);
            self.current()
        } else {
            None
        }
    }

    /// Get the current track index (0-based).
    pub fn index(&self) -> Option<usize> {
        self.current_index
    }

    /// Get the total number of tracks.
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Check if the tracklist is empty.
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
        tl.load(vec![
            "file:///a.mp3".into(),
            "file:///b.mp3".into(),
        ]);
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
        tl.next(); // -> b
        tl.next(); // -> c
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
}
