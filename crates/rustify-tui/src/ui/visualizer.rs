use std::f32::consts::PI;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

use crate::theme::Theme;

/// Number of display bars in the spectrum visualizer.
pub const BAR_COUNT: usize = 40;

/// FFT size used for spectrum analysis.
const FFT_SIZE: usize = 1024;

/// Unicode block characters ordered by height (1/8 to 8/8).
const BLOCKS: [char; 8] = [
    '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
    '\u{2588}',
];

/// Visualization display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualizerMode {
    Spectrum,
    Waveform,
}

impl VisualizerMode {
    /// Toggle between modes.
    pub fn toggle(self) -> Self {
        match self {
            VisualizerMode::Spectrum => VisualizerMode::Waveform,
            VisualizerMode::Waveform => VisualizerMode::Spectrum,
        }
    }
}

/// Persistent state for the visualizer (smoothing).
#[derive(Debug, Clone)]
pub struct VisualizerState {
    pub bars: Vec<f32>,
}

impl Default for VisualizerState {
    fn default() -> Self {
        Self {
            bars: vec![0.0; BAR_COUNT],
        }
    }
}

impl VisualizerState {
    /// Apply exponential decay smoothing: keep the max of new value and old * 0.85.
    pub fn apply_smoothing(&mut self, new_bars: &[f32]) {
        for (old, &new) in self.bars.iter_mut().zip(new_bars.iter()) {
            *old = new.max(*old * 0.85);
        }
    }
}

/// Downmix interleaved stereo samples to mono, apply a Hann window, run a
/// 1024-point FFT, and group the output into 24 log-frequency bars normalized
/// to 0.0..1.0.
pub fn compute_spectrum_bars(samples: &[f32]) -> Vec<f32> {
    // Downmix stereo (interleaved L R L R ...) to mono
    let mono: Vec<f32> = samples
        .chunks(2)
        .map(|pair| {
            if pair.len() == 2 {
                (pair[0] + pair[1]) * 0.5
            } else {
                pair[0]
            }
        })
        .collect();

    // Ensure we have at least FFT_SIZE samples; zero-pad if needed
    let n = FFT_SIZE;
    let mut windowed = vec![Complex::new(0.0f32, 0.0); n];
    for (i, w) in windowed.iter_mut().enumerate() {
        let sample = if i < mono.len() { mono[i] } else { 0.0 };
        // Hann window
        let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / n as f32).cos());
        w.re = sample * window;
    }

    // FFT
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    fft.process(&mut windowed);

    // Take magnitude of first half (positive frequencies)
    let half = n / 2; // 512
    let magnitudes: Vec<f32> = windowed[..half].iter().map(|c| c.norm()).collect();

    // Map 512 bins to 24 bars using log-frequency spacing.
    // Bin i corresponds to frequency ~ i * sample_rate / FFT_SIZE.
    // We use logarithmic spacing: bar boundaries are exponentially spaced
    // from bin 1 to bin 512.
    let mut bars = vec![0.0f32; BAR_COUNT];
    let bar_count = BAR_COUNT;
    let min_bin = 1.0f32;
    let max_bin = half as f32;
    let log_min = min_bin.ln();
    let log_max = max_bin.ln();

    for bar_idx in 0..BAR_COUNT {
        let lo = ((log_min + (log_max - log_min) * bar_idx as f32 / BAR_COUNT as f32).exp())
            as usize;
        let hi = ((log_min
            + (log_max - log_min) * (bar_idx + 1) as f32 / BAR_COUNT as f32)
            .exp()) as usize;
        let lo = lo.max(1).min(half);
        let hi = hi.max(lo + 1).min(half);

        let mut sum = 0.0f32;
        let count = (hi - lo).max(1);
        for bin in lo..hi {
            sum += magnitudes[bin];
        }
        bars[bar_idx] = sum / count as f32;
    }

    // Apply sqrt scaling for better visibility of quiet frequencies
    for bar in &mut bars {
        *bar = bar.sqrt();
    }

    // Normalize to 0.0..1.0 based on the max bar value
    let max_val = bars.iter().cloned().fold(0.0f32, f32::max);
    if max_val > 1e-6 {
        for bar in &mut bars {
            *bar = (*bar / max_val).clamp(0.0, 1.0);
        }
    }

    bars
}

/// Map a value in 0.0..1.0 to a block character given a row height.
fn value_to_block(value: f32, row: usize, total_rows: usize) -> char {
    // Each row represents 1/total_rows of the full range.
    // row 0 = top, row total_rows-1 = bottom.
    let row_bottom = 1.0 - (row + 1) as f32 / total_rows as f32;
    let row_top = 1.0 - row as f32 / total_rows as f32;
    let row_range = row_top - row_bottom;

    if value <= row_bottom {
        ' '
    } else if value >= row_top {
        BLOCKS[7] // full block
    } else {
        // Partial fill within this row
        let fill = (value - row_bottom) / row_range;
        let idx = ((fill * 8.0) as usize).min(7);
        BLOCKS[idx]
    }
}

