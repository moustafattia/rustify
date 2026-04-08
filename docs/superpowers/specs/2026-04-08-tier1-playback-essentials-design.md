# Tier 1: Playback Essentials — Design Spec

**Date:** 2026-04-08
**Status:** Draft
**Depends on:** rustify-tui v0.1 (implemented)

## Overview

Four features that make the player usable for daily listening: shuffle/repeat modes, seek keybindings, gapless playback via a dual-decode mixer, and album art rendering. This is the first of three tiers — Tier 2 (visual polish) and Tier 3 (power user) follow as separate specs.

## 1. Shuffle & Repeat

### Repeat Modes

Three-state cycle toggled by `r`:

| Mode | Behavior |
|------|----------|
| `RepeatMode::Off` | Queue plays through once and stops at the end |
| `RepeatMode::All` | After the last track, wrap to the first and continue |
| `RepeatMode::One` | Replay the current track endlessly |

`RepeatMode` is a new enum in `rustify-core::types`, alongside `PlaybackState`.

### Shuffle

Toggled by `s`:

- **On toggle-on:** Generate a Fisher-Yates permutation of queue indices. The currently-playing track stays as the current position; shuffle affects what `next()` and `previous()` return.
- **On toggle-off:** Restore original order. The currently-playing track remains current — the position is recalculated to match its original index.
- Shuffle state is deterministic per-toggle — calling `next()` repeatedly in shuffle mode walks through the same permutation.

### Core Changes (rustify-core)

**New types in `types.rs`:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    Off,
    All,
    One,
}
```

**New `PlayerCommand` variants:**

```rust
pub enum PlayerCommand {
    // ... existing variants ...
    SetShuffle(bool),
    SetRepeat(RepeatMode),
}
```

**New `PlayerEvent` variant:**

```rust
pub enum PlayerEvent {
    // ... existing variants ...
    ModeChanged { shuffle: bool, repeat: RepeatMode },
}
```

**Changes to `Tracklist`:**

New fields:
- `shuffle: bool`
- `repeat: RepeatMode`
- `shuffle_order: Vec<usize>` — permuted indices into `tracks`
- `shuffle_position: Option<usize>` — current position within `shuffle_order`

Modified methods:
- `next()` — when shuffle is on, advance through `shuffle_order`. When repeat-all, wrap around. When repeat-one, return `current()`.
- `previous()` — walk backward through `shuffle_order` when shuffled.

New methods:
- `set_shuffle(bool)` — generates or clears the permutation
- `set_repeat(RepeatMode)`
- `get_shuffle() -> bool`
- `get_repeat() -> RepeatMode`

**Changes to `Player`:**

New public methods:
- `player.set_shuffle(bool)`
- `player.set_repeat(RepeatMode)`
- `player.get_shuffle() -> bool`
- `player.get_repeat() -> RepeatMode`

These send `PlayerCommand::SetShuffle` / `SetRepeat` to the command thread. The command loop calls `tracklist.set_shuffle()` / `tracklist.set_repeat()` and emits `PlayerEvent::ModeChanged`.

### TUI Changes

**New `PlayerAction` variants:**
- `PlayerAction::ToggleShuffle`
- `PlayerAction::CycleRepeat`

**Keybindings in `app.rs`:**
- `s` → `PlayerAction::ToggleShuffle`
- `r` → `PlayerAction::CycleRepeat`

**Now-playing bar indicators** in `ui/now_playing.rs`:
- Shuffle on: display `[S]` in accent color
- Repeat All: display `[R]`
- Repeat One: display `[R1]`
- Modes off: no indicator

**App state:**
- `app.now_playing.shuffle: bool`
- `app.now_playing.repeat: RepeatMode`
- Updated on `PlayerEvent::ModeChanged`

## 2. Gapless Playback (Dual-Decode Mixer)

### Architecture

The current architecture has one decode thread feeding audio into a single bounded channel, consumed by the cpal output callback. For gapless playback (and future crossfade in Tier 3), we introduce a dual-decode architecture with a mix stage.

```
                        ┌──────────────┐
  Current track ──────► │ Decode A     │──► audio_tx_a ──┐
                        └──────────────┘                 │
                                                         ▼
                                                   ┌───────────┐
                                                   │ MixStage  │──► cpal output
                                                   └───────────┘
                        ┌──────────────┐                 ▲
  Next track ─────────► │ Decode B     │──► audio_tx_b ──┘
  (pre-buffered)        └──────────────┘
