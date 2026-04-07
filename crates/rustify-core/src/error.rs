use std::fmt;
use std::io;

/// Unified error type for all rustify-core operations.
#[derive(Debug)]
pub enum RustifyError {
    /// I/O errors (file not found, permission denied, etc.)
    Io(io::Error),
    /// Audio decoding errors (corrupt file, unsupported codec)
    Decode(String),
    /// Audio output errors (device not found, ALSA error)
    Audio(String),
    /// Metadata reading errors (corrupt tags, unsupported format)
    Metadata(String),
    /// Playlist parsing errors (invalid M3U, missing files)
    Playlist(String),
}

impl fmt::Display for RustifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "IO error: {err}"),
            Self::Decode(msg) => write!(f, "decode error: {msg}"),
            Self::Audio(msg) => write!(f, "audio error: {msg}"),
            Self::Metadata(msg) => write!(f, "metadata error: {msg}"),
            Self::Playlist(msg) => write!(f, "playlist error: {msg}"),
        }
    }
}

impl std::error::Error for RustifyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for RustifyError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Result type alias for rustify-core operations.
pub type Result<T> = std::result::Result<T, RustifyError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_io_error() {
        let err = RustifyError::Io(io::Error::new(io::ErrorKind::NotFound, "gone"));
        assert!(err.to_string().contains("IO error"));
        assert!(err.to_string().contains("gone"));
    }

    #[test]
    fn display_decode_error() {
        let err = RustifyError::Decode("bad frame".into());
        assert_eq!(err.to_string(), "decode error: bad frame");
    }

    #[test]
    fn from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "nope");
        let err: RustifyError = io_err.into();
        assert!(matches!(err, RustifyError::Io(_)));
    }

    #[test]
    fn error_source_for_io() {
        let err = RustifyError::Io(io::Error::new(io::ErrorKind::NotFound, "x"));
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn error_source_for_non_io() {
        let err = RustifyError::Decode("x".into());
        assert!(std::error::Error::source(&err).is_none());
    }
}
