use std::collections::HashMap;
use std::path::PathBuf;

use super::approval_overlay_ext;
pub use super::approval_overlay_ext::MultiQuestionState;
pub use super::approval_overlay_ext::MultiSelectState;
use super::approval_overlay_ext::build_question_header_lines;
use super::approval_overlay_ext::build_question_title;
use super::approval_overlay_ext::build_user_question_header;
use super::approval_overlay_ext::get_selected_labels;
use super::approval_overlay_ext::toggle_checkbox_label;

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
use codex_protocol::protocol_ext::PlanExitPermissionMode;
use codex_protocol::protocol_ext::UserQuestion;
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
    McpElicitation {
        server_name: String,
        request_id: RequestId,
        message: String,
    },
    /// Plan Mode approval request - user approves or rejects the plan.
    Plan {
        plan_content: String,
        plan_file_path: String,
    },
    /// Plan Mode entry request - LLM requests to enter plan mode.
    EnterPlanMode,
    /// User question request - LLM asks the user questions.
    UserQuestion {
        tool_call_id: String,
        questions: Vec<UserQuestion>,
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
    /// State for multi-question flow (answering questions one at a time)
    multi_question_state: Option<MultiQuestionState>,
    /// State for multiSelect toggle (checkbox-style selection)
    multi_select_state: Option<MultiSelectState>,
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
            multi_question_state: None,
            multi_select_state: None,
        };
        view.set_current(request);
        view
    }

    pub fn enqueue_request(&mut self, req: ApprovalRequest) {
        self.queue.push(req);
    }

    fn set_current(&mut self, request: ApprovalRequest) {
        self.current_request = Some(request.clone());
        let ApprovalRequestState { variant, header } = ApprovalRequestState::from(request.clone());
        self.current_variant = Some(variant.clone());
        self.current_complete = false;

        // Initialize multi-question and multi-select state for UserQuestion requests
        if let ApprovalRequest::UserQuestion {
            tool_call_id,
            questions,
        } = request
        {
            if questions.len() > 1 {
                // Multi-question: show one question at a time
                self.multi_question_state = Some(MultiQuestionState {
                    current_index: 0,
                    collected_answers: HashMap::new(),
                    questions: questions.clone(),
                    tool_call_id: tool_call_id.clone(),
                });
            } else {
                self.multi_question_state = None;
            }

            // Check if current question has multi_select
            let current_q = if let Some(ref mq_state) = self.multi_question_state {
                questions.get(mq_state.current_index)
            } else {
                questions.first()
            };
            if current_q.is_some_and(|q| q.multi_select) {
                self.multi_select_state = Some(MultiSelectState::default());
            } else {
                self.multi_select_state = None;
            }
        } else {
            self.multi_question_state = None;
            self.multi_select_state = None;
        }

        let (options, params) =
            Self::build_options(variant, header, &self.features, &self.multi_question_state);
        self.options = options;
        self.list = ListSelectionView::new(params, self.app_event_tx.clone());
    }

    fn build_options(
        variant: ApprovalVariant,
        header: Box<dyn Renderable>,
        features: &Features,
        multi_question_state: &Option<MultiQuestionState>,
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
            ApprovalVariant::Plan => (
                approval_overlay_ext::plan_options(),
                "Would you like to approve this plan?".to_string(),
            ),
            ApprovalVariant::EnterPlanMode => (
                approval_overlay_ext::enter_plan_mode_options(),
                "Allow the LLM to enter plan mode?".to_string(),
            ),
            ApprovalVariant::UserQuestion {
                tool_call_id,
                questions,
            } => {
                // For multi-question, show current question only
                if let Some(mq_state) = multi_question_state {
                    let current_q = &mq_state.questions[mq_state.current_index];
                    let total = mq_state.questions.len();
                    let current_num = mq_state.current_index + 1;
                    (
                        approval_overlay_ext::single_question_options(
                            tool_call_id.clone(),
                            current_q.clone(),
                            current_q.multi_select,
                        ),
                        format!(
                            "[{}/{}] {}: {}",
                            current_num, total, current_q.header, current_q.question
                        ),
                    )
                } else {
                    // Single question: show directly
                    let q = &questions[0];
                    (
                        approval_overlay_ext::single_question_options(
                            tool_call_id.clone(),
                            q.clone(),
                            q.multi_select,
                        ),
                        format!("{}: {}", q.header, q.question),
                    )
                }
            }
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
        let Some(option) = self.options.get(actual_idx).cloned() else {
            return;
        };
        if let Some(variant) = self.current_variant.clone() {
            match (&variant, &option.decision) {
                (ApprovalVariant::Exec { id, command, .. }, ApprovalDecision::Review(decision)) => {
                    self.handle_exec_decision(id, command, decision.clone());
                    self.current_complete = true;
                    self.advance_queue();
                }
                (ApprovalVariant::ApplyPatch { id, .. }, ApprovalDecision::Review(decision)) => {
                    self.handle_patch_decision(id, decision.clone());
                    self.current_complete = true;
                    self.advance_queue();
                }
                (
                    ApprovalVariant::McpElicitation {
                        server_name,
                        request_id,
                    },
                    ApprovalDecision::McpElicitation(decision),
                ) => {
                    self.handle_elicitation_decision(server_name, request_id, *decision);
                    self.current_complete = true;
                    self.advance_queue();
                }
                (
                    ApprovalVariant::Plan,
                    ApprovalDecision::PlanApproval {
                        approved,
                        permission_mode,
                    },
                ) => {
                    self.handle_plan_decision(*approved, permission_mode.clone());
                    self.current_complete = true;
                    self.advance_queue();
                }
                (
                    ApprovalVariant::EnterPlanMode,
                    ApprovalDecision::EnterPlanModeApproval { approved },
                ) => {
                    self.handle_enter_plan_mode_decision(*approved);
                    self.current_complete = true;
                    self.advance_queue();
                }
                (
                    ApprovalVariant::UserQuestion { tool_call_id, .. },
                    ApprovalDecision::UserQuestionAnswer { answers, .. },
                ) => {
                    // For multi-select mode: toggle the option instead of selecting
                    if self.multi_select_state.is_some() && actual_idx < self.options.len() - 2 {
                        // Toggle this option (not "Confirm" or "Other")
                        self.toggle_multi_select_option(actual_idx);
                        return;
                    }

                    // For multi-question: collect answer and advance
                    if let Some(ref mut mq_state) = self.multi_question_state {
                        // Collect this answer
                        for (header, answer) in answers.iter() {
                            mq_state
                                .collected_answers
                                .insert(header.clone(), answer.clone());
                        }

                        // Advance to next question
                        mq_state.current_index += 1;

                        if mq_state.current_index < mq_state.questions.len() {
                            // More questions: rebuild options for next question
                            self.advance_to_next_question();
                            return;
                        } else {
                            // All questions answered: send final answers
                            let final_answers = mq_state.collected_answers.clone();
                            let final_tool_call_id = mq_state.tool_call_id.clone();
                            self.multi_question_state = None;
                            self.multi_select_state = None;
                            self.handle_user_question_answer(&final_tool_call_id, final_answers);
                            self.current_complete = true;
                            self.advance_queue();
                        }
                    } else {
                        // Single question: send immediately
                        self.handle_user_question_answer(tool_call_id, answers.clone());
                        self.current_complete = true;
                        self.advance_queue();
                    }
                }
                (
                    ApprovalVariant::UserQuestion { .. },
                    ApprovalDecision::UserQuestionConfirm {
                        tool_call_id,
                        header,
                    },
                ) => {
                    // Multi-select confirm: collect toggled selections and send/advance
                    let selected_labels = self.get_multi_select_labels();
                    let answer = if selected_labels.is_empty() {
                        "(no selection)".to_string()
                    } else {
                        selected_labels.join(", ")
                    };

                    if let Some(ref mut mq_state) = self.multi_question_state {
                        // Multi-question: collect and maybe advance
                        mq_state.collected_answers.insert(header.clone(), answer);
                        mq_state.current_index += 1;
                        self.multi_select_state = None;

                        if mq_state.current_index < mq_state.questions.len() {
                            self.advance_to_next_question();
                            return;
                        } else {
                            let final_answers = mq_state.collected_answers.clone();
                            let final_tool_call_id = mq_state.tool_call_id.clone();
                            self.multi_question_state = None;
                            self.handle_user_question_answer(&final_tool_call_id, final_answers);
                            self.current_complete = true;
                            self.advance_queue();
                        }
                    } else {
                        // Single question multi-select: send immediately
                        let mut answers = HashMap::new();
                        answers.insert(header.clone(), answer);
                        self.multi_select_state = None;
                        self.handle_user_question_answer(tool_call_id, answers);
                        self.current_complete = true;
                        self.advance_queue();
                    }
                }
                _ => {}
            }
        }
    }

    /// Toggle a multi-select option and update the display.
    fn toggle_multi_select_option(&mut self, option_index: usize) {
        let Some(ref mut ms_state) = self.multi_select_state else {
            return;
        };

        // Toggle the selection
        if ms_state.selected_indices.contains(&option_index) {
            ms_state.selected_indices.remove(&option_index);
        } else {
            ms_state.selected_indices.insert(option_index);
        }

        // Update the option label to show [x] or [ ] using ext helper
        if let Some(option) = self.options.get_mut(option_index) {
            let is_selected = ms_state.selected_indices.contains(&option_index);
            option.label = toggle_checkbox_label(&option.label, is_selected);
        }

        // Rebuild the list items with updated labels
        self.rebuild_list_items();
    }

    /// Get the labels of all selected options in multi-select mode.
    fn get_multi_select_labels(&self) -> Vec<String> {
        let Some(ref ms_state) = self.multi_select_state else {
            return Vec::new();
        };
        get_selected_labels(&ms_state.selected_indices, &self.options)
    }

    /// Rebuild list items after option labels change (for multi-select toggle).
    fn rebuild_list_items(&mut self) {
        let items: Vec<SelectionItem> = self
            .options
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
        self.list.update_items(items);
    }

    /// Advance to the next question in multi-question flow.
    fn advance_to_next_question(&mut self) {
        let Some(ref mq_state) = self.multi_question_state else {
            return;
        };

        let current_q = &mq_state.questions[mq_state.current_index];
        let tool_call_id = mq_state.tool_call_id.clone();

        // Set up multi-select state if needed
        if current_q.multi_select {
            self.multi_select_state = Some(MultiSelectState::default());
        } else {
            self.multi_select_state = None;
        }

        // Build new options for current question using ext helper
        let options = approval_overlay_ext::single_question_options(
            tool_call_id,
            current_q.clone(),
            current_q.multi_select,
        );

        // Build title and header using ext helpers
        let title = build_question_title(
            mq_state.current_index,
            mq_state.questions.len(),
            &current_q.header,
            &current_q.question,
        );
        let header_lines = build_question_header_lines(title, current_q);
        let header = Paragraph::new(header_lines).wrap(Wrap { trim: false });

        // Update items
        let items: Vec<SelectionItem> = options
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
            header: Box::new(header),
            ..Default::default()
        };

        self.options = options;
        self.list = ListSelectionView::new(params, self.app_event_tx.clone());
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

    fn handle_plan_decision(
        &self,
        approved: bool,
        permission_mode: Option<PlanExitPermissionMode>,
    ) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::PlanModeApproval {
                approved,
                permission_mode,
            }));
    }

    fn handle_enter_plan_mode_decision(&self, approved: bool) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::EnterPlanModeApproval { approved }));
    }

    fn handle_user_question_answer(&self, tool_call_id: &str, answers: HashMap<String, String>) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::UserQuestionAnswer {
                tool_call_id: tool_call_id.to_string(),
                answers,
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
                ApprovalVariant::Plan => {
                    self.handle_plan_decision(false, None);
                }
                ApprovalVariant::EnterPlanMode => {
                    self.handle_enter_plan_mode_decision(false);
                }
                ApprovalVariant::UserQuestion { tool_call_id, .. } => {
                    // On cancel, send empty answers
                    self.handle_user_question_answer(tool_call_id, HashMap::new());
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
            ApprovalRequest::Plan {
                plan_content,
                plan_file_path,
            } => {
                // Show plan preview (first 15 lines) and file path
                let preview_lines: Vec<&str> = plan_content.lines().take(15).collect();
                let preview = preview_lines.join("\n");
                let truncated = if plan_content.lines().count() > 15 {
                    format!("{preview}\n... (truncated)")
                } else {
                    preview
                };

                let header = Paragraph::new(vec![
                    Line::from(vec!["Plan file: ".into(), plan_file_path.bold()]),
                    Line::from(""),
                    Line::from(truncated),
                ])
                .wrap(Wrap { trim: false });
                Self {
                    variant: ApprovalVariant::Plan,
                    header: Box::new(header),
                }
            }
            ApprovalRequest::EnterPlanMode => {
                let header = Paragraph::new(vec![
                    Line::from("The LLM is requesting to enter plan mode.".bold()),
                    Line::from(""),
                    Line::from("In plan mode, the LLM will:"),
                    Line::from("- Explore the codebase using read-only tools"),
                    Line::from("- Design an implementation approach"),
                    Line::from("- Write a plan file for your review"),
                    Line::from("- Ask for approval before implementing"),
                ])
                .wrap(Wrap { trim: false });
                Self {
                    variant: ApprovalVariant::EnterPlanMode,
                    header: Box::new(header),
                }
            }
            ApprovalRequest::UserQuestion {
                tool_call_id,
                questions,
            } => {
                // Build header using ext helper
                let header = build_user_question_header(&questions);
                Self {
                    variant: ApprovalVariant::UserQuestion {
                        tool_call_id,
                        questions,
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
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    ApplyPatch {
        id: String,
    },
    McpElicitation {
        server_name: String,
        request_id: RequestId,
    },
    Plan,
    EnterPlanMode,
    UserQuestion {
        tool_call_id: String,
        questions: Vec<UserQuestion>,
    },
}

#[derive(Clone)]
#[allow(dead_code)]
pub(super) enum ApprovalDecision {
    Review(ReviewDecision),
    McpElicitation(ElicitationAction),
    /// Plan Mode approval decision.
    PlanApproval {
        approved: bool,
        permission_mode: Option<PlanExitPermissionMode>,
    },
    /// Enter Plan Mode approval decision.
    EnterPlanModeApproval {
        approved: bool,
    },
    /// User Question answer decision (single-select or option toggle for multi-select).
    UserQuestionAnswer {
        tool_call_id: String,
        answers: HashMap<String, String>,
    },
    /// User Question multi-select confirm (sends all toggled selections).
    UserQuestionConfirm {
        tool_call_id: String,
        header: String,
    },
}

#[derive(Clone)]
pub(super) struct ApprovalOption {
    pub label: String,
    pub decision: ApprovalDecision,
    pub display_shortcut: Option<KeyBinding>,
    pub additional_shortcuts: Vec<KeyBinding>,
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

// plan_options(), enter_plan_mode_options(), single_question_options() are in approval_overlay_ext

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
