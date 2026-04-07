# Rustify — Embedded Rust Media Player for YoyoPod

**Date:** 2026-04-07
**Status:** Draft
**Repo:** Standalone (`rustify`)
**Target:** Raspberry Pi Zero 2W (aarch64, 512MB RAM)

---

## Overview

Rustify is an embedded Rust media player library that replaces Mopidy + GStreamer in YoyoPod. It provides local file playback with M3U playlist support, exposed to Python via PyO3. It eliminates a separate daemon process, HTTP polling, and ~70-80MB of runtime overhead, replacing it with a single `.so` that uses ~3-4MB.

### Goals

- Drop-in replacement for `MopidyClient` in YoyoPod
- Local file playback: MP3, FLAC, OGG Vorbis, WAV
- M3U playlist parsing
- File scanning and metadata extraction
- Callback-driven state (no polling)
- ALSA output, Pi Zero 2W target
- ~3-4MB RAM footprint

### Non-Goals (v1)

- Streaming radio (deferred to v2 — architecture supports it)
- PipeWire / PulseAudio (deferred — feature flag ready)
- AAC decoding (deferred — symphonia feature flag)
- Gapless playback (deferred to v1.1 — architecture supports it via pre-decode)
- Crossfade, EQ, DSP effects

---

## Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Repo | Standalone | Clean boundary, own CI, publishable wheel |
| Python binding | PyO3 + maturin | Modern standard, direct function calls, no C shim |
| Audio output | ALSA only (v1) | Lowest overhead, matches Pi default |
| Radio | v2 | Reduce scope, prove core pipeline first |
| Formats | MP3 + FLAC + OGG + WAV | All stable in symphonia, covers local libraries |
| Architecture | Fat library | Rust owns tracklist, scanning, metadata, decode, output |

---

## Crate Structure

```
rustify/
├── Cargo.toml                 # workspace root
├── crates/
│   └── rustify-core/          # pure Rust library, no Python deps
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── player.rs      # playback engine (decode -> ring buffer -> cpal)
│           ├── tracklist.rs   # queue management
│           ├── scanner.rs     # recursive file discovery
│           ├── metadata.rs    # tag reading
│           ├── playlist.rs    # M3U parser
│           ├── mixer.rs       # volume control
│           ├── types.rs       # Track, Playlist, PlaybackState, PlayerEvent, PlayerCommand
│           └── error.rs       # unified error type
├── bindings/
│   └── python/                # PyO3 extension module
│       ├── Cargo.toml         # depends on rustify-core + pyo3
│       ├── src/
│       │   ├── lib.rs         # #[pymodule] entry point
│       │   └── client.rs      # RustifyClient Python-facing class
│       └── rustify/
│           ├── __init__.py    # re-export RustifyClient
│           └── py.typed       # PEP 561 marker
├── pyproject.toml             # maturin build config
├── tests/                     # Rust integration tests
├── fixtures/                  # small test audio files (<100KB total)
└── examples/
    └── play.rs                # standalone CLI player for hardware testing
```

**Key split:** `rustify-core` is pure Rust with zero Python dependencies. It can be tested and benchmarked without Python. `bindings/python` is a thin PyO3 wrapper.

---

## Player Architecture

### Threading Model

Three threads connected by lock-free channels:

```
┌───────────────────────────────────────────────────────────┐
│  rustify-core                                             │
│                                                           │
│  ┌──────────────┐    ┌──────────────┐    ┌─────────────┐ │
│  │ Command      │    │ Decode       │    │ Output      │ │
│  │ Thread       │    │ Thread       │    │ Thread      │ │
│  │              │    │              │    │ (cpal)      │ │
│  │ receives:    │    │ symphonia    │    │             │ │
│  │  play(uri)  ─┼───>│ decode frame─┼───>│ ALSA        │ │
│  │  pause      ─┼───>│ fill ring   │    │ callback    │ │
│  │  stop       ─┼───>│ buffer      │    │ drains      │ │
│  │  next       ─┼───>│             │    │ ring buffer │ │
│  │  seek       ─┼───>│ pre-decode  │    │             │ │
│  │  volume     ─┼────┼─────────────┼───>│ gain mul    │ │
│  │              │    │             │    │             │ │
│  │ emits:       │    │ emits:      │    │             │ │
│  │  state chg   │    │  track chg  │    │             │ │
│  │  error       │    │  position   │    │             │ │
│  └──────────────┘    └─────────────┘    └─────────────┘ │
│                                                           │
│  ┌────────────────────────────────────────┐               │
│  │ Ring Buffer (crossbeam bounded)        │               │
│  │ ~2 sec of f32 stereo @ 44.1kHz        │               │
│  │ = 352,800 samples = ~1.4MB            │               │
│  └────────────────────────────────────────┘               │
└───────────────────────────────────────────────────────────┘
```

**Command thread** — owns the `Player` struct. Receives `PlayerCommand` via crossbeam channel. Manages playback state machine. Tells the decode thread what to do.

