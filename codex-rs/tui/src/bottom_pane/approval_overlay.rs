use std::collections::HashMap;
use std::path::PathBuf;

use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::list_selection_view::ListSelectionView;
use crate::bottom_pane::list_selection_view::SelectionItem;
use crate::bottom_pane::list_selection_view::SelectionViewParams;
use crate::diff_render::DiffSummary;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use crate::text_formatting::truncate_text;
use codex_core::protocol::FileChange;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

/// Request coming from the agent that needs user approval.
#[derive(Clone, Debug)]
pub(crate) enum ApprovalRequest {
    Exec {
        id: String,
        command: Vec<String>,
        reason: Option<String>,
    },
    ApplyPatch {
        id: String,
        reason: Option<String>,
        cwd: PathBuf,
        changes: HashMap<PathBuf, FileChange>,
    },
}

/// Modal overlay asking the user to approve or deny one or more requests.
pub(crate) struct ApprovalOverlay {
    current_request: Option<ApprovalRequest>,
    current_variant: Option<ApprovalVariant>,
    queue: Vec<ApprovalRequest>,
    app_event_tx: AppEventSender,
    list: ListSelectionView,
    options: Vec<ApprovalOption>,
    current_complete: bool,
    done: bool,
}

impl ApprovalOverlay {
    pub fn new(request: ApprovalRequest, app_event_tx: AppEventSender) -> Self {
        let mut view = Self {
            current_request: None,
            current_variant: None,
            queue: Vec::new(),
            app_event_tx: app_event_tx.clone(),
            list: ListSelectionView::new(Default::default(), app_event_tx),
            options: Vec::new(),
            current_complete: false,
            done: false,
        };
        view.set_current(request);
        view
    }

    pub fn enqueue_request(&mut self, req: ApprovalRequest) {
        self.queue.push(req);
    }

    fn set_current(&mut self, request: ApprovalRequest) {
        self.current_request = Some(request.clone());
        let ApprovalRequestState { variant, header } = ApprovalRequestState::from(request);
        self.current_variant = Some(variant.clone());
        self.current_complete = false;
        let (options, params) = Self::build_options(variant, header);
        self.options = options;
        self.list = ListSelectionView::new(params, self.app_event_tx.clone());
    }

    fn build_options(
        variant: ApprovalVariant,
        header: Box<dyn Renderable>,
    ) -> (Vec<ApprovalOption>, SelectionViewParams) {
        let (options, title) = match &variant {
            ApprovalVariant::Exec { .. } => (
                exec_options(),
                "Would you like to run the following command?".to_string(),
            ),
            ApprovalVariant::ApplyPatch { .. } => (
                patch_options(),
                "Would you like to make the following edits?".to_string(),
            ),
        };

        let header = Box::new(ColumnRenderable::new([
            Box::new(Line::from(title.bold())),
            Box::new(Line::from("")),
            header,
        ]));

        let items = options
            .iter()
            .map(|opt| SelectionItem {
                name: opt.label.clone(),
                display_shortcut: opt.display_shortcut,
                dismiss_on_select: false,
                ..Default::default()
            })
            .collect();

        let params = SelectionViewParams {
            footer_hint: Some(Line::from(vec![
                "Press ".into(),
                key_hint::plain(KeyCode::Enter).into(),
                " to confirm or ".into(),
                key_hint::plain(KeyCode::Esc).into(),
                " to cancel".into(),
            ])),
            items,
            header,
            ..Default::default()
        };

        (options, params)
    }

    fn apply_selection(&mut self, actual_idx: usize) {
        if self.current_complete {
            return;
        }
        let Some(option) = self.options.get(actual_idx) else {
            return;
        };
        if let Some(variant) = self.current_variant.as_ref() {
            match (&variant, option.decision) {
                (ApprovalVariant::Exec { id, command }, decision) => {
                    self.handle_exec_decision(id, command, decision);
                }
                (ApprovalVariant::ApplyPatch { id, .. }, decision) => {
                    self.handle_patch_decision(id, decision);
                }
            }
        }

        self.current_complete = true;
        self.advance_queue();
    }

    fn handle_exec_decision(&self, id: &str, command: &[String], decision: ReviewDecision) {
        if let Some(lines) = build_exec_history_lines(command.to_vec(), decision) {
            self.app_event_tx.send(AppEvent::InsertHistoryCell(Box::new(
                history_cell::new_user_approval_decision(lines),
            )));
        }
        self.app_event_tx.send(AppEvent::CodexOp(Op::ExecApproval {
            id: id.to_string(),
            decision,
        }));
    }

    fn handle_patch_decision(&self, id: &str, decision: ReviewDecision) {
        self.app_event_tx.send(AppEvent::CodexOp(Op::PatchApproval {
            id: id.to_string(),
            decision,
        }));
    }

