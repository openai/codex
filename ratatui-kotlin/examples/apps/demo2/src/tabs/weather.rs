use itertools::Itertools;
use palette::Okhsv;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};
use ratatui::style::{Color, Style};
use ratatui::symbols;
use ratatui::widgets::calendar::{CalendarEventStore, Monthly};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, Clear, LineGauge, Padding, Widget};
use time::OffsetDateTime;

use crate::{RgbSwatch, THEME, color_from_oklab};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WeatherTab {
    pub download_progress: usize,
}

impl WeatherTab {
    /// Simulate a download indicator by decrementing the row index.
    pub fn prev(&mut self) {
        self.download_progress = self.download_progress.saturating_sub(1);
    }

    /// Simulate a download indicator by incrementing the row index.
    pub fn next(&mut self) {
        self.download_progress = self.download_progress.saturating_add(1);
    }
}

impl Widget for WeatherTab {
    fn render(self, area: Rect, buf: &mut Buffer) {
        RgbSwatch.render(area, buf);
        let area = area.inner(Margin {
            vertical: 1,
            horizontal: 2,
        });
        Clear.render(area, buf);
        Block::new().style(THEME.content).render(area, buf);

        let area = area.inner(Margin {
            horizontal: 2,
            vertical: 1,
        });
        let tab_layout = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ]);
        let [main, _, gauges] = area.layout(&tab_layout);
        let main_layout = Layout::horizontal([Constraint::Length(23), Constraint::Min(0)]);
        let [calendar, charts] = main.layout(&main_layout);
        let charts_layout = Layout::vertical([Constraint::Length(29), Constraint::Min(0)]);
        let [simple, horizontal] = charts.layout(&charts_layout);

        render_calendar(calendar, buf);
        render_simple_barchart(simple, buf);
        render_horizontal_barchart(horizontal, buf);
        render_gauge(self.download_progress, gauges, buf);
    }
}

fn render_calendar(area: Rect, buf: &mut Buffer) {
    let date = OffsetDateTime::now_utc().date();
    Monthly::new(date, CalendarEventStore::today(Style::new().red().bold()))
        .block(Block::new().padding(Padding::new(0, 0, 2, 0)))
        .show_month_header(Style::new().bold())
        .show_weekdays_header(Style::new().italic())
        .render(area, buf);
}

fn render_simple_barchart(area: Rect, buf: &mut Buffer) {
    let data = [
        ("Sat", 76),
        ("Sun", 69),
        ("Mon", 65),
        ("Tue", 67),
        ("Wed", 65),
        ("Thu", 69),
        ("Fri", 73),
    ];
    let data = data
        .into_iter()
        .map(|(label, value)| {
            Bar::default()
                .value(value)
                // This doesn't actually render correctly as the text is too wide for the bar
                // See https://github.com/ratatui/ratatui/issues/513 for more info
                // (the demo GIFs hack around this by hacking the calculation in bars.rs)
                .text_value(format!("{value}°"))
                .style(if value > 70 {
                    Style::new().fg(Color::Red)
                } else {
                    Style::new().fg(Color::Yellow)
                })
                .value_style(if value > 70 {
                    Style::new().fg(Color::Gray).bg(Color::Red).bold()
                } else {
                    Style::new().fg(Color::DarkGray).bg(Color::Yellow).bold()
                })
                .label(label)
        })
        .collect_vec();
    let group = BarGroup::default().bars(&data);
    BarChart::default()
        .data(group)
        .bar_width(3)
        .bar_gap(1)
        .render(area, buf);
}

fn render_horizontal_barchart(area: Rect, buf: &mut Buffer) {
    let bg = Color::Rgb(32, 48, 96);
    let data = [
        Bar::default().text_value("Winter 37-51").value(51),
        Bar::default().text_value("Spring 40-65").value(65),
        Bar::default().text_value("Summer 54-77").value(77),
        Bar::default()
            .text_value("Fall 41-71")
            .value(71)
            .value_style(Style::new().bold()), // current season
    ];
    let group = BarGroup::default().label("GPU").bars(&data);
    BarChart::default()
        .block(Block::new().padding(Padding::new(0, 0, 2, 0)))
        .direction(Direction::Horizontal)
        .data(group)
        .bar_gap(1)
        .bar_style(Style::new().fg(bg))
        .value_style(Style::new().bg(bg).fg(Color::Gray))
        .render(area, buf);
}

#[expect(clippy::cast_precision_loss)]
pub fn render_gauge(progress: usize, area: Rect, buf: &mut Buffer) {
    let percent = (progress * 3).min(100) as f64;

    render_line_gauge(percent, area, buf);
}

#[expect(clippy::cast_possible_truncation)]
fn render_line_gauge(percent: f64, area: Rect, buf: &mut Buffer) {
    // cycle color hue based on the percent for a neat effect yellow -> red
    let hue = 90.0 - (percent as f32 * 0.6);
    let value = Okhsv::max_value();
    let filled_color = color_from_oklab(hue, Okhsv::max_saturation(), value);
    let unfilled_color = color_from_oklab(hue, Okhsv::max_saturation(), value * 0.5);
    let label = if percent < 100.0 {
        format!("Downloading: {percent}%")
    } else {
        "Download Complete!".into()
    };
    LineGauge::default()
        .ratio(percent / 100.0)
        .label(label)
        .style(Style::new().light_blue())
        .filled_style(Style::new().fg(filled_color))
        .unfilled_style(Style::new().fg(unfilled_color))
        .filled_symbol(symbols::line::THICK_HORIZONTAL)
        .unfilled_symbol(symbols::line::THICK_HORIZONTAL)
        .render(area, buf);
}
