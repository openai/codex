//! One-shot status-line transition shown when reasoning effort changes to Max
//! or Ultra.
//!
//! The previous status line slides to the right while it picks up the tier
//! accent and fades away. The tier letters then appear across the row with
//! wider peripheral gaps, converge into a centered `M A X` or `U L T R A`, and
//! fade out before the refreshed status line fades in. The clock starts only
//! when the passive footer row is rendered, so a picker, flash, or
//! instructional footer cannot consume the animation before it becomes
//! visible. ANSI-16 and unknown-color terminals skip the transition and keep
//! the normal status line; browser-backed and native terminals with ANSI-256
//! or truecolor can show the full effect.

use std::cell::Cell;
use std::time::Duration;
use std::time::Instant;

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

use crate::color::blend;
use crate::color::is_light;
use crate::line_truncation::truncate_line_to_width;
use crate::terminal_palette::best_color;
use crate::terminal_palette::default_bg;
use crate::terminal_palette::default_fg;

use super::effort_ignition::EffortTier;

const SCROLL_OUT: Duration = Duration::from_millis(620);
const LABEL_ASSEMBLE: Duration = Duration::from_millis(700);
const LABEL_HOLD: Duration = Duration::from_millis(360);
const LABEL_FADE_OUT: Duration = Duration::from_millis(340);
const STATUS_FADE_IN: Duration = Duration::from_millis(480);
const TOTAL: Duration = SCROLL_OUT
    .saturating_add(LABEL_ASSEMBLE)
    .saturating_add(LABEL_HOLD)
    .saturating_add(LABEL_FADE_OUT)
    .saturating_add(STATUS_FADE_IN);

pub(crate) const EFFORT_STATUS_LINE_FRAME_TICK: Duration = Duration::from_millis(33);

impl EffortTier {
    fn label(self) -> &'static str {
        match self {
            Self::Max => "MAX",
            Self::Ultra => "ULTRA",
        }
    }

    fn fallback_color(self) -> Color {
        match self {
            Self::Max => Color::Yellow,
            Self::Ultra => Color::Magenta,
        }
    }
}

pub(crate) struct EffortStatusLineTransition {
    tier: EffortTier,
    previous: Line<'static>,
    started_at: Cell<Option<Instant>>,
}

impl EffortStatusLineTransition {
    pub(crate) fn new(tier: EffortTier, previous: Line<'static>) -> Self {
        Self {
            tier,
            previous,
            started_at: Cell::new(None),
        }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.started_at
            .get()
            .is_some_and(|started| started.elapsed() >= TOTAL)
    }

    pub(crate) fn render_line(
        &self,
        current: Option<&Line<'static>>,
        width: u16,
    ) -> Option<Line<'static>> {
        let elapsed = match self.started_at.get() {
            Some(started) => started.elapsed(),
            None => {
                self.started_at.set(Some(Instant::now()));
                Duration::ZERO
            }
        };
        transition_line_at(self.tier, Some(&self.previous), current, elapsed, width)
    }
}

fn transition_line_at(
    tier: EffortTier,
    previous: Option<&Line<'static>>,
    current: Option<&Line<'static>>,
    elapsed: Duration,
    width: u16,
) -> Option<Line<'static>> {
    let width = usize::from(width);
    if width == 0 {
        return None;
    }

    if elapsed < SCROLL_OUT {
        let progress = elapsed.as_secs_f32() / SCROLL_OUT.as_secs_f32();
        let offset = (width as f32 * ease_in_cubic(progress)).round() as usize;
        let mut line = previous.cloned()?;
        let tint = 0.12 + 0.88 * progress;
        style_line(
            &mut line,
            tier,
            /*opacity*/ 1.0 - progress * progress,
            tint,
        );
        line.spans
            .insert(/*index*/ 0, Span::raw(" ".repeat(offset)));
        return Some(truncate_line_to_width(line, width));
    }

    let after_scroll = elapsed.saturating_sub(SCROLL_OUT);
    let label_duration = LABEL_ASSEMBLE
        .saturating_add(LABEL_HOLD)
        .saturating_add(LABEL_FADE_OUT);
    if after_scroll < label_duration {
        let (assemble, opacity) = if after_scroll < LABEL_ASSEMBLE {
            let progress = after_scroll.as_secs_f32() / LABEL_ASSEMBLE.as_secs_f32();
            (ease_out_cubic(progress), (progress / 0.55).clamp(0.0, 1.0))
        } else if after_scroll < LABEL_ASSEMBLE.saturating_add(LABEL_HOLD) {
            (1.0, 1.0)
        } else {
            let fade = after_scroll.saturating_sub(LABEL_ASSEMBLE.saturating_add(LABEL_HOLD));
            (1.0, 1.0 - fade.as_secs_f32() / LABEL_FADE_OUT.as_secs_f32())
        };
        return Some(tier_label_line(tier, width, assemble, opacity));
    }

    let fade_elapsed = after_scroll.saturating_sub(label_duration);
    let opacity = (fade_elapsed.as_secs_f32() / STATUS_FADE_IN.as_secs_f32()).clamp(0.0, 1.0);
    let mut line = current.cloned()?;
    style_line(&mut line, tier, opacity, /*tint*/ 0.0);
    Some(truncate_line_to_width(line, width))
}

