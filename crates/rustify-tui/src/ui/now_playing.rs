use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::ui::visualizer::{self, VisualizerMode};
use rustify_core::types::PlaybackState;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 || inner.width < 20 {
        return;
    }

    // When inner height >= 4, split into visualizer (top) and track info (bottom 3 rows).
    let (viz_area, info_area) = if inner.height >= 4 && app.now_playing.track.is_some() {
        let viz_height = inner.height.saturating_sub(3);
        let viz = Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: viz_height,
        };
        let info = Rect {
            x: inner.x,
            y: inner.y + viz_height,
            width: inner.width,
            height: 3.min(inner.height),
        };
        (Some(viz), info)
    } else {
        (None, inner)
    };

    // Draw the visualizer if we have space
    if let Some(viz) = viz_area {
        match app.visualizer_mode {
            VisualizerMode::Spectrum => {
                visualizer::draw_spectrum(frame, viz, &app.visualizer_state, &app.theme);
            }
            VisualizerMode::Waveform => {
                visualizer::draw_waveform(frame, viz, &app.visualizer_samples, &app.theme);
            }
        }
    }

    let inner = info_area;

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

        // Layout: [art (3 cols)] [track info (45%)] [time+vol+modes (right)]
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(3),
                Constraint::Percentage(55),
                Constraint::Percentage(30),
            ])
            .split(inner);

        // Art area (compact)
        let art_style = if app.art.has_art {
            Style::default().fg(app.theme.accent)
        } else {
            Style::default().fg(app.theme.border)
        };
        let art_placeholder = Paragraph::new("♪")
            .alignment(Alignment::Center)
            .style(art_style);
        frame.render_widget(art_placeholder, cols[0]);

        // Track info + progress bar (two lines + progress on third)
        let info_area = cols[1];
        if info_area.height >= 2 {
            // Line 1: state icon + track name
            let line1 = format!("{state_icon} {}", track.name);
            let line1_widget = Paragraph::new(line1).style(Style::default().fg(app.theme.fg));
            let line1_area = Rect {
                height: 1,
                ..info_area
            };
            frame.render_widget(line1_widget, line1_area);

            // Line 2: artist — album (with ellipsis if needed)
            let detail = format!("   {artist} — {}", track.album);
            let max_w = info_area.width as usize;
            let detail_display = if detail.len() > max_w && max_w > 3 {
                format!("{}...", &detail[..max_w - 3])
            } else {
                detail
            };
            let line2_widget =
                Paragraph::new(detail_display).style(Style::default().fg(app.theme.fg_dim));
            let line2_area = Rect {
                y: info_area.y + 1,
                height: 1,
                ..info_area
            };
            frame.render_widget(line2_widget, line2_area);
        }

        // Line 3: thin progress bar using unicode
        if info_area.height >= 3 {
            let bar_width = info_area.width as usize;
            let filled = (ratio * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(filled);
            let progress_line = Line::from(vec![
                Span::styled("━".repeat(filled), Style::default().fg(app.theme.accent)),
                Span::styled("━".repeat(empty), Style::default().fg(app.theme.border)),
            ]);
            let progress_area = Rect {
                y: info_area.y + 2,
                height: 1,
                ..info_area
            };
            frame.render_widget(Paragraph::new(progress_line), progress_area);
        }

        // Time + volume + mode indicators
        let shuffle_indicator = if app.now_playing.shuffle { "[S] " } else { "" };
        let repeat_indicator = match app.now_playing.repeat {
            rustify_core::types::RepeatMode::Off => "",
            rustify_core::types::RepeatMode::All => "[R] ",
            rustify_core::types::RepeatMode::One => "[R1] ",
        };
        let time_vol = format!(
            "{pos} / {dur}\n{shuffle_indicator}{repeat_indicator}Vol: {}",
            app.now_playing.volume
        );
        let right_widget = Paragraph::new(time_vol)
            .alignment(Alignment::Right)
            .style(Style::default().fg(app.theme.fg_dim));
        frame.render_widget(right_widget, cols[2]);
    } else {
        let paragraph = Paragraph::new("No track playing")
            .style(Style::default().fg(app.theme.border))
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