```

### Flow

1. Decode thread A decodes the current track. When remaining audio drops below ~3 seconds (calculated from packet timestamps and total duration), it sends `InternalEvent::TrackEnding { remaining_ms: u64 }`.
2. The command loop receives `TrackEnding` and checks the tracklist for a next track. If one exists, it starts Decode B, which pre-buffers into its own channel (`pending_audio_tx`).
3. When Decode A sends `TrackEnded`, the command loop tells the `MixStage` to swap: `active_rx` switches from A's channel to B's channel. Decode B becomes the new current decode.
4. Decode A's thread exits naturally.
5. The cycle repeats — the next `TrackEnding` from the now-current Decode B starts a new pending decode.

### MixStage

The mix stage lives in the cpal output callback. It manages two receive channels:

- `active_rx: Receiver<Vec<f32>>` — current track's audio
- `pending_rx: Option<Receiver<Vec<f32>>>` — next track's pre-buffered audio (set by command loop via atomic swap)

**Gapless behavior:** When `active_rx` is fully drained (returns `Err` or track has ended) and `pending_rx` is `Some`, atomically promote `pending_rx` to `active_rx`. No overlap, no gap.

**Crossfade behavior (Tier 3, not implemented now):** The MixStage struct gains a `crossfade_ms: u64` field. When nonzero, both channels are read simultaneously during the overlap window and mixed with linear fade curves. The infrastructure supports this — Tier 3 just sets the parameter.

### Core Changes

**New `InternalEvent` variant:**

```rust
enum InternalEvent {
    // ... existing ...
    TrackEnding { remaining_ms: u64 },
}
```

**Changes to `CommandLoop`:**

New field:
- `pending_decode: Option<DecodeHandle>` — the pre-started next decode
- `pending_audio_tx: Option<Sender<Vec<f32>>>` — channel for pending decode's audio
- `pending_audio_rx_slot: Arc<Mutex<Option<Receiver<Vec<f32>>>>>` — shared with cpal callback for atomic swap

Modified event handling:
- `TrackEnding` → start pending decode if next track exists
- `TrackEnded` → promote pending decode to current, signal MixStage to swap channels

**Changes to decode thread:**

The decode thread gains remaining-time awareness:
- Compute total duration from codec params (sample count / sample rate)
- Track decoded sample count
- When `(total_samples - decoded_samples) / sample_rate * 1000 < PRE_BUFFER_MS`, send `InternalEvent::TrackEnding`
- `PRE_BUFFER_MS` is a constant, initially 3000 (3 seconds)

**Changes to `create_output_stream`:**

The cpal callback reads from an `active_rx` and checks a `pending_rx_slot` (behind `Arc<Mutex<>>`) on each buffer fill. When `active_rx` is drained and the slot has a receiver, it swaps.

### Public API

No changes to the `Player` public API. Gapless is automatic and internal. The `PlayerEvent::TrackChanged` still fires when the next track starts playing.

## 3. Seek Keybindings

### Keybindings

| Key | Action | Delta |
|-----|--------|-------|
| `Left` / `h` | Seek backward | -5 seconds |
| `Right` / `l` | Seek forward | +5 seconds |
| `Shift+Left` / `Shift+H` | Seek backward (large) | -30 seconds |
| `Shift+Right` / `Shift+L` | Seek forward (large) | +30 seconds |

### Implementation

In `app.rs` `handle_key()`, these keys return `PlayerAction::Seek(delta_ms)` where `delta_ms` is a signed i64. The `main.rs` event loop computes the absolute position:

```
new_position = (current_position as i64 + delta_ms).clamp(0, track_length as i64) as u64
```

Then calls `player.seek(new_position)`.

**Optimistic UI update:** After sending the seek command, the TUI immediately updates `app.now_playing.position_ms` to the new position for instant visual feedback. The next `PositionUpdate` callback from core will correct any drift.

## 4. Album Art

### Art Extraction (rustify-core)

New module: `crates/rustify-core/src/art.rs`

```rust
/// Extract album art for a track.
/// Tries embedded cover art first (via lofty), then sidecar files.
/// Returns raw image bytes (JPEG or PNG) or None.
pub fn extract_art(path: &Path) -> Option<Vec<u8>>
```

**Embedded art:** Uses lofty's `Tag::pictures()` to find `PictureType::CoverFront` (or any picture if no front cover tagged). Returns the picture's `data` bytes.

**Sidecar fallback:** Searches the track file's parent directory for (case-insensitive): `cover.jpg`, `cover.png`, `folder.jpg`, `folder.png`, `album.jpg`, `album.png`. Returns the first match's file contents.

**Re-export:** `pub mod art;` added to `lib.rs`, `pub use art::extract_art;` for convenience.

### Rendering (rustify-tui)

**Dependencies:** `ratatui-image` (already in Cargo.toml) and `image` (already in Cargo.toml).

**Art state in `App`:**

```rust
pub struct ArtState {
    /// Current track's URI (to detect changes and avoid re-extraction)
    pub current_uri: Option<String>,
    /// Decoded image ready for rendering
    pub image: Option<Box<dyn ratatui_image::protocol::StatefulProtocol>>,
}
```

**On `TrackChanged` event:**
1. Compare `track.uri` to `art_state.current_uri`. If same, skip.
2. Call `rustify_core::art::extract_art(&uri_to_path(&track.uri))` on a background thread (don't block the event loop).
3. On result, decode bytes via `image::load_from_memory(&bytes)`, create protocol with `ratatui_image::picker::Picker`, store in `art_state.image`.
4. If no art found, set `art_state.image = None`.

**Now-playing bar layout update:**

```
┌──────────────────────────────────────────────────────────┐
│ [ART ] >> Midnight City              [S] [R]  1:42/4:03  │
│ [ART ]    M83 — Hurry Up, We're D... ━━━━░░░░  Vol: 80   │
└──────────────────────────────────────────────────────────┘
```

Art area: 6 columns wide on the left of the now-playing bar. When art is available, render via `ratatui_image::StatefulImage`. When not available, render a centered `♪` glyph in a bordered box as placeholder.

The now-playing bar height increases from 3 to 4 rows to give the art more vertical space.

## Error Handling

- **Shuffle on empty queue:** No-op. Shuffle toggles but permutation is empty.
- **Gapless pre-buffer failure:** If the next track fails to open/decode, `DecodeFailed` fires as usual. The current track finishes normally, then the player skips to the track after the failed one (or stops if none).
- **Art extraction failure:** Gracefully returns `None`. Placeholder glyph shown. No error propagated to UI — missing art is silent.
- **Seek beyond bounds:** Clamped to `0..track_length`. Seeking past the end triggers `TrackEnded` naturally from the decode thread.

## Testing

### Core Unit Tests

**Tracklist shuffle/repeat:**
- `next()` with `RepeatMode::All` wraps from last to first
- `next()` with `RepeatMode::One` returns same track
- `next()` with `RepeatMode::Off` returns `None` at end
- `set_shuffle(true)` produces a permutation covering all indices
- `set_shuffle(false)` restores original order, current track stays current
- `previous()` in shuffle mode walks backward through shuffle order

**Art extraction:**
- Extract embedded art from a tagged WAV/MP3 test fixture
- Sidecar fallback finds `cover.jpg` in track directory
- Returns `None` when no art exists

**MixStage channel swap:**
- Feed channel A with samples, verify output matches
- Set pending channel B, drain A, verify output switches to B seamlessly
- Verify no samples lost or duplicated during swap

### TUI Unit Tests

**Keybindings:**
- `s` returns `PlayerAction::ToggleShuffle`
- `r` returns `PlayerAction::CycleRepeat`
- `Left` returns `PlayerAction::Seek(-5000)`
- `Shift+Right` returns `PlayerAction::Seek(30000)`

**Now-playing bar:**
- Snapshot: shuffle indicator `[S]` visible when `shuffle = true`
- Snapshot: repeat indicator `[R1]` visible when `repeat = RepeatMode::One`
- Snapshot: art placeholder renders when no image available

### Integration Test (manual)

- Play a multi-track album, verify no silence gap between tracks
- Toggle shuffle mid-playback, verify next track comes from shuffled order
- Cycle through repeat modes, verify end-of-queue behavior for each
- Seek with arrow keys, verify position jumps in progress bar
- Play a track with embedded cover art, verify art renders

## Dependencies

| Crate | Change | Purpose |
|-------|--------|---------|
| `rustify-core` | existing | Shuffle/repeat in Tracklist, gapless in player, art extraction |
| `lofty` | existing | Extract embedded cover art pictures |
| `ratatui-image` | existing (in rustify-tui) | Render album art in terminal |
| `image` | existing (in rustify-tui) | Decode art bytes to DynamicImage |
| `rand` | **new** (in rustify-core) | Fisher-Yates shuffle RNG |

## Out of Scope (deferred to later tiers)

- Crossfade (Tier 3 — MixStage supports it, just needs fade parameter)
- Audio visualizer (Tier 2)
- Color themes (Tier 2)
- Fuzzy search (Tier 2)
- MPRIS / media keys (Tier 3)
- Scrobbling (Tier 3)
- Replay gain (Tier 3)
- Lyrics (Tier 3)
