use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::BottomPaneView;
use crate::bottom_pane::CancellationEvent;
use crate::bottom_pane::list_selection_view::ListSelectionView;
use crate::bottom_pane::list_selection_view::SelectionItem;
use crate::bottom_pane::list_selection_view::SelectionViewParams;
use crate::bottom_pane::popup_consts::STANDARD_POPUP_HINT_LINE;
use crate::exec_command::strip_bash_lc_and_escape;
use crate::history_cell;
use crate::text_formatting::truncate_text;
use crate::user_approval_widget::ApprovalRequest;
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
use ratatui::widgets::Widget;
use textwrap::wrap;

/// Modal overlay asking the user to approve or deny one or more requests.
pub(crate) struct ApprovalModalView {
    current: Option<ApprovalRequestState>,
    queue: Vec<ApprovalRequest>,
    app_event_tx: AppEventSender,
    list: ListSelectionView,
    options: Vec<ApprovalOption>,
    current_complete: bool,
    done: bool,
}

impl ApprovalModalView {
    pub fn new(request: ApprovalRequest, app_event_tx: AppEventSender) -> Self {
        let mut view = Self {
            current: Some(ApprovalRequestState::from(request)),
            queue: Vec::new(),
            app_event_tx: app_event_tx.clone(),
            list: ListSelectionView::new(
                SelectionViewParams {
                    title: String::new(),
                    ..Default::default()
                },
                app_event_tx,
            ),
            options: Vec::new(),
            current_complete: false,
            done: false,
        };
        let (options, params) = view.build_options();
        view.options = options;
        view.list = ListSelectionView::new(params, view.app_event_tx.clone());
        view
    }

    pub fn enqueue_request(&mut self, req: ApprovalRequest) {
        self.queue.push(req);
    }

    fn set_current(&mut self, request: ApprovalRequest) {
        self.current = Some(ApprovalRequestState::from(request));
        self.current_complete = false;
        let (options, params) = self.build_options();
        self.options = options;
        self.list = ListSelectionView::new(params, self.app_event_tx.clone());
    }

    fn build_options(&self) -> (Vec<ApprovalOption>, SelectionViewParams) {
        let Some(state) = self.current.as_ref() else {
            return (
                Vec::new(),
                SelectionViewParams {
                    title: String::new(),
                    ..Default::default()
                },
            );
        };
        let (options, title) = match &state.variant {
            ApprovalVariant::Exec { .. } => (exec_options(), "Allow command?".to_string()),
            ApprovalVariant::ApplyPatch { .. } => (patch_options(), "Apply changes?".to_string()),
        };

        let items = options
            .iter()
            .map(|opt| SelectionItem {
                name: opt.label.clone(),
                description: Some(opt.description.clone()),
                is_current: false,
                actions: Vec::new(),
                dismiss_on_select: false,
                search_value: None,
            })
            .collect();

        let params = SelectionViewParams {
            title,
            footer_hint: Some(STANDARD_POPUP_HINT_LINE.to_string()),
            items,
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
        if let Some(state) = self.current.as_ref() {
            match (&state.variant, option.decision) {
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
        if key_event.kind != KeyEventKind::Press {
            return false;
        }
        let KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            ..
        } = key_event
        else {
            return false;
        };
        if modifiers.contains(KeyModifiers::CONTROL) || modifiers.contains(KeyModifiers::ALT) {
            return false;
        }
        let lower = c.to_ascii_lowercase();
        if let Some(idx) = self
            .options
            .iter()
            .position(|opt| opt.shortcut.map(|s| s == lower).unwrap_or(false))
        {
            self.apply_selection(idx);
            true
        } else {
            false
        }
    }

    fn render_header(&self, area: Rect, buf: &mut Buffer) -> u16 {
        let Some(state) = self.current.as_ref() else {
            return 0;
        };
        let lines = state.header_lines(area.width);
        let mut rendered_rows = 0u16;
        for spans in lines.into_iter().take(area.height as usize) {
            let row = Rect {
                x: area.x,
                y: area.y + rendered_rows,
                width: area.width,
                height: 1,
            };
            let mut prefixed = Vec::with_capacity(spans.len() + 1);
            prefixed.extend(spans);
            Paragraph::new(Line::from(prefixed)).render(row, buf);
            rendered_rows = rendered_rows.saturating_add(1);
        }
        rendered_rows
    }
}

impl BottomPaneView for ApprovalModalView {
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
            && let Some(state) = self.current.as_ref()
        {
            match &state.variant {
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

    fn desired_height(&self, width: u16) -> u16 {
        let header_height = self
            .current
            .as_ref()
            .map(|state| state.header_lines(width).len() as u16)
            .unwrap_or(0);
        header_height + self.list.desired_height(width)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let header_lines = self
            .current
            .as_ref()
            .map(|state| state.header_lines(area.width))
            .unwrap_or_default();
        let header_height = header_lines.len() as u16;
        let header_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: header_height.min(area.height),
        };
        let mut used_height = 0u16;
        if header_height > 0 {
            used_height = self.render_header(header_area, buf);
        }
        let list_area = Rect {
            x: area.x,
            y: area.y + used_height,
            width: area.width,
            height: area.height.saturating_sub(used_height),
        };
        self.list.render(list_area, buf);
    }

    fn try_consume_approval_request(
        &mut self,
        request: ApprovalRequest,
    ) -> Option<ApprovalRequest> {
        self.enqueue_request(request);
        None
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let header_height = self
            .current
            .as_ref()
            .map(|state| state.header_lines(area.width).len() as u16)
            .unwrap_or(0);
        let list_area = Rect {
            x: area.x,
            y: area.y + header_height,
            width: area.width,
            height: area.height.saturating_sub(header_height),
        };
        self.list.cursor_pos(list_area)
    }
}

struct ApprovalRequestState {
    variant: ApprovalVariant,
    header_entries: Vec<HeaderEntry>,
}

impl ApprovalRequestState {
    fn header_lines(&self, width: u16) -> Vec<Vec<Span<'static>>> {
        if width == 0 {
            return Vec::new();
        }
        let available = width.max(1) as usize;
        let mut lines = Vec::new();
        for entry in &self.header_entries {
            match entry {
                HeaderEntry::Spacer => lines.push(Vec::new()),
                HeaderEntry::Reason(text) => {
                    for part in wrap(text, available) {
                        lines.push(vec![part.into_owned().italic()]);
                    }
                }
                HeaderEntry::Info(text) => {
                    for part in wrap(text, available) {
                        lines.push(vec![Span::from(part.into_owned())]);
                    }
                }
            }
        }
        lines
    }
}

impl From<ApprovalRequest> for ApprovalRequestState {
    fn from(value: ApprovalRequest) -> Self {
        match value {
            ApprovalRequest::Exec {
                id,
                command,
                reason,
            } => {
                let mut header_entries = Vec::new();
                if let Some(reason) = reason
                    && !reason.is_empty()
                {
                    header_entries.push(HeaderEntry::Reason(reason));
                    header_entries.push(HeaderEntry::Spacer);
                }
                let command_snippet = exec_snippet(&command);
                if !command_snippet.is_empty() {
                    header_entries.push(HeaderEntry::Info(format!("Command: {command_snippet}")));
                    header_entries.push(HeaderEntry::Spacer);
                }
                Self {
                    variant: ApprovalVariant::Exec { id, command },
                    header_entries,
                }
            }
            ApprovalRequest::ApplyPatch {
                id,
                reason,
                grant_root,
            } => {
                let mut header_entries = Vec::new();
                if let Some(reason) = reason
                    && !reason.is_empty()
                {
                    header_entries.push(HeaderEntry::Reason(reason));
                    header_entries.push(HeaderEntry::Spacer);
                }
                if let Some(root) = grant_root {
                    header_entries.push(HeaderEntry::Info(format!(
                        "Grant write access to {} for the remainder of this session.",
                        root.display()
                    )));
                    header_entries.push(HeaderEntry::Spacer);
                }
                Self {
                    variant: ApprovalVariant::ApplyPatch { id },
                    header_entries,
                }
            }
        }
    }
}

enum ApprovalVariant {
    Exec { id: String, command: Vec<String> },
    ApplyPatch { id: String },
}

#[derive(Clone)]
struct ApprovalOption {
    label: String,
    description: String,
    decision: ReviewDecision,
    shortcut: Option<char>,
}

fn exec_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Approve and run now".to_string(),
            description: "(Y) Run this command one time".to_string(),
            decision: ReviewDecision::Approved,
            shortcut: Some('y'),
        },
        ApprovalOption {
            label: "Always approve this session".to_string(),
            description: "(A) Automatically approve this command for the rest of the session"
                .to_string(),
            decision: ReviewDecision::ApprovedForSession,
            shortcut: Some('a'),
        },
        ApprovalOption {
            label: "Cancel".to_string(),
            description: "(N) Do not run the command".to_string(),
            decision: ReviewDecision::Abort,
            shortcut: Some('n'),
        },
    ]
}

