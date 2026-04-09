use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};

use crate::app::{App, Focus};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let border_style = if app.focus == Focus::Sidebar {
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.border)
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
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme.fg)
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
            .style(Style::default().fg(app.theme.accent))
            .alignment(Alignment::Center);
        frame.render_widget(header, header_area);

        // Queue items
        let list_area = Rect {
            y: queue_area.y + 1,
            height: queue_area.height.saturating_sub(1),
            ..queue_area
        };

        if app.queue.track_names.is_empty() {
            let empty = Paragraph::new("  (empty)").style(Style::default().fg(app.theme.border));
            frame.render_widget(empty, list_area);
        } else {
            let items: Vec<ListItem> = app
                .queue
                .track_names
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let style = Style::default().fg(app.theme.fg_dim);
                    ListItem::new(format!("  {}. {}", i + 1, name)).style(style)
                })
                .collect();

            let queue_list = List::new(items)
                .highlight_style(Style::default().fg(app.theme.fg).bg(app.theme.border));
            frame.render_stateful_widget(queue_list, list_area, &mut app.queue.list_state);
        }
    }
}

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
