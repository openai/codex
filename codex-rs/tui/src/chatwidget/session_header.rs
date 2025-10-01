use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::ui_consts::LIVE_PREFIX_COLS;

pub(crate) struct SessionHeader {
    model: String,
    background_process_count: usize,
    latest_background_event: Option<String>,
}

impl SessionHeader {
    pub(crate) fn new(model: String) -> Self {
        Self {
            model,
            background_process_count: 0,
            latest_background_event: None,
        }
    }

    /// Updates the header's model text.
    pub(crate) fn set_model(&mut self, model: &str) {
        if self.model != model {
            self.model = model.to_string();
        }
    }

    /// Returns `true` when the rendered header should occupy vertical space.
    pub(crate) fn desired_height(&self, width: u16) -> u16 {
        if width == 0 {
            return 0;
        }
        if self.should_show() { 1 } else { 0 }
    }

    pub(crate) fn set_background_process_count(&mut self, count: usize) -> bool {
        if self.background_process_count == count {
            return false;
        }
        self.background_process_count = count;
        true
    }

    pub(crate) fn set_latest_background_event(&mut self, message: String) -> bool {
        let trimmed = message.trim();
        let next = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };

        if self.latest_background_event == next {
            return false;
        }
        self.latest_background_event = next;
        true
    }

    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.is_empty() || !self.should_show() {
            return;
        }

        let line = Line::from(self.build_spans());
        Paragraph::new(line).render_ref(area, buf);
    }

    fn should_show(&self) -> bool {
        self.background_process_count > 0 || self.latest_background_event.is_some()
    }

    fn build_spans(&self) -> Vec<Span<'static>> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        spans.push(" ".repeat(LIVE_PREFIX_COLS as usize).into());

        let mut has_segment = false;

        if !self.model.is_empty() {
            self.push_segment(
                &mut spans,
                &mut has_segment,
                vec!["Model:".dim(), " ".into(), self.model.clone().bold()],
            );
        }

        if self.background_process_count > 0 {
            let plural = if self.background_process_count == 1 {
                "process"
            } else {
                "processes"
            };
            let label = format!("background: {} {plural}", self.background_process_count);
            self.push_segment(&mut spans, &mut has_segment, vec![label.magenta()]);
        }

        if let Some(event) = self.latest_background_event.as_ref() {
            let text = format!("last: {event}");
            self.push_segment(&mut spans, &mut has_segment, vec![text.dim()]);
        }

        spans
    }

    fn push_segment(
        &self,
        spans: &mut Vec<Span<'static>>,
        has_segment: &mut bool,
        mut segment: Vec<Span<'static>>,
    ) {
        if segment.is_empty() {
            return;
        }
        if *has_segment {
            spans.push("  • ".dim());
        }
        spans.append(&mut segment);
        *has_segment = true;
    }
}
