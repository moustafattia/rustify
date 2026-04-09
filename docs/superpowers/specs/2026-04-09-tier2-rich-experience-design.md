# Tier 2: Rich Experience — Design Spec

**Date:** 2026-04-09
**Status:** Draft
**Depends on:** Tier 1 (shuffle/repeat, seek, gapless, album art)

## Overview

Three features that make the player visually impressive and pleasant to use: an audio spectrum visualizer in the now-playing bar, a color theme system with 5 presets and custom TOML themes, and fuzzy search with ranked results and match highlighting. All changes are TUI-only except one small core addition (sample buffer for the visualizer).

## 1. Audio Spectrum Visualizer

### Layout

The now-playing bar grows from 3 rows to 6 rows when a track is playing. The top 3 rows are the visualizer; the bottom 3 rows are the existing track info, progress bar, mode indicators, and volume.

When stopped (no track playing), the bar shrinks back to 3 rows showing "No track playing."

### Two Modes

Toggled by `Shift+v` (to avoid conflict with lowercase `v` for future use):

**Spectrum bars (default):**
- FFT on recent audio samples
- ~24 vertical bars spanning bass to treble
- Rendered with unicode block characters: `▁▂▃▄▅▆▇█`
- Bars colored using the theme's `visualizer` gradient (low bars get base color, tall bars get bright color)
- Logarithmic frequency mapping: bass frequencies get more bars than treble, matching human perception

**Waveform:**
- Raw audio signal shape rendered as a braille-dot line using unicode braille characters (`⠁⠂⠄⡀⢀⠠⠐⠈` etc.)
- Scrolls left-to-right like an oscilloscope
- Single accent color from the active theme

### Data Source

**Core change (small):** The cpal output callback already processes `f32` samples. Add a shared ring buffer that the callback writes into:

```rust
// In player.rs — shared between cpal callback and Player API
sample_buffer: Arc<Mutex<VecDeque<f32>>>
```

The callback copies the last ~2048 samples (at 44.1kHz stereo, this is ~23ms of audio) into this buffer on each callback invocation. The buffer is a fixed-capacity ring — old samples are discarded when full.

**New Player API method:**

```rust
impl Player {
    /// Get a snapshot of recent audio samples for visualization.
    /// Returns up to 2048 interleaved stereo f32 samples.
    pub fn get_samples(&self) -> Vec<f32>;
}
```

This clones the current buffer contents. Called by the TUI on each tick (4Hz).

### FFT Processing (TUI-only)

**Dependency:** `rustfft` crate added to `crates/rustify-tui/Cargo.toml`.

Processing pipeline (runs on each tick when visualizer is visible):
1. Call `player.get_samples()` — get ~2048 stereo samples
2. Downmix to mono: `(left + right) / 2.0` — yields ~1024 mono samples
3. Apply Hann window to reduce spectral leakage
4. Run 1024-point real FFT via `rustfft`
5. Compute magnitude of each frequency bin: `sqrt(re² + im²)`
6. Group 512 output bins into 24 display bars using logarithmic mapping
7. Scale magnitudes to 0.0..1.0 range (with smoothing: bars decay slowly for visual appeal)
8. Map each bar's value to a unicode block character height

**New TUI module:** `crates/rustify-tui/src/ui/visualizer.rs`

Responsible for FFT computation and both rendering modes. Called from `now_playing.rs` when the visualizer area is being drawn.

### Smoothing

To avoid jittery bars, apply exponential decay: `displayed = max(new_value, displayed * 0.85)`. This makes bars fall smoothly rather than snapping to zero.

## 2. Color Themes

### Theme Struct

```rust
pub struct Theme {
    pub name: String,
    pub fg: Color,           // primary text
    pub fg_dim: Color,       // secondary/muted text
    pub accent: Color,       // highlights, selected items, progress bar, focused borders
    pub accent_dim: Color,   // focused border dim variant
    pub border: Color,       // unfocused borders
    pub error: Color,        // error/warning status messages
    pub visualizer: Vec<Color>, // gradient for spectrum bars (2-4 colors, low→high)
}
```

`Color` is `ratatui::style::Color`. Presets use named colors; custom themes use `Color::Rgb(r, g, b)` parsed from hex strings.

### Built-in Presets

| Name | accent | fg | fg_dim | border | visualizer gradient |
|------|--------|----|--------|--------|---------------------|
| `default` | Magenta | White | Gray | DarkGray | DarkGray → Magenta |
| `nord` | Cyan | #D8DEE9 | #4C566A | #3B4252 | #5E81AC → #88C0D0 |
| `dracula` | #BD93F9 | #F8F8F2 | #6272A4 | #44475A | #6272A4 → #BD93F9 |
| `gruvbox` | #FABD2F | #EBDBB2 | #928374 | #3C3836 | #689D6A → #FABD2F |
| `catppuccin` | #CBA6F7 | #CDD6F4 | #585B70 | #313244 | #585B70 → #CBA6F7 |

### Custom Themes via TOML

Users define a custom theme in `~/.config/rustify/tui.toml`:

```toml
theme = "custom"

[theme.custom]
fg = "#CDD6F4"
fg_dim = "#585B70"
accent = "#F38BA8"
accent_dim = "#A6476E"
border = "#313244"
error = "#F38BA8"
visualizer = ["#585B70", "#F38BA8"]
```

