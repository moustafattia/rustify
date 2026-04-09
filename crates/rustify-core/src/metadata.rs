use std::path::Path;

use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;

use crate::error::RustifyError;
use crate::types::{path_to_uri, uri_to_path, Track};

/// Read audio metadata from a file URI or plain path.
/// Falls back to filename-derived metadata if tags are missing.
pub fn read_metadata(uri: &str) -> Result<Track, RustifyError> {
    let path = uri_to_path(uri);
    read_metadata_from_path(&path)
}

/// Read audio metadata from a filesystem path.
pub fn read_metadata_from_path(path: &Path) -> Result<Track, RustifyError> {
    let tagged_file = Probe::open(path)
        .map_err(|e| RustifyError::Metadata(format!("failed to open {}: {e}", path.display())))?
        .read()
        .map_err(|e| {
            RustifyError::Metadata(format!("failed to read tags from {}: {e}", path.display()))
        })?;

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag());

    let name = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

    let artists = tag
        .and_then(|t| t.artist().map(|s| vec![s.to_string()]))
        .unwrap_or_default();

    let album = tag
        .and_then(|t| t.album().map(|s| s.to_string()))
        .unwrap_or_default();

    let track_no = tag.and_then(|t| t.track());

    let length = tagged_file.properties().duration().as_millis() as u64;

    Ok(Track {
        uri: path_to_uri(path),
        name,
        artists,
        album,
        length,
        track_no,
    })
}

/// Read ReplayGain track gain from audio file tags.
/// Returns the gain adjustment in dB, or None if no tag found.
pub fn read_replay_gain(path: &Path) -> Option<f32> {
    let tagged_file = Probe::open(path).ok()?.read().ok()?;
    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    if let Some(val) = tag.get_string(&ItemKey::ReplayGainTrackGain) {
        return parse_replay_gain_value(val);
    }
    None
}

fn parse_replay_gain_value(val: &str) -> Option<f32> {
    let trimmed = val
        .trim()
        .trim_end_matches(" dB")
        .trim_end_matches("dB")
        .trim();
    trimmed.parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Create a minimal valid WAV file (44-byte header + 1 second of silence).
    fn create_test_wav() -> NamedTempFile {
        let mut file = NamedTempFile::with_suffix(".wav").unwrap();
        let sample_rate: u32 = 44100;
        let channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let num_samples: u32 = sample_rate; // 1 second
        let data_size: u32 = num_samples * (bits_per_sample / 8) as u32 * channels as u32;
        let file_size: u32 = 36 + data_size;

        // RIFF header
        file.write_all(b"RIFF").unwrap();
        file.write_all(&file_size.to_le_bytes()).unwrap();
        file.write_all(b"WAVE").unwrap();
        // fmt chunk
        file.write_all(b"fmt ").unwrap();
        file.write_all(&16u32.to_le_bytes()).unwrap(); // chunk size
        file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM format
        file.write_all(&channels.to_le_bytes()).unwrap();
        file.write_all(&sample_rate.to_le_bytes()).unwrap();
        let byte_rate = sample_rate * channels as u32 * (bits_per_sample / 8) as u32;
        file.write_all(&byte_rate.to_le_bytes()).unwrap();
        let block_align = channels * (bits_per_sample / 8);
        file.write_all(&block_align.to_le_bytes()).unwrap();
        file.write_all(&bits_per_sample.to_le_bytes()).unwrap();
        // data chunk
        file.write_all(b"data").unwrap();
        file.write_all(&data_size.to_le_bytes()).unwrap();
        // Write silence
        let silence = vec![0u8; data_size as usize];
        file.write_all(&silence).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn read_metadata_from_wav_falls_back_to_filename() {
        let wav = create_test_wav();
        let track = read_metadata_from_path(wav.path()).unwrap();

        let expected_name = wav
            .path()
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert_eq!(track.name, expected_name);
        assert!(track.artists.is_empty());
        assert!(track.album.is_empty());
    }

    #[test]
    fn read_metadata_returns_file_uri() {
        let wav = create_test_wav();
        let track = read_metadata_from_path(wav.path()).unwrap();
        assert!(track.uri.starts_with("file://"));
    }

    #[test]
    fn read_metadata_reports_duration() {
        let wav = create_test_wav();
        let track = read_metadata_from_path(wav.path()).unwrap();
        assert!(track.length > 900 && track.length < 1100);
    }

    #[test]
    fn read_metadata_via_uri() {
        let wav = create_test_wav();
        let uri = path_to_uri(wav.path());
        let track = read_metadata(&uri).unwrap();
        assert!(track.length > 0);
    }

    #[test]
    fn read_metadata_nonexistent_file_returns_error() {
        let result = read_metadata("file:///nonexistent/song.mp3");
        assert!(result.is_err());
    }

    #[test]
    fn parse_replay_gain_values() {
        assert_eq!(parse_replay_gain_value("-6.5 dB"), Some(-6.5));
        assert_eq!(parse_replay_gain_value("+3.2 dB"), Some(3.2));
        assert_eq!(parse_replay_gain_value("-6.5dB"), Some(-6.5));
        assert_eq!(parse_replay_gain_value("not a number"), None);
    }
}
