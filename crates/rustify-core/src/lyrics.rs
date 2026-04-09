use std::fs;
use std::path::Path;

use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;

/// Lyrics content.
#[derive(Debug, Clone)]
pub enum Lyrics {
    /// Timestamped lines from .lrc file: (timestamp_ms, line_text)
    Synced(Vec<(u64, String)>),
    /// Plain text lyrics from audio tags
    Unsynced(String),
}

/// Extract lyrics for a track.
/// Tries embedded tags first, then .lrc sidecar file.
pub fn extract_lyrics(path: &Path) -> Option<Lyrics> {
    if let Some(lyrics) = extract_embedded_lyrics(path) {
        return Some(lyrics);
    }
    extract_lrc_sidecar(path)
}

fn extract_embedded_lyrics(path: &Path) -> Option<Lyrics> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag())?;

    if let Some(text) = tag.get_string(&ItemKey::Lyrics) {
        if !text.trim().is_empty() {
            return Some(Lyrics::Unsynced(text.to_string()));
        }
    }
    None
}

fn extract_lrc_sidecar(path: &Path) -> Option<Lyrics> {
    let lrc_path = path.with_extension("lrc");
    let content = fs::read_to_string(&lrc_path).ok()?;
    Some(parse_lrc(&content))
}

/// Parse LRC format into Lyrics.
pub fn parse_lrc(content: &str) -> Lyrics {
    let mut lines: Vec<(u64, String)> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((ts, text)) = parse_lrc_line(line) {
            lines.push((ts, text));
        }
    }

    if lines.is_empty() {
        Lyrics::Unsynced(content.to_string())
    } else {
        lines.sort_by_key(|(ts, _)| *ts);
        Lyrics::Synced(lines)
    }
}

fn parse_lrc_line(line: &str) -> Option<(u64, String)> {
    // Format: [mm:ss.xx]Text
    let close = line.find(']')?;
    if !line.starts_with('[') {
        return None;
    }
    let timestamp = &line[1..close];
    let text = line[close + 1..].to_string();

    let parts: Vec<&str> = timestamp.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let mins: u64 = parts[0].parse().ok()?;
    let secs: f64 = parts[1].parse().ok()?;
    let ms = mins * 60_000 + (secs * 1000.0) as u64;

    Some((ms, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parse_lrc_synced() {
        let content = "[00:12.50]First line\n[00:15.00]Second line\n";
        match parse_lrc(content) {
            Lyrics::Synced(lines) => {
                assert_eq!(lines.len(), 2);
                assert_eq!(lines[0].0, 12500);
                assert_eq!(lines[0].1, "First line");
                assert_eq!(lines[1].0, 15000);
            }
            _ => panic!("Expected synced lyrics"),
        }
    }

    #[test]
    fn parse_lrc_empty_returns_unsynced() {
        let content = "Just plain text\nNo timestamps here";
        match parse_lrc(content) {
            Lyrics::Unsynced(_) => {}
            _ => panic!("Expected unsynced lyrics"),
        }
    }

    #[test]
    fn sidecar_lrc_found() {
        let dir = TempDir::new().unwrap();
        let track_path = dir.path().join("song.mp3");
        let lrc_path = dir.path().join("song.lrc");
        fs::write(&track_path, b"").unwrap();
        fs::write(&lrc_path, "[00:05.00]Hello world\n").unwrap();

        let lyrics = extract_lyrics(&track_path);
        assert!(lyrics.is_some());
        match lyrics.unwrap() {
            Lyrics::Synced(lines) => assert_eq!(lines[0].1, "Hello world"),
            _ => panic!("Expected synced"),
        }
    }

    #[test]
    fn no_lyrics_returns_none() {
        let dir = TempDir::new().unwrap();
        let track_path = dir.path().join("song.mp3");
        fs::write(&track_path, b"").unwrap();
        let lyrics = extract_lyrics(&track_path);
        assert!(lyrics.is_none());
    }
}
