//! A bounded, presentation-only split-flap board for live voice transcripts.

use super::HistoryCell;
use crate::motion::MotionMode;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::visible_lines;
use crate::tui::FrameRequester;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use std::time::Duration;
use std::time::Instant;
use unicode_segmentation::UnicodeSegmentation;

#[cfg(test)]
#[path = "realtime_split_flap_tests.rs"]
mod tests;

const FRAME_INTERVAL: Duration = Duration::from_millis(45);
const ANIMATION_DURATION: Duration = Duration::from_millis(675);
const TILE_SETTLE_DURATION: Duration = Duration::from_millis(180);
const TILE_AFTERGLOW_DURATION: Duration = Duration::from_millis(135);
const FLAP_SAMPLE_WINDOW: usize = 24;

/// Four bounded, calibrated amplitude samples; this is not frequency analysis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct VoiceAmplitudeHistory([usize; 4]);

impl VoiceAmplitudeHistory {
    pub(super) fn push(&mut self, level: usize) {
        self.0.rotate_left(/*mid*/ 1);
        self.0[3] = level.min(/*other*/ 5);
    }
}

#[derive(Debug)]
pub(super) struct SplitFlapTranscriptCell {
    inner: Box<dyn HistoryCell>,
    role: String,
    target: String,
    visible_target: String,
    flap_sample: Vec<u8>,
    tile_arrivals: Vec<Instant>,
    started_at: Instant,
    phase_started_at: Instant,
    motion_mode: MotionMode,
    frame_requester: FrameRequester,
}

impl SplitFlapTranscriptCell {
    pub(super) fn new(
        render: impl Fn(&str) -> Box<dyn HistoryCell>,
        role: &str,
        target: &str,
        previous: Option<&Self>,
        discarded_prefix_bytes: usize,
        motion_mode: MotionMode,
        frame_requester: FrameRequester,
    ) -> Self {
        let now = Instant::now();
        let inner = render(target);
        let visible_target = visible_glyphs(inner.as_ref());
        let mut flap_sample = visible_target
            .bytes()
            .rev()
            .filter(u8::is_ascii_alphabetic)
            .take(FLAP_SAMPLE_WINDOW)
            .collect::<Vec<_>>();
        if flap_sample.is_empty() {
            flap_sample.extend(visible_target.bytes().rev().take(FLAP_SAMPLE_WINDOW));
        }
        flap_sample.reverse();
        let previous = previous.and_then(|cell| {
            let retained = cell.target.get(discarded_prefix_bytes..)?;
            if cell.role != role || retained.is_empty() || !target.starts_with(retained) {
                return None;
            }
            // Render the actual retained text: byte offsets are not tile offsets, and
            // matching text alone could confuse repeated old words with new arrivals.
            let retained_glyphs = if discarded_prefix_bytes == 0 {
                cell.visible_target.clone()
            } else {
                visible_glyphs(render(retained).as_ref())
            };
            let discarded_tiles = cell
                .visible_target
                .len()
                .checked_sub(retained_glyphs.len())?;
            (cell.visible_target[discarded_tiles..] == retained_glyphs
                && visible_target.starts_with(&retained_glyphs))
            .then_some((cell, discarded_tiles))
        });
        let mut tile_arrivals = previous
            .map(|(cell, discarded_tiles)| cell.tile_arrivals[discarded_tiles..].to_vec())
            .unwrap_or_default();
        let existing_tiles = tile_arrivals.len();
        for index in existing_tiles..visible_target.len() {
            let offset = u64::try_from(index.saturating_sub(existing_tiles))
                .unwrap_or(u64::MAX)
                .saturating_mul(/*rhs*/ 8)
                .min(/*other*/ 56);
            let start = if previous.is_some() {
                now.checked_sub(FRAME_INTERVAL).unwrap_or(now)
            } else {
                now
            };
            tile_arrivals.push(start + Duration::from_millis(offset));
        }
        let cell = Self {
            inner,
            role: role.to_string(),
            target: target.to_string(),
            visible_target,
            flap_sample,
            tile_arrivals,
            started_at: now,
            phase_started_at: previous
                .map(|(cell, _)| cell.phase_started_at)
                .unwrap_or(now),
            motion_mode,
            frame_requester,
        };
        if cell.is_animating(Duration::ZERO) {
            cell.frame_requester.schedule_frame_in(FRAME_INTERVAL);
        }
        cell
    }

    fn is_animating(&self, elapsed: Duration) -> bool {
        self.motion_mode == MotionMode::Animated
            && elapsed < ANIMATION_DURATION
            && !self.tile_arrivals.is_empty()
    }

