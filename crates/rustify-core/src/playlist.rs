use std::fs;
use std::path::{Path, PathBuf};

use crate::error::RustifyError;
use crate::types::{path_to_uri, Playlist, AUDIO_EXTENSIONS};

/// Parse an M3U playlist file and return resolved file:// URIs.
///
/// Handles simple M3U and extended M3U (`#EXTM3U` / `#EXTINF`).
/// Relative paths are resolved against the M3U file's parent directory.
/// Only entries with supported audio extensions are included.
pub fn parse_m3u(path: &Path) -> Result<Vec<String>, RustifyError> {
    let content = fs::read_to_string(path)
        .map_err(|e| RustifyError::Playlist(format!("failed to read {}: {e}", path.display())))?;

    let base_dir = path
        .parent()
        .ok_or_else(|| RustifyError::Playlist("M3U path has no parent directory".into()))?;

    let mut uris = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Check both OS-native absolute paths and Unix-style /paths
        // (the latter handles M3U files from Linux when developing on Windows)
        let track_path = if Path::new(line).is_absolute() || line.starts_with('/') {
            PathBuf::from(line)
        } else {
            base_dir.join(line)
        };

        if let Some(ext) = track_path.extension().and_then(|e| e.to_str()) {
            if AUDIO_EXTENSIONS
                .iter()
                .any(|&ae| ae.eq_ignore_ascii_case(ext))
            {
                uris.push(path_to_uri(&track_path));
            }
        }
    }

    Ok(uris)
}

/// Find all .m3u playlist files in a directory (non-recursive).
/// Returns metadata about each playlist including track count.
pub fn find_playlists(dir: &Path) -> Result<Vec<Playlist>, RustifyError> {
    let mut playlists = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("m3u") {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let track_count = parse_m3u(&path).map(|uris| uris.len()).unwrap_or(0);

            playlists.push(Playlist {
                uri: path_to_uri(&path),
                name,
                track_count,
            });
        }
    }

    Ok(playlists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_m3u(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    fn touch(dir: &Path, name: &str) {
        fs::write(dir.join(name), b"").unwrap();
    }

    #[test]
    fn parse_simple_m3u_absolute_paths() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(
            dir.path(),
            "test.m3u",
            "/music/song1.mp3\n/music/song2.flac\n",
        );
        let uris = parse_m3u(&m3u).unwrap();
        assert_eq!(uris.len(), 2);
        assert!(uris[0].starts_with("file://"));
        assert!(uris[0].ends_with("/music/song1.mp3"));
        assert!(uris[1].ends_with("/music/song2.flac"));
    }

    #[test]
    fn parse_m3u_relative_paths() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(
            dir.path(),
            "test.m3u",
            "songs/track.mp3\n../other/track.flac\n",
        );
        let uris = parse_m3u(&m3u).unwrap();
        assert_eq!(uris.len(), 2);
        assert!(uris[0].contains("songs"));
        assert!(uris[1].contains("other"));
    }

    #[test]
    fn parse_extended_m3u_skips_directives() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(
            dir.path(),
            "test.m3u",
            "#EXTM3U\n#EXTINF:123,Artist - Title\n/music/song.mp3\n",
        );
        let uris = parse_m3u(&m3u).unwrap();
        assert_eq!(uris.len(), 1);
        assert!(uris[0].starts_with("file://"));
        assert!(uris[0].ends_with("/music/song.mp3"));
    }

    #[test]
    fn parse_m3u_skips_blank_lines_and_comments() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(dir.path(), "test.m3u", "\n# comment\n\n/music/song.mp3\n\n");
        let uris = parse_m3u(&m3u).unwrap();
        assert_eq!(uris.len(), 1);
    }

    #[test]
    fn parse_m3u_filters_unsupported_extensions() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(
            dir.path(),
            "test.m3u",
            "/music/song.mp3\n/music/image.png\n/music/doc.txt\n",
        );
        let uris = parse_m3u(&m3u).unwrap();
        assert_eq!(uris.len(), 1);
        assert!(uris[0].ends_with(".mp3"));
    }

    #[test]
    fn parse_m3u_case_insensitive_extensions() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(
            dir.path(),
            "test.m3u",
            "/music/song.MP3\n/music/song.Flac\n",
        );
        let uris = parse_m3u(&m3u).unwrap();
        assert_eq!(uris.len(), 2);
    }

    #[test]
    fn parse_m3u_nonexistent_file_returns_error() {
        let result = parse_m3u(Path::new("/nonexistent/playlist.m3u"));
        assert!(result.is_err());
    }

    #[test]
    fn parse_empty_m3u() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(dir.path(), "empty.m3u", "");
        let uris = parse_m3u(&m3u).unwrap();
        assert!(uris.is_empty());
    }

    #[test]
    fn find_playlists_in_directory() {
        let dir = TempDir::new().unwrap();
        create_m3u(dir.path(), "chill.m3u", "/music/a.mp3\n/music/b.flac\n");
        create_m3u(dir.path(), "rock.m3u", "/music/c.ogg\n");
        touch(dir.path(), "readme.txt");

        let playlists = find_playlists(dir.path()).unwrap();
        assert_eq!(playlists.len(), 2);

        let names: Vec<&str> = playlists.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"chill"));
        assert!(names.contains(&"rock"));

        let chill = playlists.iter().find(|p| p.name == "chill").unwrap();
        assert_eq!(chill.track_count, 2);
    }
}
