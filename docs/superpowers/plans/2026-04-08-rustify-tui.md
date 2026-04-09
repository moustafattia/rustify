# Rustify TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a rich terminal music player (`rustify-tui`) on top of `rustify-core`, usable both over SSH on the YoyoPod Pi and as a standalone desktop terminal player.

**Architecture:** Ratatui + crossterm for rendering, crossbeam channels for a unified event loop multiplexing keyboard, player callbacks, and tick timer. Sidebar+Main layout with persistent now-playing bar. Single `App` struct owns all UI state; rendering is a pure function of that state.

**Tech Stack:** Rust (ratatui, crossterm, crossbeam, ratatui-image, dirs, toml, serde), depends on workspace crate `rustify-core`.

**Design Spec:** `docs/superpowers/specs/2026-04-08-rustify-tui-design.md`

---

## File Map

| File | Responsibility |
|---|---|
| `crates/rustify-tui/Cargo.toml` | Binary crate dependencies |
| `crates/rustify-tui/src/main.rs` | Entry point: terminal setup, event loop, cleanup |
| `crates/rustify-tui/src/config.rs` | `TuiConfig` — TOML parsing, defaults, platform paths |
| `crates/rustify-tui/src/event.rs` | `AppEvent` enum, input thread, tick thread |
| `crates/rustify-tui/src/app.rs` | `App` struct, state types (`Focus`, `MainView`, etc.), `handle_event()` |
| `crates/rustify-tui/src/library.rs` | `Library` index — organizes scanned tracks by artist/album |
| `crates/rustify-tui/src/ui/mod.rs` | `draw()` entry point — top-level layout split |
| `crates/rustify-tui/src/ui/sidebar.rs` | Sidebar rendering: library nav + queue |
| `crates/rustify-tui/src/ui/now_playing.rs` | Now-playing bar: art, track info, progress, volume |
| `crates/rustify-tui/src/ui/main_panel.rs` | Main panel views: Artists, Albums, Songs, Playlists, AlbumDetail, Search |

---

## Task 1: Scaffold Crate + Minimal Terminal App

**Files:**
- Create: `crates/rustify-tui/Cargo.toml`
- Create: `crates/rustify-tui/src/main.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add rustify-tui to workspace**

Edit the workspace root `Cargo.toml` to add the new crate:

```toml
[workspace]
members = ["crates/rustify-core", "crates/rustify-tui", "bindings/python"]
default-members = ["crates/rustify-core"]
resolver = "2"
```

- [ ] **Step 2: Create crate Cargo.toml**

Create `crates/rustify-tui/Cargo.toml`:

```toml
[package]
name = "rustify-tui"
version = "0.1.0"
edition = "2021"
description = "Rich terminal music player built on rustify-core"

[[bin]]
name = "rustify-tui"
path = "src/main.rs"

[dependencies]
rustify-core = { path = "../rustify-core" }
ratatui = "0.29"
crossterm = "0.28"
crossbeam = "0.8"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
dirs = "6"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Create minimal main.rs**

Create `crates/rustify-tui/src/main.rs` — sets up raw mode, alternate screen, draws a centered "Rustify" label, exits on `q`:

```rust
use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

fn main() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main loop
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let text = Paragraph::new("Rustify TUI — press q to quit")
                .alignment(Alignment::Center);
            frame.render_widget(text, area);
        })?;

        if event::poll(std::time::Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p rustify-tui`
Expected: Compiles with 0 errors.

- [ ] **Step 5: Verify it runs**

Run: `cargo run -p rustify-tui`
Expected: Alternate screen shows "Rustify TUI — press q to quit". Press `q`, terminal restores cleanly.

- [ ] **Step 6: Commit**

```bash
git add crates/rustify-tui/ Cargo.toml
git commit -m "feat(tui): scaffold rustify-tui crate with minimal terminal app"
```

---

## Task 2: Config Module

**Files:**
- Create: `crates/rustify-tui/src/config.rs`
- Modify: `crates/rustify-tui/src/main.rs` (add `mod config;`)

- [ ] **Step 1: Write tests for config**

Add to the bottom of a new file `crates/rustify-tui/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let config = TuiConfig::default();
        assert_eq!(config.alsa_device, "default");
        assert!(config.music_dirs.is_empty());
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn parse_from_toml_string() {
        let toml_str = r#"
            music_dirs = ["/home/pi/Music"]
            alsa_device = "hw:0"
            theme = "nord"
        "#;
        let config: TuiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.music_dirs, vec![std::path::PathBuf::from("/home/pi/Music")]);
        assert_eq!(config.alsa_device, "hw:0");
        assert_eq!(config.theme, "nord");
    }

    #[test]
    fn parse_partial_toml_uses_defaults() {
        let toml_str = r#"
            music_dirs = ["/Music"]
        "#;
        let config: TuiConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.alsa_device, "default");
        assert_eq!(config.theme, "default");
    }

    #[test]
    fn config_path_returns_some() {
        // dirs::config_dir() may return None in some CI environments,
        // so we just verify the function doesn't panic.
        let _ = TuiConfig::config_path();
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustify-tui`
Expected: Compilation fails — `TuiConfig` not defined.

- [ ] **Step 3: Implement TuiConfig**

Add the implementation above the tests in `crates/rustify-tui/src/config.rs`:

```rust
use std::path::PathBuf;

use serde::Deserialize;

/// TUI configuration, loaded from `~/.config/rustify/tui.toml`.
#[derive(Debug, Deserialize)]
pub struct TuiConfig {
    /// Directories to scan for music files.
    #[serde(default)]
    pub music_dirs: Vec<PathBuf>,

    /// ALSA device name passed to rustify-core.
    #[serde(default = "default_alsa_device")]
    pub alsa_device: String,

    /// Theme preset name.
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_alsa_device() -> String {
    "default".to_string()
}

fn default_theme() -> String {
    "default".to_string()
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            music_dirs: Vec::new(),
            alsa_device: default_alsa_device(),
            theme: default_theme(),
        }
    }
}

impl TuiConfig {
    /// Platform-appropriate config file path.
    /// Linux/macOS: `~/.config/rustify/tui.toml`
    /// Windows: `%APPDATA%\rustify\tui.toml`
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("rustify").join("tui.toml"))
    }

    /// Load config from disk. Returns defaults if file doesn't exist.
    /// Prints a warning to stderr if the file exists but can't be parsed.
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };

        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("rustify: failed to parse {}: {e}", path.display());
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }
}
```

- [ ] **Step 4: Add mod declaration to main.rs**

