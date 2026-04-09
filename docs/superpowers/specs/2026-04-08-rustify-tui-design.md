# Rustify TUI — Design Spec

**Date:** 2026-04-08
**Status:** Draft

## Overview

A rich terminal music player built on `rustify-core`. Serves two use cases: SSH remote control into a YoyoPod Pi, and a standalone desktop terminal player. Ships as a new binary crate (`crates/rustify-tui`) in the existing workspace.

## Architecture

### Crate Structure

New workspace member: `crates/rustify-tui` (binary crate).

Dependencies:
- `rustify-core` — playback, decoding, scanning, metadata, playlists
- `ratatui` — immediate-mode terminal UI rendering
- `crossterm` — terminal input/output backend
- `crossbeam` — channels for unified event loop (already in workspace)
- `ratatui-image` — album art rendering (sixel/kitty protocol with braille/half-block fallback)
- `serde` + `toml` — config file parsing

### Event Loop

A single `crossbeam::select!` loop on the main thread multiplexes all input sources:

```
                     ┌─────────────┐
  Keyboard/Mouse ──► │             │
                     │  AppEvent   │     ┌───────────┐
  Player callbacks ─►│  Channel    │────►│ Main      │──► ratatui render
                     │  (crossbeam)│     │ Thread    │
  Tick timer ───────►│             │     └───────────┘
                     └─────────────┘
```

**AppEvent enum:**
- `AppEvent::Key(KeyEvent)` — keyboard input from crossterm
- `AppEvent::Mouse(MouseEvent)` — mouse clicks/scroll from crossterm
- `AppEvent::Player(PlayerEvent)` — state/track/position changes from rustify-core callbacks
- `AppEvent::Tick` — ~4Hz timer for progress bar smoothing and spinner animation
- `AppEvent::ScanComplete(Library)` — background library scan finished
- `AppEvent::Error(String)` — non-player errors (scan failures, config issues)

### Thread Model

| Thread | Purpose | Lifetime |
|--------|---------|----------|
| Main | Event loop + ratatui rendering | App lifetime |
| Input | Polls crossterm events, sends `AppEvent::Key`/`Mouse` | App lifetime |
| Tick | Sends `AppEvent::Tick` at ~4Hz | App lifetime |
| Scanner | Runs `rustify-core::scanner`, builds library index | On-demand, startup |
| rustify-core command | Processes `PlayerCommand`s (unchanged) | Player lifetime |
| rustify-core decode | Decodes audio packets (unchanged) | Per-track |

The input thread calls `crossterm::event::poll()` with a timeout, then `crossterm::event::read()`, and forwards events into the shared `AppEvent` channel. The tick thread sleeps 250ms and sends `AppEvent::Tick`.

Player callbacks (registered via `player.on_state_change()`, etc.) push `AppEvent::Player(...)` into the same channel from rustify-core's internal threads.

## UI Layout

Sidebar + Main layout. Three persistent regions:

```
┌──────────────┬────────────────────────────────┐
│              │                                │
│   SIDEBAR    │         MAIN PANEL             │
│              │                                │
│  Library Nav │  (content changes by nav       │
│  ──────────  │   selection: Artists, Albums,  │
│  Queue       │   Songs, Playlists, Detail)    │
│              │                                │
│              │                                │
│              │                                │
├──────────────┴────────────────────────────────┤
│  NOW PLAYING BAR                              │
│  [art] Title — Artist    ◂◂ ▶ ▸▸   1:42/4:03 │
│        Album        ━━━━━━━━━━━░░░░░  Vol: 80 │
└───────────────────────────────────────────────┘
```

### Sidebar (always visible, left ~30% width)

**Library Nav** — four selectable entries:
- Artists
- Albums
- Songs
- Playlists

Selecting one swaps the main panel content. Active entry is highlighted.

**Queue** — below the nav, separated by a divider. Shows the current tracklist with the active track highlighted. Scrollable. Supports reorder (`Shift+j/k`) and remove (`d`).

