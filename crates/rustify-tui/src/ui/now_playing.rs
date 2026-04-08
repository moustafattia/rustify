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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use rustify_core::types::Track;

    fn make_track() -> Track {
        Track {
            uri: "file:///music/song.mp3".into(),
            name: "Midnight City".into(),
            artists: vec!["M83".into()],
            album: "Hurry Up, We're Dreaming".into(),
            length: 243_000,
            track_no: Some(1),
        }
    }

    #[test]
    fn renders_track_info_when_playing() {
        let mut app = App::new();
        app.now_playing.track = Some(make_track());
        app.now_playing.state = Some(PlaybackState::Playing);
        app.now_playing.position_ms = 102_000;
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
