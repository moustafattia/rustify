use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::widgets::ListState;
use rustify_core::types::{PlaybackState, PlayerEvent, Track};

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

/// Player command to execute after key handling.
#[derive(Debug)]
pub enum PlayerAction {
    PlayPause,
    Next,
    Previous,
    Seek(i64),
    PlayTrackUri(String),
    LoadTrackUris(Vec<String>),
    ToggleShuffle,
    CycleRepeat,
}

const NAV_ITEMS: &[&str] = &["Artists", "Albums", "Songs", "Playlists"];

/// Now-playing state cached from player callbacks.
#[derive(Debug)]
pub struct NowPlayingState {
    pub track: Option<Track>,
    pub state: Option<PlaybackState>,
    pub position_ms: u64,
    pub volume: u8,
    pub shuffle: bool,
    pub repeat: rustify_core::types::RepeatMode,
}

impl Default for NowPlayingState {
    fn default() -> Self {
        Self {
            track: None,
            state: None,
            position_ms: 0,
            volume: 0,
            shuffle: false,
            repeat: rustify_core::types::RepeatMode::Off,
        }
    }
}

/// Cached album art state.
#[derive(Debug, Default)]
pub struct ArtState {
    /// URI of the track whose art is currently cached.
    pub current_uri: Option<String>,
    /// Whether we have art for the current track.
    pub has_art: bool,
    /// Raw image bytes (for rendering by the UI layer).
    pub image_bytes: Option<Vec<u8>>,
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
    pub playlists: Vec<rustify_core::types::Playlist>,
    pub art: ArtState,

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
            playlists: Vec::new(),
            art: ArtState::default(),

            artist_list_state: ListState::default(),
            album_list_state: ListState::default(),
            song_list_state: ListState::default(),
            playlist_list_state: ListState::default(),
            detail_list_state: ListState::default(),

