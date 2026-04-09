# Tier 2: Rich Experience — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add audio spectrum visualizer, color theme system, and fuzzy search to make the TUI visually impressive and pleasant to use.

**Architecture:** Theme system is the foundation — it changes how every UI module renders colors. Fuzzy search replaces the existing substring search in the library. Visualizer adds a sample buffer to core's cpal callback and a new TUI module for FFT + rendering. All features are TUI-only except the sample buffer.

**Tech Stack:** Rust (rustfft for FFT, nucleo-matcher for fuzzy search), existing ratatui/crossterm stack

**Design Spec:** `docs/superpowers/specs/2026-04-09-tier2-rich-experience-design.md`

---

## File Map

### New files

| File | Responsibility |
|---|---|
| `crates/rustify-tui/src/theme.rs` | Theme struct, 5 presets, hex color parsing, config loading |
| `crates/rustify-tui/src/ui/visualizer.rs` | FFT processing, spectrum bars rendering, waveform rendering |

### Modified files

| File | Changes |
|---|---|
| `crates/rustify-core/Cargo.toml` | (no changes needed) |
| `crates/rustify-core/src/player.rs` | Add sample buffer to cpal callback, `get_samples()` API |
| `crates/rustify-tui/Cargo.toml` | Add `rustfft`, `nucleo-matcher` dependencies |
| `crates/rustify-tui/src/app.rs` | Add `theme: Theme`, `visualizer_mode: VisualizerMode`, `Shift+v` keybinding |
| `crates/rustify-tui/src/config.rs` | Add `CustomThemeConfig` for TOML custom themes |
| `crates/rustify-tui/src/library.rs` | Replace `search()` with `fuzzy_search()` using nucleo-matcher |
| `crates/rustify-tui/src/main.rs` | Pass sample buffer to app, load theme from config |
| `crates/rustify-tui/src/ui/mod.rs` | Pass theme to sub-renderers |
| `crates/rustify-tui/src/ui/sidebar.rs` | Use theme colors instead of hardcoded |
| `crates/rustify-tui/src/ui/main_panel.rs` | Use theme colors, render fuzzy match highlights |
| `crates/rustify-tui/src/ui/now_playing.rs` | Expand to 6 rows, integrate visualizer, use theme colors |

---

## Task 1: Color Theme System

**Files:**
- Create: `crates/rustify-tui/src/theme.rs`
- Modify: `crates/rustify-tui/src/config.rs`
- Modify: `crates/rustify-tui/src/app.rs`
- Modify: `crates/rustify-tui/src/main.rs`

- [ ] **Step 1: Write tests for theme**

Create `crates/rustify-tui/src/theme.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_magenta_accent() {
        let theme = Theme::default_theme();
        assert_eq!(theme.accent, Color::Magenta);
    }

    #[test]
    fn all_presets_load() {
        let names = ["default", "nord", "dracula", "gruvbox", "catppuccin"];
        for name in names {
            let theme = Theme::from_name(name);
            assert!(!theme.name.is_empty());
        }
    }

    #[test]
    fn unknown_name_falls_back_to_default() {
        let theme = Theme::from_name("nonexistent");
        assert_eq!(theme.name, "default");
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#FF00FF"), Some(Color::Rgb(255, 0, 255)));
        assert_eq!(parse_hex_color("#000000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn parse_hex_color_invalid() {
        assert_eq!(parse_hex_color("not-a-color"), None);
        assert_eq!(parse_hex_color("#GG00FF"), None);
    }
}
```

- [ ] **Step 2: Implement Theme struct and presets**

Add the full implementation above the tests in `theme.rs`:

