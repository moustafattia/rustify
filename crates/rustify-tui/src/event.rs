use std::thread;
use std::time::Duration;

use crossbeam::channel::{self, Receiver, Sender};
use crossterm::event::{self, Event, KeyEvent, MouseEvent};
use rustify_core::lyrics::Lyrics;
use rustify_core::types::PlayerEvent;

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
    /// Album art loaded for a track URI
    ArtLoaded { uri: String, data: Option<Vec<u8>> },
    /// Lyrics loaded for a track URI
    LyricsLoaded { uri: String, data: Option<Lyrics> },
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
            .spawn(move || loop {
                thread::sleep(Duration::from_millis(250));
                if tick_tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            })
            .expect("failed to spawn tick thread");

        // Input thread — polls crossterm events and forwards them
        let input_tx = tx.clone();
        thread::Builder::new()
            .name("rustify-input".into())
            .spawn(move || loop {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustify_core::types::PlaybackState;

    #[test]
    fn app_event_variants_exist() {
        let _ = AppEvent::Tick;
        let _ = AppEvent::Error("test".into());
    }

    #[test]
    fn event_loop_sends_ticks() {
        let event_loop = EventLoop::new();
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
