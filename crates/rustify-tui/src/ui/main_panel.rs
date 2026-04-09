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
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.border)
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
            .style(Style::default().fg(app.theme.error))
            .alignment(Alignment::Center);
        frame.render_widget(loading, inner);
        return;
    }

    let Some(ref library) = app.library else {
        let msg =
            Paragraph::new("No music directories configured.\nEdit ~/.config/rustify/tui.toml")
                .style(Style::default().fg(app.theme.border))
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
                .highlight_style(
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                )
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
                .map(|album| ListItem::new(format!("{} — {}", album.name, album.artist)))
                .collect();
            let list = List::new(items)
                .highlight_style(
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                )
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
                .highlight_style(
                    Style::default()
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            frame.render_stateful_widget(list, inner, &mut app.song_list_state);
        }
        MainView::Playlists => {
            if app.playlists.is_empty() {
                let msg = Paragraph::new("No playlists found.")
                    .style(Style::default().fg(app.theme.border));
                frame.render_widget(msg, inner);
            } else {
                let items: Vec<ListItem> = app
                    .playlists
                    .iter()
                    .map(|p| ListItem::new(format!("{} ({} tracks)", p.name, p.track_count)))
                    .collect();
                let list = List::new(items)
                    .highlight_style(
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");
                frame.render_stateful_widget(list, inner, &mut app.playlist_list_state);
            }
        }
        MainView::AlbumDetail => {
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
                        Style::default()
                            .fg(app.theme.accent)
                            .add_modifier(Modifier::BOLD),
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
        .style(Style::default().fg(app.theme.error));
    frame.render_widget(input, chunks[0]);

    // Search results
    if let Some(ref library) = app.library {
        let results = library.fuzzy_search(&app.search.query);
        let items: Vec<ListItem> = results
            .iter()
            .take(chunks[1].height as usize)
            .map(|r| {
                let artist = r
                    .track
                    .artists
                    .first()
                    .map(|a| a.as_str())
                    .unwrap_or("Unknown");
                ListItem::new(format!("{} — {}", r.track.name, artist))
            })
            .collect();
        let list = List::new(items)
            .highlight_style(Style::default().fg(app.theme.accent))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, chunks[1], &mut app.search.results_state);
    }
}

fn format_duration(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

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
