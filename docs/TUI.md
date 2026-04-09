# Rustify TUI — Terminal Music Player

A rich terminal music player built on `rustify-core`. Works over SSH on the YoyoPod Pi and as a standalone desktop terminal player.

## Quick Start

```bash
cargo run -p rustify-tui -- /path/to/music
```

Or configure music directories in `~/.config/rustify/tui.toml`:

```toml
music_dirs = ["/home/pi/Music", "/mnt/usb/music"]
```

## Keybindings

| Key | Action |
|-----|--------|
| `Space` | Play / Pause |
| `n` / `p` | Next / Previous track |
| `s` | Toggle shuffle |
| `r` | Cycle repeat: Off -> All -> One |
| `Left` / `Right` | Seek -/+ 5 seconds |
| `+` / `-` | Volume up / down |
| `j` / `k` | Navigate lists |
| `Enter` | Select / Play |
| `Tab` | Cycle focus: Sidebar <-> Main panel |
| `1`-`4` | Jump to Artists / Albums / Songs / Playlists |
| `/` | Fuzzy search |
| `a` | Add track to queue |
| `Shift+V` | Toggle visualizer: Spectrum <-> Waveform |
| `L` | Toggle lyrics overlay |
| `Esc` | Close overlay / go back |
| `q` | Quit |

## Configuration

Config file: `~/.config/rustify/tui.toml` (Linux/macOS) or `%APPDATA%\rustify\tui.toml` (Windows).

```toml
# Music directories to scan
music_dirs = ["/home/pi/Music"]

# Audio output device
alsa_device = "default"

# Theme: default, nord, dracula, gruvbox, catppuccin
theme = "default"

# Crossfade duration in ms (0 = gapless)
crossfade_ms = 0

# Replay gain normalization
replay_gain = true

# ListenBrainz scrobbling (empty = disabled)
listenbrainz_token = ""

# Custom theme overrides (optional)
[theme.custom]
accent = "#F38BA8"
fg = "#CDD6F4"
border = "#313244"
```

## Architecture

```
crates/
  rustify-core/     # Audio engine (symphonia + cpal)
  rustify-tui/      # Terminal UI (ratatui + crossterm)
  rustify-mpris/    # MPRIS2 D-Bus stub (Linux only)
bindings/
  python/           # PyO3 bindings for YoyoPod
```

### Crate: rustify-core (13 modules)

Audio playback library. Decodes MP3/FLAC/OGG/WAV via symphonia, outputs via cpal.

- **Player** — command thread + decode thread + cpal output. Non-blocking API via crossbeam channels.
- **Tracklist** — playback queue with shuffle (Fisher-Yates) and repeat modes (Off/All/One).
- **Gapless** — dual-decode architecture. Pre-buffers next track 3s before current ends. MixStage in cpal callback swaps channels seamlessly.
- **Crossfade** — extends gapless with configurable linear fade between tracks.
- **Art** — album art extraction from embedded tags (lofty) + sidecar files (cover.jpg, folder.jpg).
- **Lyrics** — extraction from embedded tags + .lrc sidecar files. Synced and unsynced.
- **Metadata** — tag reading with filename fallback. Replay gain tag parsing.
- **Scanner** — recursive directory scanning for audio files.
- **Playlist** — M3U parsing + discovery.
- **Mixer** — lock-free atomic volume control.

### Crate: rustify-tui (12 modules)

Terminal UI. Sidebar + Main panel layout with persistent now-playing bar.

- **App** — single state struct, event-driven. Handles keys, mouse, player callbacks.
- **Event loop** — crossbeam::select! multiplexing keyboard, player events, and 4Hz tick timer.
- **Library** — in-memory index. Groups tracks by artist/album. Fuzzy search via nucleo-matcher.
- **Visualizer** — FFT spectrum bars (40 bars, log-frequency, sqrt scaling) + waveform mode. Reads from core's sample buffer.
- **Themes** — 5 presets (default/nord/dracula/gruvbox/catppuccin) + custom via TOML.
- **Scrobbler** — ListenBrainz integration. Tracks play time, submits at 50%/4min threshold.
- **Config** — TOML config with platform-appropriate paths via `dirs` crate.

## What Was Built (This Session)

### Tier 1: Playback Essentials
- Shuffle (Fisher-Yates) and repeat (Off/All/One) modes in core Tracklist
- Seek keybindings (Left/Right +/-5s) with optimistic UI
- Gapless playback via dual-decode MixStage
- Album art extraction (embedded tags + sidecar files)

### Tier 2: Rich Experience
- Audio spectrum visualizer (1024-pt FFT, 40 log-frequency bars, sqrt scaling, exponential decay smoothing)
- Waveform oscilloscope mode (toggle with Shift+V)
- Color theme system (5 presets + custom TOML themes)
- Fuzzy search via nucleo-matcher (replaces substring matching)

### Tier 3: Power User
- Crossfade support (configurable duration, extends gapless mixer)
- Replay gain tag reading and volume normalization
- Lyrics extraction (embedded tags + .lrc sidecar, synced/unsynced)
- ListenBrainz scrobbling (50%/4min rules, background HTTP)
- MPRIS stub crate (ready for Linux D-Bus implementation)

## What's Next

### MPRIS Full Implementation (Linux)
The `crates/rustify-mpris` crate is stubbed. When deploying to Pi (Linux), implement the full MPRIS2 D-Bus interface using `zbus`:
- MediaPlayer2.Player: Play/Pause/Stop/Next/Previous
- Metadata publishing (track, artist, album, art URI)
- Media key capture from desktop environment

### Pi Deployment
- Cross-compile for aarch64 (Pi Zero 2W)
- Test ALSA output on hardware
- Benchmark memory usage (target: <10MB RSS)
- Test over SSH (braille art fallback, keyboard-only)

### Future Features
- Equalizer / audio effects
- Online lyrics fetching
- Last.fm scrobbling (behind same interface as ListenBrainz)
- Podcast support (RSS feeds)
- Network streaming (HTTP, maybe Spotify)
- Visualizer hide toggle keybinding

## Test Summary

139 tests across 3 crates, 24 source files.

```bash
cargo test --workspace
```
