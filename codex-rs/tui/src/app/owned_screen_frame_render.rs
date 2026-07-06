//! Rendering for frame-owned side-panel chrome.

use super::*;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

impl OwnedScreenFrameState {
    pub(in super::super) fn render_panel(
        &mut self,
        panel: OwnedScreenPanel,
        title: &str,
        lines: &[Line<'static>],
        buffer: &mut Buffer,
    ) {
        let Some(panel_layout) = self.layout.and_then(|layout| layout.panel(panel)) else {
            return;
        };
        let focused = self.focus.panel() == Some(panel);
        let content_area = match panel_layout.presentation {
            OwnedScreenPanelPresentation::Docked => {
                Clear.render(panel_layout.area, buffer);
                let header_height = 1.min(panel_layout.area.height);
                let header_area = Rect::new(
                    panel_layout.area.x,
                    panel_layout.area.y,
                    panel_layout.area.width,
                    header_height,
                );
                let header: Line<'static> = if focused {
                    vec![" ".into(), title.to_string().cyan().bold()].into()
                } else {
                    vec![" ".into(), title.to_string().bold()].into()
                };
                Paragraph::new(header).render(header_area, buffer);
                Rect::new(
                    panel_layout.area.x,
                    panel_layout.area.y.saturating_add(header_height),
                    panel_layout.area.width,
                    panel_layout.area.height.saturating_sub(header_height),
                )
            }
            OwnedScreenPanelPresentation::Overlay => {
                Clear.render(panel_layout.area, buffer);
                let border_style = if focused {
                    Style::default().cyan().bold()
                } else {
                    Style::default().dim()
                };
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {title} "))
                    .border_style(border_style);
                let content_area = block.inner(panel_layout.area);
                block.render(panel_layout.area, buffer);
                content_area
            }
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