    fn animate_lines(
        &self,
        mut lines: Vec<HyperlinkLine>,
        width: u16,
        elapsed: Duration,
    ) -> Vec<HyperlinkLine> {
        if self.motion_mode == MotionMode::Reduced {
            return lines;
        }

        let board_style = Style::new().bg(Color::Black).fg(Color::Gray);
        let mut glyph_index = 0;
        let mut word_is_unsettled = false;
        let now = self.started_at + elapsed;
        let phase_elapsed = now.saturating_duration_since(self.phase_started_at);
        let final_line = lines.iter().rposition(|line| {
            line.line
                .spans
                .iter()
                .any(|span| !span.content.trim().is_empty())
        });
        for (line_index, line) in lines.iter_mut().enumerate() {
            if line
                .line
                .spans
                .iter()
                .all(|span| span.content.trim().is_empty())
            {
                continue;
            }

            // Animated glyphs and board padding no longer map to authored source bytes.
            line.source = None;
            let original_spans = std::mem::take(&mut line.line.spans);
            line.line.style = line.line.style.patch(board_style);
            for span in original_spans {
                for grapheme in span.content.graphemes(/*is_extended*/ true) {
                    let mut style = span.style.patch(board_style);
                    let text = if is_flippable(grapheme) {
                        let position = glyph_index;
                        glyph_index += 1;
                        let arrival = self
                            .tile_arrivals
                            .get(position)
                            .copied()
                            .unwrap_or(self.started_at);
                        let tile_elapsed = now.saturating_duration_since(arrival);
                        let (text, flipping) = flap_glyph(
                            grapheme,
                            position,
                            tile_elapsed,
                            phase_elapsed,
                            &self.flap_sample,
                        );
                        if flipping {
                            word_is_unsettled = true;
                            style = style.fg(Color::DarkGray);
                        } else if tile_elapsed < TILE_SETTLE_DURATION + TILE_AFTERGLOW_DURATION {
                            style = style.fg(if self.role == "user" {
                                Color::Cyan
                            } else {
                                Color::Magenta
                            });
                        }
                        text
                    } else {
                        if grapheme == "›" {
                            style = style.fg(span.style.fg.unwrap_or(Color::Cyan));
                        } else if grapheme == "•" {
                            style = style.fg(Color::Magenta);
                        }
                        if grapheme.chars().all(char::is_whitespace) {
                            word_is_unsettled = false;
                        }
                        if word_is_unsettled && is_punctuation(grapheme) {
                            " ".to_string()
                        } else {
                            grapheme.to_string()
                        }
                    };
                    line.line.spans.push(Span::styled(text, style));
                }
            }

            let remaining = usize::from(width).saturating_sub(line.width());
            if Some(line_index) == final_line
                && remaining > 1
                && self.is_animating(elapsed)
                && self.tile_arrivals.first().is_some_and(|arrival| {
                    now.saturating_duration_since(*arrival) >= TILE_SETTLE_DURATION
                })
            {
                let phase = usize::try_from(phase_elapsed.as_millis() / FRAME_INTERVAL.as_millis())
                    .unwrap_or(usize::MAX);
                let runway = (0..remaining.saturating_sub(/*rhs*/ 1).min(/*other*/ 3))
                    .map(|index| {
                        char::from(self.flap_sample[(phase + index) % self.flap_sample.len()])
                    })
                    .collect::<String>();
                line.line.spans.push(Span::styled(
                    format!(" {runway}"),
                    board_style.fg(Color::DarkGray),
                ));
            }
            let padding = usize::from(width).saturating_sub(line.width());
            if padding > 0 {
                line.line
                    .spans
                    .push(Span::styled(" ".repeat(padding), board_style));
            }
        }

        if self.is_animating(elapsed) {
            self.frame_requester.schedule_frame_in(FRAME_INTERVAL);
        }
        lines
    }
}

impl HistoryCell for SplitFlapTranscriptCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        visible_lines(self.display_hyperlink_lines(width))
    }

    fn raw_lines(&self) -> Vec<Line<'static>> {
        self.inner.raw_lines()
    }

    fn display_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.animate_lines(
            self.inner.display_hyperlink_lines(width),
            width,
            self.started_at.elapsed(),
        )
    }

    fn transcript_hyperlink_lines(&self, width: u16) -> Vec<HyperlinkLine> {
        self.animate_lines(
            self.inner.transcript_hyperlink_lines(width),
            width,
            self.started_at.elapsed(),
        )
    }

    fn transcript_animation_tick(&self) -> Option<u64> {
        let elapsed = self.started_at.elapsed();
        self.is_animating(elapsed).then(|| {
            u64::try_from(elapsed.as_millis() / FRAME_INTERVAL.as_millis()).unwrap_or(u64::MAX)
        })
    }
}

fn visible_glyphs(cell: &dyn HistoryCell) -> String {
    let mut glyphs = String::new();
    for line in cell.display_hyperlink_lines(u16::MAX) {
        for span in line.line.spans {
            for grapheme in span.content.graphemes(/*is_extended*/ true) {
                if is_flippable(grapheme) {
                    glyphs.push_str(grapheme);
                }
            }
        }
    }
    glyphs
}

fn is_flippable(grapheme: &str) -> bool {
    grapheme.len() == 1 && grapheme.as_bytes()[0].is_ascii_alphanumeric()
}

fn is_punctuation(grapheme: &str) -> bool {
    grapheme.len() == 1 && grapheme.as_bytes()[0].is_ascii_punctuation()
}

fn flap_glyph(
    grapheme: &str,
    index: usize,
    elapsed: Duration,
    phase_elapsed: Duration,
    sample: &[u8],
) -> (String, bool) {
    let milliseconds = elapsed.as_millis();
    if milliseconds < 40 {
        return (" ".to_string(), true);
    }
    if elapsed >= TILE_SETTLE_DURATION {
        return (grapheme.to_string(), false);
    }

    let frame = usize::try_from(phase_elapsed.as_millis() / FRAME_INTERVAL.as_millis())
        .unwrap_or(usize::MAX);
    let target = usize::from(grapheme.as_bytes()[0]);
    let offset = target
        .wrapping_add(index.wrapping_mul(/*rhs*/ 7))
        .wrapping_add(frame.wrapping_mul(/*rhs*/ 11));
    let tile = char::from(sample[offset % sample.len()]);
    (tile.to_string(), true)
}