    fn advance_queue(&mut self) {
        if let Some(next) = self.queue.pop() {
            self.set_current(next);
        } else {
            self.done = true;
        }
    }

    fn try_handle_shortcut(&mut self, key_event: &KeyEvent) -> bool {
        match key_event {
            KeyEvent {
                kind: KeyEventKind::Press,
                code: KeyCode::Char('a'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(request) = self.current_request.as_ref() {
                    self.app_event_tx
                        .send(AppEvent::FullScreenApprovalRequest(request.clone()));
                    true
                } else {
                    false
                }
            }
            e => {
                if let Some(idx) = self
                    .options
                    .iter()
                    .position(|opt| opt.shortcuts().any(|s| s.is_press(*e)))
                {
                    self.apply_selection(idx);
                    true
                } else {
                    false
                }
            }
        }
    }
}

impl BottomPaneView for ApprovalOverlay {
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.try_handle_shortcut(&key_event) {
            return;
        }
        self.list.handle_key_event(key_event);
        if let Some(idx) = self.list.take_last_selected_index() {
            self.apply_selection(idx);
        }
    }

    fn on_ctrl_c(&mut self) -> CancellationEvent {
        if self.done {
            return CancellationEvent::Handled;
        }
        if !self.current_complete
            && let Some(variant) = self.current_variant.as_ref()
        {
            match &variant {
                ApprovalVariant::Exec { id, command } => {
                    self.handle_exec_decision(id, command, ReviewDecision::Abort);
                }
                ApprovalVariant::ApplyPatch { id, .. } => {
                    self.handle_patch_decision(id, ReviewDecision::Abort);
                }
            }
        }
        self.queue.clear();
        self.done = true;
        CancellationEvent::Handled
    }

    fn is_complete(&self) -> bool {
        self.done
    }

    fn try_consume_approval_request(
        &mut self,
        request: ApprovalRequest,
    ) -> Option<ApprovalRequest> {
        self.enqueue_request(request);
        None
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.list.cursor_pos(area)
    }
}

impl Renderable for ApprovalOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        self.list.desired_height(width)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.list.render(area, buf);
    }
}

struct ApprovalRequestState {
    variant: ApprovalVariant,
    header: Box<dyn Renderable>,
}

impl From<ApprovalRequest> for ApprovalRequestState {
    fn from(value: ApprovalRequest) -> Self {
        match value {
            ApprovalRequest::Exec {
                id,
                command,
                reason,
            } => {
                let mut header: Vec<Line<'static>> = Vec::new();
                if let Some(reason) = reason
                    && !reason.is_empty()
                {
                    header.push(Line::from(vec!["Reason: ".into(), reason.italic()]));
                    header.push(Line::from(""));
                }
                let full_cmd = strip_bash_lc_and_escape(&command);
                let mut full_cmd_lines = highlight_bash_to_lines(&full_cmd);
                if let Some(first) = full_cmd_lines.first_mut() {
                    first.spans.insert(0, Span::from("$ "));
                }
                header.extend(full_cmd_lines);
                Self {
                    variant: ApprovalVariant::Exec { id, command },
                    header: Box::new(Paragraph::new(header).wrap(Wrap { trim: false })),
                }
            }
            ApprovalRequest::ApplyPatch {
                id,
                reason,
                cwd,
                changes,
            } => {
                let mut header: Vec<Box<dyn Renderable>> = Vec::new();
                header.push(DiffSummary::new(changes, cwd).into());
                if let Some(reason) = reason
                    && !reason.is_empty()
                {
                    header.push(Box::new(Line::from("")));
                    header.push(Box::new(
                        Paragraph::new(reason.italic()).wrap(Wrap { trim: false }),
                    ));
                }
                Self {
                    variant: ApprovalVariant::ApplyPatch { id },
                    header: Box::new(ColumnRenderable::new(header)),
                }
            }
        }
    }
}

#[derive(Clone)]
enum ApprovalVariant {
    Exec { id: String, command: Vec<String> },
    ApplyPatch { id: String },
}

#[derive(Clone)]
struct ApprovalOption {
    label: String,
    decision: ReviewDecision,
    display_shortcut: Option<KeyBinding>,
    additional_shortcuts: Vec<KeyBinding>,
}

impl ApprovalOption {
    fn shortcuts(&self) -> impl Iterator<Item = KeyBinding> + '_ {
        self.display_shortcut
            .into_iter()
            .chain(self.additional_shortcuts.iter().copied())
    }
}

fn exec_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, proceed".to_string(),
            decision: ReviewDecision::Approved,
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
        },
        ApprovalOption {
            label: "Yes, and don't ask again for this command".to_string(),
            decision: ReviewDecision::ApprovedForSession,
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('a'))],
        },
        ApprovalOption {
            label: "No, and tell Codex what to do differently".to_string(),
            decision: ReviewDecision::Abort,
            display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
        },
    ]
}