Add `mod config;` at the top of `crates/rustify-tui/src/main.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p rustify-tui`
Expected: All 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/rustify-tui/src/config.rs crates/rustify-tui/src/main.rs
git commit -m "feat(tui): add config module with TOML parsing and platform paths"
```

---

## Task 3: Event System

**Files:**
- Create: `crates/rustify-tui/src/event.rs`
- Modify: `crates/rustify-tui/src/main.rs` (add `mod event;`)

- [ ] **Step 1: Write tests for AppEvent and EventLoop**

Add to the bottom of a new file `crates/rustify-tui/src/event.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_event_variants_exist() {
        // Verify enum construction compiles
        let _ = AppEvent::Tick;
        let _ = AppEvent::Error("test".into());
    }

    #[test]
    fn event_loop_sends_ticks() {
        let event_loop = EventLoop::new();
        // Wait for at least one tick (250ms interval + margin)
        std::thread::sleep(std::time::Duration::from_millis(350));
        let event = event_loop.receiver().try_recv();
        assert!(event.is_ok());
    }

    #[test]
    fn event_loop_receiver_is_clone_safe() {
        let event_loop = EventLoop::new();
        let rx = event_loop.receiver();
        let _rx2 = rx.clone();
    }

    #[test]
    fn sender_can_push_player_events() {
        let event_loop = EventLoop::new();
        let tx = event_loop.sender();
        tx.send(AppEvent::Player(PlayerEvent::StateChanged(
            PlaybackState::Playing,
        )))
        .unwrap();
        // Drain ticks first, find our event
        loop {
            match event_loop.receiver().try_recv() {
                Ok(AppEvent::Player(_)) => break,
                Ok(_) => continue,
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustify-tui`
Expected: Compilation fails — `AppEvent`, `EventLoop` not defined.

- [ ] **Step 3: Implement event module**

Add the implementation above the tests in `crates/rustify-tui/src/event.rs`:

```rust
use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender};
use crossterm::event::{self, Event, KeyEvent, MouseEvent};
use rustify_core::types::{PlaybackState, PlayerEvent, Track};

use crate::library::Library;

/// Unified event type for the TUI event loop.
#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard input
    Key(KeyEvent),
    /// Mouse input
    Mouse(MouseEvent),
    /// Terminal resize
    Resize(u16, u16),
    /// Player state/track/position callback
    Player(PlayerEvent),
    /// UI refresh tick (~4Hz)
    Tick,
    /// Background library scan completed
    ScanComplete(Library),
    /// Non-player error
    Error(String),
}

/// Manages background threads that feed events into a single channel.
pub struct EventLoop {
    tx: Sender<AppEvent>,
    rx: Receiver<AppEvent>,
}

impl EventLoop {
    /// Create a new event loop. Spawns the input and tick threads immediately.
    pub fn new() -> Self {
        let (tx, rx) = channel::unbounded();

        // Tick thread — sends AppEvent::Tick at ~4Hz
        let tick_tx = tx.clone();
        thread::Builder::new()
            .name("rustify-tick".into())
            .spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(250));
                    if tick_tx.send(AppEvent::Tick).is_err() {
                        break;
                    }
                }
            })
            .expect("failed to spawn tick thread");

        // Input thread — polls crossterm events and forwards them
        let input_tx = tx.clone();
        thread::Builder::new()
            .name("rustify-input".into())
            .spawn(move || {
                loop {
                    // Poll with timeout so we can detect channel disconnect
                    match event::poll(Duration::from_millis(100)) {
                        Ok(true) => match event::read() {
                            Ok(Event::Key(key)) => {
                                if input_tx.send(AppEvent::Key(key)).is_err() {
                                    break;
                                }
                            }
                            Ok(Event::Mouse(mouse)) => {
                                if input_tx.send(AppEvent::Mouse(mouse)).is_err() {
                                    break;
                                }
                            }
                            Ok(Event::Resize(w, h)) => {
                                if input_tx.send(AppEvent::Resize(w, h)).is_err() {
                                    break;
                                }
                            }
                            _ => {}
                        },
                        Ok(false) => {}
                        Err(_) => break,
                    }
                }
            })
            .expect("failed to spawn input thread");

        Self { tx, rx }
    }

    /// Get a clone of the sender for pushing events from player callbacks.
    pub fn sender(&self) -> Sender<AppEvent> {
        self.tx.clone()
    }

    /// Get the receiver for the main event loop.
    pub fn receiver(&self) -> Receiver<AppEvent> {
        self.rx.clone()
    }
}
```

**Note:** This references `crate::library::Library` which doesn't exist yet. Add a temporary stub so it compiles. Create `crates/rustify-tui/src/library.rs`:

```rust
/// In-memory music library index.
/// Populated by background scan of music directories.
#[derive(Debug)]
pub struct Library;
```

- [ ] **Step 4: Add mod declarations to main.rs**

Add to the top of `crates/rustify-tui/src/main.rs`:

```rust
mod config;
mod event;
mod library;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p rustify-tui`
Expected: All event tests pass (plus config tests from Task 2).

- [ ] **Step 6: Commit**

```bash
git add crates/rustify-tui/src/event.rs crates/rustify-tui/src/library.rs crates/rustify-tui/src/main.rs
git commit -m "feat(tui): add event system with input and tick threads"
```

---

## Task 4: App State + Event Loop Wiring

**Files:**
- Create: `crates/rustify-tui/src/app.rs`
- Modify: `crates/rustify-tui/src/main.rs` (rewire to use App + EventLoop)

- [ ] **Step 1: Write tests for App state**

Add to the bottom of a new file `crates/rustify-tui/src/app.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn make_app() -> App {
        App::new()
    }

    #[test]
    fn initial_state() {
        let app = make_app();
        assert!(!app.should_quit);
        assert_eq!(app.focus, Focus::Sidebar);
        assert_eq!(app.sidebar_nav_index, 0);
    }

    #[test]
    fn q_sets_should_quit() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn tab_cycles_focus() {
        let mut app = make_app();
        assert_eq!(app.focus, Focus::Sidebar);
        app.handle_key(make_key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Main);
        app.handle_key(make_key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn number_keys_switch_sidebar_nav() {
        let mut app = make_app();
        app.handle_key(make_key(KeyCode::Char('2')));
        assert_eq!(app.sidebar_nav_index, 1);
        assert_eq!(app.main_view, MainView::Albums);
        app.handle_key(make_key(KeyCode::Char('3')));
        assert_eq!(app.sidebar_nav_index, 2);
        assert_eq!(app.main_view, MainView::Songs);
    }

    #[test]
    fn j_k_navigates_sidebar_nav_when_focused() {
        let mut app = make_app();
        assert_eq!(app.sidebar_nav_index, 0);
        app.handle_key(make_key(KeyCode::Char('j')));
        assert_eq!(app.sidebar_nav_index, 1);
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.sidebar_nav_index, 0);
        // k at top stays at 0
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.sidebar_nav_index, 0);
    }

    #[test]
    fn enter_on_sidebar_nav_switches_view_and_focus() {
        let mut app = make_app();
        app.sidebar_nav_index = 2; // Songs
        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.main_view, MainView::Songs);
        assert_eq!(app.focus, Focus::Main);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustify-tui`
Expected: Compilation fails — `App`, `Focus`, `MainView` not defined.

- [ ] **Step 3: Implement App state and key handling**

Add the implementation above the tests in `crates/rustify-tui/src/app.rs`:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;
use rustify_core::types::{PlaybackState, Track};

use crate::library::Library;

/// Which UI region has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Main,
    Search,
}

/// Which view is displayed in the main panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainView {
    Artists,
    Albums,
    Songs,
    Playlists,
    AlbumDetail,
}

const NAV_ITEMS: &[&str] = &["Artists", "Albums", "Songs", "Playlists"];

/// Now-playing state cached from player callbacks.
#[derive(Debug, Default)]
pub struct NowPlayingState {
    pub track: Option<Track>,
    pub state: Option<PlaybackState>,
    pub position_ms: u64,
    pub volume: u8,
}

/// Search overlay state.
#[derive(Debug, Default)]
pub struct SearchState {
    pub active: bool,
    pub query: String,
    pub results_state: ListState,
}

/// Queue display state.
#[derive(Debug, Default)]
pub struct QueueState {
    pub list_state: ListState,
    pub track_uris: Vec<String>,
    pub track_names: Vec<String>,
}

/// Status message shown temporarily above the now-playing bar.
#[derive(Debug)]
pub struct StatusMessage {
    pub text: String,
    pub expires_tick: u64,
}

/// Root application state.
pub struct App {
    pub should_quit: bool,
    pub focus: Focus,
    pub sidebar_nav_index: usize,
    pub main_view: MainView,
    pub now_playing: NowPlayingState,
    pub library: Option<Library>,
    pub scanning: bool,
    pub search: SearchState,
    pub queue: QueueState,
    pub status: Option<StatusMessage>,
    pub tick_count: u64,

    // Per-view list states for ratatui
    pub artist_list_state: ListState,
    pub album_list_state: ListState,
    pub song_list_state: ListState,
    pub playlist_list_state: ListState,
    pub detail_list_state: ListState,

    // Artist/album drill-down context
    pub selected_artist: Option<String>,
    pub selected_album_index: Option<usize>,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            focus: Focus::Sidebar,
            sidebar_nav_index: 0,
            main_view: MainView::Artists,
            now_playing: NowPlayingState::default(),
            library: None,
            scanning: false,
            search: SearchState::default(),
            queue: QueueState::default(),
            status: None,
            tick_count: 0,

            artist_list_state: ListState::default(),
            album_list_state: ListState::default(),
            song_list_state: ListState::default(),
            playlist_list_state: ListState::default(),
            detail_list_state: ListState::default(),

            selected_artist: None,
            selected_album_index: None,
        }
    }

    /// Handle a key event. Returns true if the event was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        // Only handle key press events (not release/repeat)
        if key.kind != KeyEventKind::Press {
            return false;
        }

        // Global keys (work regardless of focus)
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return true;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Main,
                    Focus::Main => Focus::Sidebar,
                    Focus::Search => Focus::Main,
                };
                return true;
            }
            KeyCode::Char(c @ '1'..='4') => {
                let idx = (c as usize) - ('1' as usize);
                self.sidebar_nav_index = idx;
                self.main_view = nav_index_to_view(idx);
                return true;
            }
            KeyCode::Char(' ') => {
                // Play/pause toggle — will be wired to player in Task 11
                return true;
            }
            KeyCode::Char('n') if self.focus != Focus::Search => {
                // Next track — will be wired to player in Task 11
                return true;
            }
            KeyCode::Char('p') if self.focus != Focus::Search => {
                // Previous track — will be wired to player in Task 11
                return true;
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.now_playing.volume = (self.now_playing.volume + 5).min(100);
                return true;
            }
            KeyCode::Char('-') => {
                self.now_playing.volume = self.now_playing.volume.saturating_sub(5);
                return true;
            }
            KeyCode::Char('/') if self.focus != Focus::Search => {
                self.search.active = true;
                self.search.query.clear();
                self.focus = Focus::Search;
                return true;
            }
            KeyCode::Esc => {
                if self.search.active {
                    self.search.active = false;
                    self.focus = Focus::Main;
                    return true;
                }
                // Back navigation from album detail
                if self.main_view == MainView::AlbumDetail {
                    self.main_view = if self.selected_artist.is_some() {
                        MainView::Albums
                    } else {
                        MainView::Artists
                    };
                    return true;
                }
                return false;
            }
            _ => {}
        }

        // Search mode key handling
        if self.focus == Focus::Search {
            return self.handle_search_key(key);
        }

        // Focus-specific keys
        match self.focus {
            Focus::Sidebar => self.handle_sidebar_key(key),
            Focus::Main => self.handle_main_key(key),
            Focus::Search => false,
        }
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.sidebar_nav_index < NAV_ITEMS.len() - 1 {
                    self.sidebar_nav_index += 1;
                }
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.sidebar_nav_index = self.sidebar_nav_index.saturating_sub(1);
                true
            }
            KeyCode::Enter => {
                self.main_view = nav_index_to_view(self.sidebar_nav_index);
                self.focus = Focus::Main;
                true
            }
            _ => false,
        }
    }

    fn handle_main_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_main_selection(1);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_main_selection(-1);
                true
            }
            KeyCode::Enter => {
                self.activate_main_selection();
                true
            }
            _ => false,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.search.query.push(c);
                true
            }
            KeyCode::Backspace => {
                self.search.query.pop();
                true
            }
            KeyCode::Enter => {
                // Navigate to selected search result — wired in Task 12
                self.search.active = false;
                self.focus = Focus::Main;
                true
            }
            _ => false,
        }
    }

    fn move_main_selection(&mut self, delta: i32) {
        let list_state = self.active_list_state_mut();
        let current = list_state.selected().unwrap_or(0);
        let new = if delta > 0 {
            current.saturating_add(delta as usize)
        } else {
            current.saturating_sub((-delta) as usize)
        };
        list_state.select(Some(new));
    }

    fn activate_main_selection(&mut self) {
        match self.main_view {
            MainView::Artists => {
                // Drill into artist's albums
                if let Some(lib) = &self.library {
                    let artists: Vec<&String> = lib.artist_names();
                    if let Some(selected) = self.artist_list_state.selected() {
                        if let Some(name) = artists.get(selected) {
                            self.selected_artist = Some((*name).clone());
                            self.main_view = MainView::Albums;
                            self.album_list_state.select(Some(0));
                        }
                    }
                }
            }
            MainView::Albums => {
                // Drill into album's tracks
                if let Some(selected) = self.album_list_state.selected() {
                    self.selected_album_index = Some(selected);
                    self.main_view = MainView::AlbumDetail;
                    self.detail_list_state.select(Some(0));
                }
            }
            MainView::Songs | MainView::AlbumDetail => {
                // Play selected track — will be wired to player in Task 11
            }
            MainView::Playlists => {
                // Load selected playlist — will be wired in Task 13
            }
        }
    }

    /// Get a mutable reference to the active view's list state.
    pub fn active_list_state_mut(&mut self) -> &mut ListState {
        match self.main_view {
            MainView::Artists => &mut self.artist_list_state,
            MainView::Albums => &mut self.album_list_state,
            MainView::Songs => &mut self.song_list_state,
            MainView::Playlists => &mut self.playlist_list_state,
            MainView::AlbumDetail => &mut self.detail_list_state,
        }
    }

    /// Handle a tick event — increment counter, expire status messages.
    pub fn handle_tick(&mut self) {
        self.tick_count += 1;
        if let Some(ref status) = self.status {
            if self.tick_count >= status.expires_tick {
                self.status = None;
            }
        }
    }

    /// Set a status message that auto-dismisses after ~5 seconds (20 ticks at 4Hz).
    pub fn set_status(&mut self, text: String) {
        self.status = Some(StatusMessage {
            text,
            expires_tick: self.tick_count + 20,
        });
    }

    /// Sidebar nav item labels.
    pub fn nav_items(&self) -> &[&str] {
        NAV_ITEMS
    }
}

fn nav_index_to_view(index: usize) -> MainView {
    match index {
        0 => MainView::Artists,
        1 => MainView::Albums,
        2 => MainView::Songs,
        3 => MainView::Playlists,
        _ => MainView::Artists,
    }
}
```

