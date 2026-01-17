//! Bottom-pane UI for collecting feedback notes and driving the feedback flow.
//!
//! This module renders the overlay used to capture an optional feedback note,
//! maps selected categories to backend classifications, and emits [`AppEvent`]s
//! that advance the flow (category → consent → note → upload). It owns only the
//! transient UI state (textarea content, cursor state, completion flag) and
//! forwards a caller-provided [`codex_feedback::CodexLogSnapshot`] when the user
//! submits. Selection-parameter helpers translate categories into popup content
//! and actions without storing any long-lived state.
//!
//! The UI layer is intentionally thin: it does not build snapshots, does not
//! retry uploads, and does not own the modal stack. Instead it publishes events
//! that drive the surrounding app state and relies on the caller to decide when
//! to open or close the overlay.
//!
//! Correctness relies on using the same [`FeedbackCategory`] for UI copy and
//! classification, and on honoring `include_logs` when deciding whether to
//! attach rollout artifacts. The set of categories that surface issue URLs must
//! stay in sync with the selection list so UI copy and backend labels remain
//! aligned, and the consent prompt must reflect the same log bundle that the
//! upload path will attach.
use std::cell::RefCell;
use std::path::PathBuf;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event::FeedbackCategory;
use crate::app_event_sender::AppEventSender;
use crate::history_cell;
use crate::render::renderable::Renderable;
use codex_core::protocol::SessionSource;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::standard_popup_hint_line;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

/// Base URL for prefilled bug reports when the flow suggests filing an issue.
///
/// The final URL injects the thread identifier via a `steps` query parameter so
/// the issue template can reference the uploaded transcript.
const BASE_BUG_ISSUE_URL: &str =
    "https://github.com/openai/codex/issues/new?template=2-bug-report.yml";

/// Collects an optional feedback note and dispatches the feedback upload.
///
/// The view owns the text input widget, cursor state, and completion flag, but
/// defers log collection and persistence to the surrounding application.
/// Completion is terminal: once the user submits or cancels, the view no longer
/// accepts input or emits further events.
pub(crate) struct FeedbackNoteView {
    /// Category that drives copy, classification, and issue-link behavior.
    ///
    /// This must stay aligned with the selection list so labels and upload
    /// classifications match what the user saw.
    category: FeedbackCategory,
    /// Snapshot containing the log metadata and thread identifier to upload.
    ///
    /// The snapshot is supplied by the caller so this view never recomputes
    /// log contents; it forwards the metadata to `codex_feedback`.
    snapshot: codex_feedback::CodexLogSnapshot,
    /// Optional rollout artifact path appended to uploads when logs are shared.
    ///
    /// The path is only sent when `include_logs` is true; otherwise it is
    /// ignored even if present.
    rollout_path: Option<PathBuf>,
    /// Channel used to emit history updates or transitions after submission.
    ///
    /// Events emitted here are the only way the view communicates with the
    /// application; it does not mutate shared state directly.
    app_event_tx: AppEventSender,
    /// Whether the submission should include logs and rollout artifacts.
    ///
    /// This gates both the upload payload and the success copy shown to the
    /// user after a successful submission.
    include_logs: bool,

    /// Input widget used to capture the optional user note.
    textarea: TextArea,
    /// Mutable widget state tracked across renders and cursor queries.
    ///
    /// A `RefCell` is used because `render` and `cursor_pos` mutate state while
    /// the view is held immutably by the renderer.
    textarea_state: RefCell<TextAreaState>,
    /// Terminal flag that closes the overlay after submit or cancel.
    ///
    /// Once set, the view reports completion and ignores further input.
    complete: bool,
}

impl FeedbackNoteView {
    /// Builds a feedback note overlay for the requested category and snapshot.
    ///
    /// The snapshot and rollout path are provided by the caller so the view can
    /// render immediately without reaching into log collection.
    pub(crate) fn new(
        category: FeedbackCategory,
        snapshot: codex_feedback::CodexLogSnapshot,
        rollout_path: Option<PathBuf>,
        app_event_tx: AppEventSender,
        include_logs: bool,
    ) -> Self {
        Self {
            category,
            snapshot,
            rollout_path,
            app_event_tx,
            include_logs,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            complete: false,
        }
    }

