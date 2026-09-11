//! Astra stars throughout the untouched composer, using the terminal's colors.
//! Selecting Astra starts one flourish; composer input fades it quickly, or 15 seconds after the
//! first visible frame starts a smooth fade. The deadline keeps running while terminal focus is
//! elsewhere. Only visible, eligible frames schedule more work, and the final frame clears the
//! stars without taking over the terminal's native mouse actions.

use std::cell::Cell;
use std::sync::LazyLock;
use std::time::Duration;
use std::time::Instant;

use codex_config::types::Tui;
use ratatui::buffer::Buffer;
use ratatui::buffer::CellDiffOption;
use ratatui::layout::Position;
use ratatui::layout::Rect;
use ratatui::style::Color;
use regex_lite::Regex;
use unicode_width::UnicodeWidthStr;

use super::ChatComposer;
use super::popup_state::ActivePopup;
use crate::bottom_pane::BottomPane;
use crate::color::blend;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::default_fg;
use crate::terminal_palette::effective_stdout_color_level;
use crate::terminal_palette::rgb_color;

const FRAME_TICK: Duration = Duration::from_millis(150);
const IDLE_TIMEOUT: Duration = Duration::from_secs(15);
const IDLE_FADE: Duration = Duration::from_secs(1);
const INTERACTION_FADE: Duration = Duration::from_millis(75);
const FADE_FRAME_TICK: Duration = Duration::from_millis(25);
const DOTS: [&str; 8] = ["⠁", "⠂", "⠄", "⠈", "⠐", "⠠", "⡀", "⢀"];
static ASTRA_MODEL: LazyLock<Regex> = LazyLock::new(|| match Regex::new(r"(?i)\bastra\b") {
    Ok(regex) => regex,
    Err(error) => panic!("invalid Astra model regex: {error}"),
});

pub(super) struct Sparkle {
    started: Instant,
    phase: Cell<Phase>,
    terminal_focused: bool,
}

#[derive(Clone, Copy)]
enum Phase {
    Waiting,
    Visible {
        since: Instant,
        elapsed: Duration,
    },
    Fading {
        started: Instant,
        elapsed: Duration,
        duration: Duration,
        start_visibility: f32,
    },
    Finished,
}

struct SparkleFrame {
    elapsed: Duration,
    visibility: f32,
    next: Duration,
}

impl Sparkle {
    fn enabled_foreground(&self) -> Option<(u8, u8, u8)> {
        if matches!(self.phase.get(), Phase::Finished)
            || effective_stdout_color_level() != StdoutColorLevel::TrueColor
        {
            return None;
        }
        // Read the cached palette here: Windows may populate it after widget creation.
        default_fg()
    }

    fn interact(&self, now: Instant) {
        match self.phase.get() {
            Phase::Waiting => self.phase.set(Phase::Finished),
            Phase::Visible { since, elapsed } if now >= since + IDLE_TIMEOUT => {
                self.phase.set(Phase::Fading {
                    started: since + IDLE_TIMEOUT,
                    elapsed,
                    duration: IDLE_FADE,
                    start_visibility: 1.0,
                });
                self.interact(now);
            }
            Phase::Visible { elapsed, .. } => self.phase.set(Phase::Fading {
                started: now,
                elapsed,
                duration: INTERACTION_FADE,
                start_visibility: 1.0,
            }),
            Phase::Fading {
                started,
                elapsed,
                duration: IDLE_FADE,
                start_visibility,
            } => {
                let remaining = IDLE_FADE.saturating_sub(now.saturating_duration_since(started));
                if remaining.is_zero() {
                    self.phase.set(Phase::Finished);
                } else {
                    // Input during the timed fade finishes quickly without brightening the dots.
                    self.phase.set(Phase::Fading {
                        started: now,
                        elapsed,
                        duration: INTERACTION_FADE.min(remaining),
                        start_visibility: start_visibility * remaining.as_secs_f32()
                            / IDLE_FADE.as_secs_f32(),
                    });
                }
            }
            Phase::Fading { .. } | Phase::Finished => {}
        }
    }

    fn frame(&self, now: Instant) -> Option<SparkleFrame> {
        let elapsed = now.saturating_duration_since(self.started);
        match self.phase.get() {
            Phase::Waiting => {
                self.phase.set(Phase::Visible {
                    since: now,
                    elapsed,
                });
                Some(SparkleFrame {
                    elapsed,
                    visibility: 1.0,
                    next: FRAME_TICK,
                })
            }
            Phase::Visible {
                since,
                elapsed: last_visible_elapsed,
            } => {
                let remaining_idle =
                    IDLE_TIMEOUT.saturating_sub(now.saturating_duration_since(since));
                if remaining_idle.is_zero() {
                    // Keep the last rendered dots in place, even if this frame arrives late.
                    self.phase.set(Phase::Fading {
                        started: since + IDLE_TIMEOUT,
                        elapsed: last_visible_elapsed,
                        duration: IDLE_FADE,
                        start_visibility: 1.0,
                    });
                    return self.frame(now);
                }
                self.phase.set(Phase::Visible { since, elapsed });
                Some(SparkleFrame {
                    elapsed,
                    visibility: 1.0,
                    next: FRAME_TICK.min(remaining_idle),
                })
            }
            Phase::Fading {
                started,
                elapsed,
                duration,
                start_visibility,
            } => {
                let remaining_fade =
                    duration.saturating_sub(now.saturating_duration_since(started));
                if remaining_fade.is_zero() {
                    self.phase.set(Phase::Finished);
                    return None;
                }
                // Fade the last visible stars so neither timeout nor typing introduces dots.
                Some(SparkleFrame {
                    elapsed,
                    visibility: start_visibility * remaining_fade.as_secs_f32()
                        / duration.as_secs_f32(),
                    next: FADE_FRAME_TICK.min(remaining_fade),
                })
            }
            Phase::Finished => None,
        }
    }
}