**Decode thread** — owns the symphonia `FormatReader` + `Decoder`. Reads frames, decodes to f32 PCM, writes to ring buffer. In v1.1, pre-opens next track for gapless transition.

**Output thread** — owned by cpal. Callback pulls samples from ring buffer, applies volume gain, writes to ALSA. Writes silence on buffer underrun.

### Why Three Threads

- cpal requires a dedicated audio callback thread.
- Separating decode from commands means slow file opens (SD card seek) don't block pause/stop.
- Ring buffer decouples decode speed from output speed.

### State Machine

```
STOPPED ──play()──> PLAYING ──pause()──> PAUSED
   ^                  │    <──resume()──    │
   │                  │                     │
   └───stop()─────────┴──────stop()─────────┘
```

### Events (callback-driven, no polling)

| Event | Trigger |
|---|---|
| `StateChanged(playing/paused/stopped)` | Any transport command |
| `TrackChanged(track_info)` | Decode thread opens a new file |
| `PositionUpdate(ms)` | Every ~1 second from decode thread |
| `Error(message)` | Decode failure, file not found, ALSA error |

PyO3 callbacks use `Python::with_gil()` to invoke Python callables from Rust threads.

---

## Rust Core Modules

### types.rs (~80 LOC)

Shared data types: `Track`, `Playlist`, `PlaybackState`, `PlayerEvent`, `PlayerCommand`.

### player.rs (~450 LOC)

Playback engine. Owns three-thread lifecycle. Receives commands, manages state machine, drives decode loop, emits events.

### tracklist.rs (~150 LOC)

Queue backed by `VecDeque<String>`. Methods: `add`, `clear`, `current`, `next`, `previous`, `index`, `len`.

### scanner.rs (~100 LOC)

Recursive directory walk via `walkdir`. Filters by extension (`.mp3`, `.flac`, `.ogg`, `.wav`). Returns `file://` URIs.

### metadata.rs (~100 LOC)

Tag reading via `lofty`. Reads ID3, Vorbis comments, FLAC tags. Falls back to filename if tags are missing.

### playlist.rs (~80 LOC)

M3U parser. Handles simple and extended M3U (`#EXTINF`). Resolves relative paths against M3U file directory.

### mixer.rs (~50 LOC)

`AtomicU8` volume (0-100). Output thread reads `gain()` (0.0-1.0) without locking.

### error.rs (~40 LOC)

Unified `RustifyError` enum: `Io`, `Decode`, `Audio`, `Metadata`, `Playlist`.

### Estimated LOC

| Module | LOC |
|---|---|
| player.rs | ~450 |
| tracklist.rs | ~150 |
| scanner.rs | ~100 |
| metadata.rs | ~100 |
| playlist.rs | ~80 |
| mixer.rs | ~50 |
| types.rs | ~80 |
| error.rs | ~40 |
| lib.rs | ~30 |
| **rustify-core** | **~1,080** |
| **bindings/python** | **~300** |
| **Total** | **~1,380** |

---

## Rust Dependencies

```toml
[dependencies]
symphonia = { version = "0.5", features = ["mp3", "flac", "ogg", "wav"] }
cpal = { version = "0.17", features = ["alsa"] }
crossbeam = "0.8"
walkdir = "2"
lofty = "0.22"
serde = { version = "1", features = ["derive"] }

[features]
default = ["mp3", "flac", "ogg", "wav"]
mp3 = ["symphonia/mp3"]
flac = ["symphonia/flac"]
ogg = ["symphonia/ogg"]
wav = ["symphonia/wav"]
aac = ["symphonia/aac"]
radio = ["stream-download", "icy-metadata"]
```

---

## Python API

```python
from rustify import RustifyClient, Track, Playlist

client = RustifyClient(
    alsa_device="default",
    music_dirs=["/home/pi/Music"],
)

# Playback
client.play()
client.pause()
client.stop()
client.next_track()
client.previous_track()
client.seek(position_ms=30000)

# Volume
client.set_volume(75)
client.get_volume()

# State
client.get_playback_state()      # "playing" | "paused" | "stopped"
client.get_current_track()       # Track | None
client.get_time_position()       # int (ms)

# Tracklist
client.load_track_uris([...])
client.clear_tracklist()

# Library
client.browse_library("/home/pi/Music")  # accepts path or file:// URI
client.scan_library()

# Playlists
client.get_playlists()
client.load_playlist("/path/to/playlist.m3u")

# Metadata
client.read_metadata("file:///path/to/song.flac")

# Callbacks
client.on_track_change(callback)
client.on_state_change(callback)
client.on_position_update(callback)
client.on_error(callback)

# Lifecycle
client.shutdown()
```

### Python Types

```python
@dataclass
class Track:
    uri: str
    name: str
    artists: list[str]
    album: str
    length: int          # ms
    track_no: int | None

@dataclass
class Playlist:
    uri: str
    name: str
    track_count: int
```

Defined as PyO3 `#[pyclass]` in Rust, behave like Python dataclasses.

---

## Build & Cross-Compilation

### Local development