    /// Submits the feedback note and emits a history entry describing the result.
    ///
    /// The flow trims the current text, determines the category classification,
    /// uploads via `codex_feedback`, and posts either a success or error cell to
    /// the history stream before marking the overlay complete. The
    /// `include_logs` flag controls whether rollout artifacts are attached and
    /// which success copy is shown. A successful upload also includes the
    /// thread identifier in the rendered history entry so users can reference
    /// it in follow-up reports.
    fn submit(&mut self) {
        let note = self.textarea.text().trim().to_string();
        let reason_opt = if note.is_empty() {
            None
        } else {
            Some(note.as_str())
        };
        let rollout_path_ref = self.rollout_path.as_deref();
        let classification = feedback_classification(self.category);

        let mut thread_id = self.snapshot.thread_id.clone();

        let result = self.snapshot.upload_feedback(
            classification,
            reason_opt,
            self.include_logs,
            if self.include_logs {
                rollout_path_ref
            } else {
                None
            },
            Some(SessionSource::Cli),
        );

        match result {
            Ok(()) => {
                let prefix = if self.include_logs {
                    "• Feedback uploaded."
                } else {
                    "• Feedback recorded (no logs)."
                };
                let issue_url = issue_url_for_category(self.category, &thread_id);
                let mut lines = vec![Line::from(match issue_url.as_ref() {
                    Some(_) => format!("{prefix} Please open an issue using the following URL:"),
                    None => format!("{prefix} Thanks for the feedback!"),
                })];
                if let Some(url) = issue_url {
                    lines.extend([
                        "".into(),
                        Line::from(vec!["  ".into(), url.cyan().underlined()]),
                        "".into(),
                        Line::from(vec![
                            "  Or mention your thread ID ".into(),
                            std::mem::take(&mut thread_id).bold(),
                            " in an existing issue.".into(),
                        ]),
                    ]);
                } else {
                    lines.extend([
                        "".into(),
                        Line::from(vec![
                            "  Thread ID: ".into(),
                            std::mem::take(&mut thread_id).bold(),
                        ]),
                    ]);
                }
                self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::PlainHistoryCell::new(lines),
                )));
            }
            Err(e) => {
                self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                    history_cell::new_error_event(format!("Failed to upload feedback: {e}")),
                )));
            }
        }
        self.complete = true;
    }
}

impl BottomPaneView for FeedbackNoteView {
    /// Routes keyboard input to the textarea or triggers submit/cancel actions.
    ///
    /// The Enter key submits only when unmodified; modified Enter continues to
    /// feed the textarea so users can insert newlines.
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                self.on_ctrl_c();
            }
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.submit();
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                self.textarea.input(key_event);
            }
            other => {
                self.textarea.input(other);
            }
        }
    }

    /// Closes the overlay without submitting feedback.
    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    /// Reports whether the overlay has finished its submit/cancel flow.
    fn is_complete(&self) -> bool {
        self.complete
    }

    /// Inserts pasted text into the input buffer.
    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        true
    }
}

impl Renderable for FeedbackNoteView {
    /// Returns the height needed for the title, input area, and footer hint.
    fn desired_height(&self, width: u16) -> u16 {
        1u16 + self.input_height(width) + 3u16
    }

    /// Computes the terminal cursor position inside the input area, if visible.
    ///
    /// The cursor is only returned when the input gutter has room and the
    /// textarea height is non-zero.
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let text_area_height = self.input_height(area.width).saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        let top_line_count = 1u16;
        let textarea_rect = Rect {
            x: area.x.saturating_add(2),
            y: area.y.saturating_add(top_line_count).saturating_add(1),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }

    /// Renders the title, text input, and footer hint for the feedback overlay.
    ///
    /// Rendering proceeds in three phases: draw the title gutter, paint the
    /// input box with placeholder text when empty, and render the standardized
    /// hint line at the bottom when there is space.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let (title, placeholder) = feedback_title_and_placeholder(self.category);
        let input_height = self.input_height(area.width);

        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        let title_spans: Vec<Span<'static>> = vec![gutter(), title.bold()];
        Paragraph::new(Line::from(title_spans)).render(title_area, buf);

        let input_area = Rect {
            x: area.x,
            y: area.y.saturating_add(1),
            width: area.width,
            height: input_height,
        };
        if input_area.width >= 2 {
            for row in 0..input_area.height {
                Paragraph::new(Line::from(vec![gutter()])).render(
                    Rect {
                        x: input_area.x,
                        y: input_area.y.saturating_add(row),
                        width: 2,
                        height: 1,
                    },
                    buf,
                );
            }

            let text_area_height = input_area.height.saturating_sub(1);
            if text_area_height > 0 {
                if input_area.width > 2 {
                    let blank_rect = Rect {
                        x: input_area.x.saturating_add(2),
                        y: input_area.y,
                        width: input_area.width.saturating_sub(2),
                        height: 1,
                    };
                    Clear.render(blank_rect, buf);
                }
                let textarea_rect = Rect {
                    x: input_area.x.saturating_add(2),
                    y: input_area.y.saturating_add(1),
                    width: input_area.width.saturating_sub(2),
                    height: text_area_height,
                };
                let mut state = self.textarea_state.borrow_mut();
                StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
                if self.textarea.text().is_empty() {
                    Paragraph::new(Line::from(placeholder.dim())).render(textarea_rect, buf);
                }
            }
        }

        let hint_blank_y = input_area.y.saturating_add(input_height);
        if hint_blank_y < area.y.saturating_add(area.height) {
            let blank_area = Rect {
                x: area.x,
                y: hint_blank_y,
                width: area.width,
                height: 1,
            };
            Clear.render(blank_area, buf);
        }

        let hint_y = hint_blank_y.saturating_add(1);
        if hint_y < area.y.saturating_add(area.height) {
            Paragraph::new(standard_popup_hint_line()).render(
                Rect {
                    x: area.x,
                    y: hint_y,
                    width: area.width,
                    height: 1,
                },
                buf,
            );
        }
    }
}

impl FeedbackNoteView {
    /// Computes the height of the input region, clamping it to a small range.
    ///
    /// The height includes one row for the top padding inside the gutter and
    /// keeps the textarea between one and eight lines tall.
    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 8);
        text_height.saturating_add(1).min(9)
    }
}

/// Returns the left gutter span used by the feedback overlay.
///
/// The cyan bar visually matches other bottom-pane prompts.
fn gutter() -> Span<'static> {
    "▌ ".cyan()
}

