//! Approval overlay for user-gated actions.
//!
//! This module renders modal approval prompts for exec, patch, and MCP elicitation requests.
//! It owns the short-lived state needed to present one request at a time, queue additional
//! requests, emit the matching `AppEvent` decisions, and advance only after each request is
//! explicitly accepted or rejected by the user.
//!
//! The overlay is implemented as a `BottomPaneView` backed by a selection list, so approvals
//! remain in a single modal while status timers are paused. It does not execute commands,
//! apply patches, or persist approval policy; it only renders the prompt and forwards the
//! user's decision to the rest of the app.
//!
//! The queue is treated as a stack (`Vec::push` + `pop`), so the most recently enqueued
//! approval is presented next. Correctness relies on keeping `current_request`,
//! `current_variant`, and the `options` list in sync, and on marking `current_complete`
//! before advancing to prevent double-emitting decisions.
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
use codex_core::features::Feature;
use codex_core::features::Features;
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
///
/// Each variant carries the context needed to render the approval prompt and to emit the
/// corresponding decision back to the core protocol once the user chooses an option.
#[derive(Clone, Debug)]
pub(crate) enum ApprovalRequest {
    /// Exec request requiring user confirmation.
    Exec {
        /// Backend request id used for correlating the approval.
        id: String,

        /// Command and arguments to show and approve.
        command: Vec<String>,

        /// Optional human-readable reason shown to the user.
        reason: Option<String>,

        /// Optional policy amendment to apply when approving.
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    /// Patch application request requiring user confirmation.
    ApplyPatch {
        /// Backend request id used for correlating the approval.
        id: String,

        /// Optional human-readable reason shown to the user.
        reason: Option<String>,

        /// Working directory used to render file paths.
        cwd: PathBuf,

        /// File changes to summarize in the approval overlay.
        changes: HashMap<PathBuf, FileChange>,
    },
    /// MCP elicitation request requiring explicit user consent.
    McpElicitation {
        /// MCP server name that issued the request.
        server_name: String,

        /// Request id used by the MCP server for correlation.
        request_id: RequestId,

        /// Prompt message to display in the overlay.
        message: String,
    },
}

/// Modal overlay asking the user to approve or deny one or more requests.
///
/// The overlay owns the selection list state and advances through a queue of requests, ensuring
/// each approval is resolved before the next one is shown. It keeps rendering-specific state
/// (`ApprovalVariant`, `ApprovalOption`) derived from the current request so the UI stays
/// responsive while the decision is pending.
pub(crate) struct ApprovalOverlay {
    /// Request currently shown to the user.
    current_request: Option<ApprovalRequest>,

    /// Render-specific view of the current request.
    current_variant: Option<ApprovalVariant>,

    /// Pending approvals queued behind the current one, treated as a stack.
    queue: Vec<ApprovalRequest>,

    /// App event channel used to emit approval decisions and history entries.
    app_event_tx: AppEventSender,

    /// List view used to render and navigate approval options.
    list: ListSelectionView,

    /// Option metadata for the current request, kept in sync with `current_variant`.
    options: Vec<ApprovalOption>,

    /// Whether the current request has already been handled.
    current_complete: bool,

    /// Whether the overlay has completed all queued requests.
    done: bool,

    /// Feature flag snapshot used to conditionally show options.
    features: Features,
}

impl ApprovalOverlay {
    /// Create a new overlay seeded with the first approval request.
    ///
    /// The selection view is rebuilt from the request so the overlay is immediately ready to
    /// render without additional setup.
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

    /// Append an additional approval request to the queue.
    ///
    /// Requests are served in last-in-first-out order to ensure the newest prompt is shown next.
    pub fn enqueue_request(&mut self, req: ApprovalRequest) {
        self.queue.push(req);
    }