impl BottomPane {
    pub(crate) fn set_astra_sparkle(&mut self, model: &str, settings: &Tui) {
        let changed = if !settings.whimsy || !settings.animations || !ASTRA_MODEL.is_match(model) {
            self.composer.astra_sparkle.take().is_some()
        } else if self.composer.astra_sparkle.is_none() {
            self.composer.astra_sparkle = Some(Sparkle {
                started: Instant::now(),
                phase: Cell::new(Phase::Waiting),
                terminal_focused: true,
            });
            true
        } else {
            false
        };
        if changed && let Some(requester) = &self.composer.frame_requester {
            requester.schedule_frame();
        }
    }

    pub(crate) fn set_sparkle_terminal_focus(&mut self, focused: bool) {
        if let Some(sparkle) = &mut self.composer.astra_sparkle {
            sparkle.terminal_focused = focused;
        }
    }
}

impl ChatComposer {
    pub(super) fn interact_with_astra_sparkle(&self) {
        if let Some(sparkle) = &self.astra_sparkle {
            sparkle.interact(Instant::now());
        }
    }

    pub(super) fn render_sparkle(
        &self,
        area: Rect,
        textarea: Rect,
        cursor: Option<(u16, u16)>,
        buf: &mut Buffer,
    ) {
        self.render_sparkle_at(area, textarea, cursor, Instant::now(), buf);
    }

    fn render_sparkle_at(
        &self,
        area: Rect,
        textarea: Rect,
        cursor: Option<(u16, u16)>,
        now: Instant,
        buf: &mut Buffer,
    ) {
        if (!self.is_empty()
            || self.draft.paste_burst.is_active()
            || !self.draft.input_enabled
            || self.voice_strip.is_some()
            || !matches!(self.popups.active, ActivePopup::None))
            && let Some(sparkle) = &self.astra_sparkle
        {
            sparkle.interact(now);
        }
        if !self.has_focus || area.height < 3 || textarea.is_empty() {
            return;
        }
        if let Some(sparkle) = &self.astra_sparkle
            && sparkle.terminal_focused
            && let Some(foreground) = sparkle.enabled_foreground()
            && let Some(frame) = sparkle.frame(now)
        {
            let protected_area = if !self.is_empty() || self.draft.paste_burst.is_active() {
                Rect::new(area.x, textarea.y, area.width, textarea.height)
            } else {
                let placeholder = if self.draft.input_enabled {
                    self.placeholder_text.as_str()
                } else {
                    self.draft
                        .input_disabled_placeholder
                        .as_deref()
                        .unwrap_or("Input disabled.")
                };
                // Spaces inside the placeholder are text too; the terminal can copy them.
                Rect::new(
                    textarea.x,
                    textarea.y,
                    placeholder.width().min(usize::from(textarea.width)) as u16,
                    /*height*/ 1,
                )
            };
            render_stars(
                area,
                cursor,
                Some(protected_area),
                frame.elapsed,
                foreground,
                frame.visibility,
                buf,
            );
            if let Some(requester) = &self.frame_requester {
                requester.schedule_frame_in(frame.next);
            }
        }
    }
}

fn render_stars(
    area: Rect,
    cursor: Option<(u16, u16)>,
    protected_area: Option<Rect>,
    elapsed: Duration,
    foreground: (u8, u8, u8),
    visibility: f32,
    buf: &mut Buffer,
) {
    let time = elapsed.as_secs_f32();
    for y in area.y..area.bottom() {
        let mut occupied_until = area.x;
        for x in area.x..area.right() {
            let cell = &buf[(x, y)];
            if x < occupied_until {
                continue;
            }
            if cell.symbol() != " " {
                occupied_until = x.saturating_add(cell.symbol().width() as u16);
                continue;
            }
            // Preserve the cursor, selections, and pixels from the effort bursts.
            if cursor == Some((x, y))
                || protected_area.is_some_and(|protected| protected.contains(Position::new(x, y)))
                || !cell.modifier.is_empty()
                || cell.diff_option != CellDiffOption::None
            {
                continue;
            }
            let Color::Rgb(r, g, b) = cell.bg else {
                continue;
            };
            // A stable coordinate hash gives each star its own dot, period, and phase.
            let mut hash = u64::from(y - area.y) * 65537 + u64::from(x - area.x);
            hash = (hash ^ (hash >> 16)).wrapping_mul(0x45d9f3b);
            hash = (hash ^ (hash >> 16)).wrapping_mul(0x45d9f3b);
            hash ^= hash >> 16;
            if hash % 5 != 0 {
                continue;
            }
            let phase =
                (time / (4.0 + (hash % 31) as f32 / 10.0) + (hash % 997) as f32 / 997.0).fract();
            let brightness = (phase * std::f32::consts::PI).sin().powi(12) * 0.55 * visibility;
            if brightness < 0.04 {
                continue;
            }
            buf[(x, y)]
                .set_symbol(DOTS[(hash / 161 % 8) as usize])
                .set_fg(rgb_color(blend(foreground, (r, g, b), brightness)));
        }
    }
}

#[cfg(test)]
#[path = "sparkle_tests.rs"]
mod tests;
