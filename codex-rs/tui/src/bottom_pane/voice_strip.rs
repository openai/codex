//! Renders compact voice controls and caller-owned microphone/speaker sample histories.
//! Keep control positions stable and show mute only when the current phase permits it.

use crate::key_hint::ShortcutHint;
use crate::motion::MotionMode;
use crate::render::renderable::Renderable;
use crate::tui::FrameRequester;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use std::time::Duration;
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VoiceStripPhase {
    Connecting,
    Active,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VoiceStripState {
    pub(crate) mute_hint: Option<ShortcutHint>,
    pub(crate) phase: VoiceStripPhase,
    pub(crate) microphone_live: bool,
    pub(crate) microphone_muted: bool,
    pub(crate) microphone_history: Vec<u8>,
    pub(crate) speaker_history: Vec<u8>,
    pub(crate) activity: &'static str,
    pub(crate) animations: bool,
}

pub(super) struct VoiceStrip {
    state: VoiceStripState,
    started_at: Instant,
    frame_requester: FrameRequester,
}

impl VoiceStrip {
    pub(super) fn new(state: VoiceStripState, frame_requester: FrameRequester) -> Self {
        Self {
            state,
            started_at: Instant::now(),
            frame_requester,
        }
    }

    pub(super) fn update(&mut self, state: VoiceStripState) {
        if self.state.activity != state.activity {
            self.started_at = Instant::now();
        }
        self.state = state;
    }
}

pub(super) fn loading_glyph(started_at: Instant, mode: MotionMode) -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    if mode == MotionMode::Reduced {
        "◌"
    } else {
        let frame = started_at.elapsed().as_millis() / 100;
        FRAMES[usize::try_from(frame).unwrap_or_default() % FRAMES.len()]
    }
}

impl Renderable for VoiceStrip {
    fn desired_height(&self, width: u16) -> u16 {
        if width == 0 { 0 } else { 2 }
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        let area = Rect {
            x: area.x.saturating_add(/*rhs*/ 1),
            width: area.width.saturating_sub(/*rhs*/ 2),
            ..area
        };
        if area.is_empty() {
            return;
        }
        let connecting = self.state.phase == VoiceStripPhase::Connecting;
        let mode = MotionMode::from_animations_enabled(self.state.animations);
        if connecting && mode == MotionMode::Animated {
            self.frame_requester
                .schedule_frame_in(Duration::from_millis(100));
        }
        let marker = if connecting && self.state.microphone_live && !self.state.microphone_muted {
            loading_glyph(self.started_at, mode).red().bold()
        } else if connecting {
            loading_glyph(self.started_at, mode).cyan()
        } else if self.state.microphone_live && !self.state.microphone_muted {
            "●".red().bold()
        } else {
            "◌".dark_gray()
        };
        let activity = format!(" {}", self.state.activity).into();
        let mut status = Line::from(vec!["voice ".dim(), marker, activity]);
        let mute = self.state.mute_hint.map_or_else(
            || "/voice mute".to_string(),
            |hint| {
                format!(
                    "{} {}",
                    hint.display_label(),
                    if self.state.microphone_muted {
                        "unmute"
                    } else {
                        "mute  "
                    }
                )
            },
        );
        let full_controls = format!("{mute}   /voice stop");
        let available = usize::from(area.width);
        let can_mute = !connecting || self.state.microphone_live || self.state.microphone_muted;
        if can_mute
            && available < status.width() + full_controls.width() + 1
            && available >= "voice ".len() + full_controls.width() + 2
        {
            status.spans.pop();
        }
        let show_full_controls = can_mute && available > status.width() + full_controls.width();
        let controls = if show_full_controls {
            full_controls.as_str()
        } else {
            "/voice stop"
        };
        if available < status.width() + controls.width() + 1 {
            status = Line::from(vec!["voice".dim()]);
        }
        status.spans.push(
            " ".repeat(available.saturating_sub(status.width() + controls.width()))
                .into(),
        );
        if show_full_controls && let Some(hint) = self.state.mute_hint {
            status.spans.extend(hint.spans());
            status.spans.push(if self.state.microphone_muted {
                " unmute   /voice stop".dim()
            } else {
                " mute     /voice stop".dim()
            });
        } else {
            status.spans.push(controls.dim());
        }
        Paragraph::new(status).render(Rect::new(area.x, area.y, area.width, /*height*/ 1), buf);
        if area.height < 2 {
            return;
        }
        let meter_width = available
            .saturating_sub(/*rhs*/ 14)
            .saturating_div(/*rhs*/ 2)
            .min(/*other*/ 6);
        let mut meters = Vec::with_capacity(meter_width * 2 + 2);
        meters.push("  mic ".dim());
        append_voice_history(
            &mut meters,
            &self.state.microphone_history,
            meter_width,
            Color::Cyan,
            self.state.microphone_muted || !self.state.microphone_live,
        );
        meters.push("  codex ".dim());
        append_voice_history(
            &mut meters,
            &self.state.speaker_history,
            meter_width,
            Color::Magenta,
            /*muted*/ false,
        );
        Paragraph::new(Line::from(meters))
            .render(Rect::new(area.x, area.y + 1, area.width, /*height*/ 1), buf);
    }
}

fn append_voice_history(
    spans: &mut Vec<Span<'static>>,
    samples: &[u8],
    meter_width: usize,
    color: Color,
    muted: bool,
) {
    let samples = &samples[samples.len().saturating_sub(meter_width)..];
    for intensity in std::iter::repeat_n(/*element*/ 0, meter_width - samples.len())
        .chain(samples.iter().copied())
    {
        let height = if muted {
            0
        } else {
            usize::from(intensity)
                .saturating_mul(/*rhs*/ 7)
                .div_ceil(/*rhs*/ 255)
        };
        let style = Style::new().fg(if height == 0 { Color::DarkGray } else { color });
        spans.push(Span::styled(
            ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"][height],
            style,
        ));
    }
}

#[cfg(test)]
#[path = "voice_strip_tests.rs"]
mod tests;
