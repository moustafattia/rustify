# Rustify Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build rustify-core (pure Rust media player library) and PyO3 Python bindings for the YoyoPod project.

**Architecture:** Three-thread model (command, decode, output) connected by crossbeam channels. symphonia decodes audio, cpal outputs to ALSA. A lock-free ring buffer (bounded crossbeam channel) decouples decode from output. Exposed to Python via PyO3/maturin as `RustifyClient`.

**Tech Stack:** Rust (symphonia, cpal, crossbeam, walkdir, lofty, serde), Python (PyO3 0.23 / maturin 1.x)

**Design Spec:** `docs/specs/2026-04-07-rustify-embedded-player-design.md`

---

## File Map

### New files (rustify-core)

| File | Responsibility |
|---|---|
| `Cargo.toml` | Workspace root |
| `crates/rustify-core/Cargo.toml` | Core library dependencies |
| `crates/rustify-core/src/lib.rs` | Module declarations + re-exports |
| `crates/rustify-core/src/error.rs` | `RustifyError` enum, `Result<T>` alias |
| `crates/rustify-core/src/types.rs` | `Track`, `Playlist`, `PlaybackState`, `PlayerEvent`, `PlayerCommand`, URI helpers |
| `crates/rustify-core/src/mixer.rs` | `Mixer` — atomic volume control (0-100) |
| `crates/rustify-core/src/tracklist.rs` | `Tracklist` — VecDeque-backed playback queue |
| `crates/rustify-core/src/playlist.rs` | M3U parser + playlist discovery |
| `crates/rustify-core/src/scanner.rs` | Recursive audio file discovery via walkdir |
| `crates/rustify-core/src/metadata.rs` | Tag reading via lofty, filename fallback |
| `crates/rustify-core/src/player.rs` | Playback engine: command loop, decode thread, cpal output |

### New files (Python bindings)

| File | Responsibility |
|---|---|
| `bindings/python/Cargo.toml` | PyO3 cdylib crate |
| `bindings/python/src/lib.rs` | `#[pymodule]` entry point |
| `bindings/python/src/client.rs` | `RustifyClient` pyclass |
| `bindings/python/rustify/__init__.py` | Re-export from native module |
| `bindings/python/rustify/py.typed` | PEP 561 marker |
| `pyproject.toml` | maturin build config |

### Other files

| File | Responsibility |
|---|---|
| `examples/play.rs` | Standalone CLI player for hardware testing |
| `.gitignore` | Rust/Python ignores |
| `.github/workflows/ci.yml` | Test + clippy + fmt + wheel build |

---

### Task 1: Scaffold Workspace

**Files:**
- Create: `Cargo.toml`
- Create: `crates/rustify-core/Cargo.toml`
- Create: `crates/rustify-core/src/lib.rs`
- Create: `bindings/python/Cargo.toml`
- Create: `bindings/python/src/lib.rs`
- Create: `pyproject.toml`
- Create: `.gitignore`
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create workspace root `Cargo.toml`**

```toml
[workspace]
members = ["crates/rustify-core", "bindings/python"]
default-members = ["crates/rustify-core"]
resolver = "2"
```

- [ ] **Step 2: Create `crates/rustify-core/Cargo.toml`**

```toml
[package]
name = "rustify-core"
version = "0.1.0"
edition = "2021"
description = "Embedded Rust media player library for YoyoPod"

[dependencies]
symphonia = { version = "0.5", default-features = false, features = ["mp3", "flac", "ogg", "wav", "pcm"] }
cpal = "0.15"
crossbeam = "0.8"
walkdir = "2"
lofty = "0.22"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
tempfile = "3"
hound = "3"

[[example]]
name = "play"
path = "../../examples/play.rs"
```

Note: cpal and lofty version numbers come from the design spec (April 2026). If `cargo check` fails on versions, check crates.io for the latest compatible version and adjust.

- [ ] **Step 3: Create `crates/rustify-core/src/lib.rs`**

```rust
// Modules will be added as they are implemented.
```

- [ ] **Step 4: Create `bindings/python/Cargo.toml`**

```toml
[package]
name = "rustify-python"
version = "0.1.0"
edition = "2021"

[lib]
name = "_rustify"
crate-type = ["cdylib"]

[dependencies]
rustify-core = { path = "../../crates/rustify-core" }
pyo3 = { version = "0.23", features = ["extension-module"] }
```

- [ ] **Step 5: Create `bindings/python/src/lib.rs`**

```rust
use pyo3::prelude::*;

#[pymodule]
fn _rustify(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
```

- [ ] **Step 6: Create `pyproject.toml`**

```toml
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[project]
name = "rustify"
version = "0.1.0"
description = "Embedded Rust media player for YoyoPod"
requires-python = ">=3.9"

[tool.maturin]
features = ["pyo3/extension-module"]
manifest-path = "bindings/python/Cargo.toml"
python-source = "bindings/python"
module-name = "rustify._rustify"
```

- [ ] **Step 7: Create `.gitignore`**

```
/target/
**/*.rs.bk
*.pdb
*.so
*.dylib
*.dll

# Python
__pycache__/
*.py[cod]
*.egg-info/
dist/
*.whl

# IDE
.idea/
.vscode/
*.swp
```

- [ ] **Step 8: Create `.github/workflows/ci.yml`**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install ALSA dev headers
        run: sudo apt-get install -y libasound2-dev
      - run: cargo test --workspace
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check

  build-wheel:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install ALSA dev headers
        run: sudo apt-get install -y libasound2-dev
      - uses: PyO3/maturin-action@v1
        with:
          args: --release
```

- [ ] **Step 9: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: Compiles with no errors (warnings are OK at this stage).

If cpal or lofty version is not found, check crates.io and adjust the version in `crates/rustify-core/Cargo.toml`.

- [ ] **Step 10: Commit**

```bash
git add Cargo.toml crates/ bindings/ pyproject.toml .gitignore .github/
git commit -m "feat: scaffold Cargo workspace with rustify-core and Python bindings"
```

---

### Task 2: error.rs + types.rs

**Files:**
- Create: `crates/rustify-core/src/error.rs`
- Create: `crates/rustify-core/src/types.rs`
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Write tests for error.rs**

Add to `crates/rustify-core/src/error.rs`:

```rust
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
```

- [ ] **Step 2: Write types.rs**

Create `crates/rustify-core/src/types.rs`:

```rust
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
```

- [ ] **Step 3: Wire up modules in lib.rs**

Replace `crates/rustify-core/src/lib.rs` with:

```rust
pub mod error;
pub mod types;
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p rustify-core`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rustify-core/src/
git commit -m "feat: add error and types modules with tests"
```

