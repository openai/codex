use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;

#[derive(Debug)]
pub(crate) struct FrameTicker {
    running: Arc<AtomicBool>,
}

impl FrameTicker {
    pub(crate) fn new(app_event_tx: AppEventSender) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();
        let app_event_tx_clone = app_event_tx.clone();
        std::thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
                app_event_tx_clone.send(AppEvent::RequestRedraw);
            }
        });
        Self { running }
    }
}

impl Drop for FrameTicker {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

pub(crate) fn shimmer_spans(text: &str, frame_idx: usize) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    let padding = 10usize;
    let period = chars.len() + padding * 2;
    let pos = frame_idx % period;
    let has_true_color = supports_color::on_cached(supports_color::Stream::Stdout)
        .map(|level| level.has_16m)
        .unwrap_or(false);
    let band_half_width = 6.0;

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(chars.len());
    for (i, ch) in chars.iter().enumerate() {
        let i_pos = i as isize + padding as isize;
        let pos = pos as isize;
        let dist = (i_pos - pos).abs() as f32;

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
            Style::default().fg(color_for_level(level))
        };
        spans.push(Span::styled(ch.to_string(), style));
    }
    spans
}

fn color_for_level(level: u8) -> Color {
    if level < 128 {
        Color::DarkGray
    } else if level < 192 {
        Color::Gray
    } else {
        Color::White
    }
}
