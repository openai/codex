use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::history_cell;
use crate::history_cell::PlainHistoryCell;
use crate::render::renderable::Renderable;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::path::PathBuf;

use super::BottomPane;
use super::SelectionAction;
use super::SelectionItem;
use super::SelectionViewParams;

const BASE_ISSUE_URL: &str = "https://github.com/openai/codex/issues/new?template=2-bug-report.yml";

pub(crate) struct FeedbackView;

impl FeedbackView {
    pub fn show(
        bottom_pane: &mut BottomPane,
        file_path: PathBuf,
        snapshot: codex_feedback::CodexLogSnapshot,
    ) {
        bottom_pane.show_selection_view(Self::selection_params(file_path, snapshot));
    }

    fn selection_params(
        file_path: PathBuf,
        snapshot: codex_feedback::CodexLogSnapshot,
    ) -> SelectionViewParams {
        let header = FeedbackHeader::new(file_path);

        let thread_id = snapshot.thread_id.clone();

        let upload_action_tread_id = thread_id.clone();
        let upload_action: SelectionAction = Box::new(move |tx: &AppEventSender| {
            match snapshot.upload_to_sentry() {
                Ok(()) => {
                    let issue_url = format!(
                        "{BASE_ISSUE_URL}&steps=Uploaded%20thread:%20{upload_action_tread_id}",
                    );
                    tx.send(AppEvent::InsertHistoryCell(Box::new(PlainHistoryCell::new(vec![
                        Line::from(
                            "• Codex logs uploaded. Please open an issue using the following URL:",
                        ),
                        "".into(),
                        Line::from(vec!["  ".into(), issue_url.cyan().underlined()]),
                        "".into(),
                        Line::from(vec!["  Or mention your thread ID ".into(), upload_action_tread_id.clone().bold(),  " in an existing issue.".into()])
                    ]))));
                }
                Err(e) => {
                    tx.send(AppEvent::InsertHistoryCell(Box::new(
                        history_cell::new_error_event(format!("Failed to upload logs: {e}")),
                    )));
                }
            }
        });

        let upload_item = SelectionItem {
            name: "Yes".to_string(),
            description: Some(
                "Share the current Codex session logs with the team for troubleshooting."
                    .to_string(),
            ),
            actions: vec![upload_action],
            dismiss_on_select: true,
            ..Default::default()
        };

        let no_action: SelectionAction = Box::new(move |tx: &AppEventSender| {
            let issue_url = format!("{BASE_ISSUE_URL}&steps=Thread%20ID:%20{thread_id}",);

            tx.send(AppEvent::InsertHistoryCell(Box::new(
                PlainHistoryCell::new(vec![
                    Line::from("• Please open an issue using the following URL:"),
                    "".into(),
                    Line::from(vec!["  ".into(), issue_url.cyan().underlined()]),
                    "".into(),
                    Line::from(vec![
                        "  Or mention your thread ID ".into(),
                        thread_id.clone().bold(),
                        " in an existing issue.".into(),
                    ]),
                ]),
            )));
        });

        let no_item = SelectionItem {
            name: "No".to_string(),
            actions: vec![no_action],
            dismiss_on_select: true,
            ..Default::default()
        };

        let cancel_item = SelectionItem {
            name: "Cancel".to_string(),
            dismiss_on_select: true,
            ..Default::default()
        };

        SelectionViewParams {
            header: Box::new(header),
            items: vec![upload_item, no_item, cancel_item],
            ..Default::default()
        }
    }
}

struct FeedbackHeader {
    file_path: PathBuf,
}

impl FeedbackHeader {
    fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    fn lines(&self) -> Vec<Line<'static>> {
        vec![
            Line::from("Do you want to upload logs before reporting issue?".bold()),
            "".into(),
            Line::from(
                "Logs may include the full conversation history of this Codex process, including prompts, tool calls, and their results.",
            ),
            Line::from(
                "These logs are retained for 90 days and are used solely for troubleshooting and diagnostic purposes.",
            ),
            "".into(),
            Line::from(vec![
                "You can review the exact content of the logs before they’re uploaded at:".into(),
            ]),
            Line::from(self.file_path.display().to_string().dim()),
            "".into(),
        ]
    }
}

impl Renderable for FeedbackHeader {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        for (i, line) in self.lines().into_iter().enumerate() {
            let y = area.y.saturating_add(i as u16);
            if y >= area.y.saturating_add(area.height) {
                break;
            }
            let line_area = Rect::new(area.x, y, area.width, 1).intersection(area);
            line.render(line_area, buf);
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.lines()
            .iter()
            .map(|line| line.desired_height(width))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::bottom_pane::list_selection_view::ListSelectionView;
    use crate::style::user_message_style;
    use codex_feedback::CodexFeedback;
    use codex_protocol::ConversationId;
    use insta::assert_snapshot;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Color;
    use tokio::sync::mpsc::unbounded_channel;

    fn buffer_to_string(buffer: &Buffer) -> String {
        (0..buffer.area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..buffer.area.width {
                    let symbol = buffer[(buffer.area.x + col, buffer.area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_feedback_view_header() {
        let (tx_raw, _rx) = unbounded_channel::<AppEvent>();
        let app_event_tx = AppEventSender::new(tx_raw);
        let snapshot = CodexFeedback::new().snapshot(Some(
            ConversationId::from_string("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        ));
        let file_path = PathBuf::from("/tmp/codex-feedback.log");

        let params = FeedbackView::selection_params(file_path.clone(), snapshot);
        let view = ListSelectionView::new(params, app_event_tx);

        let width = 72;
        let height = view.desired_height(width).max(1);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let rendered =
            buffer_to_string(&buf).replace(&file_path.display().to_string(), "<LOG_PATH>");
        assert_snapshot!("feedback_view_render", rendered);

        let cell_style = buf[(area.x, area.y)].style();
        let expected_bg = user_message_style().bg.unwrap_or(Color::Reset);
        assert_eq!(cell_style.bg.unwrap_or(Color::Reset), expected_bg);
    }
}