---

### Task 3: mixer.rs

**Files:**
- Create: `crates/rustify-core/src/mixer.rs`
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Write mixer.rs with tests**

Create `crates/rustify-core/src/mixer.rs`:

```rust
use std::sync::atomic::{AtomicU8, Ordering};

/// Lock-free volume control using atomic operations.
/// Volume ranges from 0 (silent) to 100 (full).
pub struct Mixer {
    volume: AtomicU8,
}

impl Mixer {
    /// Create a new mixer with the given initial volume (clamped to 0-100).
    pub fn new(initial_volume: u8) -> Self {
        Self {
            volume: AtomicU8::new(initial_volume.min(100)),
        }
    }

    /// Set the volume (clamped to 0-100).
    pub fn set_volume(&self, volume: u8) {
        self.volume.store(volume.min(100), Ordering::Relaxed);
    }

    /// Get the current volume (0-100).
    pub fn get_volume(&self) -> u8 {
        self.volume.load(Ordering::Relaxed)
    }

    /// Get the gain multiplier (0.0 - 1.0) for applying to audio samples.
    pub fn gain(&self) -> f32 {
        self.get_volume() as f32 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_volume() {
        let mixer = Mixer::new(75);
        assert_eq!(mixer.get_volume(), 75);
    }

    #[test]
    fn clamps_initial_volume_to_100() {
        let mixer = Mixer::new(150);
        assert_eq!(mixer.get_volume(), 100);
    }

    #[test]
    fn set_and_get_volume() {
        let mixer = Mixer::new(50);
        mixer.set_volume(80);
        assert_eq!(mixer.get_volume(), 80);
    }

    #[test]
    fn clamps_set_volume_to_100() {
        let mixer = Mixer::new(50);
        mixer.set_volume(200);
        assert_eq!(mixer.get_volume(), 100);
    }

    #[test]
    fn volume_zero() {
        let mixer = Mixer::new(0);
        assert_eq!(mixer.get_volume(), 0);
        assert_eq!(mixer.gain(), 0.0);
    }

    #[test]
    fn gain_at_full_volume() {
        let mixer = Mixer::new(100);
        assert!((mixer.gain() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn gain_at_half_volume() {
        let mixer = Mixer::new(50);
        assert!((mixer.gain() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn gain_at_zero_volume() {
        let mixer = Mixer::new(0);
        assert!((mixer.gain() - 0.0).abs() < f32::EPSILON);
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

Add to `crates/rustify-core/src/lib.rs`:

```rust
pub mod error;
pub mod mixer;
pub mod types;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-core`
Expected: All tests pass (error, types, and mixer).

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-core/src/mixer.rs crates/rustify-core/src/lib.rs
git commit -m "feat: add lock-free mixer with atomic volume control"
```

---

### Task 4: tracklist.rs

**Files:**
- Create: `crates/rustify-core/src/tracklist.rs`
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Write tracklist.rs with tests**

Create `crates/rustify-core/src/tracklist.rs`:

```rust
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
        // add alone does not set current_index
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
        // Index stays at 0 (doesn't advance past end)
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
```

- [ ] **Step 2: Add module to lib.rs**

Add `pub mod tracklist;` to `crates/rustify-core/src/lib.rs` (keep alphabetical order).

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-core`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-core/src/tracklist.rs crates/rustify-core/src/lib.rs
git commit -m "feat: add tracklist with VecDeque-backed playback queue"
```

---

### Task 5: playlist.rs

**Files:**
- Create: `crates/rustify-core/src/playlist.rs`
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Write playlist.rs with tests**

Create `crates/rustify-core/src/playlist.rs`:

```rust
use std::fs;
use std::path::Path;

use crate::error::RustifyError;
use crate::types::{path_to_uri, Playlist};

/// Supported audio file extensions for playlist entries.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav"];

/// Parse an M3U playlist file and return resolved file:// URIs.
///
/// Handles simple M3U and extended M3U (`#EXTM3U` / `#EXTINF`).
/// Relative paths are resolved against the M3U file's parent directory.
/// Only entries with supported audio extensions are included.
pub fn parse_m3u(path: &Path) -> Result<Vec<String>, RustifyError> {
    let content = fs::read_to_string(path).map_err(|e| {
        RustifyError::Playlist(format!("failed to read {}: {e}", path.display()))
    })?;

    let base_dir = path
        .parent()
        .ok_or_else(|| RustifyError::Playlist("M3U path has no parent directory".into()))?;

    let mut uris = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let track_path = if Path::new(line).is_absolute() {
            Path::new(line).to_path_buf()
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
        assert_eq!(uris[0], "file:///music/song1.mp3");
        assert_eq!(uris[1], "file:///music/song2.flac");
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
        // Should be resolved relative to M3U directory
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
        assert_eq!(uris[0], "file:///music/song.mp3");
    }

    #[test]
    fn parse_m3u_skips_blank_lines_and_comments() {
        let dir = TempDir::new().unwrap();
        let m3u = create_m3u(
            dir.path(),
            "test.m3u",
            "\n# comment\n\n/music/song.mp3\n\n",
        );
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
        let m3u = create_m3u(dir.path(), "test.m3u", "/music/song.MP3\n/music/song.Flac\n");
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
        // Create M3U files with referenced tracks
        create_m3u(dir.path(), "chill.m3u", "/music/a.mp3\n/music/b.flac\n");
        create_m3u(dir.path(), "rock.m3u", "/music/c.ogg\n");
        // Create a non-M3U file (should be ignored)
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
```

- [ ] **Step 2: Add module to lib.rs**

Add `pub mod playlist;` to `crates/rustify-core/src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-core`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-core/src/playlist.rs crates/rustify-core/src/lib.rs
git commit -m "feat: add M3U playlist parser with path resolution"
```

---

### Task 6: scanner.rs

**Files:**
- Create: `crates/rustify-core/src/scanner.rs`
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Write scanner.rs with tests**

Create `crates/rustify-core/src/scanner.rs`:

```rust
use std::path::Path;

use walkdir::WalkDir;

use crate::error::RustifyError;
use crate::types::path_to_uri;

/// Supported audio file extensions.
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav"];

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
        // Should include song.mp3 and subdir, but not readme.txt
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn browse_does_not_recurse() {
        let dir = TempDir::new().unwrap();
        touch(&dir.path().join("top.mp3"));
        touch(&dir.path().join("sub/nested.mp3"));

        let entries = browse_directory(dir.path()).unwrap();
        // Should only include top.mp3 and sub/ directory
        assert_eq!(entries.len(), 2);
    }
}
```

- [ ] **Step 2: Add module to lib.rs**

Add `pub mod scanner;` to `crates/rustify-core/src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-core`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-core/src/scanner.rs crates/rustify-core/src/lib.rs
git commit -m "feat: add recursive audio file scanner with browse support"
```

---

### Task 7: metadata.rs

**Files:**
- Create: `crates/rustify-core/src/metadata.rs`
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Write metadata.rs with tests**

Create `crates/rustify-core/src/metadata.rs`:

```rust
use std::path::Path;

use lofty::prelude::*;
use lofty::probe::Probe;

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

        // WAV files without tags should fall back to filename
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
        // 1 second of audio at 44100Hz should be ~1000ms
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
}
```

Note: The lofty import paths may differ between versions. If `lofty::prelude::*` doesn't exist, use `lofty::file::TaggedFileExt` and `lofty::tag::Accessor` directly. If `Probe::open` doesn't exist, try `lofty::read_from_path(path)`.

- [ ] **Step 2: Add module to lib.rs**

Add `pub mod metadata;` to `crates/rustify-core/src/lib.rs`.

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-core`
Expected: All tests pass. If lofty API differs, adjust the imports.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-core/src/metadata.rs crates/rustify-core/src/lib.rs
git commit -m "feat: add metadata reader with lofty and filename fallback"
```

---

### Task 8: Wire up lib.rs with re-exports

**Files:**
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Update lib.rs with all modules and convenience re-exports**

Replace `crates/rustify-core/src/lib.rs` with:

```rust
pub mod error;
pub mod metadata;
pub mod mixer;
pub mod playlist;
pub mod scanner;
pub mod tracklist;
pub mod types;

// Re-export primary types at crate root for convenience.
pub use error::{Result, RustifyError};
pub use types::{PlaybackState, PlayerCommand, PlayerEvent, Playlist, Track};
```

- [ ] **Step 2: Run full test suite**

Run: `cargo test -p rustify-core`
Expected: All tests pass (error, types, mixer, tracklist, playlist, scanner, metadata).

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p rustify-core -- -D warnings`
Expected: No warnings. Fix any issues before committing.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-core/src/lib.rs
git commit -m "feat: wire up all modules in lib.rs with re-exports"
```

---

### Task 9: player.rs — Playback Engine

This is the largest task. It implements the three-thread architecture: command loop, decode thread, and cpal output.

**Files:**
- Create: `crates/rustify-core/src/player.rs`
- Modify: `crates/rustify-core/src/lib.rs`

- [ ] **Step 1: Write the player module with internal types**

Create `crates/rustify-core/src/player.rs`:

```rust
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use crossbeam::channel::{self, Receiver, Sender, TryRecvError};

use crate::error::RustifyError;
use crate::metadata::read_metadata_from_path;
use crate::mixer::Mixer;
use crate::tracklist::Tracklist;
use crate::types::{uri_to_path, PlaybackState, PlayerCommand, PlayerEvent, Track};

/// Number of audio chunks buffered between decode and output threads.
/// At ~1024 frames per chunk @ 44.1kHz stereo, each chunk is ~23ms.
/// 100 chunks provides ~2.3 seconds of buffer.
const BUFFER_CHUNKS: usize = 100;

// --- Public API ---

/// Configuration for creating a Player.
pub struct PlayerConfig {
    pub alsa_device: String,
    pub music_dirs: Vec<PathBuf>,
}

/// The main player handle. All methods are non-blocking — they send commands
/// to the internal command thread via a crossbeam channel.
pub struct Player {
    cmd_tx: Sender<PlayerCommand>,
    shared: Arc<SharedState>,
    mixer: Arc<Mixer>,
    music_dirs: Vec<PathBuf>,
    _command_thread: Option<JoinHandle<()>>,
}

impl Player {
    /// Create a new player. Spawns the command thread immediately.
    /// The output stream is created lazily on first `play()`.
    pub fn new(config: PlayerConfig) -> Result<Self, RustifyError> {
        let (cmd_tx, cmd_rx) = channel::unbounded::<PlayerCommand>();
        let shared = Arc::new(SharedState::new());
        let mixer = Arc::new(Mixer::new(100));

        let shared_clone = Arc::clone(&shared);
        let mixer_clone = Arc::clone(&mixer);
        let alsa_device = config.alsa_device.clone();

        let handle = thread::Builder::new()
            .name("rustify-cmd".into())
            .spawn(move || {
                let mut cmd_loop = CommandLoop::new(
                    cmd_rx,
                    shared_clone,
                    mixer_clone,
                    alsa_device,
                );
                cmd_loop.run();
            })
            .map_err(|e| RustifyError::Audio(format!("failed to spawn command thread: {e}")))?;

        Ok(Self {
            cmd_tx,
            shared,
            mixer,
            music_dirs: config.music_dirs,
            _command_thread: Some(handle),
        })
    }

    // --- Transport commands (non-blocking, fire-and-forget) ---

    pub fn play(&self) {
        self.cmd_tx.send(PlayerCommand::Play).ok();
    }

    pub fn pause(&self) {
        self.cmd_tx.send(PlayerCommand::Pause).ok();
    }

    pub fn stop(&self) {
        self.cmd_tx.send(PlayerCommand::Stop).ok();
    }

    pub fn next(&self) {
        self.cmd_tx.send(PlayerCommand::Next).ok();
    }

    pub fn previous(&self) {
        self.cmd_tx.send(PlayerCommand::Previous).ok();
    }

    pub fn seek(&self, position_ms: u64) {
        self.cmd_tx.send(PlayerCommand::Seek(position_ms)).ok();
    }

    pub fn set_volume(&self, volume: u8) {
        self.mixer.set_volume(volume);
    }

    pub fn get_volume(&self) -> u8 {
        self.mixer.get_volume()
    }

    pub fn load_track_uris(&self, uris: Vec<String>) {
        self.cmd_tx.send(PlayerCommand::LoadTrackUris(uris)).ok();
    }

    pub fn clear_tracklist(&self) {
        self.cmd_tx.send(PlayerCommand::ClearTracklist).ok();
    }

    pub fn shutdown(&self) {
        self.cmd_tx.send(PlayerCommand::Shutdown).ok();
    }

    // --- State queries (read from shared atomic/mutex state) ---

    pub fn get_playback_state(&self) -> PlaybackState {
        self.shared.get_playback_state()
    }

    pub fn get_current_track(&self) -> Option<Track> {
        self.shared.current_track.lock().unwrap().clone()
    }

    pub fn get_time_position(&self) -> u64 {
        self.shared.time_position_ms.load(Ordering::Relaxed)
    }

    // --- Callback registration ---

    pub fn on_state_change(&self, callback: Box<dyn Fn(PlaybackState) + Send>) {
        self.shared
            .callbacks
            .lock()
            .unwrap()
            .on_state_change
            .push(callback);
    }

    pub fn on_track_change(&self, callback: Box<dyn Fn(Track) + Send>) {
        self.shared
            .callbacks
            .lock()
            .unwrap()
            .on_track_change
            .push(callback);
    }

    pub fn on_position_update(&self, callback: Box<dyn Fn(u64) + Send>) {
        self.shared
            .callbacks
            .lock()
            .unwrap()
            .on_position_update
            .push(callback);
    }

    pub fn on_error(&self, callback: Box<dyn Fn(String) + Send>) {
        self.shared
            .callbacks
            .lock()
            .unwrap()
            .on_error
            .push(callback);
    }
}

// --- Shared State ---

struct SharedState {
    /// Encoded PlaybackState: 0=Stopped, 1=Playing, 2=Paused
    playback_state: AtomicU8,
    current_track: Mutex<Option<Track>>,
    time_position_ms: AtomicU64,
    callbacks: Mutex<Callbacks>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            playback_state: AtomicU8::new(0),
            current_track: Mutex::new(None),
            time_position_ms: AtomicU64::new(0),
            callbacks: Mutex::new(Callbacks::default()),
        }
    }

    fn get_playback_state(&self) -> PlaybackState {
        match self.playback_state.load(Ordering::Relaxed) {
            1 => PlaybackState::Playing,
            2 => PlaybackState::Paused,
            _ => PlaybackState::Stopped,
        }
    }

    fn set_playback_state(&self, state: PlaybackState) {
        let val = match state {
            PlaybackState::Stopped => 0,
            PlaybackState::Playing => 1,
            PlaybackState::Paused => 2,
        };
        self.playback_state.store(val, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct Callbacks {
    on_state_change: Vec<Box<dyn Fn(PlaybackState) + Send>>,
    on_track_change: Vec<Box<dyn Fn(Track) + Send>>,
    on_position_update: Vec<Box<dyn Fn(u64) + Send>>,
    on_error: Vec<Box<dyn Fn(String) + Send>>,
}

// --- Internal Events (decode thread -> command loop) ---

enum InternalEvent {
    TrackChanged(Track),
    Position(u64),
    TrackEnded,
    Error(String),
}

/// Control messages from command loop to decode thread.
enum DecodeControl {
    Pause,
    Resume,
    Seek(u64),
    Stop,
}

struct DecodeHandle {
    control_tx: Sender<DecodeControl>,
    _thread: JoinHandle<()>,
}

// --- Command Loop (runs on dedicated thread) ---

struct CommandLoop {
    cmd_rx: Receiver<PlayerCommand>,
    event_rx: Receiver<InternalEvent>,
    event_tx: Sender<InternalEvent>,
    shared: Arc<SharedState>,
    mixer: Arc<Mixer>,
    tracklist: Tracklist,
    decode_handle: Option<DecodeHandle>,
    audio_tx: Sender<Vec<f32>>,
    _audio_stream: Option<cpal::Stream>,
    clear_buffer: Arc<std::sync::atomic::AtomicBool>,
    alsa_device: String,
}

impl CommandLoop {
    fn new(
        cmd_rx: Receiver<PlayerCommand>,
        shared: Arc<SharedState>,
        mixer: Arc<Mixer>,
        alsa_device: String,
    ) -> Self {
        let (event_tx, event_rx) = channel::unbounded::<InternalEvent>();
        let (audio_tx, audio_rx) = channel::bounded::<Vec<f32>>(BUFFER_CHUNKS);
        let clear_buffer = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // Create the cpal output stream
        let stream =
            create_output_stream(audio_rx, Arc::clone(&mixer), Arc::clone(&clear_buffer));

        Self {
            cmd_rx,
            event_rx,
            event_tx,
            shared,
            mixer,
            tracklist: Tracklist::new(),
            decode_handle: None,
            audio_tx,
            _audio_stream: stream.ok(),
            clear_buffer,
            alsa_device,
        }
    }

    fn run(&mut self) {
        loop {
            crossbeam::select! {
                recv(self.cmd_rx) -> cmd => {
                    match cmd {
                        Ok(PlayerCommand::Shutdown) => {
                            self.stop_decode();
                            break;
                        }
                        Ok(cmd) => self.handle_command(cmd),
                        Err(_) => break, // Sender dropped
                    }
                }
                recv(self.event_rx) -> event => {
                    match event {
                        Ok(evt) => self.handle_event(evt),
                        Err(_) => {} // No decode thread running
                    }
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: PlayerCommand) {
        match cmd {
            PlayerCommand::Play => self.handle_play(),
            PlayerCommand::Pause => self.handle_pause(),
            PlayerCommand::Stop => self.handle_stop(),
            PlayerCommand::Next => self.handle_next(),
            PlayerCommand::Previous => self.handle_previous(),
            PlayerCommand::Seek(ms) => self.handle_seek(ms),
            PlayerCommand::SetVolume(vol) => self.mixer.set_volume(vol),
            PlayerCommand::LoadTrackUris(uris) => {
                self.handle_stop();
                self.tracklist.load(uris);
            }
            PlayerCommand::ClearTracklist => {
                self.handle_stop();
                self.tracklist.clear();
            }
            PlayerCommand::Shutdown => unreachable!(),
        }
    }

    fn handle_event(&mut self, event: InternalEvent) {
        match event {
            InternalEvent::TrackChanged(track) => {
                *self.shared.current_track.lock().unwrap() = Some(track.clone());
                self.emit_callbacks(PlayerEvent::TrackChanged(track));
            }
            InternalEvent::Position(ms) => {
                self.shared.time_position_ms.store(ms, Ordering::Relaxed);
                self.emit_callbacks(PlayerEvent::PositionUpdate(ms));
            }
            InternalEvent::TrackEnded => {
                // Try to advance to next track
                if let Some(uri) = self.tracklist.next() {
                    let uri = uri.to_string();
                    self.stop_decode();
                    self.start_decode(uri);
                } else {
                    // End of tracklist
                    self.stop_decode();
                    self.set_state(PlaybackState::Stopped);
                    *self.shared.current_track.lock().unwrap() = None;
                    self.shared.time_position_ms.store(0, Ordering::Relaxed);
                }
            }
            InternalEvent::Error(msg) => {
                self.emit_callbacks(PlayerEvent::Error(msg));
            }
        }
    }

    fn handle_play(&mut self) {
        match self.shared.get_playback_state() {
            PlaybackState::Stopped => {
                if let Some(uri) = self.tracklist.current() {
                    let uri = uri.to_string();
                    self.start_decode(uri);
                    self.set_state(PlaybackState::Playing);
                }
            }
            PlaybackState::Paused => {
                if let Some(ref handle) = self.decode_handle {
                    handle.control_tx.send(DecodeControl::Resume).ok();
                }
                self.set_state(PlaybackState::Playing);
            }
            PlaybackState::Playing => {} // Already playing
        }
    }

    fn handle_pause(&mut self) {
        if self.shared.get_playback_state() == PlaybackState::Playing {
            if let Some(ref handle) = self.decode_handle {
                handle.control_tx.send(DecodeControl::Pause).ok();
            }
            self.set_state(PlaybackState::Paused);
        }
    }

    fn handle_stop(&mut self) {
        self.stop_decode();
        self.set_state(PlaybackState::Stopped);
        *self.shared.current_track.lock().unwrap() = None;
        self.shared.time_position_ms.store(0, Ordering::Relaxed);
    }

    fn handle_next(&mut self) {
        if let Some(uri) = self.tracklist.next() {
            let uri = uri.to_string();
            self.stop_decode();
            self.start_decode(uri);
            self.set_state(PlaybackState::Playing);
        } else {
            self.handle_stop();
        }
    }

    fn handle_previous(&mut self) {
        if let Some(uri) = self.tracklist.previous() {
            let uri = uri.to_string();
            self.stop_decode();
            self.start_decode(uri);
            self.set_state(PlaybackState::Playing);
        }
    }

    fn handle_seek(&mut self, ms: u64) {
        if let Some(ref handle) = self.decode_handle {
            handle.control_tx.send(DecodeControl::Seek(ms)).ok();
        }
    }

    fn start_decode(&mut self, uri: String) {
        // Clear any stale audio in the buffer
        self.clear_buffer
            .store(true, Ordering::Relaxed);

        let (control_tx, control_rx) = channel::unbounded::<DecodeControl>();
        let audio_tx = self.audio_tx.clone();
        let event_tx = self.event_tx.clone();

        let handle = thread::Builder::new()
            .name("rustify-decode".into())
            .spawn(move || {
                decode_thread(uri, audio_tx, control_rx, event_tx);
            })
            .expect("failed to spawn decode thread");

        self.decode_handle = Some(DecodeHandle {
            control_tx,
            _thread: handle,
        });
    }

    fn stop_decode(&mut self) {
        if let Some(handle) = self.decode_handle.take() {
            handle.control_tx.send(DecodeControl::Stop).ok();
            // Don't join — the thread will exit when it sees Stop or channel disconnect
        }
        self.clear_buffer
            .store(true, Ordering::Relaxed);
    }

    fn set_state(&mut self, state: PlaybackState) {
        self.shared.set_playback_state(state);
        self.emit_callbacks(PlayerEvent::StateChanged(state));
    }

    fn emit_callbacks(&self, event: PlayerEvent) {
        let callbacks = self.shared.callbacks.lock().unwrap();
        match &event {
            PlayerEvent::StateChanged(state) => {
                for cb in &callbacks.on_state_change {
                    cb(*state);
                }
            }
            PlayerEvent::TrackChanged(track) => {
                for cb in &callbacks.on_track_change {
                    cb(track.clone());
                }
            }
            PlayerEvent::PositionUpdate(ms) => {
                for cb in &callbacks.on_position_update {
                    cb(*ms);
                }
            }
            PlayerEvent::Error(msg) => {
                for cb in &callbacks.on_error {
                    cb(msg.clone());
                }
            }
        }
    }
}

// --- Decode Thread ---

fn decode_thread(
    uri: String,
    audio_tx: Sender<Vec<f32>>,
    control_rx: Receiver<DecodeControl>,
    event_tx: Sender<InternalEvent>,
) {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let path = uri_to_path(&uri);

    // Read metadata for TrackChanged event
    match read_metadata_from_path(&path) {
        Ok(track) => {
            event_tx.send(InternalEvent::TrackChanged(track)).ok();
        }
        Err(e) => {
            event_tx
                .send(InternalEvent::Error(format!("metadata: {e}")))
                .ok();
        }
    }

    // Open file with symphonia
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            event_tx
                .send(InternalEvent::Error(format!("open: {e}")))
                .ok();
            return;
        }
    };

    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = match symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) {
        Ok(p) => p,
        Err(e) => {
            event_tx
                .send(InternalEvent::Error(format!("probe: {e}")))
                .ok();
            return;
        }
    };

    let mut format = probed.format;
    let track = match format.default_track() {
        Some(t) => t,
        None => {
            event_tx
                .send(InternalEvent::Error("no audio track found".into()))
                .ok();
            return;
        }
    };
    let track_id = track.id;
    let time_base = track.codec_params.time_base;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);

    let mut decoder = match symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
    {
        Ok(d) => d,
        Err(e) => {
            event_tx
                .send(InternalEvent::Error(format!("decoder: {e}")))
                .ok();
            return;
        }
    };

    let mut paused = false;
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut last_position_report_ms: u64 = 0;

    loop {
        // Check for control messages (non-blocking when not paused)
        if paused {
            // Block until we get a control message
            match control_rx.recv() {
                Ok(DecodeControl::Resume) => {
                    paused = false;
                    continue;
                }
                Ok(DecodeControl::Stop) => break,
                Ok(DecodeControl::Seek(ms)) => {
                    seek_to(&mut format, track_id, ms, time_base, sample_rate, &event_tx);
                    continue;
                }
                Ok(DecodeControl::Pause) => continue,
                Err(_) => break,
            }
        } else {
            match control_rx.try_recv() {
                Ok(DecodeControl::Stop) => break,
                Ok(DecodeControl::Pause) => {
                    paused = true;
                    continue;
                }
                Ok(DecodeControl::Resume) => {}
                Ok(DecodeControl::Seek(ms)) => {
                    seek_to(&mut format, track_id, ms, time_base, sample_rate, &event_tx);
                    continue;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }
        }

        // Decode next packet
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                event_tx.send(InternalEvent::TrackEnded).ok();
                break;
            }
            Err(e) => {
                event_tx
                    .send(InternalEvent::Error(format!("packet: {e}")))
                    .ok();
                break;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        // Compute position in milliseconds
        let position_ms = if let Some(tb) = time_base {
            (packet.ts() as f64 * tb.numer as f64 / tb.denom as f64 * 1000.0) as u64
        } else {
            (packet.ts() as f64 / sample_rate as f64 * 1000.0) as u64
        };

        // Report position every ~1 second
        if position_ms >= last_position_report_ms + 1000 || position_ms < last_position_report_ms {
            event_tx.send(InternalEvent::Position(position_ms)).ok();
            last_position_report_ms = position_ms;
        }

        // Decode the packet
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(e) => {
                // Skip corrupt frames
                event_tx
                    .send(InternalEvent::Error(format!("frame: {e}")))
                    .ok();
                continue;
            }
        };

        // Convert to interleaved f32 and send to output
        let sbuf = sample_buf.get_or_insert_with(|| {
            SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec())
        });

        sbuf.copy_interleaved_ref(decoded);
        let chunk = sbuf.samples().to_vec();

        if audio_tx.send(chunk).is_err() {
            break; // Output stream dropped
        }
    }
}