- [ ] **Step 4: Add mod declaration to main.rs**

Add `mod app;` to the top of `crates/rustify-tui/src/main.rs`.

- [ ] **Step 5: Rewire main.rs to use App + EventLoop**

Replace the body of `main()` in `crates/rustify-tui/src/main.rs`:

```rust
use std::io;

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

mod app;
mod config;
mod event;
mod library;

use app::App;
use event::{AppEvent, EventLoop};

fn main() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let _config = config::TuiConfig::load();
    let event_loop = EventLoop::new();
    let rx = event_loop.receiver();
    let mut app = App::new();

    // Main loop
    loop {
        terminal.draw(|frame| {
            let area = frame.area();
            let status = format!(
                "Rustify TUI | Focus: {:?} | View: {:?} | q to quit",
                app.focus, app.main_view
            );
            frame.render_widget(Paragraph::new(status), area);
        })?;

        match rx.recv() {
            Ok(AppEvent::Key(key)) => {
                app.handle_key(key);
            }
            Ok(AppEvent::Tick) => {
                app.handle_tick();
            }
            Ok(_) => {}
            Err(_) => break,
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    Ok(())
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p rustify-tui`
Expected: All tests pass (config + app state tests).

- [ ] **Step 7: Commit**

```bash
git add crates/rustify-tui/src/app.rs crates/rustify-tui/src/main.rs
git commit -m "feat(tui): add app state with focus management and key handling"
```

---

## Task 5: Top-Level Layout

**Files:**
- Create: `crates/rustify-tui/src/ui/mod.rs`
- Create: `crates/rustify-tui/src/ui/sidebar.rs` (stub)
- Create: `crates/rustify-tui/src/ui/now_playing.rs` (stub)
- Create: `crates/rustify-tui/src/ui/main_panel.rs` (stub)
- Modify: `crates/rustify-tui/src/main.rs` (add `mod ui;`, call `ui::draw()`)

- [ ] **Step 1: Write snapshot test for layout**

Add to the bottom of a new file `crates/rustify-tui/src/ui/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn layout_has_three_regions_at_80x24() {
        let mut app = App::new();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                draw(frame, &mut app);
            })
            .unwrap();

        // Verify the terminal rendered without panic.
        // The buffer should have content in the sidebar area (col 0),
        // the main panel area, and the bottom now-playing bar.
        let buf = terminal.backend().buffer();
        // Top-left should be part of sidebar
        assert!(buf.area().width == 80);
        assert!(buf.area().height == 24);
    }

    #[test]
    fn layout_renders_at_minimal_size() {
        let mut app = App::new();
        let backend = TestBackend::new(60, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                draw(frame, &mut app);
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        assert!(buf.area().width == 60);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustify-tui`
Expected: Compilation fails — `ui::draw` not defined.

- [ ] **Step 3: Implement draw() and sub-module stubs**

Replace the content above the tests in `crates/rustify-tui/src/ui/mod.rs`:

```rust
pub mod main_panel;
pub mod now_playing;
pub mod sidebar;

use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};

use crate::app::App;

/// Draw the full TUI layout.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Split vertically: [content area] [now-playing bar (3 rows)]
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    let content_area = vertical[0];
    let now_playing_area = vertical[1];

    // Split content horizontally: [sidebar (30%)] [main panel (70%)]
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(content_area);

    let sidebar_area = horizontal[0];
    let main_area = horizontal[1];

    // Render each region
    sidebar::draw(frame, app, sidebar_area);
    main_panel::draw(frame, app, main_area);
    now_playing::draw(frame, app, now_playing_area);
}
```

Create `crates/rustify-tui/src/ui/sidebar.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(" Library ")
        .borders(Borders::ALL)
        .border_style(if app.focus == Focus::Sidebar {
            Style::default().fg(Color::Magenta)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let items: Vec<ListItem> = app
        .nav_items()
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            let style = if i == app.sidebar_nav_index {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(name).style(style)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}
```

Create `crates/rustify-tui/src/ui/main_panel.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::{App, Focus, MainView};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = match app.main_view {
        MainView::Artists => " Artists ",
        MainView::Albums => " Albums ",
        MainView::Songs => " Songs ",
        MainView::Playlists => " Playlists ",
        MainView::AlbumDetail => " Album ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if app.focus == Focus::Main {
            Style::default().fg(Color::Magenta)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let content = if app.scanning {
        "Scanning library..."
    } else if app.library.is_none() {
        "No music directories configured. Edit ~/.config/rustify/tui.toml"
    } else {
        "Library loaded"
    };

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, area);
}
```

Create `crates/rustify-tui/src/ui/now_playing.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let text = if let Some(ref track) = app.now_playing.track {
        let artist = if track.artists.is_empty() {
            "Unknown".to_string()
        } else {
            track.artists.join(", ")
        };
        format!("{} — {}", track.name, artist)
    } else {
        "No track playing".to_string()
    };

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}
```

- [ ] **Step 4: Add mod declaration + wire up draw()**

Update `crates/rustify-tui/src/main.rs` — add `mod ui;` at the top, and replace the `terminal.draw` closure:

```rust
terminal.draw(|frame| {
    ui::draw(frame, &mut app);
})?;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p rustify-tui`
Expected: All tests pass. The snapshot tests confirm the layout renders at 80x24 and 60x20 without panicking.