            selected_artist: None,
            selected_album_index: None,
        }
    }

    /// Handle a key event. Returns a PlayerAction if a player command should be issued.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PlayerAction> {
        if key.kind != KeyEventKind::Press {
            return None;
        }

        // Global keys (work regardless of focus)
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
                return None;
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::Sidebar => Focus::Main,
                    Focus::Main => Focus::Sidebar,
                    Focus::Search => Focus::Main,
                };
                return None;
            }
            KeyCode::Char(c @ '1'..='4') => {
                let idx = (c as usize) - ('1' as usize);
                self.sidebar_nav_index = idx;
                self.main_view = nav_index_to_view(idx);
                return None;
            }
            KeyCode::Char(' ') => {
                return Some(PlayerAction::PlayPause);
            }
            KeyCode::Char('n') if self.focus != Focus::Search => {
                return Some(PlayerAction::Next);
            }
            KeyCode::Char('p') if self.focus != Focus::Search => {
                return Some(PlayerAction::Previous);
            }
            KeyCode::Char('s') if self.focus != Focus::Search => {
                return Some(PlayerAction::ToggleShuffle);
            }
            KeyCode::Char('r') if self.focus != Focus::Search => {
                return Some(PlayerAction::CycleRepeat);
            }
            KeyCode::Left => {
                return Some(PlayerAction::Seek(-5000));
            }
            KeyCode::Right => {
                return Some(PlayerAction::Seek(5000));
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.now_playing.volume = (self.now_playing.volume + 5).min(100);
                return None;
            }
            KeyCode::Char('-') => {
                self.now_playing.volume = self.now_playing.volume.saturating_sub(5);
                return None;
            }
            KeyCode::Char('/') if self.focus != Focus::Search => {
                self.search.active = true;
                self.search.query.clear();
                self.focus = Focus::Search;
                return None;
            }
            KeyCode::Esc => {
                if self.search.active {
                    self.search.active = false;
                    self.focus = Focus::Main;
                    return None;
                }
                if self.main_view == MainView::AlbumDetail {
                    self.main_view = if self.selected_artist.is_some() {
                        MainView::Albums
                    } else {
                        MainView::Artists
                    };
                    return None;
                }
            }
            _ => {}
        }

        // Search mode key handling
        if self.focus == Focus::Search {
            self.handle_search_key(key);
            return None;
        }

        // Focus-specific keys
        match self.focus {
            Focus::Sidebar => {
                self.handle_sidebar_key(key);
            }
            Focus::Main => {
                return self.handle_main_key(key);
            }
            Focus::Search => {}
        }
        None
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.sidebar_nav_index < NAV_ITEMS.len() - 1 {
                    self.sidebar_nav_index += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.sidebar_nav_index = self.sidebar_nav_index.saturating_sub(1);
            }
            KeyCode::Enter => {
                self.main_view = nav_index_to_view(self.sidebar_nav_index);
                self.focus = Focus::Main;
            }
            _ => {}
        }
    }

    fn handle_main_key(&mut self, key: KeyEvent) -> Option<PlayerAction> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_main_selection(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_main_selection(-1);
            }
            KeyCode::Enter => {
                return self.activate_main_selection();
            }
            KeyCode::Char('a') => {
                // Add selected track to queue
                if let Some(track) = self.get_selected_track() {
                    self.add_to_queue(track.uri, track.name);
                }
            }
            _ => {}
        }
        None
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                self.search.query.push(c);
            }
            KeyCode::Backspace => {
                self.search.query.pop();
            }
            KeyCode::Enter => {
                self.search.active = false;
                self.focus = Focus::Main;
            }
            _ => {}
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

    fn activate_main_selection(&mut self) -> Option<PlayerAction> {
        match self.main_view {
            MainView::Artists => {
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
                None
            }
            MainView::Albums => {
                if let Some(selected) = self.album_list_state.selected() {
                    self.selected_album_index = Some(selected);
                    self.main_view = MainView::AlbumDetail;
                    self.detail_list_state.select(Some(0));
                }
                None
            }
            MainView::Songs => {
                if let Some(track) = self.get_selected_track() {
                    let uri = track.uri.clone();
                    return Some(PlayerAction::PlayTrackUri(uri));
                }
                None
            }
            MainView::AlbumDetail => {
                // Play album from selected track
                if let Some(uris) = self.get_album_detail_uris() {
                    return Some(PlayerAction::LoadTrackUris(uris));
                }
                None
            }
            MainView::Playlists => None,
        }
    }

    /// Get the currently selected track in the active view (cloned).
    fn get_selected_track(&self) -> Option<Track> {
        let lib = self.library.as_ref()?;
        match self.main_view {
            MainView::Songs => {
                let idx = self.song_list_state.selected()?;
                lib.all_tracks().get(idx).cloned()
            }
            MainView::AlbumDetail => {
                let albums = if let Some(ref artist) = self.selected_artist {
                    lib.albums_by_artist(artist).to_vec()
                } else {
                    lib.all_albums().into_iter().cloned().collect()
                };
                let album = albums.get(self.selected_album_index?)?;
                let idx = self.detail_list_state.selected()?;
                album.tracks.get(idx).cloned()
            }
            _ => None,
        }
    }

    /// Get all track URIs from the currently viewed album detail.
    fn get_album_detail_uris(&self) -> Option<Vec<String>> {
        let lib = self.library.as_ref()?;
        let albums = if let Some(ref artist) = self.selected_artist {
            lib.albums_by_artist(artist).to_vec()
        } else {
            lib.all_albums().into_iter().cloned().collect()
        };
        let album = albums.get(self.selected_album_index?)?;
        Some(album.tracks.iter().map(|t| t.uri.clone()).collect())
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

    /// Handle a tick event.
    pub fn handle_tick(&mut self) {
        self.tick_count += 1;
        if let Some(ref status) = self.status {
            if self.tick_count >= status.expires_tick {
                self.status = None;
            }
        }
    }

    /// Handle a player event (callback from rustify-core).
    pub fn handle_player_event(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::StateChanged(state) => {
                self.now_playing.state = Some(state);
            }
            PlayerEvent::TrackChanged(track) => {
                // Clear art cache when track changes
                if self.art.current_uri.as_deref() != Some(&track.uri) {
                    self.art.current_uri = Some(track.uri.clone());
                    self.art.has_art = false;
                    self.art.image_bytes = None;
                }
                self.now_playing.track = Some(track);
                self.now_playing.position_ms = 0;
            }
            PlayerEvent::PositionUpdate(ms) => {
                self.now_playing.position_ms = ms;
            }
            PlayerEvent::Error(msg) => {
                self.set_status(msg);
            }
            PlayerEvent::ModeChanged { shuffle, repeat } => {
                self.now_playing.shuffle = shuffle;
                self.now_playing.repeat = repeat;
            }
        }
    }

    /// Handle a mouse click at terminal coordinates.
    pub fn handle_mouse_click(&mut self, x: u16, y: u16, term_width: u16, term_height: u16) {
        let sidebar_width = term_width * 30 / 100;
        let now_playing_height = 3u16;
        let content_height = term_height.saturating_sub(now_playing_height);

        if y < content_height {
            if x < sidebar_width {
                self.focus = Focus::Sidebar;
                if y >= 1 && y <= 4 {
                    let nav_index = (y - 1) as usize;
                    if nav_index < NAV_ITEMS.len() {
                        self.sidebar_nav_index = nav_index;
                        self.main_view = nav_index_to_view(nav_index);
                    }
                }
            } else {
                self.focus = Focus::Main;
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

    // --- Queue operations ---

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
            if let Some(selected) = self.queue.list_state.selected() {
                if selected >= self.queue.track_uris.len() && !self.queue.track_uris.is_empty() {
                    self.queue
                        .list_state
                        .select(Some(self.queue.track_uris.len() - 1));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventState, KeyModifiers};

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
        let _ = app.handle_key(make_key(KeyCode::Char('q')));
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
        app.handle_key(make_key(KeyCode::Char('k')));
        assert_eq!(app.sidebar_nav_index, 0);
    }

    #[test]
    fn enter_on_sidebar_nav_switches_view_and_focus() {
        let mut app = make_app();
        app.sidebar_nav_index = 2;
        app.handle_key(make_key(KeyCode::Enter));
        assert_eq!(app.main_view, MainView::Songs);
        assert_eq!(app.focus, Focus::Main);
    }

    #[test]
    fn space_returns_play_pause_action() {
        let mut app = make_app();
        let action = app.handle_key(make_key(KeyCode::Char(' ')));
        assert!(matches!(action, Some(PlayerAction::PlayPause)));
    }

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
        app.handle_player_event(PlayerEvent::TrackChanged(track));
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

    #[test]
    fn mouse_click_in_sidebar_area_sets_focus() {
        let mut app = make_app();
        app.focus = Focus::Main;
        app.handle_mouse_click(5, 3, 80, 24);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn mouse_click_in_main_area_sets_focus() {
        let mut app = make_app();
        app.focus = Focus::Sidebar;
        app.handle_mouse_click(30, 3, 80, 24);
        assert_eq!(app.focus, Focus::Main);
    }

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
        app.reorder_queue(0, 1);
        assert_eq!(app.queue.track_names[0], "B");
        assert_eq!(app.queue.track_names[1], "A");
    }

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
}
