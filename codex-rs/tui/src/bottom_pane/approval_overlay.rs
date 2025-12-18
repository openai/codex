use std::collections::HashMap;
use std::path::PathBuf;

use crate::app_event::AppEvent;
use crate::app_event::NetworkProxyDecision;
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
use codex_core::config::types::NetworkProxyMode;
use codex_core::features::Feature;
use codex_core::features::Features;
use codex_core::network_proxy::NetworkProxyBlockedRequest;
use codex_core::protocol::ElicitationAction;
use codex_core::protocol::ExecPolicyAmendment;
use codex_core::protocol::FileChange;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use mcp_types::RequestId;
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
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    ApplyPatch {
        id: String,
        reason: Option<String>,
        cwd: PathBuf,
        changes: HashMap<PathBuf, FileChange>,
    },
    Network {
        request: NetworkProxyBlockedRequest,
    },
    McpElicitation {
        server_name: String,
        request_id: RequestId,
        message: String,
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
    features: Features,
}

impl ApprovalOverlay {
    pub fn new(request: ApprovalRequest, app_event_tx: AppEventSender, features: Features) -> Self {
        let mut view = Self {
            current_request: None,
            current_variant: None,
            queue: Vec::new(),
            app_event_tx: app_event_tx.clone(),
            list: ListSelectionView::new(Default::default(), app_event_tx),
            options: Vec::new(),
            current_complete: false,
            done: false,
            features,
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
        let (options, params) = Self::build_options(variant, header, &self.features);
        self.options = options;
        self.list = ListSelectionView::new(params, self.app_event_tx.clone());
    }

    fn build_options(
        variant: ApprovalVariant,
        header: Box<dyn Renderable>,
        features: &Features,
    ) -> (Vec<ApprovalOption>, SelectionViewParams) {
        let (options, title) = match &variant {
            ApprovalVariant::Exec {
                proposed_execpolicy_amendment,
                ..
            } => (
                exec_options(proposed_execpolicy_amendment.clone(), features),
                "Would you like to run the following command?".to_string(),
            ),
            ApprovalVariant::ApplyPatch { .. } => (
                patch_options(),
                "Would you like to make the following edits?".to_string(),
            ),
            ApprovalVariant::Network { preflight_only, .. } => (
                network_options(*preflight_only),
                "Allow network access to this domain?".to_string(),
            ),
            ApprovalVariant::McpElicitation { server_name, .. } => (
                elicitation_options(),
                format!("{server_name} needs your approval."),
            ),
        };

        let header = Box::new(ColumnRenderable::with([
            Line::from(title.bold()).into(),
            Line::from("").into(),
            header,
        ]));

        let items = options
            .iter()
            .map(|opt| SelectionItem {
                name: opt.label.clone(),
                display_shortcut: opt
                    .display_shortcut
                    .or_else(|| opt.additional_shortcuts.first().copied()),
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
            match (variant, &option.decision) {
                (ApprovalVariant::Exec { id, command, .. }, ApprovalDecision::Review(decision)) => {
                    self.handle_exec_decision(id, command, decision.clone());
                }
                (ApprovalVariant::ApplyPatch { id, .. }, ApprovalDecision::Review(decision)) => {
                    self.handle_patch_decision(id, decision.clone());
                }
                (ApprovalVariant::Network { host, .. }, ApprovalDecision::Network(decision)) => {
                    self.handle_network_decision(host, *decision);
                }
                (
                    ApprovalVariant::McpElicitation {
                        server_name,
                        request_id,
                    },
                    ApprovalDecision::McpElicitation(decision),
                ) => {
                    self.handle_elicitation_decision(server_name, request_id, *decision);
                }
                _ => {}
            }
        }

        self.current_complete = true;
        self.advance_queue();
    }

    fn handle_exec_decision(&self, id: &str, command: &[String], decision: ReviewDecision) {
        let cell = history_cell::new_approval_decision_cell(command.to_vec(), decision.clone());
        self.app_event_tx.send(AppEvent::InsertHistoryCell(cell));
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

    fn handle_network_decision(&self, host: &str, decision: NetworkProxyDecision) {
        let cell = history_cell::new_network_approval_decision_cell(host.to_string(), decision);
        self.app_event_tx.send(AppEvent::InsertHistoryCell(cell));
        self.app_event_tx.send(AppEvent::NetworkProxyDecision {
            host: host.to_string(),
            decision,
        });
    }

    fn handle_elicitation_decision(
        &self,
        server_name: &str,
        request_id: &RequestId,
        decision: ElicitationAction,
    ) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::ResolveElicitation {
                server_name: server_name.to_string(),
                request_id: request_id.clone(),
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
                ApprovalVariant::Exec { id, command, .. } => {
                    self.handle_exec_decision(id, command, ReviewDecision::Abort);
                }
                ApprovalVariant::ApplyPatch { id, .. } => {
                    self.handle_patch_decision(id, ReviewDecision::Abort);
                }
                ApprovalVariant::Network { host, .. } => {
                    self.handle_network_decision(host, NetworkProxyDecision::Deny);
                }
                ApprovalVariant::McpElicitation {
                    server_name,
                    request_id,
                } => {
                    self.handle_elicitation_decision(
                        server_name,
                        request_id,
                        ElicitationAction::Cancel,
                    );
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
}

impl Renderable for ApprovalOverlay {
    fn desired_height(&self, width: u16) -> u16 {
        self.list.desired_height(width)
    }

    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.list.render(area, buf);
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.list.cursor_pos(area)
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
                proposed_execpolicy_amendment,
            } => {
                let mut header: Vec<Line<'static>> = Vec::new();
                if let Some(reason) = reason {
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
                    variant: ApprovalVariant::Exec {
                        id,
                        command,
                        proposed_execpolicy_amendment,
                    },
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
                if let Some(reason) = reason
                    && !reason.is_empty()
                {
                    header.push(Box::new(
                        Paragraph::new(Line::from_iter(["Reason: ".into(), reason.italic()]))
                            .wrap(Wrap { trim: false }),
                    ));
                    header.push(Box::new(Line::from("")));
                }
                header.push(DiffSummary::new(changes, cwd).into());
                Self {
                    variant: ApprovalVariant::ApplyPatch { id },
                    header: Box::new(ColumnRenderable::with(header)),
                }
            }
            ApprovalRequest::Network { request } => {
                let mut header: Vec<Line<'static>> = Vec::new();
                let host = request.host.trim().to_string();
                if !host.is_empty() {
                    header.push(Line::from(vec!["Host: ".into(), host.clone().bold()]));
                }
                let reason = request.reason.trim().to_string();
                if !reason.is_empty() {
                    let reason_label = network_reason_label(&reason);
                    header.push(Line::from(vec!["Reason: ".into(), reason_label.into()]));
                    if let Some(hint) = network_reason_hint(&reason) {
                        header.push(Line::from(vec!["Hint: ".into(), hint.dim()]));
                    }
                }
                if let Some(method) = request
                    .method
                    .as_ref()
                    .filter(|value| !value.is_empty())
                    .cloned()
                {
                    header.push(Line::from(vec!["Method: ".into(), method.into()]));
                }
                if cfg!(debug_assertions) {
                    let protocol = request.protocol.trim().to_string();
                    if !protocol.is_empty() {
                        header.push(Line::from(vec!["Protocol: ".into(), protocol.into()]));
                    }
                    if let Some(mode) = request.mode {
                        let label = match mode {
                            NetworkProxyMode::Limited => "limited",
                            NetworkProxyMode::Full => "full",
                        };
                        header.push(Line::from(vec!["Mode: ".into(), label.into()]));
                    }
                }
                if let Some(client) = request
                    .client
                    .as_ref()
                    .filter(|value| !value.is_empty())
                    .cloned()
                {
                    header.push(Line::from(vec!["Client: ".into(), client.dim()]));
                }
                let preflight_only = request.protocol.trim().eq_ignore_ascii_case("preflight");
                Self {
                    variant: ApprovalVariant::Network {
                        host,
                        preflight_only,
                    },
                    header: Box::new(Paragraph::new(header).wrap(Wrap { trim: false })),
                }
            }
            ApprovalRequest::McpElicitation {
                server_name,
                request_id,
                message,
            } => {
                let header = Paragraph::new(vec![
                    Line::from(vec!["Server: ".into(), server_name.clone().bold()]),
                    Line::from(""),
                    Line::from(message),
                ])
                .wrap(Wrap { trim: false });
                Self {
                    variant: ApprovalVariant::McpElicitation {
                        server_name,
                        request_id,
                    },
                    header: Box::new(header),
                }
            }
        }
    }
}

fn network_reason_label(reason: &str) -> String {
    match reason {
        "not_allowed" => "Domain not in allowlist".to_string(),
        "not_allowed_local" => "Loopback blocked by policy".to_string(),
        "denied" => "Domain denied by denylist".to_string(),
        "method_not_allowed" => "Method blocked by network mode".to_string(),
        "mitm_required" => "MITM required for limited HTTPS".to_string(),
        _ => reason.to_string(),
    }
}

fn network_reason_hint(reason: &str) -> Option<&'static str> {
    match reason {
        "not_allowed_local" => Some("Allow loopback or add the host to the allowlist."),
        "method_not_allowed" => Some("Switch to full mode or enable MITM to allow this method."),
        "mitm_required" => Some("Enable MITM or switch to full mode for HTTPS tunneling."),
        _ => None,
    }
}

#[derive(Clone)]
enum ApprovalVariant {
    Exec {
        id: String,
        command: Vec<String>,
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    ApplyPatch {
        id: String,
    },
    Network {
        host: String,
        preflight_only: bool,
    },
    McpElicitation {
        server_name: String,
        request_id: RequestId,
    },
}

#[derive(Clone)]
enum ApprovalDecision {
    Review(ReviewDecision),
    Network(NetworkProxyDecision),
    McpElicitation(ElicitationAction),
}

#[derive(Clone)]
struct ApprovalOption {
    label: String,
    decision: ApprovalDecision,
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

fn exec_options(
    proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    features: &Features,
) -> Vec<ApprovalOption> {
    vec![ApprovalOption {
        label: "Yes, proceed".to_string(),
        decision: ApprovalDecision::Review(ReviewDecision::Approved),
        display_shortcut: None,
        additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
    }]
    .into_iter()
    .chain(
        proposed_execpolicy_amendment
            .filter(|_| features.enabled(Feature::ExecPolicy))
            .map(|prefix| {
                let rendered_prefix = strip_bash_lc_and_escape(prefix.command());
                ApprovalOption {
                    label: format!(
                        "Yes, and don't ask again for commands that start with `{rendered_prefix}`"
                    ),
                    decision: ApprovalDecision::Review(
                        ReviewDecision::ApprovedExecpolicyAmendment {
                            proposed_execpolicy_amendment: prefix,
                        },
                    ),
                    display_shortcut: None,
                    additional_shortcuts: vec![key_hint::plain(KeyCode::Char('p'))],
                }
            }),
    )
    .chain([ApprovalOption {
        label: "No, and tell Codex what to do differently".to_string(),
        decision: ApprovalDecision::Review(ReviewDecision::Abort),
        display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
        additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
    }])
    .collect()
}

fn patch_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, proceed".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::Approved),
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
        },
        ApprovalOption {
            label: "No, and tell Codex what to do differently".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::Abort),
            display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
        },
    ]
}