- [ ] **Step 6: Verify it runs visually**

Run: `cargo run -p rustify-tui`
Expected: Three bordered regions — sidebar with "Library" title on the left, main panel on the right, now-playing bar at the bottom. `q` quits.

- [ ] **Step 7: Commit**

```bash
git add crates/rustify-tui/src/ui/
git commit -m "feat(tui): add three-region layout with sidebar, main panel, and now-playing bar"
```

---

## Task 6: Now-Playing Bar (Full)

**Files:**
- Modify: `crates/rustify-tui/src/ui/now_playing.rs`

- [ ] **Step 1: Write snapshot test for now-playing bar**

Add to the bottom of `crates/rustify-tui/src/ui/now_playing.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rustify_core::types::{PlaybackState, Track};

    fn make_track() -> Track {
        Track {
            uri: "file:///music/song.mp3".into(),
            name: "Midnight City".into(),
            artists: vec!["M83".into()],
            album: "Hurry Up, We're Dreaming".into(),
            length: 243_000, // 4:03
            track_no: Some(1),
        }
    }

    #[test]
    fn renders_track_info_when_playing() {
        let mut app = App::new();
        app.now_playing.track = Some(make_track());
        app.now_playing.state = Some(PlaybackState::Playing);
        app.now_playing.position_ms = 102_000; // 1:42
        app.now_playing.volume = 80;

        let backend = TestBackend::new(80, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw(frame, &mut app, frame.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("Midnight City"));
        assert!(content.contains("M83"));
    }

    #[test]
    fn renders_no_track_when_stopped() {
        let mut app = App::new();
        let backend = TestBackend::new(80, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw(frame, &mut app, frame.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("No track"));
    }
}
```

- [ ] **Step 2: Run tests to verify they pass with stub (or need updates)**

Run: `cargo test -p rustify-tui -- now_playing`
Expected: Tests may pass with the stub or fail on the assertion. Either way, proceed to the full implementation.

- [ ] **Step 3: Implement full now-playing bar with progress**

Replace the content of `crates/rustify-tui/src/ui/now_playing.rs` (above tests):

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::app::App;
use rustify_core::types::PlaybackState;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width < 20 {
        return;
    }

    if let Some(ref track) = app.now_playing.track {
        let artist = if track.artists.is_empty() {
            "Unknown".to_string()
        } else {
            track.artists.join(", ")
        };

        let state_icon = match app.now_playing.state {
            Some(PlaybackState::Playing) => ">>",
            Some(PlaybackState::Paused) => "||",
            _ => "--",
        };

        let pos = format_time(app.now_playing.position_ms);
        let dur = format_time(track.length);

        let ratio = if track.length > 0 {
            (app.now_playing.position_ms as f64 / track.length as f64).min(1.0)
        } else {
            0.0
        };

        // Layout: [track info left] [progress center] [time+vol right]
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(35),
                Constraint::Percentage(45),
                Constraint::Percentage(20),
            ])
            .split(inner);

        // Left: track info
        let info = format!("{state_icon} {}\n   {artist} — {}", track.name, track.album);
        let info_widget = Paragraph::new(info)
            .style(Style::default().fg(Color::White));
        frame.render_widget(info_widget, cols[0]);

        // Center: progress bar
        if cols[1].height > 0 {
            let gauge = Gauge::default()
                .ratio(ratio)
                .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
                .label("");
            let gauge_area = Rect {
                y: cols[1].y + cols[1].height.saturating_sub(1),
                height: 1,
                ..cols[1]
            };
            frame.render_widget(gauge, gauge_area);
        }

        // Right: time + volume
        let time_vol = format!("{pos} / {dur}\nVol: {}", app.now_playing.volume);
        let right_widget = Paragraph::new(time_vol)
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(right_widget, cols[2]);
    } else {
        let paragraph = Paragraph::new("No track playing")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }
}

fn format_time(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rustify-tui -- now_playing`
Expected: Both tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rustify-tui/src/ui/now_playing.rs
git commit -m "feat(tui): implement now-playing bar with progress gauge and track info"
```

---

## Task 7: Sidebar (Full)

**Files:**
- Modify: `crates/rustify-tui/src/ui/sidebar.rs`

- [ ] **Step 1: Write tests for sidebar rendering**

Add to the bottom of `crates/rustify-tui/src/ui/sidebar.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn sidebar_shows_all_nav_items() {
        let mut app = App::new();
        let backend = TestBackend::new(24, 20);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                draw(frame, &mut app, frame.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("Artists"));
        assert!(content.contains("Albums"));
        assert!(content.contains("Songs"));
        assert!(content.contains("Playlists"));
    }

    #[test]
    fn sidebar_shows_queue_section() {
        let mut app = App::new();
        app.queue.track_names = vec!["Song A".into(), "Song B".into()];

        let backend = TestBackend::new(24, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw(frame, &mut app, frame.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("Queue"));
        assert!(content.contains("Song A"));
    }
}
```

- [ ] **Step 2: Implement full sidebar with nav + queue**

Replace the content of `crates/rustify-tui/src/ui/sidebar.rs` (above tests):

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_style = if app.focus == Focus::Sidebar {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Library ")
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Split inner area: [nav items (5 rows)] [queue (remaining)]
    let nav_height = 5u16; // 4 items + 1 divider
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(nav_height.min(inner.height)),
            Constraint::Min(0),
        ])
        .split(inner);

    // Nav items
    let nav_items: Vec<ListItem> = app
        .nav_items()
        .iter()
        .enumerate()
        .map(|(i, &name)| {
            let marker = if i == app.sidebar_nav_index {
                "> "
            } else {
                "  "
            };
            let style = if i == app.sidebar_nav_index {
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(format!("{marker}{name}")).style(style)
        })
        .collect();

    let nav_list = List::new(nav_items);
    frame.render_widget(nav_list, chunks[0]);

    // Queue section
    if chunks[1].height > 1 {
        let queue_area = chunks[1];

        // Queue header
        let header_area = Rect {
            height: 1,
            ..queue_area
        };
        let header = Paragraph::new("── Queue ──")
            .style(Style::default().fg(Color::Magenta))
            .alignment(Alignment::Center);
        frame.render_widget(header, header_area);

        // Queue items
        let list_area = Rect {
            y: queue_area.y + 1,
            height: queue_area.height.saturating_sub(1),
            ..queue_area
        };

        if app.queue.track_names.is_empty() {
            let empty = Paragraph::new("  (empty)")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(empty, list_area);
        } else {
            let items: Vec<ListItem> = app
                .queue
                .track_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let style = Style::default().fg(Color::Gray);
                    ListItem::new(format!("  {}. {}", i + 1, name)).style(style)
                })
                .collect();

            let queue_list = List::new(items)
                .highlight_style(Style::default().fg(Color::White).bg(Color::DarkGray));
            frame.render_stateful_widget(queue_list, list_area, &mut app.queue.list_state);
        }
    }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p rustify-tui -- sidebar`
Expected: Both sidebar tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-tui/src/ui/sidebar.rs
git commit -m "feat(tui): implement sidebar with nav items and queue display"
```

---

## Task 8: Library Index

**Files:**
- Modify: `crates/rustify-tui/src/library.rs` (replace stub)

- [ ] **Step 1: Write tests for library building**