/// Returns the user-facing title and placeholder for a feedback category.
///
/// Both strings are localized per category so copy and classification remain
/// aligned throughout the feedback flow.
fn feedback_title_and_placeholder(category: FeedbackCategory) -> (String, String) {
    match category {
        FeedbackCategory::BadResult => (
            "Tell us more (bad result)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
        FeedbackCategory::GoodResult => (
            "Tell us more (good result)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
        FeedbackCategory::Bug => (
            "Tell us more (bug)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
        FeedbackCategory::Other => (
            "Tell us more (other)".to_string(),
            "(optional) Write a short description to help us further".to_string(),
        ),
    }
}

/// Maps a feedback category to the upload classification string.
///
/// Keep this mapping in sync with the selection list and UI copy so analytics
/// and labels reflect what the user actually chose.
fn feedback_classification(category: FeedbackCategory) -> &'static str {
    match category {
        FeedbackCategory::BadResult => "bad_result",
        FeedbackCategory::GoodResult => "good_result",
        FeedbackCategory::Bug => "bug",
        FeedbackCategory::Other => "other",
    }
}

/// Returns the issue URL for categories that should file a bug report.
///
/// The thread identifier is injected so the issue template can reference the
/// relevant transcript without additional user input.
fn issue_url_for_category(category: FeedbackCategory, thread_id: &str) -> Option<String> {
    match category {
        FeedbackCategory::Bug | FeedbackCategory::BadResult | FeedbackCategory::Other => Some(
            format!("{BASE_BUG_ISSUE_URL}&steps=Uploaded%20thread:%20{thread_id}"),
        ),
        FeedbackCategory::GoodResult => None,
    }
}

/// Builds the selection popup for choosing a feedback category.
///
/// Each selection item emits an [`AppEvent::OpenFeedbackConsent`] action for
/// the chosen category so the caller can decide whether to show the upload
/// consent dialog.
pub(crate) fn feedback_selection_params(
    app_event_tx: AppEventSender,
) -> super::SelectionViewParams {
    super::SelectionViewParams {
        title: Some("How was this?".to_string()),
        items: vec![
            make_feedback_item(
                app_event_tx.clone(),
                "bug",
                "Crash, error message, hang, or broken UI/behavior.",
                FeedbackCategory::Bug,
            ),
            make_feedback_item(
                app_event_tx.clone(),
                "bad result",
                "Output was off-target, incorrect, incomplete, or unhelpful.",
                FeedbackCategory::BadResult,
            ),
            make_feedback_item(
                app_event_tx.clone(),
                "good result",
                "Helpful, correct, high‑quality, or delightful result worth celebrating.",
                FeedbackCategory::GoodResult,
            ),
            make_feedback_item(
                app_event_tx,
                "other",
                "Slowness, feature suggestion, UX feedback, or anything else.",
                FeedbackCategory::Other,
            ),
        ],
        ..Default::default()
    }
}

/// Builds the selection popup shown when feedback is disabled by configuration.
///
/// The popup is read-only and only provides a close action.
pub(crate) fn feedback_disabled_params() -> super::SelectionViewParams {
    super::SelectionViewParams {
        title: Some("Sending feedback is disabled".to_string()),
        subtitle: Some("This action is disabled by configuration.".to_string()),
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![super::SelectionItem {
            name: "Close".to_string(),
            dismiss_on_select: true,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Builds a selection item that opens the feedback consent prompt.
///
/// The returned item owns an action closure that captures the category so the
/// selection list stays data-driven.
fn make_feedback_item(
    app_event_tx: AppEventSender,
    name: &str,
    description: &str,
    category: FeedbackCategory,
) -> super::SelectionItem {
    let action: super::SelectionAction = Box::new(move |_sender: &AppEventSender| {
        app_event_tx.send(AppEvent::OpenFeedbackConsent { category });
    });
    super::SelectionItem {
        name: name.to_string(),
        description: Some(description.to_string()),
        actions: vec![action],
        dismiss_on_select: true,
        ..Default::default()
    }
}

/// Builds the upload consent popup for a given feedback category.
///
/// The header lists the standard log filename plus any rollout artifact name
/// provided by `rollout_path`.
pub(crate) fn feedback_upload_consent_params(
    app_event_tx: AppEventSender,
    category: FeedbackCategory,
    rollout_path: Option<std::path::PathBuf>,
) -> super::SelectionViewParams {
    use super::popup_consts::standard_popup_hint_line;
    let yes_action: super::SelectionAction = Box::new({
        let tx = app_event_tx.clone();
        move |sender: &AppEventSender| {
            let _ = sender;
            tx.send(AppEvent::OpenFeedbackNote {
                category,
                include_logs: true,
            });
        }
    });

    let no_action: super::SelectionAction = Box::new({
        let tx = app_event_tx;
        move |sender: &AppEventSender| {
            let _ = sender;
            tx.send(AppEvent::OpenFeedbackNote {
                category,
                include_logs: false,
            });
        }
    });

    let mut header_lines: Vec<Box<dyn crate::render::renderable::Renderable>> = vec![
        Line::from("Upload logs?".bold()).into(),
        Line::from("").into(),
        Line::from("The following files will be sent:".dim()).into(),
        Line::from(vec!["  • ".into(), "codex-logs.log".into()]).into(),
    ];
    if let Some(path) = rollout_path.as_deref()
        && let Some(name) = path.file_name().map(|s| s.to_string_lossy().to_string())
    {
        header_lines.push(Line::from(vec!["  • ".into(), name.into()]).into());
    }

    super::SelectionViewParams {
        footer_hint: Some(standard_popup_hint_line()),
        items: vec![
            super::SelectionItem {
                name: "Yes".to_string(),
                description: Some(
                    "Share the current Codex session logs with the team for troubleshooting."
                        .to_string(),
                ),
                actions: vec![yes_action],
                dismiss_on_select: true,
                ..Default::default()
            },
            super::SelectionItem {
                name: "No".to_string(),
                description: Some("".to_string()),
                actions: vec![no_action],
                dismiss_on_select: true,
                ..Default::default()
            },
        ],
        header: Box::new(crate::render::renderable::ColumnRenderable::with(
            header_lines,
        )),
        ..Default::default()
    }
}

/// Snapshot coverage for feedback view rendering and issue URL behavior.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use crate::app_event_sender::AppEventSender;

    /// Renders the feedback overlay to a trimmed string for snapshot testing.
    fn render(view: &FeedbackNoteView, width: u16) -> String {
        let height = view.desired_height(width);
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        view.render(area, &mut buf);

        let mut lines: Vec<String> = (0..area.height)
            .map(|row| {
                let mut line = String::new();
                for col in 0..area.width {
                    let symbol = buf[(area.x + col, area.y + row)].symbol();
                    if symbol.is_empty() {
                        line.push(' ');
                    } else {
                        line.push_str(symbol);
                    }
                }
                line.trim_end().to_string()
            })
            .collect();

        while lines.first().is_some_and(|l| l.trim().is_empty()) {
            lines.remove(0);
        }
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }

    /// Builds a feedback note view with an empty snapshot for rendering tests.
    fn make_view(category: FeedbackCategory) -> FeedbackNoteView {
        let (tx_raw, _rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let snapshot = codex_feedback::CodexFeedback::new().snapshot(None);
        FeedbackNoteView::new(category, snapshot, None, tx, true)
    }

    /// Ensures the bad-result overlay renders a stable snapshot.
    #[test]
    fn feedback_view_bad_result() {
        let view = make_view(FeedbackCategory::BadResult);
        let rendered = render(&view, 60);
        insta::assert_snapshot!("feedback_view_bad_result", rendered);
    }

    /// Ensures the good-result overlay renders a stable snapshot.
    #[test]
    fn feedback_view_good_result() {
        let view = make_view(FeedbackCategory::GoodResult);
        let rendered = render(&view, 60);
        insta::assert_snapshot!("feedback_view_good_result", rendered);
    }

    /// Ensures the bug overlay renders a stable snapshot.
    #[test]
    fn feedback_view_bug() {
        let view = make_view(FeedbackCategory::Bug);
        let rendered = render(&view, 60);
        insta::assert_snapshot!("feedback_view_bug", rendered);
    }

    /// Ensures the other overlay renders a stable snapshot.
    #[test]
    fn feedback_view_other() {
        let view = make_view(FeedbackCategory::Other);
        let rendered = render(&view, 60);
        insta::assert_snapshot!("feedback_view_other", rendered);
    }

    /// Verifies which categories produce issue URLs.
    #[test]
    fn issue_url_available_for_bug_bad_result_and_other() {
        let bug_url = issue_url_for_category(FeedbackCategory::Bug, "thread-1");
        assert!(
            bug_url
                .as_deref()
                .is_some_and(|url| url.contains("template=2-bug-report"))
        );

        let bad_result_url = issue_url_for_category(FeedbackCategory::BadResult, "thread-2");
        assert!(bad_result_url.is_some());

        let other_url = issue_url_for_category(FeedbackCategory::Other, "thread-3");
        assert!(other_url.is_some());

        assert!(issue_url_for_category(FeedbackCategory::GoodResult, "t").is_none());
    }
}
