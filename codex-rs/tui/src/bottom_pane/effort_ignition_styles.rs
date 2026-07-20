use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;

use crate::color::blend;
use crate::terminal_palette::StdoutColorLevel;
use crate::terminal_palette::best_color_for_level;

use super::EffortTier;
use super::IgnitionStyle;

const WAVE_HALF_WIDTH: f32 = 9.0;
const PULSE_HALF_WIDTH: f32 = 4.5;
const SPARK_START: Duration = Duration::from_millis(900);
const SPARK_FRAME: Duration = Duration::from_millis(100);
const SPARK_GLYPHS: &[&str] = &["·", "✦", "✧"];

/// Band entries are `(launch_or_speed, travel_or_phase, strength_or_hue)`.
type Band = (f32, f32, f32);

fn bands(style: IgnitionStyle, tier: EffortTier) -> &'static [Band] {
    match (style, tier) {
        (IgnitionStyle::Wave, EffortTier::Max) => &[(0.10, 0.75, 1.0)],
        (IgnitionStyle::Wave, EffortTier::Ultra) => &[(0.10, 0.70, 1.0), (0.35, 0.55, 1.0)],
        (IgnitionStyle::Aurora, EffortTier::Max) => &[(0.35, 0.15, 0.0), (-0.50, 0.60, 1.0)],
        (IgnitionStyle::Aurora, EffortTier::Ultra) => {
            &[(0.35, 0.15, 0.0), (-0.50, 0.60, 1.0), (0.75, 0.35, 2.0)]
        }
        (IgnitionStyle::Pulse, EffortTier::Max) => &[(0.10, 0.60, 1.0)],
        (IgnitionStyle::Pulse, EffortTier::Ultra) => &[(0.10, 0.55, 0.8), (0.45, 0.55, 1.1)],
    }
}

/// Paint target clipped to the composer band. Backgrounds blend from the band
/// tint; glyphs only land outside the protected draft and attachment rows.
pub(super) struct Canvas<'a> {
    pub(super) area: Rect,
    pub(super) protected: Rect,
    pub(super) buf: &'a mut Buffer,
    pub(super) band_rgb: (u8, u8, u8),
    pub(super) color_level: StdoutColorLevel,
}

impl Canvas<'_> {
    fn tint(&mut self, x: u16, y: u16, hue: (u8, u8, u8), alpha: f32) {
        if alpha < 0.02 || x >= self.area.width || y >= self.area.height {
            return;
        }
        let color = best_color_for_level(
            blend(hue, self.band_rgb, alpha.clamp(0.0, 0.6)),
            self.color_level,
        );
        if color != Color::default() {
            self.buf[(self.area.x + x, self.area.y + y)].set_bg(color);
        }
    }

    fn tint_column(&mut self, x: u16, hue: (u8, u8, u8), alpha: f32) {
        for y in 0..self.area.height {
            self.tint(x, y, hue, alpha);
        }
    }

    fn glyph(&mut self, x: u16, y: u16, glyph: &'static str, hue: (u8, u8, u8), strength: f32) {
        if x >= self.area.width || y >= self.area.height {
            return;
        }
        let x = self.area.x + x;
        let y = self.area.y + y;
        if self.protected.contains((x, y).into()) || self.buf[(x, y)].symbol() != " " {
            return;
        }
        let color = best_color_for_level(
            blend(hue, self.band_rgb, strength.clamp(0.25, 1.0)),
            self.color_level,
        );
        if color != Color::default() {
            self.buf[(x, y)]
                .set_symbol(glyph)
                .set_style(Style::default().fg(color).add_modifier(Modifier::BOLD));
        }
    }
}

fn crest(distance: f32) -> f32 {
    if distance >= 1.0 {
        0.0
    } else {
        0.5 * (1.0 + (std::f32::consts::PI * distance).cos())
    }
}

fn ease_in_out(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    if progress < 0.5 {
        4.0 * progress * progress * progress
    } else {
        let inverse = -2.0 * progress + 2.0;
        1.0 - inverse * inverse * inverse / 2.0
    }
}

pub(super) fn envelope(elapsed: f32, total: f32, fade_in: f32, fade_out: f32) -> f32 {
    if elapsed <= 0.0 || elapsed >= total {
        return 0.0;
    }
    (elapsed / fade_in.max(f32::EPSILON))
        .min((total - elapsed) / fade_out.max(f32::EPSILON))
        .clamp(0.0, 1.0)
}