### Main Panel (right ~70% width)

Content determined by sidebar nav selection:

| View | Content | Interactions |
|------|---------|-------------|
| Artists | Alphabetical artist list | `Enter` → show albums by artist |
| Albums | Album list (all, or filtered by selected artist) | `Enter` → show album tracks |
| Songs | Flat list of all tracks, sortable by name/artist/album | `Enter` → play, `a` → add to queue |
| Playlists | Saved M3U playlists | `Enter` → load, `n` → new, `d` → delete |
| Album Detail | Track listing for a single album with album header | `Enter` → play all from track, `a` → enqueue |
| Search | Fuzzy search overlay triggered by `/` | Filters across artists/albums/tracks |

### Now-Playing Bar (always visible, bottom 3-4 rows)

- Album art thumbnail (braille/half-block rendering; sixel/kitty when terminal supports it via `ratatui-image`)
- Track title, artist name, album name
- Progress bar with elapsed / total time
- Transport state indicator (play/pause icon)
- Volume level indicator

## Features

### Playback Controls

All transport commands delegate to `rustify-core::Player`:
- Play / Pause (toggle)
- Stop
- Next / Previous track
- Seek (left/right arrow in now-playing focus)
- Volume up / down

### Library Browser

On startup, `rustify-core::scanner::scan_directory()` scans configured music directories. The TUI reads metadata via `rustify-core::metadata::read_metadata_from_path()` for each discovered file and builds an in-memory index:

```
Library
├── artists: BTreeMap<String, Artist>
│   └── Artist { name, albums: Vec<AlbumRef> }
├── albums: Vec<Album>
│   └── Album { name, artist, tracks: Vec<TrackRef>, art_path: Option<PathBuf> }
└── tracks: Vec<Track>  // rustify-core::types::Track
```

The scan runs on a background thread. The UI shows a loading spinner until `AppEvent::ScanComplete` arrives. Rescanning is triggered manually via a keybinding (`R`).

### Playlist Management

- **Load** — select an M3U file from the Playlists view, loads into queue via `player.load_track_uris()`
- **Create** — `Ctrl+S` saves the current queue as a new M3U file; prompts for name via inline text input
- **Delete** — `d` on a playlist in the list view, with confirmation
- M3U files are discovered by scanning music directories for `.m3u` / `.m3u8` extensions (already supported by `rustify-core::playlist`)

### Search

`/` opens a search overlay — a text input at the top of the main panel with live-filtered results below. Searches across track names, artist names, and album names. Case-insensitive substring matching for v1 (full fuzzy/fzf-style can be added later). `Esc` closes the overlay. `Enter` on a result navigates to it.

### Album Art

Uses `ratatui-image` crate:
- **Sixel / Kitty protocol** — high-fidelity rendering when the terminal supports it (iTerm2, Kitty, WezTerm, foot)
- **Braille / half-block fallback** — works in any terminal including over SSH
- Art source: embedded cover art from audio file tags (via lofty), or `cover.jpg` / `folder.jpg` in the album directory

## Input

### Keyboard (primary)

| Key | Action |
|-----|--------|
| `j` / `↓` | Navigate down in lists |
| `k` / `↑` | Navigate up in lists |
| `Enter` | Select / play |
| `Space` | Toggle play / pause |
| `n` | Next track |
| `p` | Previous track |
| `+` / `=` | Volume up |
| `-` | Volume down |
| `Tab` | Cycle focus: sidebar → main panel → sidebar |
| `/` | Open search overlay |
| `Esc` | Close overlay / go back |
| `a` | Add selected track to queue |
| `d` | Remove from queue / delete playlist (with confirm) |
| `Shift+j/k` | Reorder queue items |
| `Ctrl+S` | Save queue as M3U playlist |
| `R` | Rescan library |
| `1`-`4` | Jump to Artists / Albums / Songs / Playlists |
| `q` | Quit |

### Mouse (optional, desktop convenience)

