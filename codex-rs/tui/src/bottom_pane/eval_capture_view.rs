use std::cell::RefCell;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::StatefulWidgetRef;
use ratatui::widgets::Widget;

use crate::app_event::AppEvent;
use crate::app_event::EvalCaptureStartMarker;
use crate::app_event_sender::AppEventSender;
use crate::render::renderable::Renderable;

use super::CancellationEvent;
use super::bottom_pane_view::BottomPaneView;
use super::popup_consts::standard_popup_hint_line;
use super::textarea::TextArea;
use super::textarea::TextAreaState;

pub(crate) fn eval_capture_intro_params(
    app_event_tx: AppEventSender,
) -> super::SelectionViewParams {
    let continue_action: super::SelectionAction = Box::new({
        let tx = app_event_tx.clone();
        move |_sender: &AppEventSender| {
            tx.send(AppEvent::EvalCaptureIntroContinue);
        }
    });

    let basic_feedback_action: super::SelectionAction = Box::new({
        let tx = app_event_tx;
        move |_sender: &AppEventSender| {
            tx.send(AppEvent::OpenFeedbackConsent {
                category: crate::app_event::FeedbackCategory::BadResult,
            });
        }
    });

    let header_lines: Vec<Box<dyn crate::render::renderable::Renderable>> = vec![
        Line::from("Capture eval sample".bold()).into(),
        Line::from("").into(),
        Line::from("Here's everything we capture as an eval sample.".dim()).into(),
        Line::from("It's stored locally on your computer:".dim()).into(),
        Line::from(vec!["  • ".into(), "manifest.json".into()]).into(),
        Line::from(vec!["  • ".into(), "rollout.jsonl".into()]).into(),
        Line::from(vec!["  • ".into(), "repo.patch".into()]).into(),
        Line::from(vec!["  • ".into(), "codex-logs.log".into()]).into(),
        Line::from("").into(),
        Line::from("Next, you can optionally upload this bundle to the team.".dim()).into(),
    ];

    super::SelectionViewParams {
        footer_hint: Some(standard_popup_hint_line()),
        title: None,
        header: Box::new(crate::render::renderable::ColumnRenderable::with(
            header_lines,
        )),
        items: vec![
            super::SelectionItem {
                name: "Continue".to_string(),
                description: Some("Create an eval sample bundle locally.".to_string()),
                actions: vec![continue_action],
                dismiss_on_select: true,
                ..Default::default()
            },
            super::SelectionItem {
                name: "Send basic feedback instead".to_string(),
                description: Some(
                    "Skip eval capture and send basic feedback to the team.".to_string(),
                ),
                actions: vec![basic_feedback_action],
                dismiss_on_select: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

pub(crate) fn eval_capture_send_consent_params(
    app_event_tx: AppEventSender,
) -> super::SelectionViewParams {
    let upload_action: super::SelectionAction = Box::new({
        let tx = app_event_tx.clone();
        move |_sender: &AppEventSender| {
            tx.send(AppEvent::EvalCaptureSendConsent { upload: true });
        }
    });

    let skip_action: super::SelectionAction = Box::new({
        let tx = app_event_tx;
        move |_sender: &AppEventSender| {
            tx.send(AppEvent::EvalCaptureSendConsent { upload: false });
        }
    });

    let header_lines: Vec<Box<dyn crate::render::renderable::Renderable>> = vec![
        Line::from("Send eval sample to the codex team?".bold()).into(),
        Line::from("").into(),
        Line::from("If you choose Yes, it will upload the full bundle".dim()).into(),
        Line::from("to the team:".dim()).into(),
        Line::from(vec!["  • ".into(), "manifest.json".into()]).into(),
        Line::from(vec!["  • ".into(), "rollout.jsonl".into()]).into(),
        Line::from(vec!["  • ".into(), "repo.patch".into()]).into(),
        Line::from(vec!["  • ".into(), "codex-logs.log".into()]).into(),
        Line::from("").into(),
        Line::from("This may include file paths, code snippets, and tool outputs.".dim()).into(),
    ];

    super::SelectionViewParams {
        footer_hint: Some(standard_popup_hint_line()),
        title: None,
        header: Box::new(crate::render::renderable::ColumnRenderable::with(
            header_lines,
        )),
        items: vec![
            super::SelectionItem {
                name: "Yes".to_string(),
                description: Some(
                    "Upload the full bundle to the team for troubleshooting and model improvement."
                        .to_string(),
                ),
                actions: vec![upload_action],
                dismiss_on_select: true,
                ..Default::default()
            },
            super::SelectionItem {
                name: "No".to_string(),
                description: Some("Keep it local only.".to_string()),
                actions: vec![skip_action],
                dismiss_on_select: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

pub(crate) fn eval_capture_start_picker_params(
    app_event_tx: AppEventSender,
    options: Vec<EvalCaptureStartMarker>,
    initial_selected_idx: Option<usize>,
) -> super::SelectionViewParams {
    let items = options
        .into_iter()
        .map(|marker| {
            let action: super::SelectionAction = Box::new({
                let tx = app_event_tx.clone();
                let marker = marker.clone();
                move |_sender: &AppEventSender| {
                    tx.send(AppEvent::OpenEvalCaptureNotes {
                        start_marker: marker.clone(),
                    });
                }
            });
            let disabled_reason = (!marker_has_repo_snapshot(&marker))
                .then_some("No repo snapshot available for this message.".to_string());
            super::SelectionItem {
                name: marker_display_name(&marker),
                // For the eval-capture start picker, we render the full message directly as the
                // row name (including a relative timestamp) to avoid duplicating it in the
                // description column.
                actions: vec![action],
                dismiss_on_select: true,
                disabled_reason,
                ..Default::default()
            }
        })
        .collect();

    super::SelectionViewParams {
        footer_hint: Some(standard_popup_hint_line()),
        title: Some("Pick the start message".to_string()),
        subtitle: Some(
            "Pick the first instruction for the specific code change or interaction you wanted to give feedback on."
                .to_string(),
        ),
        items,
        initial_selected_idx,
        show_numbers: true,
        ..Default::default()
    }
}

fn marker_display_name(marker: &EvalCaptureStartMarker) -> String {
    match marker {
        EvalCaptureStartMarker::RolloutLineIndex { display, .. }
        | EvalCaptureStartMarker::RolloutLineTimestamp { display, .. } => display.clone(),
    }
}

fn marker_has_repo_snapshot(marker: &EvalCaptureStartMarker) -> bool {
    match marker {
        EvalCaptureStartMarker::RolloutLineIndex { repo_snapshot, .. }
        | EvalCaptureStartMarker::RolloutLineTimestamp { repo_snapshot, .. } => {
            repo_snapshot.is_some()
        }
    }
}

pub(crate) struct EvalCaptureNotesView {
    start_marker: EvalCaptureStartMarker,
    app_event_tx: AppEventSender,

    textarea: TextArea,
    textarea_state: RefCell<TextAreaState>,
    complete: bool,
}

impl EvalCaptureNotesView {
    pub(crate) fn new(start_marker: EvalCaptureStartMarker, app_event_tx: AppEventSender) -> Self {
        Self {
            start_marker,
            app_event_tx,
            textarea: TextArea::new(),
            textarea_state: RefCell::new(TextAreaState::default()),
            complete: false,
        }
    }

    fn submit(&mut self) {
        let what_went_wrong = self.textarea.text().trim().to_string();
        self.app_event_tx.send(AppEvent::CreateEvalCaptureBundle {
            start_marker: self.start_marker.clone(),
            what_went_wrong,
            what_good_looks_like: String::new(),
        });
        self.complete = true;
    }
}

impl BottomPaneView for EvalCaptureNotesView {
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
            other => {
                self.textarea.input(other);
            }
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        self.complete = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.complete
    }

    fn handle_paste(&mut self, pasted: String) -> bool {
        if pasted.is_empty() {
            return false;
        }
        self.textarea.insert_str(&pasted);
        true
    }
}

impl Renderable for EvalCaptureNotesView {
    fn desired_height(&self, width: u16) -> u16 {
        self.header_lines(width).len() as u16 + self.input_height(width) + 2u16
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        if area.height < 2 || area.width <= 2 {
            return None;
        }
        let text_area_height = self.input_height(area.width).saturating_sub(1);
        if text_area_height == 0 {
            return None;
        }
        let top_line_count = self.header_lines(area.width).len() as u16;
        let textarea_rect = Rect {
            x: area.x.saturating_add(2),
            y: area.y.saturating_add(top_line_count).saturating_add(1),
            width: area.width.saturating_sub(2),
            height: text_area_height,
        };
        let state = *self.textarea_state.borrow();
        self.textarea.cursor_pos_with_state(textarea_rect, state)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let placeholder = "Type here.";

        let input_height = self.input_height(area.width);
        let header_lines = self.header_lines(area.width);
        let header_height = header_lines.len() as u16;

        let title_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: header_height,
        };
        let input_area = Rect {
            x: area.x,
            y: area.y.saturating_add(header_height),
            width: area.width,
            height: input_height,
        };

        Clear.render(area, buf);

        Paragraph::new(header_lines).render(title_area, buf);

        let textarea_rect = Rect {
            x: input_area.x.saturating_add(2),
            y: input_area.y.saturating_add(1),
            width: input_area.width.saturating_sub(2),
            height: input_area.height.saturating_sub(1),
        };
        let mut state = self.textarea_state.borrow_mut();
        StatefulWidgetRef::render_ref(&(&self.textarea), textarea_rect, buf, &mut state);
        if self.textarea.text().is_empty() {
            Paragraph::new(Line::from(placeholder.dim())).render(textarea_rect, buf);
        }

        let hint_area = Rect {
            x: area.x,
            y: area
                .y
                .saturating_add(header_height)
                .saturating_add(input_height)
                .saturating_add(1),
            width: area.width,
            height: 1,
        };
        Paragraph::new(Line::from("Enter to continue • Esc to go back".dim()))
            .render(hint_area, buf);
    }
}

impl EvalCaptureNotesView {
    fn header_lines(&self, width: u16) -> Vec<Line<'static>> {
        let title = "What went wrong with this interaction, and what should have happened for this to be a correct, delightful response?";
        let subtitle = "Please be specific, and can refer to any aspect of model behavior, including tool calls, code changes, or messages.";
        let usable_width = width.saturating_sub(2).max(1) as usize;
        let mut lines = Vec::new();
        for line in textwrap::wrap(title, usable_width) {
            lines.push(Line::from(line.into_owned().bold()));
        }
        for line in textwrap::wrap(subtitle, usable_width) {
            lines.push(Line::from(line.into_owned().dim()));
        }
        lines
    }

    fn input_height(&self, width: u16) -> u16 {
        let usable_width = width.saturating_sub(2);
        let text_height = self.textarea.desired_height(usable_width).clamp(1, 8);
        text_height.saturating_add(1).min(9)
    }
}
