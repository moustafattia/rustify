pub mod main_panel;
pub mod now_playing;
pub mod sidebar;

use ratatui::prelude::*;

use crate::app::App;

/// Draw the full TUI layout.
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

        let buf = terminal.backend().buffer();
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
}