    /// Make a request the active one and rebuild the option list.
    ///
    /// This resets completion tracking and re-seeds the selection list with the options derived
    /// from the incoming request.
    fn set_current(&mut self, request: ApprovalRequest) {
        self.current_request = Some(request.clone());
        let ApprovalRequestState { variant, header } = ApprovalRequestState::from(request);
        self.current_variant = Some(variant.clone());
        self.current_complete = false;
        let (options, params) = Self::build_options(variant, header, &self.features);
        self.options = options;
        self.list = ListSelectionView::new(params, self.app_event_tx.clone());
    }

    /// Build the options list and selection view params for a request variant.
    ///
    /// The header combines a shared title with the variant-specific renderable content.
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

    /// Apply the selected option and advance the queue.
    ///
    /// The current request is marked complete before moving on so repeated selections do not
    /// double-emit decisions.
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

    /// Emit a decision for an exec approval request.
    ///
    /// Exec decisions also create a history cell so the user can see the outcome in the log.
    fn handle_exec_decision(&self, id: &str, command: &[String], decision: ReviewDecision) {
        let cell = history_cell::new_approval_decision_cell(command.to_vec(), decision.clone());
        self.app_event_tx.send(AppEvent::InsertHistoryCell(cell));
        self.app_event_tx.send(AppEvent::CodexOp(Op::ExecApproval {
            id: id.to_string(),
            decision,
        }));
    }

    /// Emit a decision for a patch approval request.
    fn handle_patch_decision(&self, id: &str, decision: ReviewDecision) {
        self.app_event_tx.send(AppEvent::CodexOp(Op::PatchApproval {
            id: id.to_string(),
            decision,
        }));
    }

    /// Emit a decision for an MCP elicitation request.
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

    /// Advance to the next queued request, or mark the overlay done.
    ///
    /// The queue is treated as a stack, so the most recently enqueued request is shown next.
    fn advance_queue(&mut self) {
        if let Some(next) = self.queue.pop() {
            self.set_current(next);
        } else {
            self.done = true;
        }
    }

    /// Check key bindings and apply any matching option shortcut.
    ///
    /// Ctrl+A opens the full-screen approval view; all other matches map to the configured
    /// per-option shortcuts.
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
    /// Handle keyboard input routed to the overlay.
    fn handle_key_event(&mut self, key_event: KeyEvent) {
        if self.try_handle_shortcut(&key_event) {
            return;
        }
        self.list.handle_key_event(key_event);
        if let Some(idx) = self.list.take_last_selected_index() {
            self.apply_selection(idx);
        }
    }

    /// Handle Ctrl+C by cancelling the overlay.
    ///
    /// If a request is active, the cancellation emits the corresponding abort decision before
    /// clearing the rest of the queue.
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

    /// Return true once all approvals are complete.
    fn is_complete(&self) -> bool {
        self.done
    }

    /// Queue a new approval request while the overlay is active.
    fn try_consume_approval_request(
        &mut self,
        request: ApprovalRequest,
    ) -> Option<ApprovalRequest> {
        self.enqueue_request(request);
        None
    }
}

impl Renderable for ApprovalOverlay {
    /// Return the height required to render the overlay.
    fn desired_height(&self, width: u16) -> u16 {
        self.list.desired_height(width)
    }

    /// Render the overlay into the given buffer region.
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.list.render(area, buf);
    }

    /// Return the cursor position for the overlay, if any.
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.list.cursor_pos(area)
    }
}

/// Precomputed rendering state derived from an approval request.
///
/// This bundles the renderable header with the simplified approval variant used to build
/// option labels and shortcuts.
struct ApprovalRequestState {
    /// Renderable view of the request.
    variant: ApprovalVariant,

    /// Header content shown above the option list.
    header: Box<dyn Renderable>,
}

