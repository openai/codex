//! Approval modal rendering and decision routing for high-risk operations.
//!
//! This module converts agent approval requests (exec/apply-patch/MCP
//! elicitation) into a list-selection view with action-specific options and
//! shortcuts. It owns two important contracts:
//!
//! 1. Selection always emits an explicit decision event back to the app.
//! 2. MCP elicitation keeps `Esc` mapped to `Cancel`, even with custom
//!    keybindings, so dismissal never silently becomes "continue without info".
//!
//! This module does not evaluate whether an action is safe to run; it only
//! presents choices and routes user decisions.

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
use crate::key_hint::KeyBindingListExt;
use crate::keymap::ApprovalKeymap;
use crate::keymap::ListKeymap;
use crate::render::highlight::highlight_bash_to_lines;
use crate::render::renderable::ColumnRenderable;
use crate::render::renderable::Renderable;
use codex_core::features::Features;
use codex_core::protocol::ElicitationAction;
use codex_core::protocol::ExecPolicyAmendment;
use codex_core::protocol::FileChange;
use codex_core::protocol::NetworkApprovalContext;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_protocol::mcp::RequestId;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
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
        network_approval_context: Option<NetworkApprovalContext>,
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    ApplyPatch {
        id: String,
        reason: Option<String>,
        cwd: PathBuf,
        changes: HashMap<PathBuf, FileChange>,
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
    approval_keymap: ApprovalKeymap,
    list_keymap: ListKeymap,
}

impl ApprovalOverlay {
    pub fn new(
        request: ApprovalRequest,
        app_event_tx: AppEventSender,
        features: Features,
        approval_keymap: ApprovalKeymap,
        list_keymap: ListKeymap,
    ) -> Self {
        let mut view = Self {
            current_request: None,
            current_variant: None,
            queue: Vec::new(),
            app_event_tx: app_event_tx.clone(),
            list: ListSelectionView::new(Default::default(), app_event_tx, list_keymap.clone()),
            options: Vec::new(),
            current_complete: false,
            done: false,
            features,
            approval_keymap,
            list_keymap,
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
        let (options, params) =
            Self::build_options(variant, header, &self.features, &self.approval_keymap);
        self.options = options;
        self.list =
            ListSelectionView::new(params, self.app_event_tx.clone(), self.list_keymap.clone());
    }

    fn build_options(
        variant: ApprovalVariant,
        header: Box<dyn Renderable>,
        features: &Features,
        approval_keymap: &ApprovalKeymap,
    ) -> (Vec<ApprovalOption>, SelectionViewParams) {
        let (options, title) = match &variant {
            ApprovalVariant::Exec {
                network_approval_context,
                proposed_execpolicy_amendment,
                ..
            } => (
                exec_options(
                    proposed_execpolicy_amendment.clone(),
                    network_approval_context.as_ref(),
                    features,
                    approval_keymap,
                ),
                network_approval_context.as_ref().map_or_else(
                    || "Would you like to run the following command?".to_string(),
                    |network_approval_context| {
                        format!(
                            "Do you want to approve access to \"{}\"?",
                            network_approval_context.host
                        )
                    },
                ),
            ),
            ApprovalVariant::ApplyPatch { .. } => (
                patch_options(approval_keymap),
                "Would you like to make the following edits?".to_string(),
            ),
            ApprovalVariant::McpElicitation { server_name, .. } => (
                elicitation_options(approval_keymap),
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
                display_shortcut: opt.shortcuts.first().copied(),
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
            turn_id: None,
            decision,
        }));
    }

