use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
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
    #[allow(dead_code)] // used by Python bindings layer
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
                let mut cmd_loop = CommandLoop::new(cmd_rx, shared_clone, mixer_clone, alsa_device);
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

    pub fn set_shuffle(&self, on: bool) {
        self.cmd_tx.send(PlayerCommand::SetShuffle(on)).ok();
    }

    pub fn set_repeat(&self, mode: crate::types::RepeatMode) {
        self.cmd_tx.send(PlayerCommand::SetRepeat(mode)).ok();
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

    pub fn on_mode_change(
        &self,
        callback: Box<dyn Fn(bool, crate::types::RepeatMode) + Send>,
    ) {
        self.shared
            .callbacks
            .lock()
            .unwrap()
            .on_mode_change
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

impl Drop for Player {
    fn drop(&mut self) {
        self.cmd_tx.send(PlayerCommand::Shutdown).ok();
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
    on_mode_change: Vec<Box<dyn Fn(bool, crate::types::RepeatMode) + Send>>,
}

// --- Internal Events (decode thread -> command loop) ---

enum InternalEvent {
    TrackChanged(Track),
    Position(u64),
    TrackEnded,
    /// Decode thread failed to open/decode the track and exited.
    /// Command loop must reset state to Stopped.
    DecodeFailed(String),
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
    clear_buffer: Arc<AtomicBool>,
    #[allow(dead_code)]
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
        let clear_buffer = Arc::new(AtomicBool::new(false));

        // Create the cpal output stream
        let stream = create_output_stream(audio_rx, Arc::clone(&mixer), Arc::clone(&clear_buffer));

        if let Err(ref e) = stream {
            eprintln!("rustify: failed to create audio stream: {e}");
        }

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
                    if let Ok(evt) = event {
                        self.handle_event(evt);
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
            PlayerCommand::SetShuffle(on) => {
                self.tracklist.set_shuffle(on);
                self.emit_callbacks(PlayerEvent::ModeChanged {
                    shuffle: self.tracklist.get_shuffle(),
                    repeat: self.tracklist.get_repeat(),
                });
            }
            PlayerCommand::SetRepeat(mode) => {
                self.tracklist.set_repeat(mode);
                self.emit_callbacks(PlayerEvent::ModeChanged {
                    shuffle: self.tracklist.get_shuffle(),
                    repeat: self.tracklist.get_repeat(),
                });
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
                    // End of tracklist — let remaining audio drain naturally.
                    // Don't call stop_decode() which would clear the buffer
                    // and cut off the last seconds of audio.
                    self.decode_handle = None;
                    self.set_state(PlaybackState::Stopped);
                    *self.shared.current_track.lock().unwrap() = None;
                    self.shared.time_position_ms.store(0, Ordering::Relaxed);
                }
            }
            InternalEvent::DecodeFailed(msg) => {
                // Decode thread exited without producing audio.
                // Reset state so the player doesn't get stuck in Playing.
                self.decode_handle = None;
                self.set_state(PlaybackState::Stopped);
                *self.shared.current_track.lock().unwrap() = None;
                self.shared.time_position_ms.store(0, Ordering::Relaxed);
                self.emit_callbacks(PlayerEvent::Error(msg));
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
            // Clear buffered pre-seek audio so it doesn't keep playing
            self.clear_buffer.store(true, Ordering::Relaxed);
            handle.control_tx.send(DecodeControl::Seek(ms)).ok();
        }
    }

    fn start_decode(&mut self, uri: String) {
        // Clear any stale audio in the buffer
        self.clear_buffer.store(true, Ordering::Relaxed);

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
        self.clear_buffer.store(true, Ordering::Relaxed);
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
            PlayerEvent::ModeChanged { shuffle, repeat } => {
                for cb in &callbacks.on_mode_change {
                    cb(*shuffle, *repeat);
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
    use symphonia::core::formats::FormatOptions;
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
                .send(InternalEvent::DecodeFailed(format!("open: {e}")))
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
                .send(InternalEvent::DecodeFailed(format!("probe: {e}")))
                .ok();
            return;
        }
    };

    let mut format = probed.format;
    let track = match format.default_track() {
        Some(t) => t,
        None => {
            event_tx
                .send(InternalEvent::DecodeFailed("no audio track found".into()))
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
                .send(InternalEvent::DecodeFailed(format!("decoder: {e}")))
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
    use symphonia::core::formats::{SeekMode, SeekTo};

    let seek_ts = if let Some(tb) = time_base {
        (ms as f64 / 1000.0 * tb.denom as f64 / tb.numer as f64) as u64
    } else {
        (ms as f64 / 1000.0 * sample_rate as f64) as u64
    };

    if let Err(e) = format.seek(
        SeekMode::Coarse,
        SeekTo::TimeStamp {
            ts: seek_ts,
            track_id,
        },
    ) {
        event_tx
            .send(InternalEvent::Error(format!("seek: {e}")))
            .ok();
    }
}

// --- Output Stream (cpal) ---

fn create_output_stream(
    audio_rx: Receiver<Vec<f32>>,
    mixer: Arc<Mixer>,
    clear_buffer: Arc<AtomicBool>,
) -> Result<cpal::Stream, RustifyError> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| RustifyError::Audio("no default output device".into()))?;

    let supported_config = device
        .default_output_config()
        .map_err(|e| RustifyError::Audio(e.to_string()))?;

    let device_channels = supported_config.channels() as usize;

    // Force f32 sample format — our decode pipeline outputs f32.
    // Override the device default which may be i16/u16 on some ALSA backends.
    let config = cpal::StreamConfig {
        channels: supported_config.channels(),
        sample_rate: supported_config.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let mut buf: VecDeque<f32> = VecDeque::with_capacity(8192);

    // Decoded audio is interleaved stereo (L R L R...).
    // The device may have more channels (e.g. 8 on some USB audio).
    // We map stereo to the first 2 device channels and silence the rest.
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

                // Process one device frame at a time
                for frame in data.chunks_mut(device_channels) {
                    // Pop one stereo pair (L, R) from decoded audio
                    let left = if buf.is_empty() {
                        match audio_rx.try_recv() {
                            Ok(chunk) => {
                                buf.extend(chunk);
                                buf.pop_front().unwrap_or(0.0)
                            }
                            Err(_) => 0.0,
                        }
                    } else {
                        buf.pop_front().unwrap_or(0.0)
                    };
                    let right = buf.pop_front().unwrap_or(left);

                    // Map stereo to device channels, silence extras
                    for (i, sample) in frame.iter_mut().enumerate() {
                        *sample = match i {
                            0 => left * gain,
                            1 => right * gain,
                            _ => 0.0,
                        };
                    }
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
