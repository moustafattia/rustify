# Tier 3: Power User — Design Spec

**Date:** 2026-04-09
**Status:** Draft
**Depends on:** Tier 1 (gapless/dual-decode mixer), Tier 2 (themes, visualizer)

## Overview

Five features that make the player a daily driver: crossfade between tracks, MPRIS media key support on Linux, ListenBrainz scrobbling, replay gain normalization, and lyrics display. These span core engine changes, a new optional crate, external API integration, and TUI additions.

## 1. Crossfade

### Architecture

The Tier 1 gapless architecture already supports two simultaneous decode channels with a MixStage in the cpal callback. Crossfade extends this: instead of a hard switch from active to pending channel, both are read simultaneously during an overlap window and mixed with linear fade curves.

### Config

New field in `tui.toml`:

```toml
# Crossfade duration in milliseconds. 0 = gapless (no overlap).
crossfade_ms = 3000
```

### Core Changes

**New `PlayerCommand` variant:**

```rust
SetCrossfade(u64), // duration in ms
```

**New `Player` API:**

```rust
pub fn set_crossfade(&self, ms: u64);
```

**MixStage changes in `create_output_stream()`:**

Add shared state for crossfade:
- `crossfade_ms: Arc<AtomicU64>` — crossfade duration, 0 = gapless
- `crossfade_progress: f32` — 0.0 to 1.0, tracks current position in the fade

When `crossfade_ms > 0` and both `active_rx` and `pending_rx` have audio:
- Read from both channels
- Outgoing: `sample * (1.0 - progress)`
- Incoming: `sample * progress`
- Increment progress based on sample rate and crossfade duration
- When progress reaches 1.0, complete the swap (same as gapless)

**TrackEnding timing:** The `PRE_BUFFER_MS` constant (currently 3000) should be `max(3000, crossfade_ms)` so the pending decode starts early enough for the full crossfade window.

### TUI Changes

None — crossfade is automatic. The config value is read at startup and passed to the player.

## 2. MPRIS (Linux Media Key Support)

### Architecture

New optional workspace crate: `crates/rustify-mpris`. Feature-gated behind a `mpris` Cargo feature so it compiles to a no-op on non-Linux platforms.

Uses `zbus` crate for D-Bus communication on Linux.

### MPRIS2 Interfaces Implemented

**`org.mpris.MediaPlayer2`:**
- `Identity` — "Rustify"
- `Raise` / `Quit` — no-op / send shutdown

**`org.mpris.MediaPlayer2.Player`:**
- `PlaybackStatus` — "Playing" / "Paused" / "Stopped"
- `Metadata` — track title, artist, album, art URI, length
- `Play` / `Pause` / `PlayPause` / `Stop` / `Next` / `Previous` — forward to `Player`
- `Position` — current playback position in microseconds
- `Volume` — 0.0 to 1.0 (mapped from 0-100)

### Integration

The MPRIS module receives a clone of the `Player` handle and the event channel sender. It:
1. Spawns a D-Bus event loop thread (`zbus::Connection::session()`)
2. Registers the MPRIS2 object at `/org/mpris/MediaPlayer2`
3. Listens for incoming D-Bus method calls (media keys) and translates to `PlayerCommand`s
4. Subscribes to `PlayerEvent`s to update MPRIS properties (metadata, playback status)

### Crate Structure

```
crates/rustify-mpris/
├── Cargo.toml
└── src/
    └── lib.rs    # MPRIS2 D-Bus interface + event loop
```

**Cargo.toml dependencies:**
- `zbus = "5"` (Linux only, via `[target.'cfg(target_os = "linux")'.dependencies]`)
- `rustify-core` (for Player, PlayerEvent types)

### Feature Gate

In `rustify-tui/Cargo.toml`:

```toml
[features]
default = ["mpris"]
mpris = ["rustify-mpris"]

[dependencies]
rustify-mpris = { path = "../rustify-mpris", optional = true }
```

In `main.rs`, conditionally start MPRIS:

```rust
#[cfg(feature = "mpris")]
{
    rustify_mpris::start(&player, tx.clone());
}
```

### Non-Linux

