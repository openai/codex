//! Reports command-center action failures above the retained dashboard.

use super::*;
use crate::wrapping::word_wrap_lines;
use ratatui::buffer::Buffer;

struct AgentsOverviewErrorHeader(Vec<Line<'static>>);

impl Renderable for AgentsOverviewErrorHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        Renderable::render(
            &Paragraph::new(word_wrap_lines(&self.0, usize::from(area.width))),
            area,
            buf,
        );
    }

    fn desired_height(&self, width: u16) -> u16 {
        word_wrap_lines(&self.0, usize::from(width)).len() as u16
    }
}

impl App {
    pub(in crate::app) fn add_agents_overview_error(&mut self, message: String) {
        if self
            .chat_widget
            .selected_index_for_present_view(AGENTS_OVERVIEW_VIEW_ID)
            .is_some()
        {
            let items = vec![SelectionItem {
                name: "Return to command center".to_string(),
                dismiss_on_select: true,
                ..Default::default()
            }];
            self.chat_widget.show_selection_view(SelectionViewParams {
                header: Box::new(AgentsOverviewErrorHeader(vec![
                    Line::from("Unable to complete action".bold()),
                    Line::from(message.clone().dim()),
                ])),
                items,
                ..Default::default()
            });
        }
        self.chat_widget.add_error_message(message);
    }
}
