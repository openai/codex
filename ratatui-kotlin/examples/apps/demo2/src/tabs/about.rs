use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::widgets::{
    Block, Borders, Clear, MascotEyeColor, Padding, Paragraph, RatatuiMascot, Widget, Wrap,
};

use crate::{RgbSwatch, THEME};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AboutTab {
    row_index: usize,
}

impl AboutTab {
    pub fn prev_row(&mut self) {
        self.row_index = self.row_index.saturating_sub(1);
    }

    pub fn next_row(&mut self) {
        self.row_index = self.row_index.saturating_add(1);
    }
}

impl Widget for AboutTab {
    fn render(self, area: Rect, buf: &mut Buffer) {
        RgbSwatch.render(area, buf);
        let layout = Layout::horizontal([Constraint::Length(34), Constraint::Min(0)]);
        let [logo_area, description] = area.layout(&layout);
        render_crate_description(description, buf);
        let eye_state = if self.row_index % 2 == 0 {
            MascotEyeColor::Default
        } else {
            MascotEyeColor::Red
        };
        RatatuiMascot::default().set_eye(eye_state).render(
            logo_area.inner(Margin {
                vertical: 0,
                horizontal: 2,
            }),
            buf,
        );
    }
}

fn render_crate_description(area: Rect, buf: &mut Buffer) {
    let area = area.inner(Margin {
        vertical: 4,
        horizontal: 2,
    });
    Clear.render(area, buf); // clear out the color swatches
    Block::new().style(THEME.content).render(area, buf);
    let area = area.inner(Margin {
        vertical: 1,
        horizontal: 2,
    });
    let text = "- cooking up terminal user interfaces -

    Ratatui is a Rust crate that provides widgets (e.g. Paragraph, Table) and draws them to the \
    screen efficiently every frame.";
    Paragraph::new(text)
        .style(THEME.description)
        .block(
            Block::new()
                .title(" Ratatui ")
                .title_alignment(Alignment::Center)
                .borders(Borders::TOP)
                .border_style(THEME.description_title)
                .padding(Padding::new(0, 0, 0, 0)),
        )
        .wrap(Wrap { trim: true })
        .scroll((0, 0))
        .render(area, buf);
}
