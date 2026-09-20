//! Decoration for a fresh owned-screen conversation, independent of transcript/history data.
//!
//! Eligibility is opt-in at genuine thread creation and ends on first submission/activity.
//! The UI supplies an explicit presentation; unused screen space only controls placement.
//! Rendering never changes layout or cursor state; reduced motion hides the decoration entirely.
//! The stage is anchored near the top-right corner, with room around the terminal edges.
//! A nonempty composer or unfocused terminal fades and holds the current Codex pose.
//! The intact logo spins immediately; focus loss and drafting fade it into rest.
//! Motion is limited to three rotations, followed by a fade to nothing. Focus and drafting
//! pause that budget; completed animations never restart until a genuinely fresh thread.
//! Owners pause visible time when a handoff skips rendering; returning never catches up offscreen.
//! Onboarding can reserve a header stage and retain the faded mark after the same sequence.

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
use ratatui::layout::Size;
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

#[derive(Clone, Copy)]
pub(crate) enum AnimationEnd {
    Hide,
    Faded,
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
    cell_aspect: Option<(Size, f64)>,
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

    /// Paint only in unused cells. Returns a redraw deadline only for visible motion.
    pub(crate) fn render(
        &mut self,
        screen: Size,
        bottom: Rect,
        buffer: &mut Buffer,
        presentation: Presentation,
    ) -> Option<Duration> {
        if !self.eligible {
            return None;
        }
        if presentation == Presentation::Hidden {
            self.pause_clock();
            return None;
        }
        let aspect = match self.cell_aspect {
            Some((size, aspect)) if size == screen => aspect,
            _ => {
                let aspect = crossterm::terminal::window_size()
                    .ok()
                    .filter(|size| {
                        size.width > 0 && size.height > 0 && size.columns > 0 && size.rows > 0
                    })
                    .map_or(/*default*/ 0.5, |size| {
                        (f64::from(size.width) * f64::from(size.rows)
                            / (f64::from(size.height) * f64::from(size.columns)))
                        .clamp(/*min*/ 0.25, /*max*/ 1.0)
                    });
                self.cell_aspect = Some((screen, aspect));
                aspect
            }
        };
        let Some(area) = stage(screen, bottom, buffer, aspect) else {
            self.pause_clock();
            return None;
        };
        self.render_in(area, buffer, presentation, AnimationEnd::Hide)
    }

    /// Paint the shared logo sequence inside a caller-owned, reserved rectangle.
    /// The caller clears the stage and keeps its layout stable after motion finishes.
    pub(crate) fn render_in(
        &mut self,
        area: Rect,
        buffer: &mut Buffer,
        presentation: Presentation,
        end: AnimationEnd,
    ) -> Option<Duration> {
        let now = Instant::now();
        self.render_in_at(area, buffer, presentation, end, now)
    }

    fn render_in_at(
        &mut self,
        area: Rect,
        buffer: &mut Buffer,
        presentation: Presentation,
        end: AnimationEnd,
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
                self.spin_elapsed += if self.spin_elapsed >= sequence::SPIN_DURATION {
                    self.fade_elapsed
                        .saturating_sub(sequence::STATIC_FADE)
                        .min(elapsed)
                } else {
                    elapsed
                };
            }
        }
        self.static_mark = Some(static_mark);
        let completing = !static_mark && self.spin_elapsed >= sequence::SPIN_DURATION;
        let finished = self.spin_elapsed >= sequence::SPIN_DURATION + sequence::COMPLETION_FADE;
        if finished && matches!(end, AnimationEnd::Hide) {
            self.dismiss();
            return None;
        }
        let phase = if finished {
            0.0
        } else {
            self.spin_elapsed.min(sequence::SPIN_DURATION).as_secs_f64() / sequence::LOOP_SECONDS
        };
        let settling = !finished && static_mark && self.fade_elapsed < sequence::STATIC_FADE;
        self.opacity = if finished {
            sequence::STATIC_OPACITY
        } else if completing {
            let opacity = sequence::completion_opacity(self.spin_elapsed - sequence::SPIN_DURATION);
            match end {
                AnimationEnd::Hide => opacity,
                AnimationEnd::Faded => {
                    sequence::STATIC_OPACITY + (1.0 - sequence::STATIC_OPACITY) * opacity
                }
            }
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
        (!finished && (!static_mark || settling || completing)).then_some(FRAME_INTERVAL)
    }
}

fn stage(screen: Size, bottom: Rect, buffer: &Buffer, aspect: f64) -> Option<Rect> {
    const RIGHT_MARGIN: u16 = 4;
    const TOP_MARGIN: u16 = 2;
    (32..=MAX_COLUMNS.min(screen.width.saturating_sub(RIGHT_MARGIN * 2)))
        .rev()
        .find_map(|width| {
            let height = (f64::from(width) * 550.0 / 800.0 * aspect).round() as u16;
            if height == 0 || height > MAX_ROWS || height > screen.height {
                return None;
            }
            let area = Rect::new(
                screen.width - width - RIGHT_MARGIN,
                TOP_MARGIN,
                width,
                height,
            );
            (area.bottom() < bottom.y
                && area.intersection(buffer.area) == area
                && (area.y..area.bottom())
                    .all(|y| (area.x..area.right()).all(|x| buffer[(x, y)].symbol() == " ")))
            .then_some(area)
        })
}

#[cfg(test)]
#[path = "empty_state_animation_tests.rs"]
pub(crate) mod tests;