fn patch_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Approve".to_string(),
            description: "Apply the proposed changes (shortcut: Y)".to_string(),
            decision: ReviewDecision::Approved,
            shortcut: Some('y'),
        },
        ApprovalOption {
            label: "Cancel".to_string(),
            description: "Do not apply the changes (shortcut: N)".to_string(),
            decision: ReviewDecision::Abort,
            shortcut: Some('n'),
        },
    ]
}

fn build_exec_history_lines(
    command: Vec<String>,
    decision: ReviewDecision,
) -> Option<Vec<Line<'static>>> {
    use ReviewDecision::*;

    let snippet_span = Span::from(exec_snippet(&command)).dim();

    let (symbol, summary): (Span<'static>, Vec<Span<'static>>) = match decision {
        Approved => (
            "✔ ".green(),
            vec![
                "You ".into(),
                "approved".bold(),
                " codex to run ".into(),
                snippet_span.clone(),
                " this time".bold(),
            ],
        ),
        ApprovedForSession => (
            "✔ ".green(),
            vec![
                "You ".into(),
                "approved".bold(),
                " codex to run ".into(),
                snippet_span.clone(),
                " every time this session".bold(),
            ],
        ),
        Denied => (
            "✗ ".red(),
            vec![
                "You ".into(),
                "did not approve".bold(),
                " codex to run ".into(),
                snippet_span.clone(),
            ],
        ),
        Abort => (
            "✗ ".red(),
            vec![
                "You ".into(),
                "canceled".bold(),
                " the request to run ".into(),
                snippet_span.clone(),
            ],
        ),
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

enum HeaderEntry {
    Reason(String),
    Info(String),
    Spacer,
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
        let mut view = ApprovalModalView::new(make_exec_request(), tx);
        view.enqueue_request(make_exec_request());
        assert_eq!(CancellationEvent::Handled, view.on_ctrl_c());
        assert!(view.queue.is_empty());
        assert!(view.is_complete());
    }

    #[test]
    fn shortcut_triggers_selection() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = ApprovalModalView::new(make_exec_request(), tx);
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

        let view = ApprovalModalView::new(exec_request, tx);
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 6));
        view.render(Rect::new(0, 0, 80, 6), &mut buf);

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
                .any(|line| line.contains("Command: echo hello world")),
            "expected header to include command snippet, got {rendered:?}"
        );
    }

    #[test]
    fn enter_sets_last_selected_index_without_dismissing() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = ApprovalModalView::new(make_exec_request(), tx);
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