```bash
cargo test --workspace                # Rust tests
cd bindings/python && maturin develop --release  # Python wheel
python -c "from rustify import RustifyClient"    # smoke test
```

### Cross-compile for Pi Zero 2W (aarch64)

```bash
cargo install cross
cross build --target aarch64-unknown-linux-gnu --release

# Python wheel
maturin build --release --target aarch64-unknown-linux-gnu
scp target/wheels/*.whl rpi-zero:~
ssh rpi-zero "pip install rustify-*.whl"
```

### CI (GitHub Actions)

```yaml
jobs:
  test:
    - cargo test --workspace
    - cargo clippy -- -D warnings
    - cargo fmt --check

  build-wheel:
    - uses: PyO3/maturin-action@v1
      with:
        target: aarch64-unknown-linux-gnu
        args: --release

  python-test:
    - maturin develop
    - pytest tests/
```

### Pi system dependency

```bash
sudo apt install libasound2-dev   # only if building on Pi
```

---

## Testing Strategy

### Rust unit tests (no hardware)

| Module | Tests |
|---|---|
| tracklist | add, clear, next, prev, index, empty edge cases |
| playlist | M3U parsing: simple, extended, relative paths, malformed |
| metadata | Tag reading: ID3, Vorbis, FLAC, missing tags, corrupt file |
| scanner | Directory walk, extension filter, nested directories |
| mixer | Volume set/get, gain conversion, clamping 0-100 |

### Rust integration tests (decode pipeline)

Decode fixture files to memory, verify sample count and duration. No ALSA needed.

### Test fixtures (<100KB total, checked into repo)

- `fixtures/silence_1s.mp3` — 1 second silence
- `fixtures/sine_440hz.flac` — 440Hz tone, verifiable samples
- `fixtures/tagged.ogg` — has artist/album/title tags
- `fixtures/no_tags.wav` — filename-only metadata fallback
- `fixtures/test_playlist.m3u` — references the above files

### Python integration tests

- Import test
- Metadata reading
- Library scanning
- Playlist parsing
- Tracklist operations

### Hardware testing (manual, on Pi)

```bash
./play /home/pi/Music/song.mp3
./play --playlist /home/pi/Music/chill.m3u
./play --scan /home/pi/Music
```

---

## Integration with YoyoPod

### Files that change

| File | Change |
|---|---|
| `yoyopy/audio/local_service.py` | Swap `MopidyClient` for `RustifyClient` |
| `yoyopy/audio/history.py` | Accept `rustify.Track` in `from_track()` |
| `yoyopy/app.py` | Replace Mopidy init/connect/polling with RustifyClient + callbacks |
| `yoyopy/config/models.py` | Replace `mopidy_host`/`mopidy_port` with `music_dirs`/`alsa_device` |
| `pyproject.toml` | Add `rustify` dependency, remove `requests` |

### Files that don't change

| File | Why |
|---|---|
| `yoyopy/coordinators/playback.py` | Already works on events, not Mopidy |
| `yoyopy/events.py` | `TrackChangedEvent` / `PlaybackStateChangedEvent` unchanged |
| `yoyopy/fsm.py` | Music FSM is transport-agnostic |
| All screen files | Screens consume events, don't know about the player backend |

### Callback wiring (app.py)

```python
# Before
self.mopidy_client.start_polling()
self.mopidy_client.on_track_change(self._on_track_changed)

# After
self.player = RustifyClient(
    alsa_device=self.config.audio.playback_device,
    music_dirs=self.config.audio.music_dirs,
)
self.player.on_track_change(self._on_track_changed)
self.player.on_state_change(self._on_playback_state_changed)
```

### Dependency chain

```
Before: yoyo-py -> requests -> Mopidy (daemon) -> GStreamer -> ALSA
After:  yoyo-py -> rustify (.so) -> ALSA
```

### What gets removed from Pi after validation

```bash
sudo apt remove mopidy mopidy-local
sudo apt autoremove   # removes GStreamer plugins
```

---

## RAM Footprint

| Component | Estimate |
|---|---|
| symphonia decoder state | ~1-2MB |
| Ring buffer (2s stereo f32 @ 44.1kHz) | ~1.4MB |
| cpal ALSA stream | ~100KB |
| Tracklist (1,000 tracks) | ~200KB |
| M3U playlist cache | ~50KB |
| File scanner cache (2,000 files) | ~500KB |
| Rust runtime + PyO3 bridge | ~600KB |
| **Total** | **~3-4MB** |

Compared to Mopidy + GStreamer: **~70-80MB**. ~95% reduction.

---

## v2 Roadmap (not in scope for v1)

| Feature | Crates | Estimated effort |
|---|---|---|
| Streaming radio (Icecast/Shoutcast) | stream-download, icy-metadata | ~300-400 LOC |
| Gapless playback | Pre-decode in decode thread | ~200 LOC |
| AAC decoding | symphonia `aac` feature flag | 1 line in Cargo.toml |
| PipeWire output | cpal feature flag | Build config only |
| Crossfade | Gain ramp in output thread | ~100 LOC |
| Resampling | rubato crate | ~50 LOC wiring |
