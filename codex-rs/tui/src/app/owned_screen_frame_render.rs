//! Rendering for frame-owned side-panel chrome.

use super::*;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::style::Styled;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

impl OwnedScreenFrameState {
    pub(in super::super) fn panel_body(&self, panel: OwnedScreenPanel) -> Option<Rect> {
        self.layout
            .and_then(|layout| layout.panel(panel))
            .map(panel_body)
    }

    pub(in super::super) fn render_panel_chrome(
        &self,
        panel: OwnedScreenPanel,
        title: &str,
        buffer: &mut Buffer,
    ) -> Option<Rect> {
        let panel_layout = self.layout.and_then(|layout| layout.panel(panel))?;
        let focused = self.focus.panel() == Some(panel);
        let title = self.panel_title(panel, title, focused);
        Clear.render(panel_layout.area, buffer);
        match panel_layout.presentation {
            OwnedScreenPanelPresentation::Docked => {
                let header_height = 1.min(panel_layout.area.height);
                let header_area = Rect::new(
                    panel_layout.area.x,
                    panel_layout.area.y,
                    panel_layout.area.width,
                    header_height,
                );
                Paragraph::new(title).render(header_area, buffer);
            }
            OwnedScreenPanelPresentation::Overlay => {
                let border_style = if focused {
                    Style::default().cyan().bold()
                } else {
                    Style::default().dim()
                };
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(border_style)
                    .render(panel_layout.area, buffer);
            }
        }
        Some(panel_body(panel_layout))
    }

    pub(in super::super) fn render_panel(
        &mut self,
        panel: OwnedScreenPanel,
        title: &str,
        lines: &[Line<'static>],
        buffer: &mut Buffer,
    ) {
        let Some(content_area) = self.render_panel_chrome(panel, title, buffer) else {
            return;
        };
        let state = self.panel_state_mut(panel);
        state.content_height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
        state.viewport_height = content_area.height;
        state.clamp_scroll();
        let visible = lines
            .iter()
            .skip(usize::from(state.scroll))
            .take(usize::from(content_area.height))
            .cloned()
            .collect::<Vec<_>>();
        Paragraph::new(visible).render(content_area, buffer);
    }

    fn panel_title(&self, panel: OwnedScreenPanel, title: &str, focused: bool) -> Line<'static> {
        if panel != OwnedScreenPanel::Summary {
            let title = if focused {
                title.to_string().cyan().bold()
            } else {
                title.to_string().bold()
            };
            return vec![" ".into(), title, " ".into()].into();
        }

        let selected_style = if focused {
            Style::default().cyan().bold()
        } else {
            Style::default().bold()
        };
        let inactive_style = Style::default().dim();
        let summary_style = if self.right_rail_content == OwnedScreenRightRailContent::Summary {
            selected_style
        } else {
            inactive_style
        };
        let browser_style = if self.right_rail_content == OwnedScreenRightRailContent::Browser {
            selected_style
        } else {
            inactive_style
        };
        vec![
            " ".into(),
            "Summary".set_style(summary_style),
            " | ".dim(),
            "Browser".set_style(browser_style),
            " ".into(),
        ]
        .into()
    }

    pub(in super::super) fn render_dividers(&self, buffer: &mut Buffer) {
        let Some(layout) = self.layout else {
            return;
        };
        for panel in [OwnedScreenPanel::Sidebar, OwnedScreenPanel::Summary] {
            let Some(area) = layout.divider(panel) else {
                continue;
            };
            let active = self.focus.panel() == Some(panel)
                || matches!(
                    self.interaction,
                    FrameInteraction::Resize {
                        panel: active_panel,
                        ..
                    } if active_panel == panel
                );
            let style = if active {
                Style::default().cyan().bold()
            } else {
                Style::default().dim()
            };
            for y in area.y..area.bottom() {
                buffer[(area.x, y)]
                    .set_symbol(if active { "┃" } else { "│" })
                    .set_style(style);
            }
        }
    }
}

fn panel_body(panel_layout: OwnedScreenPanelLayout) -> Rect {
    match panel_layout.presentation {
        OwnedScreenPanelPresentation::Docked => {
            let header_height = 1.min(panel_layout.area.height);
            Rect::new(
                panel_layout.area.x,
                panel_layout.area.y.saturating_add(header_height),
                panel_layout.area.width,
                panel_layout.area.height.saturating_sub(header_height),
            )
        }
        OwnedScreenPanelPresentation::Overlay => Block::default()
            .borders(Borders::ALL)
            .inner(panel_layout.area),
    }
}
