use std::fs;
use std::path::Path;

use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;

/// Sidecar filenames to search for album art (checked case-insensitively).
const SIDECAR_NAMES: &[&str] = &[
    "cover.jpg",
    "cover.png",
    "folder.jpg",
    "folder.png",
    "album.jpg",
    "album.png",
];

/// Extract album art for a track file.
/// Tries embedded cover art first (via lofty), then sidecar files in the
/// track's directory. Returns raw image bytes (JPEG or PNG) or None.
pub fn extract_art(path: &Path) -> Option<Vec<u8>> {
    // Try embedded art first
    if let Some(art) = extract_embedded(path) {
        return Some(art);
    }

    // Fall back to sidecar files
    extract_sidecar(path)
}

/// Extract embedded cover art from audio file tags.
fn extract_embedded(path: &Path) -> Option<Vec<u8>> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    // Prefer CoverFront, fall back to any picture
    let picture = tag
        .pictures()
        .iter()
        .find(|p| p.pic_type() == PictureType::CoverFront)
        .or_else(|| tag.pictures().first())?;

    Some(picture.data().to_vec())
}

/// Search the track's parent directory for sidecar art files.
fn extract_sidecar(path: &Path) -> Option<Vec<u8>> {
    let parent = path.parent()?;

    let entries: Vec<_> = fs::read_dir(parent).ok()?.filter_map(|e| e.ok()).collect();

    for sidecar_name in SIDECAR_NAMES {
        for entry in &entries {
            let file_name = entry.file_name();
            let name_str = file_name.to_string_lossy();
            if name_str.eq_ignore_ascii_case(sidecar_name) {
                if let Ok(data) = fs::read(entry.path()) {
                    return Some(data);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn sidecar_cover_found() {
        let dir = TempDir::new().unwrap();
        let cover_path = dir.path().join("cover.jpg");
        fs::write(&cover_path, b"fake-jpeg-data").unwrap();

        let track_path = dir.path().join("song.mp3");
        fs::write(&track_path, b"").unwrap();

        let art = extract_art(&track_path);
        assert!(art.is_some());
        assert_eq!(art.unwrap(), b"fake-jpeg-data");
    }

    #[test]
    fn sidecar_folder_jpg_found() {
        let dir = TempDir::new().unwrap();
        let cover_path = dir.path().join("folder.jpg");
        fs::write(&cover_path, b"folder-art").unwrap();

        let track_path = dir.path().join("song.mp3");
        fs::write(&track_path, b"").unwrap();

        let art = extract_art(&track_path);
        assert!(art.is_some());
        assert_eq!(art.unwrap(), b"folder-art");
    }

    #[test]
    fn no_art_returns_none() {
        let dir = TempDir::new().unwrap();
        let track_path = dir.path().join("song.mp3");
        fs::write(&track_path, b"").unwrap();

        let art = extract_art(&track_path);
        assert!(art.is_none());
    }

    #[test]
    fn sidecar_case_insensitive() {
        let dir = TempDir::new().unwrap();
        let cover_path = dir.path().join("Cover.JPG");
        fs::write(&cover_path, b"case-insensitive").unwrap();

        let track_path = dir.path().join("song.mp3");
        fs::write(&track_path, b"").unwrap();

        let art = extract_art(&track_path);
        assert!(art.is_some());
    }
}