fn ease_in_cubic(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    progress * progress * progress
}

fn ease_out_cubic(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    let inverse = 1.0 - progress;
    1.0 - inverse * inverse * inverse
}

fn tier_label_line(tier: EffortTier, width: usize, assemble: f32, opacity: f32) -> Line<'static> {
    let letters = tier.label().chars().collect::<Vec<_>>();
    let gap_count = letters.len().saturating_sub(1);
    let compact_width = letters.len().saturating_add(gap_count);
    let max_extra = width.saturating_sub(compact_width);
    let spread = (max_extra as f32 * (1.0 - assemble.clamp(0.0, 1.0))).round() as usize;
    let gap_weights = (0..gap_count)
        .map(|index| {
            let position = index.saturating_mul(2).saturating_add(1);
            position.abs_diff(gap_count).saturating_add(1)
        })
        .collect::<Vec<_>>();
    let weight_total = gap_weights.iter().sum::<usize>().max(1);
    let gaps = gap_weights
        .iter()
        .map(|weight| 1 + spread.saturating_mul(*weight) / weight_total)
        .collect::<Vec<_>>();
    let label_width = letters.len().saturating_add(gaps.iter().sum::<usize>());
    let left_padding = width.saturating_sub(label_width) / 2;
    let mut spans = vec![Span::raw(" ".repeat(left_padding))];

    for (index, letter) in letters.iter().enumerate() {
        let mut style = Style::default().add_modifier(Modifier::BOLD);
        let center = letters.len().saturating_sub(1);
        let edge = if center == 0 {
            0.0
        } else {
            index.saturating_mul(2).abs_diff(center) as f32 / center as f32
        };
        let stagger = 0.22 * edge;
        let letter_opacity = ((opacity - stagger) / (1.0 - stagger)).clamp(0.0, 1.0);
        apply_fade(&mut style, tier, letter_opacity, /*tint*/ 1.0);
        spans.push(Span::styled(letter.to_string(), style));
        if let Some(gap) = gaps.get(index) {
            spans.push(Span::raw(" ".repeat(*gap)));
        }
    }

    truncate_line_to_width(Line::from(spans), width)
}

fn style_line(line: &mut Line<'static>, tier: EffortTier, opacity: f32, tint: f32) {
    apply_fade(&mut line.style, tier, opacity, tint);
    for span in &mut line.spans {
        apply_fade(&mut span.style, tier, opacity, tint);
    }
}

fn apply_fade(style: &mut Style, tier: EffortTier, opacity: f32, tint: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    let tint = tint.clamp(0.0, 1.0);
    if matches!(style.fg, Some(color) if !matches!(color, Color::Rgb(..))) {
        if opacity < 0.7 {
            style.add_modifier |= Modifier::DIM;
        }
        return;
    }
    let Some(background) = default_bg() else {
        if tint > 0.5 {
            style.fg = Some(tier.fallback_color());
        }
        if opacity < 0.7 {
            style.add_modifier |= Modifier::DIM;
        }
        return;
    };
    let foreground = match style.fg {
        Some(Color::Rgb(r, g, b)) => (r, g, b),
        _ => default_fg().unwrap_or_else(|| {
            if is_light(background) {
                (32, 32, 32)
            } else {
                (224, 224, 224)
            }
        }),
    };
    let accent = tier.accent_rgb(is_light(background));
    let tinted = blend(accent, foreground, tint);
    let faded = blend(tinted, background, opacity);
    let color = best_color(faded);
    style.fg = Some(if color == Color::default() {
        tier.fallback_color()
    } else {
        color
    });
    if opacity < 0.7 {
        style.add_modifier |= Modifier::DIM;
    }
}

#[cfg(test)]
#[path = "effort_status_line_tests.rs"]
mod tests;
