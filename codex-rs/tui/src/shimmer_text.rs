use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();

/// Ensure the process start time is initialized. Call early in app startup
/// so all animations key off a common origin.
pub(crate) fn init_process_start() {
    let _ = PROCESS_START.set(Instant::now());
}

fn elapsed_since_start() -> Duration {
    let start = PROCESS_START.get_or_init(Instant::now);
    start.elapsed()
}

/// Compute grayscale shimmer spans for the provided text based on elapsed
/// time since process start. Uses a cosine falloff across a small band to
/// achieve a smooth highlight that sweeps across the text.
pub(crate) fn shimmer_spans(text: &str) -> Vec<Span<'static>> {
    let header_chars: Vec<char> = text.chars().collect();

    // Synchronize the shimmer so that all instances start at the beginning
    // and reach the end at the same time, regardless of length. We achieve
    // this by mapping elapsed time into a global sweep fraction in [0, 1),
    // then scaling that fraction across the character indices of this text.
    // The bright band width (in characters) remains constant.
    let len = header_chars.len();
    if len == 0 {
        return Vec::new();
    }

    // Width of the bright band (in characters).
    let band_half_width = (len as f32) / 4.0;

    // Use character-based padding: pretend the string is longer by
    // `PADDING * 2` characters and move at a constant velocity over time.
    // We compute the cycle duration in time (including pre/post time derived
    // from character padding at constant velocity) and wrap using time modulo
    // rather than modulo on character distance.
    const SWEEP_SECONDS: f32 = 1.5; // time to traverse the visible text
    let PADDING: f32 = band_half_width;
    let elapsed = elapsed_since_start().as_secs_f32();
    let pos = (elapsed % SWEEP_SECONDS) / SWEEP_SECONDS * (len as f32 + PADDING * 2.0) - PADDING;

    let has_true_color = supports_color::on_cached(supports_color::Stream::Stdout)
        .map(|level| level.has_16m)
        .unwrap_or(false);

    let mut header_spans: Vec<Span<'static>> = Vec::with_capacity(header_chars.len());
    for (i, ch) in header_chars.iter().enumerate() {
        let i_pos = i as f32;
        let dist = (i_pos - pos).abs();

        let t = if dist <= band_half_width {
            let x = std::f32::consts::PI * (dist / band_half_width);
            0.5 * (1.0 + x.cos())
        } else {
            0.0
        };

        let brightness = 0.4 + 0.6 * t;
        let level = (brightness * 255.0).clamp(0.0, 255.0) as u8;
        let style = if has_true_color {
            Style::default()
                .fg(Color::Rgb(level, level, level))
                .add_modifier(Modifier::BOLD)
        } else {
            // Bold makes dark gray and gray look the same, so don't use it
            // when true color is not supported.
            Style::default().fg(color_for_level(level))
        };

        header_spans.push(Span::styled(ch.to_string(), style));
    }

    header_spans
}

//

/// Utility used for 16-color terminals to approximate grayscale.
pub(crate) fn color_for_level(level: u8) -> Color {
    if level < 128 {
        Color::DarkGray
    } else if level < 192 {
        Color::Gray
    } else {
        Color::White
    }
}