- Click on sidebar nav items to switch views
- Click on list items to select
- Click on now-playing transport icons
- Scroll wheel on lists

Mouse is purely additive — every interaction has a keyboard equivalent.

## Configuration

Config file: `~/.config/rustify/tui.toml`

```toml
# Music directories to scan
music_dirs = ["/home/pi/Music", "/mnt/usb/music"]

# Audio device (passed to rustify-core)
alsa_device = "default"

# Theme preset: "default", "light", "nord", "dracula"
theme = "default"

# Custom keybinding overrides (optional)
[keys]
quit = "q"
play_pause = " "
next = "n"
previous = "p"
```

On Pi: `/home/pi/.config/rustify/tui.toml`. On Linux/macOS desktop: `~/.config/rustify/tui.toml` (XDG). On Windows: `%APPDATA%\rustify\tui.toml`. The app uses `dirs` crate to resolve the platform-appropriate config directory.

## App State

A single `App` struct owns all mutable UI state:

```rust
struct App {
    player: Player,              // rustify-core player handle
    library: Option<Library>,    // None until scan completes
    focus: Focus,                // Sidebar | Main | Search
    sidebar: SidebarState,       // selected nav item, queue scroll/selection
    main_view: MainView,         // active view enum + per-view list state
    now_playing: NowPlayingState, // cached track, position, playback state
    search: SearchState,         // query string, filtered results
    config: TuiConfig,           // parsed config file
    should_quit: bool,           // exit flag
}
```

The event loop calls `app.handle_event(event)` which mutates state, then `ui::draw(&app, &mut frame)` which reads state to render.

## Error Handling

Errors surface in the UI — the app never panics on recoverable errors.

- **Player errors** (decode failure, missing audio device) — displayed in a dismissable status line above the now-playing bar. Auto-dismiss after 5 seconds. Decode failures cause the player to skip to the next track (already handled by rustify-core's `DecodeFailed` → `TrackEnded` flow).
- **Scan errors** (permission denied, unreadable files) — skipped individually. After scan completes, a summary is shown: "Scanned 342 tracks (3 errors)". Detailed errors logged to stderr.
- **Config errors** (missing file, parse error) — fall back to defaults, show a one-time warning on startup.
- **Terminal errors** (resize) — crossterm emits resize events; the UI re-renders at the new size. All layout uses ratatui's constraint-based system, so it adapts automatically.
- **Lost SSH connection** — process receives SIGHUP, cleanup runs (restore terminal state via crossterm).

## Testing

### Unit Tests

`App` state transitions tested without a terminal:
- Key handling: given a state + key event, assert the resulting state change
- View switching: sidebar selection changes main panel view
- Queue manipulation: add, remove, reorder

### Snapshot Tests

Render to `ratatui::Terminal<TestBackend>`, assert buffer contents:
- Layout renders correctly at various terminal sizes (80x24, 120x40, minimal 60x20)
- Now-playing bar shows correct track info
- List scrolling and selection highlighting

### Integration

Playback correctness is covered by existing `rustify-core` tests. TUI integration (play a file, verify it renders, interact via keys) is manual testing.

## Dependencies Summary

| Crate | Version | Purpose |
|-------|---------|---------|
| `rustify-core` | workspace | Playback, scanning, metadata, playlists |
| `ratatui` | latest | Terminal UI framework |
| `crossterm` | latest | Terminal backend (input, raw mode, alternate screen) |
| `crossbeam` | 0.8 | Event channels (already in workspace) |
| `ratatui-image` | latest | Album art rendering (sixel/kitty/braille) |
| `serde` | 1 | Config deserialization (already in workspace) |
| `toml` | latest | Config file parsing |
| `dirs` | latest | Platform-appropriate config/data directories |

## Out of Scope (for v1)

- Network streaming (Spotify, HTTP streams)
- MPRIS D-Bus integration
- Lyrics display
- Equalizer / audio effects
- Multi-user / server mode
- Themes beyond built-in presets
