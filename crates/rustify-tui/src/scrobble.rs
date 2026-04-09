use rustify_core::types::Track;

/// Scrobbling state tracker.
pub struct Scrobbler {
    token: String,
    current_track: Option<Track>,
    accumulated_ms: u64,
    last_position_ms: u64,
    scrobbled: bool,
    playing: bool,
}

impl Scrobbler {
    pub fn new(token: String) -> Self {
        Self {
            token,
            current_track: None,
            accumulated_ms: 0,
            last_position_ms: 0,
            scrobbled: false,
            playing: false,
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.token.is_empty()
    }

    /// Call on TrackChanged. Scrobbles previous track if eligible, resets for new track.
    pub fn on_track_changed(&mut self, track: &Track) {
        // Scrobble previous track if eligible
        if let Some(ref prev) = self.current_track {
            if self.is_scrobble_eligible(prev) && !self.scrobbled {
                self.submit_scrobble(prev);
            }
        }

        // Reset for new track
        self.current_track = Some(track.clone());
        self.accumulated_ms = 0;
        self.last_position_ms = 0;
        self.scrobbled = false;

        // Send "now playing"
        if self.is_enabled() {
            self.submit_now_playing(track);
        }
    }

    /// Call on PositionUpdate.
    pub fn on_position_update(&mut self, position_ms: u64) {
        if !self.playing || !self.is_enabled() {
            return;
        }

        // Accumulate play time (handle seeks by checking delta)
        if position_ms > self.last_position_ms {
            let delta = position_ms - self.last_position_ms;
            // Only count reasonable deltas (< 2 seconds) to filter out seeks
            if delta < 2000 {
                self.accumulated_ms += delta;
            }
        }
        self.last_position_ms = position_ms;

        // Check scrobble threshold
        if !self.scrobbled {
            if let Some(ref track) = self.current_track {
                if self.is_scrobble_eligible(track) {
                    self.scrobbled = true;
                    self.submit_scrobble(track);
                }
            }
        }
    }

    /// Call on StateChanged.
    pub fn on_state_changed(&mut self, playing: bool) {
        self.playing = playing;
    }

    fn is_scrobble_eligible(&self, track: &Track) -> bool {
        // Track must be > 30 seconds
        if track.length < 30_000 {
            return false;
        }
        // Played > 50% OR > 4 minutes
        let half = track.length / 2;
        self.accumulated_ms >= half || self.accumulated_ms >= 240_000
    }

    fn submit_now_playing(&self, track: &Track) {
        let token = self.token.clone();
        let track = track.clone();
        std::thread::Builder::new()
            .name("rustify-scrobble".into())
            .spawn(move || {
                let artist = track.artists.first().map(|a| a.as_str()).unwrap_or("Unknown");
                let body = format!(
                    r#"{{"listen_type":"playing_now","payload":[{{"track_metadata":{{"artist_name":"{}","track_name":"{}","release_name":"{}"}}}}]}}"#,
                    escape_json(artist),
                    escape_json(&track.name),
                    escape_json(&track.album)
                );
                let _ = ureq::post("https://api.listenbrainz.org/1/submit-listens")
                    .header("Authorization", &format!("Token {token}"))
                    .header("Content-Type", "application/json")
                    .send(body.as_bytes());
            })
            .ok();
    }

    fn submit_scrobble(&self, track: &Track) {
        let token = self.token.clone();
        let track = track.clone();
        std::thread::Builder::new()
            .name("rustify-scrobble".into())
            .spawn(move || {
                let artist = track.artists.first().map(|a| a.as_str()).unwrap_or("Unknown");
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let body = format!(
                    r#"{{"listen_type":"single","payload":[{{"listened_at":{},"track_metadata":{{"artist_name":"{}","track_name":"{}","release_name":"{}"}}}}]}}"#,
                    now,
                    escape_json(artist),
                    escape_json(&track.name),
                    escape_json(&track.album)
                );
                let _ = ureq::post("https://api.listenbrainz.org/1/submit-listens")
                    .header("Authorization", &format!("Token {token}"))
                    .header("Content-Type", "application/json")
                    .send(body.as_bytes());
            })
            .ok();
    }
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(length_ms: u64) -> Track {
        Track {
            uri: "file:///test.mp3".into(),
            name: "Test".into(),
            artists: vec!["Artist".into()],
            album: "Album".into(),
            length: length_ms,
            track_no: None,
        }
    }

    #[test]
    fn scrobble_eligible_at_50_percent() {
        let mut s = Scrobbler::new(String::new());
        s.current_track = Some(make_track(200_000));
        s.accumulated_ms = 100_001; // > 50%
        assert!(s.is_scrobble_eligible(&make_track(200_000)));
    }

    #[test]
    fn scrobble_eligible_at_4_minutes() {
        let mut s = Scrobbler::new(String::new());
        s.accumulated_ms = 240_001;
        assert!(s.is_scrobble_eligible(&make_track(600_000)));
    }

    #[test]
    fn not_eligible_short_play() {
        let mut s = Scrobbler::new(String::new());
        s.accumulated_ms = 30_000; // 30s of a 200s track
        assert!(!s.is_scrobble_eligible(&make_track(200_000)));
    }

    #[test]
    fn not_eligible_short_track() {
        let mut s = Scrobbler::new(String::new());
        s.accumulated_ms = 25_000;
        assert!(!s.is_scrobble_eligible(&make_track(25_000))); // < 30s track
    }

    #[test]
    fn disabled_with_empty_token() {
        let s = Scrobbler::new(String::new());
        assert!(!s.is_enabled());
    }

    #[test]
    fn enabled_with_token() {
        let s = Scrobbler::new("my-token".into());
        assert!(s.is_enabled());
    }
}