fn band_sample(
    style: IgnitionStyle,
    band: &Band,
    elapsed: f32,
    column: u16,
    width: u16,
) -> (usize, f32) {
    let column = f32::from(column);
    let width = f32::from(width);
    let (first, second, third) = *band;
    match style {
        IgnitionStyle::Wave => {
            let (launch, travel) = (first, second);
            let progress = (elapsed - launch) / travel;
            if !(0.0..=1.0).contains(&progress) {
                return (0, 0.0);
            }
            let center = ease_in_out(progress) * (width + 2.0 * WAVE_HALF_WIDTH) - WAVE_HALF_WIDTH;
            (0, crest((column - center).abs() / WAVE_HALF_WIDTH))
        }
        IgnitionStyle::Aurora => {
            let (speed, phase, hue) = (first, second, third as usize);
            let center =
                (0.5 + 0.38 * (std::f32::consts::TAU * (speed * elapsed + phase)).sin()) * width;
            let half_width = (width * 0.22).max(4.0);
            (hue, crest((column - center).abs() / half_width))
        }
        IgnitionStyle::Pulse => {
            let (launch, travel, strength) = (first, second, third);
            let progress = (elapsed - launch) / travel;
            if !(0.0..=1.0).contains(&progress) {
                return (0, 0.0);
            }
            let inverse = 1.0 - progress;
            let radius =
                (1.0 - inverse * inverse * inverse) * (width / 2.0 + 2.0 * PULSE_HALF_WIDTH);
            let distance = (column - width / 2.0).abs();
            (
                0,
                crest((distance - radius).abs() / PULSE_HALF_WIDTH)
                    * strength
                    * (1.0 - 0.6 * progress),
            )
        }
    }
}

pub(super) fn spark_frame(elapsed: Duration, start: Duration) -> Option<&'static str> {
    let frame = elapsed.checked_sub(start)?.as_millis() / SPARK_FRAME.as_millis();
    SPARK_GLYPHS.get(frame as usize).copied()
}

fn paint_bands(
    tier: EffortTier,
    style: IgnitionStyle,
    elapsed: Duration,
    hues: [(u8, u8, u8); 3],
    canvas: &mut Canvas<'_>,
) {
    let elapsed = elapsed.as_secs_f32();
    let total = style.total_duration(tier).as_secs_f32();
    let fade = if style == IgnitionStyle::Aurora {
        envelope(
            elapsed, total, /*fade_in*/ 0.25, /*fade_out*/ 0.40,
        )
    } else {
        1.0
    };
    for column in 0..canvas.area.width {
        let mut weights = [0.0_f32; 3];
        for band in bands(style, tier) {
            let (hue, strength) = band_sample(style, band, elapsed, column, canvas.area.width);
            weights[hue] = if style == IgnitionStyle::Aurora {
                weights[hue] + strength
            } else {
                weights[hue].max(strength)
            };
        }
        let weight = weights.iter().sum::<f32>();
        if weight <= 0.01 {
            continue;
        }
        let mut rgb = [0.0_f32; 3];
        for (weight, (red, green, blue)) in weights.into_iter().zip(hues) {
            rgb[0] += weight * f32::from(red);
            rgb[1] += weight * f32::from(green);
            rgb[2] += weight * f32::from(blue);
        }
        let hue = (
            (rgb[0] / weight) as u8,
            (rgb[1] / weight) as u8,
            (rgb[2] / weight) as u8,
        );
        let alpha = if style == IgnitionStyle::Aurora {
            (weight * 0.40).min(0.50) * fade
        } else {
            weight * 0.55
        };
        canvas.tint_column(column, hue, alpha);
    }
}

pub(super) fn paint_style(
    tier: EffortTier,
    style: IgnitionStyle,
    elapsed: Duration,
    hues: [(u8, u8, u8); 3],
    canvas: &mut Canvas<'_>,
) {
    paint_bands(tier, style, elapsed, hues, canvas);
    if style == IgnitionStyle::Wave
        && tier == EffortTier::Ultra
        && let Some(glyph) = spark_frame(elapsed, SPARK_START)
    {
        canvas.glyph(
            canvas.area.width.saturating_sub(2),
            /*y*/ 0,
            glyph,
            hues[0],
            /*strength*/ 1.0,
        );
    }
}