- `Theme` struct with fields: `name`, `fg`, `fg_dim`, `accent`, `accent_dim`, `border`, `error`, `visualizer: Vec<Color>`
- `Theme::default_theme()` — magenta accent, white fg, darkgray borders
- `Theme::nord()` — cyan accent (#88C0D0)
- `Theme::dracula()` — purple accent (#BD93F9)
- `Theme::gruvbox()` — yellow accent (#FABD2F)
- `Theme::catppuccin()` — mauve accent (#CBA6F7)
- `Theme::from_name(name: &str) -> Theme` — match on name string
- `pub fn parse_hex_color(hex: &str) -> Option<Color>` — parse `#RRGGBB` to `Color::Rgb`

- [ ] **Step 3: Add CustomThemeConfig to config.rs**

Add to `crates/rustify-tui/src/config.rs`:

```rust
#[derive(Debug, Default, Deserialize)]
pub struct CustomThemeConfig {
    pub fg: Option<String>,
    pub fg_dim: Option<String>,
    pub accent: Option<String>,
    pub accent_dim: Option<String>,
    pub border: Option<String>,
    pub error: Option<String>,
    pub visualizer: Option<Vec<String>>,
}
```

Add field to `TuiConfig`:

```rust
#[serde(default)]
pub custom_theme: Option<CustomThemeConfig>,
```

- [ ] **Step 4: Add theme to App and wire in main.rs**

Add `pub theme: Theme` to `App` struct. Initialize from config in `main.rs`:

```rust
let theme = theme::Theme::from_config(&config);
app.theme = theme;
```

Add `mod theme;` to main.rs.

- [ ] **Step 5: Replace hardcoded colors in all UI modules**

Update `sidebar.rs`, `main_panel.rs`, `now_playing.rs`, and `ui/mod.rs`:
- `Color::Magenta` → `app.theme.accent`
- `Color::DarkGray` → `app.theme.border`
- `Color::White` → `app.theme.fg`
- `Color::Gray` → `app.theme.fg_dim`
- `Color::Yellow` (status/error) → `app.theme.error`

- [ ] **Step 6: Run tests, commit**

Run: `cargo test --workspace`
Commit: `git commit -m "feat(tui): add color theme system with 5 presets and custom TOML themes"`

---

## Task 2: Fuzzy Search

**Files:**
- Modify: `crates/rustify-tui/Cargo.toml` (add `nucleo-matcher`)
- Modify: `crates/rustify-tui/src/library.rs`
- Modify: `crates/rustify-tui/src/ui/main_panel.rs`

- [ ] **Step 1: Add nucleo-matcher dependency**

Add to `crates/rustify-tui/Cargo.toml`:

```toml
nucleo-matcher = "0.3"
```

- [ ] **Step 2: Write tests for fuzzy search**

Add to tests in `library.rs`:

```rust
    #[test]
    fn fuzzy_search_finds_exact_match() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("Midnight City");
        assert!(!results.is_empty());
        assert_eq!(results[0].track.name, "Midnight City");
    }

    #[test]
    fn fuzzy_search_partial_match() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("mid cit");
        assert!(!results.is_empty());
        assert_eq!(results[0].track.name, "Midnight City");
    }

    #[test]
    fn fuzzy_search_empty_query_returns_empty() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("");
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_search_no_match() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("zzzzzzz");
        assert!(results.is_empty());
    }

    #[test]
    fn fuzzy_search_returns_matched_indices() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.fuzzy_search("Midnight");
        assert!(!results[0].matched_indices.is_empty());
    }
```

- [ ] **Step 3: Implement fuzzy_search**

Replace the `search()` method in `library.rs` with `fuzzy_search()`:

```rust
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

pub struct SearchResult<'a> {
    pub track: &'a Track,
    pub score: u32,
    pub matched_indices: Vec<u32>,
}

impl Library {
    pub fn fuzzy_search(&self, query: &str) -> Vec<SearchResult<'_>> {
        if query.is_empty() {
            return Vec::new();
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut results: Vec<SearchResult<'_>> = self.tracks
            .iter()
            .filter_map(|track| {
                let haystack = format!(
                    "{} {} {}",
                    track.name,
                    track.artists.first().unwrap_or(&String::new()),
                    track.album
                );
                let mut indices = Vec::new();
                let mut buf = Vec::new();
                let utf32 = Utf32Str::new(&haystack, &mut buf);
                let score = pattern.score(utf32, &mut matcher)?;
                pattern.indices(utf32, &mut matcher, &mut indices);
                Some(SearchResult {
                    track,
                    score,
                    matched_indices: indices,
                })
            })
            .collect();

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(50);
        results
    }
}
```

- [ ] **Step 4: Update main_panel.rs search rendering**

Update `draw_search()` in `main_panel.rs` to use `fuzzy_search()` and highlight matched characters using `matched_indices` with the theme's accent color.

- [ ] **Step 5: Remove old search() method**

Delete the old `Library::search()` method and its tests. Update any remaining call sites.

- [ ] **Step 6: Run tests, commit**

Run: `cargo test --workspace`
Commit: `git commit -m "feat(tui): replace substring search with fuzzy matching via nucleo-matcher"`

---

## Task 3: Sample Buffer (Core)

**Files:**
- Modify: `crates/rustify-core/src/player.rs`

- [ ] **Step 1: Add sample buffer to Player**

Add a shared sample buffer alongside the existing `SharedState`:

```rust
// In Player struct
sample_buffer: Arc<Mutex<VecDeque<f32>>>,
```

- [ ] **Step 2: Write samples in cpal callback**

In the cpal output callback, after computing each output sample, push it into the shared buffer (capped at 4096 samples):

```rust
// After writing to frame
if let Ok(mut buf) = sample_buffer.try_lock() {
    buf.push_back(left);
    buf.push_back(right);
    while buf.len() > 4096 {
        buf.pop_front();
    }
}
```

- [ ] **Step 3: Add get_samples() API**

```rust
impl Player {
    pub fn get_samples(&self) -> Vec<f32> {
        self.sample_buffer.lock().unwrap().iter().copied().collect()
    }
}
```

- [ ] **Step 4: Run tests, commit**

Run: `cargo test -p rustify-core`
Commit: `git commit -m "feat(core): add sample buffer for audio visualization"`

---

## Task 4: Audio Visualizer

**Files:**
- Create: `crates/rustify-tui/src/ui/visualizer.rs`
- Modify: `crates/rustify-tui/Cargo.toml` (add `rustfft`)
- Modify: `crates/rustify-tui/src/app.rs` (add visualizer mode state)
- Modify: `crates/rustify-tui/src/ui/now_playing.rs` (expand layout, integrate visualizer)
- Modify: `crates/rustify-tui/src/main.rs` (pass player reference for samples)

- [ ] **Step 1: Add rustfft dependency**

```toml
rustfft = "6"
```

- [ ] **Step 2: Write tests for visualizer**

Create `crates/rustify-tui/src/ui/visualizer.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_produces_24_bars() {
        let samples = vec![0.0f32; 1024];
        let bars = compute_spectrum_bars(&samples);
        assert_eq!(bars.len(), 24);
    }

    #[test]
    fn silent_input_produces_zero_bars() {
        let samples = vec![0.0f32; 1024];
        let bars = compute_spectrum_bars(&samples);
        assert!(bars.iter().all(|&b| b == 0.0));
    }

    #[test]
    fn smoothing_decays() {
        let mut state = VisualizerState::new();
        state.bars = vec![1.0; 24];
        let new_bars = vec![0.0; 24];
        state.apply_smoothing(&new_bars);
        // After one smoothing step, bars should be 0.85 (decay factor)
        assert!(state.bars[0] > 0.8 && state.bars[0] < 0.9);
    }
}
```

- [ ] **Step 3: Implement visualizer module**

Full implementation of `visualizer.rs`:

- `VisualizerMode` enum: `Spectrum`, `Waveform`
- `VisualizerState` struct: holds previous bar values for smoothing
- `compute_spectrum_bars(samples: &[f32]) -> Vec<f32>`: downmix mono, Hann window, 1024-pt FFT, log-frequency binning into 24 bars, normalize to 0.0..1.0
- `apply_smoothing(&mut self, new_bars: &[f32])`: exponential decay `max(new, old * 0.85)`
- `draw_spectrum(frame, area, state, theme)`: render bars with unicode blocks `▁▂▃▄▅▆▇█`
- `draw_waveform(frame, area, samples, theme)`: render braille-dot oscilloscope line

- [ ] **Step 4: Add VisualizerMode and state to App**

In `app.rs`:
```rust
pub visualizer_mode: ui::visualizer::VisualizerMode,
pub visualizer_state: ui::visualizer::VisualizerState,
```

Add `Shift+v` keybinding to toggle mode.

- [ ] **Step 5: Expand now-playing bar to 6 rows**

Update `ui/mod.rs` layout: now-playing bar grows from `Constraint::Length(3)` to `Constraint::Length(6)` when a track is playing. Top 3 rows render the visualizer, bottom 3 rows keep existing track info/progress/modes.

Update `now_playing.rs` to split its area and call `visualizer::draw_spectrum()` or `visualizer::draw_waveform()` based on mode.

- [ ] **Step 6: Wire sample buffer in main.rs**

Pass player's `get_samples()` result into the app on each tick:

```rust
Ok(AppEvent::Tick) => {
    app.handle_tick();
    // Feed visualizer with fresh samples
    let samples = player.get_samples();
    app.update_visualizer(&samples);
}
```

- [ ] **Step 7: Run tests, commit**

Run: `cargo test --workspace`
Commit: `git commit -m "feat(tui): add audio spectrum visualizer with FFT and waveform modes"`

---

## Summary

| Task | Feature | Files touched |
|------|---------|---------------|
| 1 | Color themes | theme.rs (new), config.rs, app.rs, main.rs, all ui/*.rs |
| 2 | Fuzzy search | library.rs, main_panel.rs, Cargo.toml |
| 3 | Sample buffer | player.rs (core) |
| 4 | Visualizer | visualizer.rs (new), now_playing.rs, app.rs, main.rs, Cargo.toml |

Order matters: themes first (changes all UI modules), then fuzzy search (independent), then sample buffer + visualizer (depends on themes for colors).

Run full test suite after each task: `cargo test --workspace`
