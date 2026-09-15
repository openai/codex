//! Paint signed bar cells and numeric axes using the shared layout and scale.

use super::Renderer;
use super::layout::Layout;
use super::tick;
use crate::analytics::styles::secondary_style;
use crate::analytics::styles::series_colors;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::text::Line;

/// A signed stack grows away from the baseline; amounts are nonnegative within each band.
#[derive(Clone, Copy)]
pub(super) enum Band {
    Positive,
    Negative,
}

impl Band {
    pub(super) fn amount(self, value: f64) -> f64 {
        match self {
            Self::Positive => value,
            Self::Negative => -value,
        }
        .max(/*other*/ 0.0)
    }

    fn row_y(self, baseline: usize, row: usize) -> usize {
        match self {
            Self::Positive => baseline - row - 1,
            Self::Negative => baseline + row + 1,
        }
    }

    fn partial_glyph(self, level: usize) -> char {
        // Reversing the complementary lower block paints a top-anchored partial refund.
        let index = match self {
            Self::Negative if level < 8 => 8 - level,
            Self::Positive | Self::Negative => level,
        };
        [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][index]
    }
}

impl Renderer<'_> {
    pub(super) fn paint_bars(&self, buf: &mut Buffer) {
        let Layout {
            axis_width,
            baseline,
            count,
            height,
            plot_width,
            start,
            ..
        } = self.layout;
        let colors = series_colors();
        for (index, parts) in self.values[start..start + count].iter().enumerate() {
            let left = axis_width + index * plot_width / count;
            let step = (index + 1) * plot_width / count - index * plot_width / count;
            let bar_width = (step * 2 / 3).clamp(/*min*/ 1, /*max*/ 12);
            let x = left + (step - bar_width) / 2;
            for band in self.bands {
                let plotted = parts.iter().map(|value| band.amount(*value)).sum::<f64>();
                let scaled = ((plotted * (height * 8) as f64 / self.peak) as usize)
                    .max(usize::from(plotted > 0.0));
                for row in 0..height {
                    let level = scaled.saturating_sub(row * 8).min(/*other*/ 8);
                    if level == 0 {
                        continue;
                    }
                    let reverse = matches!(band, Band::Negative) && level < 8;
                    let glyph = band.partial_glyph(level);
                    let bottom = (row * 8) as f64 * self.peak / (height * 8) as f64;
                    let top =
                        ((row * 8 + level) as f64 * self.peak / (height * 8) as f64).min(plotted);
                    let sample = bottom + (top - bottom) / 2.0;
                    let mut cumulative = 0.0;
                    let color = parts
                        .iter()
                        .position(|value| {
                            cumulative += band.amount(*value);
                            sample < cumulative
                        })
                        .unwrap_or(/*default*/ 3);
                    let y = band.row_y(baseline, row);
                    for col in x..x + bar_width {
                        let cell = &mut buf[(col as u16, y as u16)];
                        cell.set_char(glyph).set_fg(colors[color]);
                        if reverse {
                            cell.set_style(Style::default().reversed());
                        }
                    }
                }
            }
        }
    }

    pub(super) fn paint_axis(&self, buf: &mut Buffer) {
        let Layout {
            axis_width,
            baseline,
            height,
            width,
            ..
        } = self.layout;
        for x in axis_width..width {
            buf[(x as u16, baseline as u16)]
                .set_char('─')
                .set_style(Style::default().dim());
        }
        if axis_width > 0 {
            let mut ticks = vec![(1, self.peak), (baseline, 0.0)];
            if height >= 4 {
                ticks.push((1 + height / 2, self.peak / 2.0));
            }
            if self.bands.len() == 2 {
                ticks.push((baseline + height, -self.peak));
            }
            for (y, value) in ticks {
                let label = tick(value);
                let x = axis_width.saturating_sub(label.len() + 1);
                buf.set_line(
                    x as u16,
                    y as u16,
                    &Line::from(label.set_style(secondary_style())),
                    axis_width as u16,
                );
            }
        }
    }
}