/// Render the spectrum visualizer bars into the given area.
pub fn draw_spectrum(frame: &mut Frame, area: Rect, state: &VisualizerState, theme: &Theme) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let bar_count = state.bars.len();
    let total_rows = area.height as usize;

    // Determine color(s) from theme
    let color = if theme.visualizer.is_empty() {
        theme.accent
    } else {
        theme.visualizer[theme.visualizer.len() - 1]
    };

    let mut lines = Vec::with_capacity(total_rows);
    for row in 0..total_rows {
        let mut spans = Vec::with_capacity(area.width as usize);
        for col in 0..area.width as usize {
            let bar_idx = col * bar_count / area.width as usize;
            let bar_idx = bar_idx.min(bar_count.saturating_sub(1));
            let val = state.bars[bar_idx];
            let ch = value_to_block(val, row, total_rows);
            spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

/// Render a waveform display from raw samples using block characters.
pub fn draw_waveform(frame: &mut Frame, area: Rect, samples: &[f32], theme: &Theme) {
    if area.width == 0 || area.height == 0 || samples.is_empty() {
        return;
    }

    let color = if theme.visualizer.is_empty() {
        theme.accent
    } else {
        theme.visualizer[theme.visualizer.len() - 1]
    };

    // Downmix stereo to mono for display
    let mono: Vec<f32> = samples
        .chunks(2)
        .map(|pair| {
            if pair.len() == 2 {
                (pair[0] + pair[1]) * 0.5
            } else {
                pair[0]
            }
        })
        .collect();

    let total_rows = area.height as usize;
    let width = area.width as usize;

    // Map each column to a sample range, get the absolute peak
    let mut col_values: Vec<f32> = Vec::with_capacity(width);
    for col in 0..width {
        let start = col * mono.len() / width;
        let end = ((col + 1) * mono.len() / width)
            .max(start + 1)
            .min(mono.len());
        let peak = mono[start..end]
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        col_values.push(peak.clamp(0.0, 1.0));
    }

    let mut lines = Vec::with_capacity(total_rows);
    for row in 0..total_rows {
        let mut spans = Vec::with_capacity(width);
        for col in 0..width {
            let ch = value_to_block(col_values[col], row, total_rows);
            spans.push(Span::styled(ch.to_string(), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spectrum_returns_24_bars() {
        let samples = vec![0.5f32; 2048]; // stereo
        let bars = compute_spectrum_bars(&samples);
        assert_eq!(bars.len(), BAR_COUNT);
    }

    #[test]
    fn silent_input_returns_zeros() {
        let samples = vec![0.0f32; 2048];
        let bars = compute_spectrum_bars(&samples);
        assert_eq!(bars.len(), BAR_COUNT);
        for &bar in &bars {
            assert!(bar.abs() < 1e-6, "expected zero bar, got {bar}");
        }
    }

    #[test]
    fn smoothing_decays() {
        let mut state = VisualizerState::default();
        // Set bars high
        let high: Vec<f32> = vec![1.0; BAR_COUNT];
        state.apply_smoothing(&high);
        assert!((state.bars[0] - 1.0).abs() < 1e-6);

        // Now feed zeros — should decay to 0.85
        let low: Vec<f32> = vec![0.0; BAR_COUNT];
        state.apply_smoothing(&low);
        assert!((state.bars[0] - 0.85).abs() < 1e-6);

        // Another decay step: 0.85 * 0.85 = 0.7225
        state.apply_smoothing(&low);
        assert!((state.bars[0] - 0.7225).abs() < 1e-3);
    }

    #[test]
    fn bars_are_normalized_to_unit_range() {
        // Generate a simple sine wave as stereo
        let mut samples = Vec::with_capacity(2048);
        for i in 0..1024 {
            let val = (2.0 * PI * 100.0 * i as f32 / 44100.0).sin();
            samples.push(val);
            samples.push(val);
        }
        let bars = compute_spectrum_bars(&samples);
        for &bar in &bars {
            assert!(bar >= 0.0 && bar <= 1.0, "bar out of range: {bar}");
        }
    }

    #[test]
    fn mode_toggle() {
        assert_eq!(VisualizerMode::Spectrum.toggle(), VisualizerMode::Waveform);
        assert_eq!(VisualizerMode::Waveform.toggle(), VisualizerMode::Spectrum);
    }
}
