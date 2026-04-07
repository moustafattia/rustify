use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Metadata for a single audio track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// File URI (e.g., "file:///path/to/song.mp3")
    pub uri: String,
    /// Track title (falls back to filename if no tags)
    pub name: String,
    /// Artist names
    pub artists: Vec<String>,
    /// Album name
    pub album: String,
    /// Duration in milliseconds
    pub length: u64,
    /// Track number within album
    pub track_no: Option<u32>,
}

/// Metadata about a playlist file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    /// File URI of the .m3u file
    pub uri: String,
    /// Playlist name (derived from filename)
    pub name: String,
    /// Number of tracks in the playlist
    pub track_count: usize,
}

/// Playback state of the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Events emitted by the player to registered callbacks.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    StateChanged(PlaybackState),
    TrackChanged(Track),
    PositionUpdate(u64),
    Error(String),
}

/// Commands sent to the player's command thread.
#[derive(Debug)]
pub enum PlayerCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek(u64),
    SetVolume(u8),
    LoadTrackUris(Vec<String>),
    ClearTracklist,
    Shutdown,
}

/// Convert a `file://` URI to a filesystem path.
/// Also accepts plain paths (returned as-is).
pub fn uri_to_path(uri: &str) -> PathBuf {
    uri.strip_prefix("file://")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(uri))
}

/// Convert a filesystem path to a `file://` URI.
pub fn path_to_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_creation() {
        let track = Track {
            uri: "file:///music/song.mp3".into(),
            name: "Song".into(),
            artists: vec!["Artist".into()],
            album: "Album".into(),
            length: 180_000,
            track_no: Some(1),
        };
        assert_eq!(track.name, "Song");
        assert_eq!(track.length, 180_000);
    }

    #[test]
    fn track_serde_roundtrip() {
        let track = Track {
            uri: "file:///music/song.mp3".into(),
            name: "Song".into(),
            artists: vec!["Artist".into()],
            album: "Album".into(),
            length: 180_000,
            track_no: Some(1),
        };
        let json = serde_json::to_string(&track).unwrap();
        let decoded: Track = serde_json::from_str(&json).unwrap();
        assert_eq!(track, decoded);
    }

    #[test]
    fn uri_to_path_with_scheme() {
        let path = uri_to_path("file:///home/pi/Music/song.mp3");
        assert_eq!(path, PathBuf::from("/home/pi/Music/song.mp3"));
    }

    #[test]
    fn uri_to_path_plain_path() {
        let path = uri_to_path("/home/pi/Music/song.mp3");
        assert_eq!(path, PathBuf::from("/home/pi/Music/song.mp3"));
    }

    #[test]
    fn path_to_uri_conversion() {
        let uri = path_to_uri(Path::new("/home/pi/Music/song.mp3"));
        assert_eq!(uri, "file:///home/pi/Music/song.mp3");
    }

    #[test]
    fn playback_state_equality() {
        assert_eq!(PlaybackState::Stopped, PlaybackState::Stopped);
        assert_ne!(PlaybackState::Playing, PlaybackState::Paused);
    }
}