    fn handle_patch_decision(&self, id: &str, decision: ReviewDecision) {
        self.app_event_tx.send(AppEvent::CodexOp(Op::PatchApproval {
            id: id.to_string(),
            decision,
        }));
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

    /// Apply approval-specific shortcuts before delegating to list navigation.
    ///
    /// `open_fullscreen` is handled here because it is orthogonal to list item
    /// selection and should work regardless of current highlighted row.
    fn try_handle_shortcut(&mut self, key_event: &KeyEvent) -> bool {
        if key_event.kind == KeyEventKind::Press
            && self.approval_keymap.open_fullscreen.is_pressed(*key_event)
            && let Some(request) = self.current_request.as_ref()
        {
            self.app_event_tx
                .send(AppEvent::FullScreenApprovalRequest(request.clone()));
            return true;
        }

        if let Some(idx) = self
            .options
            .iter()
            .position(|opt| opt.shortcuts.iter().any(|s| s.is_press(*key_event)))
        {
            self.apply_selection(idx);
            true
        } else {
            false
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
                network_approval_context,
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
                        network_approval_context,
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

#[derive(Clone)]
enum ApprovalVariant {
    Exec {
        id: String,
        command: Vec<String>,
        network_approval_context: Option<NetworkApprovalContext>,
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    ApplyPatch {
        id: String,
    },
    McpElicitation {
        server_name: String,
        request_id: RequestId,
    },
}

#[derive(Clone)]
enum ApprovalDecision {
    Review(ReviewDecision),
    McpElicitation(ElicitationAction),
}

#[derive(Clone)]
struct ApprovalOption {
    label: String,
    decision: ApprovalDecision,
    shortcuts: Vec<KeyBinding>,
}

fn exec_options(
    proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    network_approval_context: Option<&NetworkApprovalContext>,
    _features: &Features,
    keymap: &ApprovalKeymap,
) -> Vec<ApprovalOption> {
    if network_approval_context.is_some() {
        return vec![
            ApprovalOption {
                label: "Yes, just this once".to_string(),
                decision: ApprovalDecision::Review(ReviewDecision::Approved),
                shortcuts: keymap.approve.clone(),
            },
            ApprovalOption {
                label: "Yes, and allow this host for this session".to_string(),
                decision: ApprovalDecision::Review(ReviewDecision::ApprovedForSession),
                shortcuts: keymap.approve_for_session.clone(),
            },
            ApprovalOption {
                label: "No, and tell Codex what to do differently".to_string(),
                decision: ApprovalDecision::Review(ReviewDecision::Abort),
                shortcuts: keymap.decline.clone(),
            },
        ];
    }

    vec![ApprovalOption {
        label: "Yes, proceed".to_string(),
        decision: ApprovalDecision::Review(ReviewDecision::Approved),
        shortcuts: keymap.approve.clone(),
    }]
    .into_iter()
    .chain(proposed_execpolicy_amendment.and_then(|prefix| {
        let rendered_prefix = strip_bash_lc_and_escape(prefix.command());
        if rendered_prefix.contains('\n') || rendered_prefix.contains('\r') {
            return None;
        }

        Some(ApprovalOption {
            label: format!(
                "Yes, and don't ask again for commands that start with `{rendered_prefix}`"
            ),
            decision: ApprovalDecision::Review(ReviewDecision::ApprovedExecpolicyAmendment {
                proposed_execpolicy_amendment: prefix,
            }),
            shortcuts: keymap.approve_for_prefix.clone(),
        })
    }))
    .chain([ApprovalOption {
        label: "No, and tell Codex what to do differently".to_string(),
        decision: ApprovalDecision::Review(ReviewDecision::Abort),
        shortcuts: keymap.decline.clone(),
    }])
    .collect()
}

fn patch_options(keymap: &ApprovalKeymap) -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, proceed".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::Approved),
            shortcuts: keymap.approve.clone(),
        },
        ApprovalOption {
            label: "Yes, and don't ask again for these files".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::ApprovedForSession),
            shortcuts: keymap.approve_for_session.clone(),
        },
        ApprovalOption {
            label: "No, and tell Codex what to do differently".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::Abort),
            shortcuts: keymap.decline.clone(),
        },
    ]
}

