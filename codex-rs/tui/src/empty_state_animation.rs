//! The blossom welcome animation for onboarding, with a full-color final pose.
//! Visible time pauses while hidden; conversation lifecycle tracking is retained for the header.

mod geometry;
mod lighting;
mod paths;
mod policy;
mod renderer;
mod sequence;

use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::terminal_palette;
use lighting::Lighting;
pub(crate) use policy::Presentation;
pub(crate) use policy::is_startup_cell;
use renderer::MAX_COLUMNS;
use renderer::MAX_ROWS;
use renderer::Renderer;

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_millis(/*millis*/ 50);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposerState {
    Empty,
    Draft,
}

#[derive(Default)]
pub(crate) struct EmptyStateAnimation {
    eligible: bool,
    spin_elapsed: Duration,
    last_frame: Option<Instant>,
    fade_elapsed: Duration,
    static_mark: Option<bool>,
    opacity: f32,
    fade_from: f32,
    renderer: Option<Renderer>,
}

impl EmptyStateAnimation {
    pub(crate) fn start_fresh(&mut self) {
        self.eligible = true;
        self.spin_elapsed = Duration::ZERO;
        self.last_frame = None;
        self.fade_elapsed = Duration::ZERO;
        self.static_mark = None;
        self.opacity = 1.0;
    }

    pub(crate) fn dismiss(&mut self) {
        self.eligible = false;
    }

    /// Stop visible time at the last painted frame, including when no hidden frame was drawn.
    /// Resume (after process suspension) may call this after the interruption; phase is retained.
    pub(crate) fn pause_clock(&mut self) {
        self.last_frame = None;
    }

    /// Paint the shared logo sequence inside a caller-owned, reserved rectangle.
    /// The caller clears the stage and keeps its layout stable after motion finishes.
    pub(crate) fn render_in(
        &mut self,
        area: Rect,
        buffer: &mut Buffer,
        presentation: Presentation,
    ) -> Option<Duration> {
        let now = Instant::now();
        self.render_in_at(area, buffer, presentation, now)
    }

    fn render_in_at(
        &mut self,
        area: Rect,
        buffer: &mut Buffer,
        presentation: Presentation,
        now: Instant,
    ) -> Option<Duration> {
        if !self.eligible
            || presentation == Presentation::Hidden
            || area.is_empty()
            || area.width > MAX_COLUMNS
            || area.height > MAX_ROWS
            || area.intersection(buffer.area) != area
        {
            self.pause_clock();
            return None;
        }
        let static_mark = presentation == Presentation::Faded;
        let previous_frame = self.last_frame.replace(now);
        if self.static_mark != Some(static_mark) {
            self.fade_elapsed = if self.static_mark.is_none() {
                sequence::STATIC_FADE
            } else {
                Duration::ZERO
            };
            self.fade_from = self.opacity;
        } else if let Some(previous) = previous_frame {
            let elapsed = now.saturating_duration_since(previous);
            self.fade_elapsed += elapsed;
            if !static_mark {
                self.spin_elapsed += elapsed;
            }
        }
        self.static_mark = Some(static_mark);
        let finished = self.spin_elapsed >= sequence::SPIN_DURATION;
        let phase =
            self.spin_elapsed.min(sequence::SPIN_DURATION).as_secs_f64() / sequence::LOOP_SECONDS;
        let settling = !finished && static_mark && self.fade_elapsed < sequence::STATIC_FADE;
        self.opacity = if finished {
            1.0
        } else if static_mark {
            sequence::static_opacity(self.fade_elapsed, self.fade_from)
        } else {
            1.0
        };
        if !static_mark && !finished {
            self.opacity = self.fade_from
                + (self.opacity - self.fade_from)
                    * sequence::progress(self.fade_elapsed, sequence::STATIC_FADE) as f32;
        }
        let background = terminal_palette::default_bg();
        let color_level = if background.is_some() {
            terminal_palette::effective_stdout_color_level()
        } else {
            terminal_palette::StdoutColorLevel::Unknown
        };
        let background = background.unwrap_or((15, 20, 37));
        let light = Lighting::terminal(
            terminal_palette::default_fg().unwrap_or((210, 221, 235)),
            background,
        );
        let cells = self.renderer.get_or_insert_with(Renderer::default).frame(
            area.width,
            area.height,
            phase,
            &light,
        );
        for (i, cell) in cells.iter().enumerate().filter(|(_, cell)| cell.dots != 0) {
            let [_, r, g, b] = cell.rgb.to_be_bytes();
            let target = &mut buffer[(
                area.x + i as u16 % area.width,
                area.y + i as u16 / area.width,
            )];
            target.set_char(char::from_u32(0x2800 + u32::from(cell.dots)).unwrap_or(' '));
            let color = crate::color::blend((r, g, b), background, self.opacity);
            let color = if color_level == terminal_palette::StdoutColorLevel::Ansi256 {
                // Four bits per channel bound the cache and avoid palette searches each frame.
                static COLORS: [OnceLock<Color>; 4096] = [const { OnceLock::new() }; 4096];
                let (r, g, b) = (color.0 >> 4, color.1 >> 4, color.2 >> 4);
                let index = usize::from(r) * 256 + usize::from(g) * 16 + usize::from(b);
                *COLORS[index].get_or_init(|| {
                    terminal_palette::best_color_for_level(
                        (r * 16 + 8, g * 16 + 8, b * 16 + 8),
                        color_level,
                    )
                })
            } else {
                terminal_palette::best_color_for_level(color, color_level)
            };
            target.set_fg(color);
            // Default-color terminals can only step between normal and dim intensity.
            if self.opacity < 0.5
                && matches!(
                    color_level,
                    terminal_palette::StdoutColorLevel::Ansi16
                        | terminal_palette::StdoutColorLevel::Unknown
                )
            {
                target.set_style(target.style().dim());
            }
        }
        (!finished && (!static_mark || settling)).then_some(FRAME_INTERVAL)
    }
}

#[cfg(test)]
#[path = "empty_state_animation_tests.rs"]
mod tests;
