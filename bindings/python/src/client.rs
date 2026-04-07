use std::path::{Path, PathBuf};

use pyo3::prelude::*;

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