Replace the content of `crates/rustify-tui/src/library.rs` with tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rustify_core::types::Track;

    fn make_tracks() -> Vec<Track> {
        vec![
            Track {
                uri: "file:///music/m83/hurry/midnight.mp3".into(),
                name: "Midnight City".into(),
                artists: vec!["M83".into()],
                album: "Hurry Up, We're Dreaming".into(),
                length: 243_000,
                track_no: Some(1),
            },
            Track {
                uri: "file:///music/m83/hurry/reunion.mp3".into(),
                name: "Reunion".into(),
                artists: vec!["M83".into()],
                album: "Hurry Up, We're Dreaming".into(),
                length: 407_000,
                track_no: Some(2),
            },
            Track {
                uri: "file:///music/m83/saturdays/kim.mp3".into(),
                name: "Kim & Jessie".into(),
                artists: vec!["M83".into()],
                album: "Saturdays = Youth".into(),
                length: 315_000,
                track_no: Some(1),
            },
            Track {
                uri: "file:///music/radiohead/ok/paranoid.mp3".into(),
                name: "Paranoid Android".into(),
                artists: vec!["Radiohead".into()],
                album: "OK Computer".into(),
                length: 383_000,
                track_no: Some(2),
            },
        ]
    }

    #[test]
    fn build_library_groups_by_artist() {
        let lib = Library::from_tracks(make_tracks());
        let names = lib.artist_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&&"M83".to_string()));
        assert!(names.contains(&&"Radiohead".to_string()));
    }

    #[test]
    fn artist_albums_returns_correct_albums() {
        let lib = Library::from_tracks(make_tracks());
        let albums = lib.albums_by_artist("M83");
        assert_eq!(albums.len(), 2);
        let album_names: Vec<&str> = albums.iter().map(|a| a.name.as_str()).collect();
        assert!(album_names.contains(&"Hurry Up, We're Dreaming"));
        assert!(album_names.contains(&"Saturdays = Youth"));
    }

    #[test]
    fn album_tracks_returns_sorted_tracks() {
        let lib = Library::from_tracks(make_tracks());
        let albums = lib.albums_by_artist("M83");
        let hurry = albums.iter().find(|a| a.name.contains("Hurry")).unwrap();
        assert_eq!(hurry.tracks.len(), 2);
        assert_eq!(hurry.tracks[0].name, "Midnight City");
        assert_eq!(hurry.tracks[1].name, "Reunion");
    }

    #[test]
    fn all_tracks_returns_everything() {
        let lib = Library::from_tracks(make_tracks());
        assert_eq!(lib.all_tracks().len(), 4);
    }

    #[test]
    fn all_albums_returns_everything() {
        let lib = Library::from_tracks(make_tracks());
        assert_eq!(lib.all_albums().len(), 3);
    }

    #[test]
    fn empty_library() {
        let lib = Library::from_tracks(vec![]);
        assert!(lib.artist_names().is_empty());
        assert!(lib.all_tracks().is_empty());
        assert!(lib.all_albums().is_empty());
    }

    #[test]
    fn search_finds_matching_tracks() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.search("midnight");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Midnight City");
    }

    #[test]
    fn search_is_case_insensitive() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.search("PARANOID");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_matches_artist_names() {
        let lib = Library::from_tracks(make_tracks());
        let results = lib.search("radiohead");
        assert_eq!(results.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustify-tui -- library`
Expected: Compilation fails — `Library::from_tracks`, etc. not defined.

- [ ] **Step 3: Implement Library**

Add the implementation above the tests in `crates/rustify-tui/src/library.rs`:

```rust
use std::collections::BTreeMap;

use rustify_core::types::Track;

/// An album in the library index.
#[derive(Debug, Clone)]
pub struct Album {
    pub name: String,
    pub artist: String,
    pub tracks: Vec<Track>,
}

/// In-memory music library index, organized by artist and album.
#[derive(Debug)]
pub struct Library {
    /// Artists sorted alphabetically, each with their albums.
    artists: BTreeMap<String, Vec<Album>>,
    /// Flat list of all tracks for the Songs view.
    tracks: Vec<Track>,
}

impl Library {
    /// Build a library index from a flat list of tracks.
    /// Groups by artist → album, sorts tracks within albums by track number.
    pub fn from_tracks(tracks: Vec<Track>) -> Self {
        let mut artist_albums: BTreeMap<String, BTreeMap<String, Vec<Track>>> = BTreeMap::new();

        for track in &tracks {
            let artist_name = if track.artists.is_empty() {
                "Unknown Artist".to_string()
            } else {
                track.artists[0].clone()
            };

            artist_albums
                .entry(artist_name)
                .or_default()
                .entry(track.album.clone())
                .or_default()
                .push(track.clone());
        }

        let artists: BTreeMap<String, Vec<Album>> = artist_albums
            .into_iter()
            .map(|(artist_name, albums_map)| {
                let mut albums: Vec<Album> = albums_map
                    .into_iter()
                    .map(|(album_name, mut album_tracks)| {
                        album_tracks.sort_by_key(|t| t.track_no.unwrap_or(u32::MAX));
                        Album {
                            name: album_name,
                            artist: artist_name.clone(),
                            tracks: album_tracks,
                        }
                    })
                    .collect();
                albums.sort_by(|a, b| a.name.cmp(&b.name));
                (artist_name, albums)
            })
            .collect();

        let mut sorted_tracks = tracks;
        sorted_tracks.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Self {
            artists,
            tracks: sorted_tracks,
        }
    }

    /// Sorted list of artist names.
    pub fn artist_names(&self) -> Vec<&String> {
        self.artists.keys().collect()
    }

    /// Albums for a given artist name.
    pub fn albums_by_artist(&self, artist: &str) -> &[Album] {
        self.artists
            .get(artist)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// All albums across all artists (sorted by name).
    pub fn all_albums(&self) -> Vec<&Album> {
        let mut albums: Vec<&Album> = self.artists.values().flat_map(|a| a.iter()).collect();
        albums.sort_by(|a, b| a.name.cmp(&b.name));
        albums
    }

    /// All tracks (sorted by name).
    pub fn all_tracks(&self) -> &[Track] {
        &self.tracks
    }

    /// Case-insensitive substring search across track names, artist names, and album names.
    pub fn search(&self, query: &str) -> Vec<&Track> {
        let query_lower = query.to_lowercase();
        self.tracks
            .iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&query_lower)
                    || t.album.to_lowercase().contains(&query_lower)
                    || t.artists
                        .iter()
                        .any(|a| a.to_lowercase().contains(&query_lower))
            })
            .collect()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rustify-tui -- library`
Expected: All 9 library tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rustify-tui/src/library.rs
git commit -m "feat(tui): implement library index with artist/album grouping and search"
```

---

## Task 9: Main Panel Views

**Files:**
- Modify: `crates/rustify-tui/src/ui/main_panel.rs`

- [ ] **Step 1: Write tests for main panel views**

Add to the bottom of `crates/rustify-tui/src/ui/main_panel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::library::Library;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rustify_core::types::Track;

    fn make_app_with_library() -> App {
        let tracks = vec![
            Track {
                uri: "file:///music/a.mp3".into(),
                name: "Alpha".into(),
                artists: vec!["Artist A".into()],
                album: "Album One".into(),
                length: 200_000,
                track_no: Some(1),
            },
            Track {
                uri: "file:///music/b.mp3".into(),
                name: "Beta".into(),
                artists: vec!["Artist B".into()],
                album: "Album Two".into(),
                length: 300_000,
                track_no: Some(1),
            },
        ];
        let mut app = App::new();
        app.library = Some(Library::from_tracks(tracks));
        app.artist_list_state.select(Some(0));
        app
    }

    fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw(frame, app, frame.area());
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    #[test]
    fn artists_view_shows_artist_names() {
        let mut app = make_app_with_library();
        app.main_view = MainView::Artists;
        let content = render_to_string(&mut app, 40, 15);
        assert!(content.contains("Artist A"));
        assert!(content.contains("Artist B"));
    }

    #[test]
    fn songs_view_shows_track_names() {
        let mut app = make_app_with_library();
        app.main_view = MainView::Songs;
        let content = render_to_string(&mut app, 40, 15);
        assert!(content.contains("Alpha"));
        assert!(content.contains("Beta"));
    }

    #[test]
    fn scanning_shows_loading_message() {
        let mut app = App::new();
        app.scanning = true;
        let content = render_to_string(&mut app, 40, 15);
        assert!(content.contains("Scanning"));
    }
}
```

- [ ] **Step 2: Implement full main panel views**

Replace the content of `crates/rustify-tui/src/ui/main_panel.rs` (above tests):

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::{App, Focus, MainView};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = match app.main_view {
        MainView::Artists => " Artists ",
        MainView::Albums => {
            if app.selected_artist.is_some() {
                " Albums "
            } else {
                " All Albums "
            }
        }
        MainView::Songs => " Songs ",
        MainView::Playlists => " Playlists ",
        MainView::AlbumDetail => " Tracks ",
    };

    let border_style = if app.focus == Focus::Main {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    // Search overlay takes priority
    if app.search.active {
        draw_search(frame, app, inner);
        return;
    }

    if app.scanning {
        let loading = Paragraph::new("Scanning library...")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center);
        frame.render_widget(loading, inner);
        return;
    }

    let Some(ref library) = app.library else {
        let msg = Paragraph::new("No music directories configured.\nEdit ~/.config/rustify/tui.toml")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(msg, inner);
        return;
    };

    match app.main_view {
        MainView::Artists => {
            let names = library.artist_names();
            let items: Vec<ListItem> = names
                .iter()
                .map(|name| ListItem::new(name.as_str()))
                .collect();
            let list = List::new(items)
                .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, inner, &mut app.artist_list_state);
        }
        MainView::Albums => {
            let albums = if let Some(ref artist) = app.selected_artist {
                library.albums_by_artist(artist).iter().collect::<Vec<_>>()
            } else {
                library.all_albums()
            };
            let items: Vec<ListItem> = albums
                .iter()
                .map(|album| {
                    ListItem::new(format!("{} — {}", album.name, album.artist))
                })
                .collect();
            let list = List::new(items)
                .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, inner, &mut app.album_list_state);
        }
        MainView::Songs => {
            let tracks = library.all_tracks();
            let items: Vec<ListItem> = tracks
                .iter()
                .map(|t| {
                    let artist = t.artists.first().map(|a| a.as_str()).unwrap_or("Unknown");
                    ListItem::new(format!("{} — {}", t.name, artist))
                })
                .collect();
            let list = List::new(items)
                .highlight_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, inner, &mut app.song_list_state);
        }
        MainView::Playlists => {
            let msg = Paragraph::new("No playlists found.")
                .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(msg, inner);
            // Full playlist rendering is added in Task 11
        }
        MainView::AlbumDetail => {
            // Get the selected album's tracks
            let albums = if let Some(ref artist) = app.selected_artist {
                library.albums_by_artist(artist).iter().collect::<Vec<_>>()
            } else {
                library.all_albums()
            };

            if let Some(&album) = app.selected_album_index.and_then(|i| albums.get(i)) {
                let items: Vec<ListItem> = album
                    .tracks
                    .iter()
                    .map(|t| {
                        let num = t.track_no.map(|n| format!("{n:2}. ")).unwrap_or_default();
                        let dur = format_duration(t.length);
                        ListItem::new(format!("{num}{}  [{dur}]", t.name))
                    })
                    .collect();
                let list = List::new(items)
                    .highlight_style(
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");
                frame.render_stateful_widget(list, inner, &mut app.detail_list_state);
            }
        }
    }
}