On non-Linux platforms, the `rustify-mpris` crate either doesn't compile (feature not enabled) or provides a no-op `start()` function. No conditional compilation in the TUI beyond the feature gate.

## 3. Scrobbling (ListenBrainz)

### Config

```toml
# ListenBrainz user token (from https://listenbrainz.org/settings/)
listenbrainz_token = ""
```

Empty token = scrobbling disabled.

### Scrobble Rules (ListenBrainz standard)

A track is scrobbled when:
- It has played for at least **50% of its duration**, OR
- It has played for at least **4 minutes** (240,000 ms)
- AND the track is longer than 30 seconds

### Architecture

New TUI module: `crates/rustify-tui/src/scrobble.rs`

**State:**
- `current_track: Option<Track>` — the track being monitored
- `play_start_ms: u64` — timestamp when the track started playing
- `accumulated_ms: u64` — total play time (pauses don't count)
- `scrobbled: bool` — whether this track has already been scrobbled

**Events handled:**
- `TrackChanged` — submit scrobble for previous track if eligible, reset state for new track, send "now playing" notification
- `PositionUpdate` — update accumulated play time, check scrobble threshold
- `StateChanged(Paused)` — pause accumulation
- `StateChanged(Playing)` — resume accumulation

**HTTP requests** (via `ureq` crate, blocking, on background thread):

1. **Now Playing:** POST to `api.listenbrainz.org/1/submit-listens` with `listen_type: "playing_now"`
2. **Scrobble:** POST with `listen_type: "single"` and `listened_at` timestamp

**Error handling:** Log failures to stderr. Don't retry — if the network is down, the scrobble is lost (standard scrobbler behavior).

### Dependencies

Add `ureq = "3"` to `crates/rustify-tui/Cargo.toml`.

## 4. Replay Gain

### Tag Reading

lofty (already a dependency) reads ReplayGain tags:
- ID3v2: `TXXX:replaygain_track_gain` — e.g. `"-6.5 dB"`
- Vorbis/FLAC: `REPLAYGAIN_TRACK_GAIN` — e.g. `"-6.5 dB"`

### Core Addition

New function in `crates/rustify-core/src/metadata.rs`:

```rust
/// Read ReplayGain track gain from audio file tags.
/// Returns the gain adjustment in dB, or None if no tag found.
pub fn read_replay_gain(path: &Path) -> Option<f32>
```

Parses the tag value, strips " dB" suffix, parses as `f32`.

### Volume Adjustment

The TUI handles replay gain at the application level:

On `TrackChanged`:
1. Read `replay_gain_db` from the new track's tags
2. Compute gain factor: `10.0_f32.powf(rg_db / 20.0)`
3. Adjust effective volume: `player.set_volume((base_volume as f32 * gain_factor).clamp(0.0, 100.0) as u8)`

The `base_volume` is the user's chosen volume (from +/- keys). Replay gain adjusts on top of it.

### Config

```toml
# Apply ReplayGain normalization (true/false)
replay_gain = true
```

### TUI State

- `app.replay_gain_enabled: bool` — from config
- `app.base_volume: u8` — user's chosen volume level
- `app.now_playing.volume` — effective volume after replay gain adjustment

## 5. Lyrics

### Lyrics Extraction (Core)

New module: `crates/rustify-core/src/lyrics.rs`

```rust
/// Lyrics content — either synced (timestamped) or unsynced (plain text).
#[derive(Debug, Clone)]
pub enum Lyrics {
    /// Timestamped lines from .lrc file: (timestamp_ms, line_text)
    Synced(Vec<(u64, String)>),
    /// Plain text lyrics from audio tags
    Unsynced(String),
}

/// Extract lyrics for a track.
/// Tries embedded tags first, then .lrc sidecar file.
pub fn extract_lyrics(path: &Path) -> Option<Lyrics>
```

**Embedded lyrics:** lofty reads USLT (unsynced) and SYLT (synced) frames from ID3v2, and `LYRICS` from Vorbis comments. Most embedded lyrics are unsynced plain text.

**Sidecar .lrc:** Look for `{filename_without_ext}.lrc` in the same directory as the track. Parse LRC format:
- `[mm:ss.xx]Line of lyrics` — synced line
- Lines without timestamps — treated as unsynced

### TUI Display

**Toggle:** `L` key toggles lyrics overlay in the main panel (replaces current view content).

**Synced lyrics rendering:**
- Show 5-7 lines centered on the current line
- Current line highlighted in accent color
- Past lines in dim color, future lines in normal color
- Auto-scrolls as `PositionUpdate` events arrive — find the line whose timestamp is closest to (but not exceeding) current position

**Unsynced lyrics rendering:**
- Static scrollable text in the main panel
- User scrolls with `j`/`k` as usual
- No auto-scroll (no timing information)

**No lyrics:** Show "No lyrics found" in dim text.

### Lyrics State in App

```rust
pub struct LyricsState {
    pub active: bool,        // overlay visible
    pub lyrics: Option<Lyrics>,
    pub current_line: usize, // for synced lyrics
    pub scroll_offset: usize, // for unsynced lyrics
}
```

Loaded on background thread (like album art) on `TrackChanged`.

## Error Handling

- **Crossfade with decode failure:** If pending decode fails, crossfade aborts and the outgoing track finishes normally (fades to silence). Same `DecodeFailed` path as gapless.
- **MPRIS D-Bus unavailable:** Log warning, continue without media keys. Common on headless Pi or non-Linux.
- **Scrobble HTTP failure:** Log to stderr, don't retry. No user-visible error — scrobbling is best-effort.
- **ReplayGain tag missing:** No adjustment applied, plays at user-set volume. Silent — no error.
- **ReplayGain extreme values:** Clamp gain factor to 0.1x–2.0x (±20dB) to prevent blowing out speakers.
- **LRC parse failure:** Skip malformed lines, use what we can parse. Log warning.
- **No lyrics found:** Show placeholder text, not an error.

## Testing

### Core Unit Tests

**Crossfade MixStage:**
- Two channels with known samples, verify linear mix at 50% crossfade point
- Verify crossfade completes and fully swaps to new channel

**Replay gain parsing:**
- Parse "-6.5 dB" → -6.5f32
- Parse "+3.2 dB" → 3.2f32
- Parse missing tag → None
- Gain factor: -6dB → ~0.5, +6dB → ~2.0

**Lyrics extraction:**
- Parse LRC file with timestamps → Synced
- Read embedded USLT tag → Unsynced
- No lyrics → None
- Malformed LRC lines skipped

### TUI Unit Tests

**Scrobbler:**
- Track played 51% → scrobble eligible
- Track played 49% but > 4 min → scrobble eligible
- Track played 10% and < 4 min → not eligible
- Track < 30 seconds → never scrobble

**Lyrics display:**
- Synced: correct current line for given position_ms
- Synced: auto-scrolls on position update
- Toggle on/off with L key

### Integration Tests

**MPRIS:** Manual — play a track, verify media keys work, verify desktop shows metadata.
**Scrobbling:** Manual — play a track > 50%, verify submission on ListenBrainz profile.
**Crossfade:** Manual — play album, verify smooth transitions.

## Dependencies

| Crate | Where | Purpose |
|-------|-------|---------|
| `zbus` 5 | rustify-mpris (new crate) | D-Bus MPRIS2 on Linux |
| `ureq` 3 | rustify-tui | HTTP POST for ListenBrainz scrobbling |

Existing crates used: `lofty` (replay gain tags, lyrics tags), core mixer (volume adjustment).

## Config Summary

All new config fields in `tui.toml`:

```toml
# Crossfade duration (0 = gapless, no overlap)
crossfade_ms = 0

# ListenBrainz scrobbling token (empty = disabled)
listenbrainz_token = ""

# Replay gain normalization
replay_gain = true
```

## Out of Scope

- Last.fm scrobbling (can be added later behind same interface)
- Online lyrics fetching (network lyrics services)
- MPRIS on Windows/macOS (no D-Bus)
- Crossfade with different sample rates between tracks
- Album-level replay gain (track gain only for v1)
- SYLT (synced lyrics) from ID3v2 tags (complex binary format — LRC sidecar covers synced use case)