Missing fields fall back to the `default` preset values. The `theme.custom` section is optional — if `theme = "nord"`, no custom section is needed.

### New TUI Module

`crates/rustify-tui/src/theme.rs`

Responsible for:
- `Theme` struct definition
- 5 built-in preset functions: `Theme::default_theme()`, `Theme::nord()`, etc.
- `Theme::from_config(config: &TuiConfig) -> Theme` — resolves preset name or parses custom section
- Helper: `parse_hex_color(hex: &str) -> Color` for `#RRGGBB` strings

### Wiring

- `App` gains a `pub theme: Theme` field, set once at startup
- All UI modules read colors from `app.theme` instead of hardcoded values
- Every `Color::Magenta` → `app.theme.accent`, every `Color::DarkGray` → `app.theme.border`, etc.
- Config field `theme: String` already exists in `TuiConfig` — just needs wiring to `Theme::from_config()`
- Theme selection requires app restart (no hot-reload — YAGNI)

### Config Extension

Add to the `TuiConfig` struct:

```rust
#[serde(default)]
pub custom_theme: Option<CustomThemeConfig>,
```

Where `CustomThemeConfig` is a struct with optional hex color fields matching the `Theme` fields.

## 3. Fuzzy Search

### Behavior

The existing `/` search overlay is upgraded from substring matching to ranked fuzzy matching.

- As the user types, results update live
- Results ranked by match score (best match first), single flat list
- Each result shows: `track name — artist`
- Matched characters highlighted in the theme's accent color
- `j`/`k` or arrows navigate results, `Enter` plays the selected track, `Esc` closes

### Dependency

`nucleo-matcher` crate added to `crates/rustify-tui/Cargo.toml`. Lightweight, same engine as the helix editor's fuzzy picker. Pure Rust, no async.

### Changes to Library

Replace the existing `search()` method in `crates/rustify-tui/src/library.rs`:

```rust
pub struct SearchResult<'a> {
    pub track: &'a Track,
    pub score: u32,
    pub matched_indices: Vec<u32>,
}

impl Library {
    /// Fuzzy search across track names, artist names, and album names.
    /// Returns results ranked by match quality (best first).
    pub fn fuzzy_search(&self, query: &str) -> Vec<SearchResult<'_>>;
}
```

**Implementation:**
1. For each track, build a search string: `"{name} {artist} {album}"`
2. Run `nucleo_matcher::Matcher::fuzzy_match()` on each search string against the query
3. Collect results with score > 0
4. Sort by score descending
5. Return top 50 results (cap to avoid rendering thousands)

### Rendering

In `crates/rustify-tui/src/ui/main_panel.rs`, update the `draw_search()` function:

- Use `matched_indices` from `SearchResult` to apply accent-colored style to matched characters in each result line
- Unmatched characters use `theme.fg`
- The search input line shows the query with a blinking cursor

### Migration

The old `Library::search()` method is removed and replaced by `fuzzy_search()`. All call sites (just the search overlay in `main_panel.rs`) are updated.

## Error Handling

- **Visualizer with no samples:** Show flat bars (all zero height). Happens when stopped or during buffering.
- **FFT on too few samples:** Pad with zeros if buffer has fewer than 1024 samples.
- **Invalid theme hex color:** Log warning, fall back to `default` theme's value for that field.
- **Custom theme missing fields:** Fall back to `default` preset values per-field.
- **Fuzzy search with empty query:** Show no results (empty list), same as current behavior.
- **nucleo-matcher returns no matches:** Show "No results" text.

## Testing

### Core Unit Tests

**Sample buffer:**
- `player.get_samples()` returns empty vec when no audio playing
- Buffer is bounded (doesn't grow unbounded)

### TUI Unit Tests

**Visualizer:**
- FFT pipeline produces correct number of bars (24) from 1024 input samples
- Smoothing decays values correctly
- Spectrum and waveform render without panic at various widths

**Theme:**
- Each preset loads without error
- `parse_hex_color("#FF00FF")` returns `Color::Rgb(255, 0, 255)`
- Custom theme from TOML with partial fields merges with defaults
- Invalid hex gracefully falls back to default

**Fuzzy search:**
- Exact match ranks highest
- Prefix match ranks above middle match
- Query "mid cit" matches "Midnight City" with correct indices
- Empty query returns no results
- Results capped at 50

### Snapshot Tests

- Now-playing bar renders with visualizer area at 80x24
- Theme colors applied to sidebar, main panel, now-playing bar
- Search results show highlighted matched characters

## Dependencies

| Crate | Where | Purpose |
|-------|-------|---------|
| `rustfft` | rustify-tui | FFT for spectrum visualizer |
| `nucleo-matcher` | rustify-tui | Fuzzy string matching |

No new core dependencies. The sample buffer uses existing `Arc<Mutex<>>` and `VecDeque`.

## Out of Scope

- Visualizer audio effects (reverb, echo) — not a visualizer concern
- Theme hot-reload — restart required
- Search across playlists — tracks/artists/albums only
- Visualizer in album detail view — only in now-playing bar
- Custom visualizer bar count — fixed at 24