fn patch_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, proceed".to_string(),
            decision: ReviewDecision::Approved,
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
        },
        ApprovalOption {
            label: "No, and tell Codex what to do differently".to_string(),
            decision: ReviewDecision::Abort,
            display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
        },
    ]
}

fn build_exec_history_lines(
    command: Vec<String>,
    decision: ReviewDecision,
) -> Option<Vec<Line<'static>>> {
    use ReviewDecision::*;

    let (symbol, summary): (Span<'static>, Vec<Span<'static>>) = match decision {
        Approved => {
            let snippet = Span::from(exec_snippet(&command)).dim();
            (
                "✔ ".green(),
                vec![
                    "You ".into(),
                    "approved".bold(),
                    " codex to run ".into(),
                    snippet,
                    " this time".bold(),
                ],
            )
        }
        ApprovedForSession => {
            let snippet = Span::from(exec_snippet(&command)).dim();
            (
                "✔ ".green(),
                vec![
                    "You ".into(),
                    "approved".bold(),
                    " codex to run ".into(),
                    snippet,
                    " every time this session".bold(),
                ],
            )
        }
        Denied => {
            let snippet = Span::from(exec_snippet(&command)).dim();
            (
                "✗ ".red(),
                vec![
                    "You ".into(),
                    "did not approve".bold(),
                    " codex to run ".into(),
                    snippet,
                ],
            )
        }
        Abort => {
            let snippet = Span::from(exec_snippet(&command)).dim();
            (
                "✗ ".red(),
                vec![
                    "You ".into(),
                    "canceled".bold(),
                    " the request to run ".into(),
                    snippet,
                ],
            )
        }
    };

    let mut lines = Vec::new();
    let mut spans = Vec::new();
    spans.push(symbol);
    spans.extend(summary);
    lines.push(Line::from(spans));
    Some(lines)
}

fn truncate_exec_snippet(full_cmd: &str) -> String {
    let mut snippet = match full_cmd.split_once('\n') {
        Some((first, _)) => format!("{first} ..."),
        None => full_cmd.to_string(),
    };
    snippet = truncate_text(&snippet, 80);
    snippet
}

fn exec_snippet(command: &[String]) -> String {
    let full_cmd = strip_bash_lc_and_escape(command);
    truncate_exec_snippet(&full_cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use tokio::sync::mpsc::unbounded_channel;

    fn make_exec_request() -> ApprovalRequest {
        ApprovalRequest::Exec {
            id: "test".to_string(),
            command: vec!["echo".to_string(), "hi".to_string()],
            reason: Some("reason".to_string()),
        }
    }

    #[test]
    fn ctrl_c_aborts_and_clears_queue() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = ApprovalOverlay::new(make_exec_request(), tx);
        view.enqueue_request(make_exec_request());
        assert_eq!(CancellationEvent::Handled, view.on_ctrl_c());
        assert!(view.queue.is_empty());
        assert!(view.is_complete());
    }

    #[test]
    fn shortcut_triggers_selection() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = ApprovalOverlay::new(make_exec_request(), tx);
        assert!(!view.is_complete());
        view.handle_key_event(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        // We expect at least one CodexOp message in the queue.
        let mut saw_op = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, AppEvent::CodexOp(_)) {
                saw_op = true;
                break;
            }
        }
        assert!(saw_op, "expected approval decision to emit an op");
    }

    #[test]
    fn header_includes_command_snippet() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let command = vec!["echo".into(), "hello".into(), "world".into()];
        let exec_request = ApprovalRequest::Exec {
            id: "test".into(),
            command,
            reason: None,
        };

        let view = ApprovalOverlay::new(exec_request, tx);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, view.desired_height(80)));
        view.render(Rect::new(0, 0, 80, view.desired_height(80)), &mut buf);

        let rendered: Vec<String> = (0..buf.area.height)
            .map(|row| {
                (0..buf.area.width)
                    .map(|col| buf[(col, row)].symbol().to_string())
                    .collect()
            })
            .collect();
        assert!(
            rendered
                .iter()
                .any(|line| line.contains("echo hello world")),
            "expected header to include command snippet, got {rendered:?}"
        );
    }

    #[test]
    fn enter_sets_last_selected_index_without_dismissing() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = ApprovalOverlay::new(make_exec_request(), tx);
        view.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        view.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert!(
            view.is_complete(),
            "exec approval should complete without queued requests"
        );

        let mut decision = None;
        while let Ok(ev) = rx.try_recv() {
            if let AppEvent::CodexOp(Op::ExecApproval { decision: d, .. }) = ev {
                decision = Some(d);
                break;
            }
        }
        assert_eq!(decision, Some(ReviewDecision::ApprovedForSession));
    }
}
