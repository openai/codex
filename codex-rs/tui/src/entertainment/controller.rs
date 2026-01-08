use std::collections::VecDeque;

use ratatui::text::Line;
use tracing::info;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell::HistoryCell;

const PROMPT_TEMPLATE: &str = include_str!("prompt.md");
const HISTORY_LIMIT: usize = 10;

#[derive(Debug)]
pub(crate) struct EntertainmentController {
    enabled: bool,
    history: VecDeque<String>,
}

impl EntertainmentController {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            history: VecDeque::new(),
        }
    }

    pub(crate) fn record_history_cell(&mut self, cell: &dyn HistoryCell) {
        let lines = cell.transcript_lines(u16::MAX);
        let text = render_lines(&lines).join("\n");
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.history.push_back(text.to_string());
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
    }

    pub(crate) fn request_generation(&self, app_event_tx: &AppEventSender) {
        if !self.enabled {
            return;
        }
        let prompt = self.build_prompt();
        info!(
            history_len = self.history.len(),
            prompt_len = prompt.len(),
            "requesting entertainment text generation"
        );
        app_event_tx.send(AppEvent::GenerateEntertainmentTexts { prompt });
    }

    fn build_prompt(&self) -> String {
        let history = if self.history.is_empty() {
            "- (no recent history)".to_string()
        } else {
            let mut out = String::new();
            for entry in &self.history {
                out.push_str("- ");
                out.push_str(entry);
                out.push('\n');
            }
            out.trim_end().to_string()
        };

        PROMPT_TEMPLATE.replace("{{INSERT_CONTEXT_HERE}}", &history)
    }
}

fn render_lines(lines: &[Line<'static>]) -> Vec<String> {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect()
}