fn seek_to(
    format: &mut Box<dyn symphonia::core::formats::FormatReader>,
    track_id: u32,
    ms: u64,
    time_base: Option<symphonia::core::units::TimeBase>,
    sample_rate: u32,
    event_tx: &Sender<InternalEvent>,
) {
    let seek_ts = if let Some(tb) = time_base {
        (ms as f64 / 1000.0 * tb.denom as f64 / tb.numer as f64) as u64
    } else {
        (ms as f64 / 1000.0 * sample_rate as f64) as u64
    };

    if let Err(e) = format.seek(SeekMode::Coarse, SeekTo::TimeStamp { ts: seek_ts, track_id }) {
        event_tx
            .send(InternalEvent::Error(format!("seek: {e}")))
            .ok();
    }
}

// --- Output Stream (cpal) ---

fn create_output_stream(
    audio_rx: Receiver<Vec<f32>>,
    mixer: Arc<Mixer>,
    clear_buffer: Arc<std::sync::atomic::AtomicBool>,
) -> Result<cpal::Stream, RustifyError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| RustifyError::Audio("no default output device".into()))?;

    let config = device
        .default_output_config()
        .map_err(|e| RustifyError::Audio(e.to_string()))?;

    let config: cpal::StreamConfig = config.into();

    let mut buf: VecDeque<f32> = VecDeque::with_capacity(8192);

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                // Check if we should clear stale audio
                if clear_buffer.swap(false, Ordering::Relaxed) {
                    buf.clear();
                    while audio_rx.try_recv().is_ok() {}
                }

                let gain = mixer.gain();
                for sample in data.iter_mut() {
                    if buf.is_empty() {
                        match audio_rx.try_recv() {
                            Ok(chunk) => buf.extend(chunk),
                            Err(_) => {
                                *sample = 0.0;
                                continue;
                            }
                        }
                    }
                    *sample = buf.pop_front().unwrap_or(0.0) * gain;
                }
            },
            |err| {
                eprintln!("cpal stream error: {err}");
            },
            None,
        )
        .map_err(|e| RustifyError::Audio(e.to_string()))?;

    stream
        .play()
        .map_err(|e| RustifyError::Audio(e.to_string()))?;

    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    #[test]
    fn shared_state_initial_values() {
        let state = SharedState::new();
        assert_eq!(state.get_playback_state(), PlaybackState::Stopped);
        assert!(state.current_track.lock().unwrap().is_none());
        assert_eq!(state.time_position_ms.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn shared_state_playback_transitions() {
        let state = SharedState::new();
        state.set_playback_state(PlaybackState::Playing);
        assert_eq!(state.get_playback_state(), PlaybackState::Playing);

        state.set_playback_state(PlaybackState::Paused);
        assert_eq!(state.get_playback_state(), PlaybackState::Paused);

        state.set_playback_state(PlaybackState::Stopped);
        assert_eq!(state.get_playback_state(), PlaybackState::Stopped);
    }

    #[test]
    fn player_new_starts_in_stopped_state() {
        // This test only works if an audio device is available.
        // On CI without audio, it will fail at stream creation but the
        // command thread will still start with Stopped state.
        let config = PlayerConfig {
            alsa_device: "default".into(),
            music_dirs: vec![],
        };
        let player = Player::new(config);
        // Player creation may fail without audio device — that's expected on CI.
        if let Ok(player) = player {
            assert_eq!(player.get_playback_state(), PlaybackState::Stopped);
            assert!(player.get_current_track().is_none());
            assert_eq!(player.get_time_position(), 0);
            player.shutdown();
        }
    }

    #[test]
    fn player_volume_control_bypasses_command_thread() {
        let config = PlayerConfig {
            alsa_device: "default".into(),
            music_dirs: vec![],
        };
        if let Ok(player) = Player::new(config) {
            player.set_volume(75);
            assert_eq!(player.get_volume(), 75);
            player.shutdown();
        }
    }
}
```

- [ ] **Step 2: Add player module to lib.rs**

Add to `crates/rustify-core/src/lib.rs`:

```rust
pub mod error;
pub mod metadata;
pub mod mixer;
pub mod player;
pub mod playlist;
pub mod scanner;
pub mod tracklist;
pub mod types;

pub use error::{Result, RustifyError};
pub use player::{Player, PlayerConfig};
pub use types::{PlaybackState, PlayerCommand, PlayerEvent, Playlist, Track};
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-core`
Expected: Foundation module tests pass. Player tests may skip on CI if no audio device.

Run: `cargo clippy -p rustify-core -- -D warnings`
Fix any issues.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-core/src/player.rs crates/rustify-core/src/lib.rs
git commit -m "feat: add three-thread playback engine with symphonia + cpal"
```

---

### Task 10: Python Bindings

**Files:**
- Modify: `bindings/python/src/lib.rs`
- Create: `bindings/python/src/client.rs`
- Create: `bindings/python/rustify/__init__.py`
- Create: `bindings/python/rustify/py.typed`

- [ ] **Step 1: Write the Python-facing types and module entry point**

Replace `bindings/python/src/lib.rs`:

```rust
mod client;

use pyo3::prelude::*;

use client::{PyPlaylist, PyTrack, RustifyClient};

#[pymodule]
fn _rustify(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_class::<RustifyClient>()?;
    m.add_class::<PyTrack>()?;
    m.add_class::<PyPlaylist>()?;
    Ok(())
}
```

- [ ] **Step 2: Write the RustifyClient pyclass**

Create `bindings/python/src/client.rs`:

```rust
use std::path::{Path, PathBuf};

use pyo3::prelude::*;
use pyo3::types::PyString;

use rustify_core::player::{Player, PlayerConfig};
use rustify_core::types::PlaybackState;
use rustify_core::{metadata, playlist, scanner, types};

// --- Python-facing data types ---

#[pyclass(name = "Track")]
#[derive(Clone)]
pub struct PyTrack {
    #[pyo3(get)]
    pub uri: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub artists: Vec<String>,
    #[pyo3(get)]
    pub album: String,
    #[pyo3(get)]
    pub length: u64,
    #[pyo3(get)]
    pub track_no: Option<u32>,
}

#[pymethods]
impl PyTrack {
    fn __repr__(&self) -> String {
        format!("Track(name={:?}, artists={:?})", self.name, self.artists)
    }
}

impl From<types::Track> for PyTrack {
    fn from(t: types::Track) -> Self {
        Self {
            uri: t.uri,
            name: t.name,
            artists: t.artists,
            album: t.album,
            length: t.length,
            track_no: t.track_no,
        }
    }
}

#[pyclass(name = "Playlist")]
#[derive(Clone)]
pub struct PyPlaylist {
    #[pyo3(get)]
    pub uri: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub track_count: usize,
}

#[pymethods]
impl PyPlaylist {
    fn __repr__(&self) -> String {
        format!(
            "Playlist(name={:?}, track_count={})",
            self.name, self.track_count
        )
    }
}

impl From<types::Playlist> for PyPlaylist {
    fn from(p: types::Playlist) -> Self {
        Self {
            uri: p.uri,
            name: p.name,
            track_count: p.track_count,
        }
    }
}

// --- RustifyClient ---

#[pyclass]
pub struct RustifyClient {
    player: Player,
    music_dirs: Vec<PathBuf>,
}

#[pymethods]
impl RustifyClient {
    #[new]
    #[pyo3(signature = (alsa_device = "default".to_string(), music_dirs = vec![]))]
    fn new(alsa_device: String, music_dirs: Vec<String>) -> PyResult<Self> {
        let dirs: Vec<PathBuf> = music_dirs.iter().map(PathBuf::from).collect();
        let player = Player::new(PlayerConfig {
            alsa_device,
            music_dirs: dirs.clone(),
        })
        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(Self {
            player,
            music_dirs: dirs,
        })
    }

    // --- Transport ---

    fn play(&self) {
        self.player.play();
    }

    fn pause(&self) {
        self.player.pause();
    }

    fn stop(&self) {
        self.player.stop();
    }

    fn next_track(&self) {
        self.player.next();
    }

    fn previous_track(&self) {
        self.player.previous();
    }

    fn seek(&self, position_ms: u64) {
        self.player.seek(position_ms);
    }

    // --- Volume ---

    fn set_volume(&self, volume: u8) {
        self.player.set_volume(volume);
    }

    fn get_volume(&self) -> u8 {
        self.player.get_volume()
    }

    // --- State queries ---

    fn get_playback_state(&self) -> &'static str {
        match self.player.get_playback_state() {
            PlaybackState::Playing => "playing",
            PlaybackState::Paused => "paused",
            PlaybackState::Stopped => "stopped",
        }
    }

    fn get_current_track(&self) -> Option<PyTrack> {
        self.player.get_current_track().map(PyTrack::from)
    }

    fn get_time_position(&self) -> u64 {
        self.player.get_time_position()
    }

    // --- Tracklist ---

    fn load_track_uris(&self, uris: Vec<String>) {
        self.player.load_track_uris(uris);
    }

    fn clear_tracklist(&self) {
        self.player.clear_tracklist();
    }

    // --- Library ---

    fn browse_library(&self, path: String) -> PyResult<Vec<String>> {
        let p = types::uri_to_path(&path);
        scanner::browse_directory(&p)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    fn scan_library(&self) -> PyResult<Vec<String>> {
        let mut all_uris = Vec::new();
        for dir in &self.music_dirs {
            match scanner::scan_directory(dir) {
                Ok(uris) => all_uris.extend(uris),
                Err(e) => {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
                }
            }
        }
        all_uris.sort();
        Ok(all_uris)
    }

    // --- Playlists ---

    fn get_playlists(&self) -> PyResult<Vec<PyPlaylist>> {
        let mut all_playlists = Vec::new();
        for dir in &self.music_dirs {
            match playlist::find_playlists(dir) {
                Ok(pls) => all_playlists.extend(pls.into_iter().map(PyPlaylist::from)),
                Err(e) => {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
                }
            }
        }
        Ok(all_playlists)
    }

    fn load_playlist(&self, path: String) -> PyResult<()> {
        let uris = playlist::parse_m3u(Path::new(&path))
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        self.player.load_track_uris(uris);
        Ok(())
    }

    // --- Metadata ---

    fn read_metadata(&self, uri: String) -> PyResult<PyTrack> {
        metadata::read_metadata(&uri)
            .map(PyTrack::from)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))
    }

    // --- Callbacks ---

    fn on_track_change(&self, callback: PyObject) {
        self.player
            .on_track_change(Box::new(move |track: types::Track| {
                Python::with_gil(|py| {
                    let py_track = PyTrack::from(track);
                    if let Err(e) = callback.call1(py, (py_track,)) {
                        eprintln!("Python on_track_change callback error: {e}");
                    }
                });
            }));
    }

    fn on_state_change(&self, callback: PyObject) {
        self.player
            .on_state_change(Box::new(move |state: PlaybackState| {
                Python::with_gil(|py| {
                    let state_str = match state {
                        PlaybackState::Playing => "playing",
                        PlaybackState::Paused => "paused",
                        PlaybackState::Stopped => "stopped",
                    };
                    if let Err(e) = callback.call1(py, (state_str,)) {
                        eprintln!("Python on_state_change callback error: {e}");
                    }
                });
            }));
    }

    fn on_position_update(&self, callback: PyObject) {
        self.player
            .on_position_update(Box::new(move |ms: u64| {
                Python::with_gil(|py| {
                    if let Err(e) = callback.call1(py, (ms,)) {
                        eprintln!("Python on_position_update callback error: {e}");
                    }
                });
            }));
    }

    fn on_error(&self, callback: PyObject) {
        self.player
            .on_error(Box::new(move |msg: String| {
                Python::with_gil(|py| {
                    if let Err(e) = callback.call1(py, (msg,)) {
                        eprintln!("Python on_error callback error: {e}");
                    }
                });
            }));
    }

    // --- Lifecycle ---

    fn shutdown(&self) {
        self.player.shutdown();
    }
}
```

- [ ] **Step 3: Create Python package files**

Create `bindings/python/rustify/__init__.py`:

```python
"""Rustify — Embedded Rust media player for YoyoPod."""

from rustify._rustify import RustifyClient, Track, Playlist

__all__ = ["RustifyClient", "Track", "Playlist"]
```

Create `bindings/python/rustify/py.typed` (empty file — PEP 561 marker):

```
```

- [ ] **Step 4: Verify Rust compilation**

Run: `cargo check --workspace`
Expected: Both crates compile.

- [ ] **Step 5: Commit**

```bash
git add bindings/ pyproject.toml
git commit -m "feat: add PyO3 Python bindings with RustifyClient"
```

---

### Task 11: CLI Example

**Files:**
- Create: `examples/play.rs`

- [ ] **Step 1: Write the CLI player example**

Create `examples/play.rs`:

```rust
use std::env;
use std::io::{self, BufRead};
use std::path::Path;

use rustify_core::player::{Player, PlayerConfig};
use rustify_core::types::path_to_uri;
use rustify_core::{playlist, scanner};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: play <file_or_dir> [--playlist] [--scan]");
        eprintln!("  play song.mp3           Play a single file");
        eprintln!("  play --scan /Music      Scan directory, play all");
        eprintln!("  play --playlist mix.m3u  Load M3U playlist");
        std::process::exit(1);
    }

    let config = PlayerConfig {
        alsa_device: "default".to_string(),
        music_dirs: vec![],
    };

    let player = Player::new(config).expect("Failed to create player");

    // Register display callbacks
    player.on_state_change(Box::new(|state| {
        println!("[State] {state:?}");
    }));
    player.on_track_change(Box::new(|track| {
        let artist = if track.artists.is_empty() {
            "Unknown".to_string()
        } else {
            track.artists.join(", ")
        };
        println!("[Track] {artist} — {}", track.name);
    }));
    player.on_position_update(Box::new(|ms| {
        let secs = ms / 1000;
        let mins = secs / 60;
        print!("\r[{:02}:{:02}]", mins, secs % 60);
    }));
    player.on_error(Box::new(|msg| {
        eprintln!("[Error] {msg}");
    }));

    // Parse args and load tracks
    let is_scan = args.contains(&"--scan".to_string());
    let is_playlist = args.contains(&"--playlist".to_string());
    let path_arg = args
        .iter()
        .find(|a| !a.starts_with('-') && *a != &args[0])
        .expect("No path provided");

    if is_scan {
        let uris = scanner::scan_directory(Path::new(path_arg)).expect("Scan failed");
        println!("Found {} tracks", uris.len());
        player.load_track_uris(uris);
    } else if is_playlist {
        let uris = playlist::parse_m3u(Path::new(path_arg)).expect("Playlist parse failed");
        println!("Loaded {} tracks from playlist", uris.len());
        player.load_track_uris(uris);
    } else {
        player.load_track_uris(vec![path_to_uri(Path::new(path_arg))]);
    }

    player.play();

    println!("Commands: play, pause, stop, next, prev, vol <0-100>, quit");
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        match line.trim() {
            "play" | "p" => player.play(),
            "pause" => player.pause(),
            "stop" | "s" => player.stop(),
            "next" | "n" => player.next(),
            "prev" => player.previous(),
            "quit" | "q" => {
                player.shutdown();
                break;
            }
            cmd if cmd.starts_with("vol ") => {
                if let Ok(vol) = cmd[4..].parse::<u8>() {
                    player.set_volume(vol);
                    println!("Volume: {vol}");
                }
            }
            "" => {}
            other => println!("Unknown command: {other}"),
        }
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p rustify-core --example play`
Expected: Compiles (won't run without audio device + actual files).

- [ ] **Step 3: Commit**

```bash
git add examples/play.rs
git commit -m "feat: add CLI player example for hardware testing"
```

---

### Task 12: Final Integration Verification

- [ ] **Step 1: Run full Rust test suite**

Run: `cargo test -p rustify-core`
Expected: All unit tests pass. Player tests may be skipped on headless CI.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace -- -D warnings`
Expected: No warnings. Fix any issues.

- [ ] **Step 3: Check formatting**

Run: `cargo fmt --check`
Expected: All files formatted. If not, run `cargo fmt` to fix.

- [ ] **Step 4: Verify workspace check**

Run: `cargo check --workspace`
Expected: Both rustify-core and rustify-python crates compile.

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "chore: fix clippy warnings and formatting"
```