impl From<ApprovalRequest> for ApprovalRequestState {
    /// Build a renderable state from the incoming request.
    ///
    /// The header includes the optional reason, a preview of the command or diff, and any
    /// prompting text needed for the active variant.
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

/// Render-specific view of an approval request.
///
/// This variant strips out data that is only needed for the header rendering so option building
/// can focus on the decision payload.
#[derive(Clone)]
enum ApprovalVariant {
    /// Exec approval with command preview.
    Exec {
        /// Backend request id for correlating decisions.
        id: String,

        /// Command to display and approve.
        command: Vec<String>,

        /// Optional execpolicy amendment suggested with the request.
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    /// Patch approval with diff summary.
    ApplyPatch {
        /// Backend request id for correlating decisions.
        id: String,
    },
    /// MCP elicitation approval.
    McpElicitation {
        /// MCP server name associated with the request.
        server_name: String,

        /// Request id used by the MCP server for correlation.
        request_id: RequestId,
    },
}

/// Decision payload emitted when an option is selected.
///
/// Each variant matches the protocol-level decision type for its approval flow.
#[derive(Clone)]
enum ApprovalDecision {
    /// Standard approve/deny decision used for exec and patch requests.
    Review(ReviewDecision),

    /// Elicitation response for MCP prompts.
    McpElicitation(ElicitationAction),
}

/// A single selectable option within an approval prompt.
///
/// Options map user-visible labels and shortcuts to the protocol decision they should emit.
#[derive(Clone)]
struct ApprovalOption {
    /// Label shown in the selection list.
    label: String,

    /// Decision emitted when the option is selected.
    decision: ApprovalDecision,

    /// Primary shortcut displayed next to the option, if any.
    display_shortcut: Option<KeyBinding>,

    /// Additional shortcuts that trigger the option.
    additional_shortcuts: Vec<KeyBinding>,
}

impl ApprovalOption {
    /// Iterate over all shortcuts that should activate this option.
    fn shortcuts(&self) -> impl Iterator<Item = KeyBinding> + '_ {
        self.display_shortcut
            .into_iter()
            .chain(self.additional_shortcuts.iter().copied())
    }
}

/// Build options for exec approvals, including optional policy amendments.
///
/// The optional execpolicy amendment is only offered when the feature flag is enabled and the
/// command prefix can be rendered as a single line.
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
            .and_then(|prefix| {
                let rendered_prefix = strip_bash_lc_and_escape(prefix.command());
                if rendered_prefix.contains('\n') || rendered_prefix.contains('\r') {
                    return None;
                }

                Some(ApprovalOption {
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
                })
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

/// Build options for patch approvals.
///
/// Patch approvals include a session-scoped approval option so the user can suppress follow-up
/// prompts for the same set of files.
fn patch_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, proceed".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::Approved),
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
        },
        ApprovalOption {
            label: "Yes, and don't ask again for these files".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::ApprovedForSession),
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('a'))],
        },
        ApprovalOption {
            label: "No, and tell Codex what to do differently".to_string(),
            decision: ApprovalDecision::Review(ReviewDecision::Abort),
            display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
        },
    ]
}

/// Build options for MCP elicitation approvals.
///
/// These options map directly to the `ElicitationAction` variants expected by the core protocol.
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

/// Tests for approval overlay behavior and shortcuts.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_event::AppEvent;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc::unbounded_channel;

    /// Build a basic exec approval request for tests.
    fn make_exec_request() -> ApprovalRequest {
        ApprovalRequest::Exec {
            id: "test".to_string(),
            command: vec!["echo".to_string(), "hi".to_string()],
            reason: Some("reason".to_string()),
            proposed_execpolicy_amendment: None,
        }
    }

    /// Ctrl+C cancels the overlay and clears pending requests.
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

    /// Shortcut keys trigger the expected approval selection.
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

    /// Exec policy shortcut emits an approval with the proposed amendment.
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

    /// Exec policy shortcut is hidden when the feature flag is disabled.
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

    /// Renders a command snippet in the approval header.
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

    /// Wraps exec approval history cells with the expected indentation.
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

    /// Enter selects the default option without dismissing queued approvals.
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