fn draw_search(frame: &mut Frame, app: &mut App, area: Rect) {
    // Split: [search input (1 row)] [results (remaining)]
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);

    // Search input
    let input = Paragraph::new(format!("/ {}", app.search.query))
        .style(Style::default().fg(Color::Yellow));
    frame.render_widget(input, chunks[0]);

    // Search results
    if let Some(ref library) = app.library {
        let results = library.search(&app.search.query);
        let items: Vec<ListItem> = results
            .iter()
            .take(chunks[1].height as usize)
            .map(|t| {
                let artist = t.artists.first().map(|a| a.as_str()).unwrap_or("Unknown");
                ListItem::new(format!("{} — {}", t.name, artist))
            })
            .collect();
        let list = List::new(items)
            .highlight_style(Style::default().fg(Color::Magenta))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut app.search.results_state);
    }
}

fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p rustify-tui -- main_panel`
Expected: All 3 main panel tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-tui/src/ui/main_panel.rs
git commit -m "feat(tui): implement main panel with artist/album/song list views and search overlay"
```

---

## Task 10: Player Integration

**Files:**
- Modify: `crates/rustify-tui/src/main.rs`
- Modify: `crates/rustify-tui/src/app.rs`

This task wires `rustify-core::Player` into the app — player callbacks push into the event channel, and key handlers send commands to the player.

- [ ] **Step 1: Write tests for player event handling in App**

Add to the existing tests in `crates/rustify-tui/src/app.rs`:

```rust
    #[test]
    fn player_state_change_updates_now_playing() {
        let mut app = make_app();
        app.handle_player_event(PlayerEvent::StateChanged(PlaybackState::Playing));
        assert_eq!(app.now_playing.state, Some(PlaybackState::Playing));
    }

    #[test]
    fn player_track_change_updates_now_playing() {
        let mut app = make_app();
        let track = Track {
            uri: "file:///test.mp3".into(),
            name: "Test".into(),
            artists: vec!["Artist".into()],
            album: "Album".into(),
            length: 100_000,
            track_no: None,
        };
        app.handle_player_event(PlayerEvent::TrackChanged(track.clone()));
        assert_eq!(app.now_playing.track.as_ref().unwrap().name, "Test");
    }

    #[test]
    fn player_position_update_updates_now_playing() {
        let mut app = make_app();
        app.handle_player_event(PlayerEvent::PositionUpdate(42_000));
        assert_eq!(app.now_playing.position_ms, 42_000);
    }

    #[test]
    fn player_error_sets_status_message() {
        let mut app = make_app();
        app.handle_player_event(PlayerEvent::Error("decode failed".into()));
        assert!(app.status.is_some());
        assert!(app.status.as_ref().unwrap().text.contains("decode failed"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustify-tui -- app`
Expected: Compilation fails — `handle_player_event` not defined.

- [ ] **Step 3: Add handle_player_event to App**

Add this method to the `impl App` block in `crates/rustify-tui/src/app.rs`:

```rust
    /// Handle a player event (callback from rustify-core).
    pub fn handle_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::StateChanged(state) => {
                self.now_playing.state = Some(state);
            }
            PlayerEvent::TrackChanged(track) => {
                self.now_playing.track = Some(track);
                self.now_playing.position_ms = 0;
            }
            PlayerEvent::PositionUpdate(ms) => {
                self.now_playing.position_ms = ms;
            }
            PlayerEvent::Error(msg) => {
                self.set_status(msg);
            }
        }
    }
```

Also add the missing import at the top of `app.rs`:

```rust
use rustify_core::types::{PlaybackState, PlayerEvent, Track};
```

