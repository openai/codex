//! The blossom welcome animation for onboarding, with a full-color final pose.
//! Visible time pauses while hidden. Fresh conversations start settled and can replay on a click.

mod geometry;
mod greetings;
mod lighting;
mod paths;
mod policy;
mod renderer;
mod sequence;

use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;

use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
use ratatui::buffer::Buffer;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::motion::MotionMode;
use crate::terminal_palette;
pub(crate) use greetings::Greeting;
use lighting::Lighting;
pub(crate) use policy::Presentation;
pub(crate) use policy::is_startup_cell;
use renderer::MAX_COLUMNS;
use renderer::MAX_ROWS;
use renderer::Renderer;

pub(crate) const FRAME_INTERVAL: Duration = Duration::from_millis(/*millis*/ 50);
// The preview's first visible pose (10.8s / 7.2s), also the start of an idle-screen replay.
const SETTLED_BLOSSOM: f64 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComposerState {
    Empty,
    Draft,
}

#[derive(Default)]
pub(crate) struct EmptyStateAnimation {
    eligible: bool,
    // Initialized once for fresh threads; never initialized for a resumed or forked thread.
    // Headers retain the selection after the temporary blossom is dismissed.
    pub(crate) greeting: Arc<OnceLock<Greeting>>,
    spin_elapsed: Duration,
    last_frame: Option<Instant>,
    fade_elapsed: Duration,
    static_mark: Option<bool>,
    opacity: f32,
    fade_from: f32,
    renderer: Option<Renderer>,
    stage: Option<Rect>,
    replaying: bool,
}

impl EmptyStateAnimation {
    pub(crate) fn is_eligible(&self) -> bool {
        self.eligible
    }

    pub(crate) fn start_fresh(&mut self) {
        self.cancel_replay();
        self.eligible = true;
        self.greeting.get_or_init(Greeting::choose);
        self.spin_elapsed = Duration::ZERO;
        self.last_frame = None;
        self.fade_elapsed = Duration::ZERO;
        self.static_mark = None;
        self.opacity = 1.0;
    }

    /// Keep the provisional pose and phrase in headers already bound to the live thread.
    pub(crate) fn continue_from(&mut self, source: &mut Self) {
        let mut previous = std::mem::take(source);
        if !previous.is_eligible() {
            previous.start_fresh();
        }
        if let Some(greeting) = previous.greeting.get() {
            let _ = self.greeting.set(*greeting);
        }
        previous.greeting = Arc::clone(&self.greeting);
        *self = previous;
    }

    pub(crate) fn dismiss(&mut self) {
        self.eligible = false;
        self.cancel_replay();
    }

    pub(crate) fn cancel_replay(&mut self) {
        self.stage = None;
        self.replaying = false;
        self.pause_clock();
    }

    /// Replay only on the drawn blossom; unrelated gestures keep their normal owner.
    pub(crate) fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        let inside = self
            .stage
            .is_some_and(|stage| stage.contains(Position::new(mouse.column, mouse.row)))
            && mouse.modifiers.is_empty();
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) if inside => {
                self.start_fresh();
                self.spin_elapsed =
                    Duration::from_secs_f64(SETTLED_BLOSSOM * sequence::LOOP_SECONDS);
                self.opacity = sequence::STATIC_OPACITY;
                self.static_mark = Some(true);
                self.replaying = true;
                true
            }
            _ => false,
        }
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
        self.paint_frame(area, buffer, phase, self.opacity);
        (!finished && (!static_mark || settling)).then_some(FRAME_INTERVAL)
    }

    /// Draw only in space the caller has cleared and owns. The centered blossom returns
    /// on clear, without restarting or scheduling animation.
    pub(crate) fn render_first_screen(
        &mut self,
        available: Rect,
        buffer: &mut Buffer,
        composer: Option<ComposerState>,
        motion: MotionMode,
    ) -> Option<Duration> {
        const STAGE_ROWS: u16 = 21;
        const MIN_STAGE_ROWS: u16 = 14;
        let screen = buffer.area;
        let width = screen.width.saturating_sub(/*rhs*/ 4).min(MAX_COLUMNS);
        let height = width * STAGE_ROWS / MAX_COLUMNS;
        if motion == MotionMode::Reduced
            || composer != Some(ComposerState::Empty)
            || height < MIN_STAGE_ROWS
            || height > screen.height
        {
            self.cancel_replay();
            return None;
        }
        let stage = Rect::new(
            screen.x + (screen.width - width) / 2,
            screen.y + (screen.height - height) / 2,
            width,
            height,
        );
        if stage.intersection(available) != stage {
            self.cancel_replay();
            return None;
        }
        self.stage = Some(stage);
        if self.replaying {
            if let Some(delay) = self.render_in(stage, buffer, Presentation::Animated) {
                return Some(delay);
            }
            // Paint the idle pose over the final full-color frame in the same update.
            self.replaying = false;
        }
        self.paint_frame(stage, buffer, SETTLED_BLOSSOM, sequence::STATIC_OPACITY);
        None
    }

    fn paint_frame(&mut self, area: Rect, buffer: &mut Buffer, phase: f64, opacity: f32) {
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
            let color = crate::color::blend((r, g, b), background, opacity);
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
            if opacity < 0.5
                && matches!(
                    color_level,
                    terminal_palette::StdoutColorLevel::Ansi16
                        | terminal_palette::StdoutColorLevel::Unknown
                )
            {
                target.set_style(target.style().dim());
            }
        }
    }
}

#[cfg(test)]
#[path = "empty_state_animation_tests.rs"]
mod tests;
