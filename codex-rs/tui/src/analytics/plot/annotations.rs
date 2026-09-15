//! Missing-day markers, daily-value callouts, and stable calendar labels.

use super::Renderer;
use super::amount;
use super::layout::Layout;
use super::painting::Band;
use crate::analytics::styles::secondary_style;
use crate::color::is_light;
use crate::style::accent_style;
use crate::terminal_palette::default_bg;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

impl Renderer<'_> {
    pub(super) fn annotate_missing(&self, buf: &mut Buffer) {
        let Layout {
            count,
            height,
            start,
            ..
        } = self.layout;
        for (index, parts) in self.values[start..start + count].iter().enumerate() {
            // The selected zero/missing value gets its own callout below; avoid repeating it.
            if start + index != self.cursor {
                let marker = match self.days[start + index].1 {
                    None => Some('—'),
                    Some(day) if day.total == 0.0 && parts.iter().all(|value| *value == 0.0) => {
                        Some('0')
                    }
                    Some(_) => None,
                };
                if let Some(marker) = marker {
                    buf[(self.layout.center(index) as u16, height as u16)]
                        .set_char(marker)
                        .set_style(Style::default().dim());
                }
            }
        }
    }

    pub(super) fn annotate_selection(&self, buf: &mut Buffer) {
        let Layout {
            axis_width,
            baseline,
            height,
            marker_row,
            plot_width,
            start,
            width,
            ..
        } = self.layout;
        // Give the selected value first choice of space; the other labels never overwrite it.
        for index in
            std::iter::once(self.cursor).chain((start..start + self.layout.count).filter(|index| {
                self.days.len() <= 7
                    && *index != self.cursor
                    && self.days[*index].1.is_some_and(|day| {
                        day.total != 0.0 || self.values[*index].iter().any(|value| *value != 0.0)
                    })
            }))
        {
            let total = self.days[index].1.map_or(/*default*/ 0.0, |day| day.total);
            let label = self.days[index]
                .1
                .map_or_else(|| "—".into(), |_| amount(total, self.unit));
            // A partial number can mean a different amount; leave the cursor when the value cannot fit.
            if label.width() > plot_width {
                continue;
            }
            let x = self.layout.center(index - start);
            let label_width = label.width();
            let label_x = x
                .saturating_sub(label_width / 2)
                .clamp(axis_width, width - label_width);
            let occupied = label_x.saturating_sub(/*rhs*/ 1).max(axis_width)
                ..(label_x + label_width + 1).min(width);
            let below = total < 0.0;
            let band = if below {
                Band::Negative
            } else {
                Band::Positive
            };
            let extent = self.values[index]
                .iter()
                .map(|value| band.amount(*value))
                .sum::<f64>();
            let rows = ((extent * height as f64 / self.peak).ceil() as usize).min(height);
            let cap = if below {
                baseline + rows
            } else {
                baseline - rows
            };
            let mut label_y = if below {
                cap + 1
            } else {
                cap.saturating_sub(/*rhs*/ 1)
            };
            // Keep the callout close without painting over a neighboring bar. A guide connects any gap.
            while occupied
                .clone()
                .any(|col| buf[(col as u16, label_y as u16)].symbol() != " ")
            {
                if below && label_y + 1 < marker_row {
                    label_y += 1;
                } else if !below && label_y > 0 {
                    label_y -= 1;
                } else {
                    break;
                }
            }
            if occupied
                .clone()
                .any(|col| buf[(col as u16, label_y as u16)].symbol() != " ")
            {
                continue;
            }
            // Use white on dark backgrounds and the terminal foreground on light backgrounds.
            let style = if index == self.cursor {
                let selected = Style::default().bold().not_dim();
                if default_bg().is_some_and(is_light) {
                    selected
                } else {
                    selected.white()
                }
            } else {
                Style::default().dim()
            };
            let guide = if below {
                cap + 1..label_y
            } else {
                label_y + 1..cap
            };
            for y in guide {
                let cell = &mut buf[(x as u16, y as u16)];
                if cell.symbol() == " " {
                    cell.set_char('┊').set_style(style);
                }
            }
            buf.set_line(
                label_x as u16,
                label_y as u16,
                &Line::from(label.set_style(style)),
                label_width as u16,
            );
        }
        let x = self.layout.center(self.cursor - start);
        buf[(x as u16, marker_row as u16)]
            .set_char('▲')
            .set_style(accent_style());
    }

    pub(super) fn annotate_calendar(&self, buf: &mut Buffer) {
        let Layout {
            axis_width,
            count,
            marker_row,
            plot_width,
            start,
            width,
            ..
        } = self.layout;
        // Calendar ticks depend only on the range/width, never on the current selection.
        let mut occupied = vec![false; width];
        // Give the two range endpoints priority over intermediate weekly ticks.
        for index in std::iter::once(/*value*/ 0).chain((1..count).rev()) {
            if self.days.len() > 7
                && !(start + index).is_multiple_of(/*rhs*/ 7)
                && index + 1 != count
            {
                continue;
            }
            let format = if self.days.len() <= 7 && plot_width / count < 7 {
                "%-d"
            } else {
                "%b %-d"
            };
            let mut label = self.days[start + index].0.format(format).to_string();
            if label.width() > plot_width {
                label = self.days[start + index].0.format("%-d").to_string();
            }
            if label.width() > plot_width {
                continue;
            }
            let offset = self
                .layout
                .center(index)
                .saturating_sub(label.len() / 2)
                .clamp(axis_width, width - label.len());
            if occupied[offset.saturating_sub(/*rhs*/ 1)..(offset + label.len() + 1).min(width)]
                .iter()
                .any(|used| *used)
            {
                continue;
            }
            occupied[offset..offset + label.len()].fill(/*value*/ true);
            let label = if start + index == self.cursor {
                label.set_style(accent_style()).underlined()
            } else {
                label.set_style(secondary_style())
            };
            buf.set_line(
                offset as u16,
                (marker_row + 1) as u16,
                &Line::from(label),
                (width - offset) as u16,
            );
        }
    }
}
