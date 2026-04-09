use std::io;
use std::path::PathBuf;
use std::thread;

use crossterm::event::{MouseButton, MouseEventKind};
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
mod theme;
mod ui;

use app::App;
use event::{AppEvent, EventLoop};
use library::Library;

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

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
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

    let tx_mode = tx.clone();
    player.on_mode_change(Box::new(move |shuffle, repeat| {
        tx_mode
            .send(AppEvent::Player(PlayerEvent::ModeChanged {
                shuffle,
                repeat,
            }))
            .ok();
    }));

    let mut app = App::new();
    app.theme = theme::Theme::from_config(&config);
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
                if let Some(action) = app.handle_key(key) {
                    match action {
                        app::PlayerAction::PlayPause => {
                            match app.now_playing.state {
                                Some(rustify_core::types::PlaybackState::Playing) => {
                                    player.pause()
                                }
                                _ => player.play(),
                            }
                        }
                        app::PlayerAction::Next => player.next(),
                        app::PlayerAction::Previous => player.previous(),
                        app::PlayerAction::Seek(delta) => {
                            let track_len = app
                                .now_playing
                                .track
                                .as_ref()
                                .map(|t| t.length as i64)
                                .unwrap_or(0);
                            let new_pos = (app.now_playing.position_ms as i64 + delta)
                                .clamp(0, track_len) as u64;
                            player.seek(new_pos);
                            app.now_playing.position_ms = new_pos;
                        }
                        app::PlayerAction::PlayTrackUri(uri) => {
                            player.load_track_uris(vec![uri]);
                            player.play();
                        }
                        app::PlayerAction::LoadTrackUris(uris) => {
                            player.load_track_uris(uris);
                            player.play();
                        }
                        app::PlayerAction::ToggleShuffle => {
                            let new_state = !app.now_playing.shuffle;
                            player.set_shuffle(new_state);
                        }
                        app::PlayerAction::CycleRepeat => {
                            let new_mode = app.now_playing.repeat.cycle();
                            player.set_repeat(new_mode);
                        }
                    }
                }
                player.set_volume(app.now_playing.volume);
            }
            Ok(AppEvent::Mouse(mouse)) => {
                if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                    let size = terminal.size().unwrap_or_default();
                    app.handle_mouse_click(mouse.column, mouse.row, size.width, size.height);
                }
            }
            Ok(AppEvent::Player(event)) => {
                // Trigger background art extraction on track change
                if let PlayerEvent::TrackChanged(ref track) = event {
                    let uri = track.uri.clone();
                    let art_tx = tx.clone();
                    thread::Builder::new()
                        .name("rustify-art".into())
                        .spawn(move || {
                            let path = rustify_core::types::uri_to_path(&uri);
                            let data = rustify_core::art::extract_art(&path);
                            art_tx
                                .send(AppEvent::ArtLoaded {
                                    uri,
                                    data,
                                })
                                .ok();
                        })
                        .ok();
                }
                app.handle_player_event(event);
            }
            Ok(AppEvent::ArtLoaded { uri, data }) => {
                if app.art.current_uri.as_deref() == Some(&uri) {
                    app.art.has_art = data.is_some();
                    app.art.image_bytes = data;
                }
            }
            Ok(AppEvent::Tick) => {
                app.handle_tick();
                // Feed audio samples to visualizer
                let samples = player.get_samples();
                if !samples.is_empty() {
                    app.visualizer_samples = samples.clone();
                    let new_bars = ui::visualizer::compute_spectrum_bars(&samples);
                    app.visualizer_state.apply_smoothing(&new_bars);
                }
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
