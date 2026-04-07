use std::path::Path;

use walkdir::WalkDir;

use crate::error::RustifyError;
use crate::types::{path_to_uri, AUDIO_EXTENSIONS};

/// Recursively scan a directory for audio files.
/// Returns sorted `file://` URIs for all files matching supported extensions.
pub fn scan_directory(path: &Path) -> Result<Vec<String>, RustifyError> {
    if !path.is_dir() {
        return Err(RustifyError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("not a directory: {}", path.display()),
        )));
    }

    let mut uris = Vec::new();

    for entry in WalkDir::new(path).follow_links(true) {
        let entry = entry.map_err(|e| {
            RustifyError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;

        if !entry.file_type().is_file() {
            continue;
        }

        if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
            if AUDIO_EXTENSIONS
                .iter()
                .any(|&ae| ae.eq_ignore_ascii_case(ext))
            {
                uris.push(path_to_uri(entry.path()));
            }
        }
    }

    uris.sort();
    Ok(uris)
}

/// List the contents of a single directory (non-recursive).
/// Returns URIs for audio files and subdirectories.
pub fn browse_directory(path: &Path) -> Result<Vec<String>, RustifyError> {
    if !path.is_dir() {
        return Err(RustifyError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("not a directory: {}", path.display()),
        )));
    }

    let mut entries = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_dir() {
            entries.push(path_to_uri(&entry_path));
        } else if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
            if AUDIO_EXTENSIONS
                .iter()
                .any(|&ae| ae.eq_ignore_ascii_case(ext))
            {
                entries.push(path_to_uri(&entry_path));
            }
        }
    }

    entries.sort();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"").unwrap();
    }

    #[test]
    fn scan_finds_audio_files() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("song.mp3"));
        touch(&dir.path().join("track.flac"));
        touch(&dir.path().join("sound.ogg"));
        touch(&dir.path().join("clip.wav"));

        let uris = scan_directory(dir.path()).unwrap();
        assert_eq!(uris.len(), 4);
    }

    #[test]
    fn scan_ignores_non_audio_files() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("song.mp3"));
        touch(&dir.path().join("readme.txt"));
        touch(&dir.path().join("image.png"));
        touch(&dir.path().join("cover.jpg"));

        let uris = scan_directory(dir.path()).unwrap();
        assert_eq!(uris.len(), 1);
        assert!(uris[0].ends_with(".mp3"));
    }

    #[test]
    fn scan_recurses_into_subdirectories() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("artist1/album1/track1.mp3"));
        touch(&dir.path().join("artist1/album2/track2.flac"));
        touch(&dir.path().join("artist2/track3.ogg"));

        let uris = scan_directory(dir.path()).unwrap();
        assert_eq!(uris.len(), 3);
    }

    #[test]
    fn scan_returns_sorted_uris() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("c.mp3"));
        touch(&dir.path().join("a.mp3"));
        touch(&dir.path().join("b.mp3"));

        let uris = scan_directory(dir.path()).unwrap();
        assert!(uris[0] < uris[1]);
        assert!(uris[1] < uris[2]);
    }

    #[test]
    fn scan_case_insensitive_extensions() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("LOUD.MP3"));
        touch(&dir.path().join("quiet.Flac"));

        let uris = scan_directory(dir.path()).unwrap();
        assert_eq!(uris.len(), 2);
    }

    #[test]
    fn scan_returns_file_uris() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("song.mp3"));

        let uris = scan_directory(dir.path()).unwrap();
        assert!(uris[0].starts_with("file://"));
    }

    #[test]
    fn scan_nonexistent_directory_returns_error() {
        let result = scan_directory(Path::new("/nonexistent/path"));
        assert!(result.is_err());
    }

    #[test]
    fn scan_empty_directory() {
        let dir = TempDir::new().unwrap();
        let uris = scan_directory(dir.path()).unwrap();
        assert!(uris.is_empty());
    }

    #[test]
    fn browse_lists_files_and_dirs() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("song.mp3"));
        fs::create_dir(dir.path().join("subdir")).unwrap();
        touch(&dir.path().join("readme.txt"));

        let entries = browse_directory(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn browse_does_not_recurse() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("top.mp3"));
        touch(&dir.path().join("sub/nested.mp3"));

        let entries = browse_directory(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