/// Build MCP elicitation options with stable cancellation semantics.
///
/// `Esc` is always treated as cancel for elicitation prompts, even if users
/// customize `decline`/`cancel` bindings. We keep this as a hard contract so
/// dismissal remains a safe abort path and never silently maps to "continue
/// without requested info." Any decline/cancel overlap is removed from the
/// decline option in elicitation mode to preserve this invariant.
fn elicitation_options(keymap: &ApprovalKeymap) -> Vec<ApprovalOption> {
    let mut cancel_shortcuts = vec![key_hint::plain(KeyCode::Esc)];
    for shortcut in &keymap.cancel {
        if !cancel_shortcuts.contains(shortcut) {
            cancel_shortcuts.push(*shortcut);
        }
    }

    let decline_shortcuts: Vec<KeyBinding> = keymap
        .decline
        .iter()
        .copied()
        .filter(|shortcut| !cancel_shortcuts.contains(shortcut))
        .collect();

    vec![
        ApprovalOption {
            label: "Yes, provide the requested info".to_string(),
            decision: ApprovalDecision::McpElicitation(ElicitationAction::Accept),
            shortcuts: keymap.approve.clone(),
        },
        ApprovalOption {
            label: "No, but continue without it".to_string(),
            decision: ApprovalDecision::McpElicitation(ElicitationAction::Decline),
            shortcuts: decline_shortcuts,
        },
        ApprovalOption {
            label: "Cancel this request".to_string(),
            decision: ApprovalDecision::McpElicitation(ElicitationAction::Cancel),
            shortcuts: cancel_shortcuts,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use codex_core::protocol::NetworkApprovalProtocol;
    use crossterm::event::KeyModifiers;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    fn make_overlay(
        request: ApprovalRequest,
        app_event_tx: AppEventSender,
        features: Features,
    ) -> ApprovalOverlay {
        let keymap = crate::keymap::RuntimeKeymap::defaults();
        make_overlay_with_keymap(
            request,
            app_event_tx,
            features,
            keymap.approval,
            keymap.list,
        )
    }

    fn make_overlay_with_keymap(
        request: ApprovalRequest,
        app_event_tx: AppEventSender,
        features: Features,
        approval_keymap: ApprovalKeymap,
        list_keymap: ListKeymap,
    ) -> ApprovalOverlay {
        ApprovalOverlay::new(
            request,
            app_event_tx,
            features,
            approval_keymap,
            list_keymap,
        )
    }

    fn make_exec_request() -> ApprovalRequest {
        ApprovalRequest::Exec {
            id: "test".to_string(),
            command: vec!["echo".to_string(), "hi".to_string()],
            reason: Some("reason".to_string()),
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
        }
    }

    fn make_elicitation_request() -> ApprovalRequest {
        ApprovalRequest::McpElicitation {
            server_name: "test-server".to_string(),
            request_id: RequestId::String("request-1".to_string()),
            message: "Need more information".to_string(),
        }
    }

    #[test]
    fn ctrl_c_aborts_and_clears_queue() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = make_overlay(make_exec_request(), tx, Features::with_defaults());
        view.enqueue_request(make_exec_request());
        assert_eq!(CancellationEvent::Handled, view.on_ctrl_c());
        assert!(view.queue.is_empty());
        assert!(view.is_complete());
    }

    #[test]
    fn shortcut_triggers_selection() {
        let (tx, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let mut view = make_overlay(make_exec_request(), tx, Features::with_defaults());
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
        let mut view = make_overlay(
            ApprovalRequest::Exec {
                id: "test".to_string(),
                command: vec!["echo".to_string()],
                reason: None,
                network_approval_context: None,
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
    fn header_includes_command_snippet() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let command = vec!["echo".into(), "hello".into(), "world".into()];
        let exec_request = ApprovalRequest::Exec {
            id: "test".into(),
            command,
            reason: None,
            network_approval_context: None,
            proposed_execpolicy_amendment: None,
        };

        let view = make_overlay(exec_request, tx, Features::with_defaults());
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
    fn network_exec_options_use_expected_labels_and_hide_execpolicy_amendment() {
        let network_context = NetworkApprovalContext {
            host: "example.com".to_string(),
            protocol: NetworkApprovalProtocol::Https,
        };
        let keymap = crate::keymap::RuntimeKeymap::defaults();
        let options = exec_options(
            Some(ExecPolicyAmendment::new(vec!["curl".to_string()])),
            Some(&network_context),
            &Features::with_defaults(),
            &keymap.approval,
        );

        let labels: Vec<String> = options.into_iter().map(|option| option.label).collect();
        assert_eq!(
            labels,
            vec![
                "Yes, just this once".to_string(),
                "Yes, and allow this host for this session".to_string(),
                "No, and tell Codex what to do differently".to_string(),
            ]
        );
    }

    #[test]
    fn network_exec_prompt_title_includes_host() {
        let (tx, _rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx);
        let exec_request = ApprovalRequest::Exec {
            id: "test".into(),
            command: vec!["curl".into(), "https://example.com".into()],
            reason: Some("network request blocked".into()),
            network_approval_context: Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Https,
            }),
            proposed_execpolicy_amendment: Some(ExecPolicyAmendment::new(vec!["curl".into()])),
        };

        let view = make_overlay(exec_request, tx, Features::with_defaults());
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, view.desired_height(100)));
        view.render(Rect::new(0, 0, 100, view.desired_height(100)), &mut buf);

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
                .any(|line| line.contains("Do you want to approve access to \"example.com\"?")),
            "expected network title to include host, got {rendered:?}"
        );
        assert!(
            !rendered.iter().any(|line| line.contains("don't ask again")),
            "network prompt should not show execpolicy option, got {rendered:?}"
        );
    }

    #[test]
    fn ctrl_shift_a_opens_fullscreen() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = make_overlay(make_exec_request(), tx, Features::with_defaults());

        view.handle_key_event(KeyEvent::new(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ));

        let mut saw_fullscreen = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, AppEvent::FullScreenApprovalRequest(_)) {
                saw_fullscreen = true;
                break;
            }
        }
        assert!(saw_fullscreen, "expected ctrl+shift+a to open fullscreen");
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
    fn esc_cancels_mcp_elicitation() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = make_overlay(make_elicitation_request(), tx, Features::with_defaults());

        view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        let mut decision = None;
        while let Ok(ev) = rx.try_recv() {
            if let AppEvent::CodexOp(Op::ResolveElicitation { decision: d, .. }) = ev {
                decision = Some(d);
                break;
            }
        }
        assert_eq!(decision, Some(ElicitationAction::Cancel));
    }

    #[test]
    fn esc_still_cancels_elicitation_with_custom_overlap() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut keymap = crate::keymap::RuntimeKeymap::defaults();
        keymap.approval.decline = vec![
            key_hint::plain(KeyCode::Esc),
            key_hint::plain(KeyCode::Char('n')),
        ];
        keymap.approval.cancel = vec![key_hint::plain(KeyCode::Char('x'))];

        let mut view = make_overlay_with_keymap(
            make_elicitation_request(),
            tx,
            Features::with_defaults(),
            keymap.approval,
            keymap.list,
        );

        view.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let mut esc_decision = None;
        while let Ok(ev) = rx.try_recv() {
            if let AppEvent::CodexOp(Op::ResolveElicitation { decision, .. }) = ev {
                esc_decision = Some(decision);
                break;
            }
        }
        assert_eq!(esc_decision, Some(ElicitationAction::Cancel));

        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut keymap = crate::keymap::RuntimeKeymap::defaults();
        keymap.approval.decline = vec![
            key_hint::plain(KeyCode::Esc),
            key_hint::plain(KeyCode::Char('n')),
        ];
        keymap.approval.cancel = vec![key_hint::plain(KeyCode::Char('x'))];

        let mut view = make_overlay_with_keymap(
            make_elicitation_request(),
            tx,
            Features::with_defaults(),
            keymap.approval,
            keymap.list,
        );
        view.handle_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        let mut n_decision = None;
        while let Ok(ev) = rx.try_recv() {
            if let AppEvent::CodexOp(Op::ResolveElicitation { decision, .. }) = ev {
                n_decision = Some(decision);
                break;
            }
        }
        assert_eq!(n_decision, Some(ElicitationAction::Decline));
    }

    #[test]
    fn enter_sets_last_selected_index_without_dismissing() {
        let (tx_raw, mut rx) = unbounded_channel::<AppEvent>();
        let tx = AppEventSender::new(tx_raw);
        let mut view = make_overlay(make_exec_request(), tx, Features::with_defaults());
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