fn network_options(preflight_only: bool) -> Vec<ApprovalOption> {
    let mut options = Vec::new();
    let mut allow_session = ApprovalOption {
        label: if preflight_only {
            "Allow for session".to_string()
        } else {
            "Allow".to_string()
        },
        decision: ApprovalDecision::Network(NetworkProxyDecision::AllowSession),
        display_shortcut: None,
        additional_shortcuts: vec![key_hint::plain(KeyCode::Char('s'))],
    };
    if preflight_only {
        allow_session
            .additional_shortcuts
            .push(key_hint::plain(KeyCode::Char('y')));
    }
    options.push(allow_session);
    options.push(ApprovalOption {
        label: "Allow always (add to allowlist)".to_string(),
        decision: ApprovalDecision::Network(NetworkProxyDecision::AllowAlways),
        display_shortcut: None,
        additional_shortcuts: vec![key_hint::plain(KeyCode::Char('a'))],
    });
    options.push(ApprovalOption {
        label: "Deny (add to denylist)".to_string(),
        decision: ApprovalDecision::Network(NetworkProxyDecision::Deny),
        display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
        additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
    });
    options
}

fn elicitation_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, provide the requested info".to_string(),
            decision: ApprovalDecision::McpElicitation(ElicitationAction::Accept),
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
        },
        ApprovalOption {
            label: "No, but continue without it".to_string(),
            decision: ApprovalDecision::McpElicitation(ElicitationAction::Decline),
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
        },
        ApprovalOption {
            label: "Cancel this request".to_string(),
            decision: ApprovalDecision::McpElicitation(ElicitationAction::Cancel),
            display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('c'))],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    fn make_exec_request() -> ApprovalRequest {
        ApprovalRequest::Exec {
            id: "test".to_string(),
            command: vec!["echo".to_string(), "hi".to_string()],
            reason: Some("reason".to_string()),
            proposed_execpolicy_amendment: None,
        }
    }

    #[test]
    fn ctrl_c_aborts_and_clears_queue() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = ApprovalOverlay::new(make_exec_request(), tx, Features::with_defaults());
        view.enqueue_request(make_exec_request());
        assert_eq!(CancellationEvent::Handled, view.on_ctrl_c());
        assert!(view.queue.is_empty());
        assert!(view.is_complete());
    }

    #[test]
    fn shortcut_triggers_selection() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = ApprovalOverlay::new(make_exec_request(), tx, Features::with_defaults());
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
    fn exec_prefix_option_emits_execpolicy_amendment() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = ApprovalOverlay::new(
            ApprovalRequest::Exec {
                id: "test".to_string(),
                command: vec!["echo".to_string()],
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                    "echo".to_string(),
                ])),
            },
            tx,
            Features::with_defaults(),
        );
        view.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        let mut saw_op = false;
        while let Ok(ev) = rx.try_recv() {
            if let AppEvent::CodexOp(Op::ExecApproval { decision, .. }) = ev {
                assert_eq!(
                    decision,
                    ReviewDecision::ApprovedExecpolicyAmendment {
                        proposed_execpolicy_amendment: ExecPolicyAmendment::new(vec![
                            "echo".to_string()
                        ])
                    }
                );
                saw_op = true;
                break;
            }
        }
        assert!(
            saw_op,
            "expected approval decision to emit an op with command prefix"
        );
    }

    #[test]
    fn exec_prefix_option_hidden_when_execpolicy_disabled() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = ApprovalOverlay::new(
            ApprovalRequest::Exec {
                id: "test".to_string(),
                command: vec!["echo".to_string()],
                reason: None,
                proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec![
                    "echo".to_string(),
                ])),
            },
            tx,
            {
                let mut features = Features::with_defaults();
                features.disable(Feature::ExecPolicy);
                features
            },
        );
        assert_eq!(view.options.len(), 2);
        view.handle_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        assert!(!view.is_complete());
        assert!(rx.try_recv().is_err());
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
            proposed_execpolicy_amendment: None,
        };

        let view = ApprovalOverlay::new(exec_request, tx, Features::with_defaults());
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
    fn exec_history_cell_wraps_with_two_space_indent() {
        let command = vec![
            "/bin/zsh".into(),
            "-lc".into(),
            "git add tui/src/render/mod.rs tui/src/render/renderable.rs".into(),
        ];
        let cell = history_cell::new_approval_decision_cell(command, ReviewDecision::Approved);
        let lines = cell.display_lines(28);
        let rendered: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect();
        let expected = vec![
            "✔ You approved codex to run".to_string(),
            "  git add tui/src/render/".to_string(),
            "  mod.rs tui/src/render/".to_string(),
            "  renderable.rs this time".to_string(),
        ];
        assert_eq!(rendered, expected);
    }

    #[test]
    fn enter_sets_last_selected_index_without_dismissing() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = ApprovalOverlay::new(make_exec_request(), tx, Features::with_defaults());
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
        assert_eq!(decision, Some(ReviewDecision::Approved));
    }
}