(The `PlayerEvent` import is new; `PlaybackState` and `Track` were already imported.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p rustify-tui -- app`
Expected: All app tests pass (including new player event tests).

- [ ] **Step 5: Rewire main.rs with full player integration**

Replace `crates/rustify-tui/src/main.rs` with:

```rust
use std::io;
use std::path::PathBuf;
use std::thread;

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;

use rustify_core::metadata::read_metadata;
use rustify_core::player::{Player, PlayerConfig};
use rustify_core::scanner;
use rustify_core::types::PlayerEvent;

mod app;
mod config;
mod event;
mod library;
mod ui;

use app::App;
use event::{AppEvent, EventLoop};
use library::Library;

fn main() -> io::Result<()> {
    // Load config
    let config = config::TuiConfig::load();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create event loop
    let event_loop = EventLoop::new();
    let rx = event_loop.receiver();
    let tx = event_loop.sender();

    // Create player
    let player_config = PlayerConfig {
        alsa_device: config.alsa_device.clone(),
        music_dirs: config.music_dirs.clone(),
    };

    let player = match Player::new(player_config) {
        Ok(p) => p,
        Err(e) => {
            // Restore terminal before printing error
            disable_raw_mode()?;
            execute!(
                terminal.backend_mut(),
                LeaveAlternateScreen,
                crossterm::event::DisableMouseCapture
            )?;
            eprintln!("rustify: failed to create player: {e}");
            std::process::exit(1);
        }
    };

    // Register player callbacks to push into event channel
    let tx_state = tx.clone();
    player.on_state_change(Box::new(move |state| {
        tx_state
            .send(AppEvent::Player(PlayerEvent::StateChanged(state)))
            .ok();
    }));

    let tx_track = tx.clone();
    player.on_track_change(Box::new(move |track| {
        tx_track
            .send(AppEvent::Player(PlayerEvent::TrackChanged(track)))
            .ok();
    }));

    let tx_pos = tx.clone();
    player.on_position_update(Box::new(move |ms| {
        tx_pos
            .send(AppEvent::Player(PlayerEvent::PositionUpdate(ms)))
            .ok();
    }));

    let tx_err = tx.clone();
    player.on_error(Box::new(move |msg| {
        tx_err
            .send(AppEvent::Player(PlayerEvent::Error(msg)))
            .ok();
    }));

    let mut app = App::new();
    app.now_playing.volume = player.get_volume();

    // Start background library scan if music_dirs configured
    if !config.music_dirs.is_empty() {
        app.scanning = true;
        let music_dirs = config.music_dirs.clone();
        let scan_tx = tx.clone();
        thread::Builder::new()
            .name("rustify-scan".into())
            .spawn(move || {
                let library = scan_library(&music_dirs);
                scan_tx.send(AppEvent::ScanComplete(library)).ok();
            })
            .expect("failed to spawn scan thread");
    }

    // Main event loop
    loop {
        terminal.draw(|frame| {
            ui::draw(frame, &mut app);
        })?;

        match rx.recv() {
            Ok(AppEvent::Key(key)) => {
                app.handle_key(key);
                // Sync volume changes to player
                player.set_volume(app.now_playing.volume);
            }
            Ok(AppEvent::Player(event)) => {
                app.handle_player_event(event);
            }
            Ok(AppEvent::Tick) => {
                app.handle_tick();
            }
            Ok(AppEvent::ScanComplete(library)) => {
                app.library = Some(library);
                app.scanning = false;
                app.artist_list_state.select(Some(0));
            }
            Ok(AppEvent::Error(msg)) => {
                app.set_status(msg);
            }
            Ok(_) => {}
            Err(_) => break,
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup
    player.shutdown();
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    Ok(())
}

/// Scan all configured music directories and build a Library.
fn scan_library(music_dirs: &[PathBuf]) -> Library {
    let mut all_tracks = Vec::new();

    for dir in music_dirs {
        match scanner::scan_directory(dir) {
            Ok(uris) => {
                for uri in uris {
                    match read_metadata(&uri) {
                        Ok(track) => all_tracks.push(track),
                        Err(e) => eprintln!("rustify: metadata error: {e}"),
                    }
                }
            }
            Err(e) => eprintln!("rustify: scan error for {}: {e}", dir.display()),
        }
    }

    Library::from_tracks(all_tracks)
}
```

- [ ] **Step 6: Add player transport commands to key handler**

In `crates/rustify-tui/src/app.rs`, the `handle_key` method has placeholder comments for play/pause/next/prev. These will be driven by the main loop — `main.rs` checks what key was pressed and calls the player directly. Add a method to `App` that returns an optional command:

```rust
/// Player command to execute after key handling.
#[derive(Debug)]
pub enum PlayerAction {
    PlayPause,
    Next,
    Previous,
    Seek(i64), // delta in ms
    PlayTrackUri(String),
    LoadTrackUris(Vec<String>),
}
```

Update `handle_key` to return `Option<PlayerAction>` instead of `bool`. Replace the placeholder matches:

```rust
    KeyCode::Char(' ') => {
        return Some(PlayerAction::PlayPause);
    }
    KeyCode::Char('n') if self.focus != Focus::Search => {
        return Some(PlayerAction::Next);
    }
    KeyCode::Char('p') if self.focus != Focus::Search => {
        return Some(PlayerAction::Previous);
    }
```

And update the `handle_key` signature:

```rust
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PlayerAction> {
```

Where the old `return true` becomes `return None` (event consumed, no player action) and the player-related returns use `Some(...)`.

Then in `main.rs`, update the key handler:

```rust
Ok(AppEvent::Key(key)) => {
    if let Some(action) = app.handle_key(key) {
        match action {
            app::PlayerAction::PlayPause => {
                match app.now_playing.state {
                    Some(rustify_core::types::PlaybackState::Playing) => player.pause(),
                    _ => player.play(),
                }
            }
            app::PlayerAction::Next => player.next(),
            app::PlayerAction::Previous => player.previous(),
            app::PlayerAction::Seek(delta) => {
                let pos = app.now_playing.position_ms as i64 + delta;
                player.seek(pos.max(0) as u64);
            }
            app::PlayerAction::PlayTrackUri(uri) => {
                player.load_track_uris(vec![uri]);
                player.play();
            }
            app::PlayerAction::LoadTrackUris(uris) => {
                player.load_track_uris(uris);
                player.play();
            }
        }
    }
    player.set_volume(app.now_playing.volume);
}
```

- [ ] **Step 7: Update existing tests for new return type**

In the test module of `app.rs`, update assertions. Where tests checked `app.handle_key(...)` returning a bool, they now check for `None` or `Some(PlayerAction::...)`:

```rust
    #[test]
    fn q_sets_should_quit() {
        let mut app = make_app();
        let _ = app.handle_key(make_key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    // Space returns PlayPause action
    #[test]
    fn space_returns_play_pause_action() {
        let mut app = make_app();
        let action = app.handle_key(make_key(KeyCode::Char(' ')));
        assert!(matches!(action, Some(PlayerAction::PlayPause)));
    }
```

- [ ] **Step 8: Run all tests**

Run: `cargo test -p rustify-tui`
Expected: All tests pass.

- [ ] **Step 9: Verify it compiles**

Run: `cargo build -p rustify-tui`
Expected: Compiles with 0 errors.

- [ ] **Step 10: Commit**

```bash
git add crates/rustify-tui/src/
git commit -m "feat(tui): wire player integration with callbacks, transport commands, and library scanning"
```

---

## Task 11: Playlist Management

**Files:**
- Modify: `crates/rustify-tui/src/app.rs` (playlist state + actions)
- Modify: `crates/rustify-tui/src/ui/main_panel.rs` (playlist view rendering)

- [ ] **Step 1: Write tests for playlist actions**

Add to the tests in `crates/rustify-tui/src/app.rs`:

```rust
    #[test]
    fn save_queue_as_m3u_generates_content() {
        let mut app = make_app();
        app.queue.track_uris = vec![
            "file:///music/a.mp3".into(),
            "file:///music/b.flac".into(),
        ];
        let content = app.generate_m3u_content();
        assert!(content.contains("#EXTM3U"));
        assert!(content.contains("/music/a.mp3"));
        assert!(content.contains("/music/b.flac"));
    }
```

- [ ] **Step 2: Add M3U generation to App**

Add to the `impl App` block in `crates/rustify-tui/src/app.rs`:

```rust
    /// Generate M3U content from the current queue.
    pub fn generate_m3u_content(&self) -> String {
        let mut content = String::from("#EXTM3U\n");
        for uri in &self.queue.track_uris {
            let path = uri.strip_prefix("file://").unwrap_or(uri);
            content.push_str(path);
            content.push('\n');
        }
        content
    }
```

- [ ] **Step 3: Add playlist data to App state**

Add a field to `App`:

```rust
    pub playlists: Vec<rustify_core::types::Playlist>,
```

Initialize it as `playlists: Vec::new()` in `App::new()`.

- [ ] **Step 4: Update main panel to render playlists**

In `crates/rustify-tui/src/ui/main_panel.rs`, replace the `MainView::Playlists` arm:

```rust
        MainView::Playlists => {
            if app.playlists.is_empty() {
                let msg = Paragraph::new("No playlists found.")
                    .style(Style::default().fg(Color::DarkGray));
                frame.render_widget(msg, inner);
            } else {
                let items: Vec<ListItem> = app
                    .playlists
                    .iter()
                    .map(|p| {
                        ListItem::new(format!("{} ({} tracks)", p.name, p.track_count))
                    })
                    .collect();
                let list = List::new(items)
                    .highlight_style(
                        Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");
                frame.render_stateful_widget(list, inner, &mut app.playlist_list_state);
            }
        }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p rustify-tui`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/rustify-tui/src/app.rs crates/rustify-tui/src/ui/main_panel.rs
git commit -m "feat(tui): add playlist view rendering and M3U generation from queue"
```

---

## Task 12: Queue Management

**Files:**
- Modify: `crates/rustify-tui/src/app.rs`

- [ ] **Step 1: Write tests for queue operations**

Add to tests in `crates/rustify-tui/src/app.rs`:

```rust
    #[test]
    fn add_to_queue() {
        let mut app = make_app();
        app.add_to_queue("file:///music/test.mp3".into(), "Test Song".into());
        assert_eq!(app.queue.track_uris.len(), 1);
        assert_eq!(app.queue.track_names.len(), 1);
        assert_eq!(app.queue.track_names[0], "Test Song");
    }

    #[test]
    fn remove_from_queue() {
        let mut app = make_app();
        app.add_to_queue("file:///a.mp3".into(), "A".into());
        app.add_to_queue("file:///b.mp3".into(), "B".into());
        app.add_to_queue("file:///c.mp3".into(), "C".into());
        app.remove_from_queue(1);
        assert_eq!(app.queue.track_uris.len(), 2);
        assert_eq!(app.queue.track_names[1], "C");
    }

    #[test]
    fn reorder_queue_down() {
        let mut app = make_app();
        app.add_to_queue("file:///a.mp3".into(), "A".into());
        app.add_to_queue("file:///b.mp3".into(), "B".into());
        app.add_to_queue("file:///c.mp3".into(), "C".into());
        app.reorder_queue(0, 1); // Move A down
        assert_eq!(app.queue.track_names[0], "B");
        assert_eq!(app.queue.track_names[1], "A");
    }

    #[test]
    fn reorder_queue_up() {
        let mut app = make_app();
        app.add_to_queue("file:///a.mp3".into(), "A".into());
        app.add_to_queue("file:///b.mp3".into(), "B".into());
        app.add_to_queue("file:///c.mp3".into(), "C".into());
        app.reorder_queue(2, 1); // Move C up
        assert_eq!(app.queue.track_names[1], "C");
        assert_eq!(app.queue.track_names[2], "B");
    }
```

- [ ] **Step 2: Implement queue operations**

Add to the `impl App` block in `crates/rustify-tui/src/app.rs`:

```rust
    /// Add a track to the end of the queue.
    pub fn add_to_queue(&mut self, uri: String, name: String) {
        self.queue.track_uris.push(uri);
        self.queue.track_names.push(name);
    }

    /// Remove a track from the queue by index.
    pub fn remove_from_queue(&mut self, index: usize) {
        if index < self.queue.track_uris.len() {
            self.queue.track_uris.remove(index);
            self.queue.track_names.remove(index);
            // Adjust list state if needed
            if let Some(selected) = self.queue.list_state.selected() {
                if selected >= self.queue.track_uris.len() && !self.queue.track_uris.is_empty() {
                    self.queue.list_state.select(Some(self.queue.track_uris.len() - 1));
                }
            }
        }
    }

    /// Swap two tracks in the queue.
    pub fn reorder_queue(&mut self, from: usize, to: usize) {
        if from < self.queue.track_uris.len() && to < self.queue.track_uris.len() {
            self.queue.track_uris.swap(from, to);
            self.queue.track_names.swap(from, to);
        }
    }

    /// Get all queue URIs (for loading into player).
    pub fn queue_uris(&self) -> Vec<String> {
        self.queue.track_uris.clone()
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-tui -- queue`
Expected: All 4 queue tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-tui/src/app.rs
git commit -m "feat(tui): add queue management operations (add, remove, reorder)"
```

---

## Task 13: Album Art

**Files:**
- Modify: `crates/rustify-tui/Cargo.toml` (add ratatui-image)
- Modify: `crates/rustify-tui/src/ui/now_playing.rs`

- [ ] **Step 1: Add ratatui-image dependency**

Add to `[dependencies]` in `crates/rustify-tui/Cargo.toml`:

```toml
ratatui-image = "3"
image = "0.25"
```

- [ ] **Step 2: Update now-playing bar to show album art**

In `crates/rustify-tui/src/ui/now_playing.rs`, add album art rendering. Update the layout to include an art area on the left:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::app::App;
use rustify_core::types::PlaybackState;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width < 20 {
        return;
    }

    if let Some(ref track) = app.now_playing.track {
        let artist = if track.artists.is_empty() {
            "Unknown".to_string()
        } else {
            track.artists.join(", ")
        };

        let state_icon = match app.now_playing.state {
            Some(PlaybackState::Playing) => ">>",
            Some(PlaybackState::Paused) => "||",
            _ => "--",
        };

        let pos = format_time(app.now_playing.position_ms);
        let dur = format_time(track.length);

        let ratio = if track.length > 0 {
            (app.now_playing.position_ms as f64 / track.length as f64).min(1.0)
        } else {
            0.0
        };

        // Layout: [track info left] [progress center] [time+vol right]
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(35),
                Constraint::Percentage(45),
                Constraint::Percentage(20),
            ])
            .split(inner);

        // Left: track info
        let info = format!("{state_icon} {}\n   {artist} — {}", track.name, track.album);
        let info_widget = Paragraph::new(info).style(Style::default().fg(Color::White));
        frame.render_widget(info_widget, cols[0]);

        // Center: progress bar
        if cols[1].height > 0 {
            let gauge = Gauge::default()
                .ratio(ratio)
                .gauge_style(Style::default().fg(Color::Magenta).bg(Color::DarkGray))
                .label("");
            let gauge_area = Rect {
                y: cols[1].y + cols[1].height.saturating_sub(1),
                height: 1,
                ..cols[1]
            };
            frame.render_widget(gauge, gauge_area);
        }

        // Right: time + volume
        let time_vol = format!("{pos} / {dur}\nVol: {}", app.now_playing.volume);
        let right_widget = Paragraph::new(time_vol)
            .alignment(Alignment::Right)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(right_widget, cols[2]);
    } else {
        let paragraph = Paragraph::new("No track playing")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, inner);
    }
}

fn format_time(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}
```

Note: Full `ratatui-image` integration for embedded cover art requires loading images from audio tags (via lofty) at runtime. This is complex and depends on terminal capabilities. For now, the dependency is added and the crate is available. The actual image rendering widget (`StatefulImage`) can be wired in when album art extraction is implemented — that's a stretch goal for v1.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p rustify-tui`
Expected: Compiles with 0 errors.

- [ ] **Step 4: Run all tests**

Run: `cargo test -p rustify-tui`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rustify-tui/Cargo.toml crates/rustify-tui/src/ui/now_playing.rs
git commit -m "feat(tui): add ratatui-image dependency and prepare album art support"
```

---

## Task 14: Mouse Support

**Files:**
- Modify: `crates/rustify-tui/src/app.rs`

- [ ] **Step 1: Write tests for mouse handling**

Add to tests in `crates/rustify-tui/src/app.rs`:

```rust
    #[test]
    fn mouse_click_in_sidebar_area_sets_focus() {
        let mut app = make_app();
        app.focus = Focus::Main;
        // Simulate click in sidebar region (x < 30% of 80 = 24)
        app.handle_mouse_click(5, 3, 80, 24);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn mouse_click_in_main_area_sets_focus() {
        let mut app = make_app();
        app.focus = Focus::Sidebar;
        // Simulate click in main panel region (x >= 30% of 80 = 24)
        app.handle_mouse_click(30, 3, 80, 24);
        assert_eq!(app.focus, Focus::Main);
    }
```

- [ ] **Step 2: Implement mouse handling**

Add to `impl App` in `crates/rustify-tui/src/app.rs`:

```rust
    /// Handle a mouse click at terminal coordinates.
    /// `term_width` and `term_height` are needed to determine which region was clicked.
    pub fn handle_mouse_click(&mut self, x: u16, y: u16, term_width: u16, term_height: u16) {
        let sidebar_width = term_width * 30 / 100;
        let now_playing_height = 3u16;
        let content_height = term_height.saturating_sub(now_playing_height);

        if y < content_height {
            // Click in content area
            if x < sidebar_width {
                self.focus = Focus::Sidebar;
                // Check if clicking on a nav item (rows 1-4 inside border)
                if y >= 1 && y <= 4 {
                    let nav_index = (y - 1) as usize;
                    if nav_index < self.nav_items().len() {
                        self.sidebar_nav_index = nav_index;
                        self.main_view = nav_index_to_view(nav_index);
                    }
                }
            } else {
                self.focus = Focus::Main;
            }
        }
        // Clicks on now-playing bar are ignored for now
    }
```

- [ ] **Step 3: Wire mouse events in main.rs**

In the event loop in `main.rs`, add a handler for `AppEvent::Mouse`:

```rust
Ok(AppEvent::Mouse(mouse)) => {
    use crossterm::event::{MouseEventKind, MouseButton};
    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        let size = terminal.size().unwrap_or_default();
        app.handle_mouse_click(mouse.column, mouse.row, size.width, size.height);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p rustify-tui`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/rustify-tui/src/app.rs crates/rustify-tui/src/main.rs
git commit -m "feat(tui): add mouse click support for focus switching and nav selection"
```

---

## Task 15: Status Bar + Error Display

**Files:**
- Modify: `crates/rustify-tui/src/ui/mod.rs`

- [ ] **Step 1: Write test for status bar rendering**

Add to the tests in `crates/rustify-tui/src/ui/mod.rs`:

```rust
    #[test]
    fn status_message_renders_when_set() {
        let mut app = App::new();
        app.set_status("Scanned 42 tracks".into());

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                draw(frame, &mut app);
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(content.contains("Scanned 42 tracks"));
    }
```

- [ ] **Step 2: Update layout to include status line**

In `crates/rustify-tui/src/ui/mod.rs`, update the `draw` function to add a status line above the now-playing bar when a status message is active:

```rust
pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Determine if we need a status line
    let has_status = app.status.is_some();
    let status_height = if has_status { 1 } else { 0 };

    // Split vertically: [content] [status (optional)] [now-playing (3)]
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(status_height),
            Constraint::Length(3),
        ])
        .split(area);

    let content_area = vertical[0];
    let status_area = vertical[1];
    let now_playing_area = vertical[2];

    // Split content horizontally: [sidebar (30%)] [main panel (70%)]
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(content_area);

    let sidebar_area = horizontal[0];
    let main_area = horizontal[1];

    // Render each region
    sidebar::draw(frame, app, sidebar_area);
    main_panel::draw(frame, app, main_area);
    now_playing::draw(frame, app, now_playing_area);

    // Render status line if present
    if let Some(ref status) = app.status {
        let status_widget = ratatui::widgets::Paragraph::new(status.text.as_str())
            .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
        frame.render_widget(status_widget, status_area);
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p rustify-tui`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/rustify-tui/src/ui/mod.rs
git commit -m "feat(tui): add status bar for error messages and scan results"
```

---

## Task 16: Final Integration + CLI Args

**Files:**
- Modify: `crates/rustify-tui/src/main.rs`

- [ ] **Step 1: Add CLI argument parsing**

Update `main()` in `crates/rustify-tui/src/main.rs` to accept optional path arguments (override config music_dirs):

```rust
fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Load config
    let mut config = config::TuiConfig::load();

    // CLI args override config music_dirs
    let extra_dirs: Vec<PathBuf> = args[1..]
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();
    if !extra_dirs.is_empty() {
        config.music_dirs.extend(extra_dirs);
    }

    // ... rest of main unchanged
}
```

- [ ] **Step 2: Verify full build**

Run: `cargo build -p rustify-tui`
Expected: Compiles with 0 errors.

- [ ] **Step 3: Run full test suite**

Run: `cargo test -p rustify-tui`
Expected: All tests pass.

- [ ] **Step 4: Run clippy**

Run: `cargo clippy -p rustify-tui -- -D warnings`
Expected: No warnings.

- [ ] **Step 5: Run fmt check**

Run: `cargo fmt -p rustify-tui -- --check`
Expected: No formatting issues.

- [ ] **Step 6: Commit**

```bash
git add crates/rustify-tui/src/main.rs
git commit -m "feat(tui): add CLI argument support for music directory override"
```

---

## Summary

After completing all 16 tasks, `rustify-tui` provides:

- **Rich TUI layout** — sidebar (nav + queue), main panel (artists/albums/songs/playlists/search), now-playing bar
- **Full playback control** — play/pause, next/prev, seek, volume via keyboard
- **Library browser** — background scan, artist → album → track drill-down
- **Search** — case-insensitive substring search across library
- **Playlist management** — view playlists, generate M3U from queue
- **Queue management** — add, remove, reorder tracks
- **Mouse support** — optional click-to-focus, click nav items
- **Status messages** — auto-dismissing error/info display
- **Album art readiness** — ratatui-image dependency added, ready for cover art rendering
- **Config** — `~/.config/rustify/tui.toml` with platform-appropriate paths
- **CLI args** — override music dirs from command line

Run with: `cargo run -p rustify-tui -- /path/to/music`
