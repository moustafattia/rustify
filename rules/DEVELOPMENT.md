# Rustify Development Rules

Guidelines established during the initial TUI development session. Follow these in future sessions.

## Architecture Principles

### Crate Boundaries
- `rustify-core` is a pure library — no TUI, no terminal, no config files. Exposed via PyO3 and consumed by the TUI binary.
- `rustify-tui` is the binary crate. It owns all UI state, config loading, and user interaction.
- `rustify-mpris` is optional and feature-gated. No-op on non-Linux.
- New features go in the crate closest to their responsibility. Audio processing = core. Display = TUI.

### Event Architecture
- Core uses crossbeam channels: `PlayerCommand` (in) and `PlayerEvent` (out via callbacks).
- TUI has a unified `AppEvent` channel multiplexing keyboard, mouse, player callbacks, and tick timer.
- Player callbacks run on core's internal threads — they push `AppEvent::Player(...)` into the TUI's channel.
- Background work (scanning, art extraction, lyrics loading) spawns threads that send results via the same channel.

### State Management
- Single `App` struct owns all mutable UI state.
- Rendering is a pure function: `ui::draw(&app, &mut frame)` reads state, never mutates it.
- State updates happen in the event loop, before the next draw call.
- Player state (track, position, playback state) is cached in `app.now_playing` from callbacks.

## Code Patterns

### Adding a New Player Feature
1. Add the type/enum to `types.rs` (commands, events)
2. Implement the logic in the relevant core module (tracklist, player, new module)
3. Add `Player` API method that sends a `PlayerCommand`
4. Add callback registration if the feature emits events
5. Wire in TUI: `PlayerAction` variant, keybinding in `app.rs`, `main.rs` action handler

### Adding a New TUI View/Panel
1. Add state struct to `app.rs`
2. Add rendering function in the appropriate `ui/*.rs` module
3. Wire keybinding to toggle/navigate
4. Add snapshot test with `TestBackend`

### Adding Config Fields
1. Add field to `TuiConfig` in `config.rs` with `#[serde(default)]`
2. Add to the `Default` impl
3. Wire in `main.rs` at startup

## Testing

### What to Test
- Core: unit tests for every public method. State transitions, edge cases.
- TUI: snapshot tests (render to `TestBackend`, assert buffer contents). Key handling tests (given state + key, assert resulting state).
- No integration tests requiring audio hardware — those are manual.

### Test Patterns
- Use `TestBackend::new(width, height)` for snapshot tests
- Use helper functions: `make_key(KeyCode)`, `make_app()`, `make_track()`
- Test actual behavior, not mocks

## Commit Style

Format: `type(scope): description`

- `feat(core):` — new core library feature
- `feat(tui):` — new TUI feature
- `feat:` — spans both crates
- `fix(tui):` — bug fix
- `docs:` — documentation only

## File Organization

### Core modules
| Module | Responsibility |
|--------|---------------|
| `types.rs` | Shared types: Track, PlaybackState, PlayerCommand, PlayerEvent, RepeatMode |
| `player.rs` | Playback engine: command loop, decode thread, cpal output, gapless mixer |
| `tracklist.rs` | Queue with shuffle/repeat |
| `mixer.rs` | Atomic volume control |
| `metadata.rs` | Tag reading, replay gain |
| `scanner.rs` | Directory scanning |
| `playlist.rs` | M3U parsing |
| `art.rs` | Album art extraction |
| `lyrics.rs` | Lyrics extraction (tags + LRC) |
| `error.rs` | Error types |

### TUI modules
| Module | Responsibility |
|--------|---------------|
| `main.rs` | Entry point, event loop, player wiring |
| `app.rs` | All state, key/mouse handling, PlayerAction dispatch |
| `config.rs` | TOML config parsing |
| `event.rs` | AppEvent enum, input/tick threads |
| `library.rs` | In-memory index, fuzzy search |
| `theme.rs` | Color themes, presets, hex parsing |
| `scrobble.rs` | ListenBrainz scrobbling |
| `ui/mod.rs` | Top-level layout |
| `ui/sidebar.rs` | Nav + queue |
| `ui/main_panel.rs` | Content views (artists/albums/songs/playlists/search) |
| `ui/now_playing.rs` | Track info, progress, visualizer integration |
| `ui/visualizer.rs` | FFT spectrum + waveform rendering |

## Design Specs and Plans

Design specs go in `docs/superpowers/specs/`. Implementation plans in `docs/superpowers/plans/`.

Each tier of features follows the cycle: spec -> plan -> implement -> test -> commit.

Existing specs:
- `2026-04-08-rustify-tui-design.md` — original TUI design
- `2026-04-08-tier1-playback-essentials-design.md` — shuffle, repeat, seek, gapless, album art
- `2026-04-09-tier2-rich-experience-design.md` — visualizer, themes, fuzzy search
- `2026-04-09-tier3-power-user-design.md` — crossfade, MPRIS, scrobbling, replay gain, lyrics

## Pi Constraints

- Target: Raspberry Pi Zero 2W (aarch64, 512MB RAM)
- ALSA-only audio output
- Memory efficiency matters — avoid unbounded buffers
- SSH use case: no mouse expected, braille art fallback, keyboard-only
- Binary size target: <10MB (vs 70-80MB for Mopidy+GStreamer)
