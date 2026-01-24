//! Extension types and functions for approval overlay.
//!
//! Separated to minimize upstream merge conflicts when syncing with upstream.
//! Contains Plan Mode and UserQuestion related functionality.

use std::collections::HashMap;
use std::collections::HashSet;

use codex_protocol::protocol_ext::PlanExitPermissionMode;
use codex_protocol::protocol_ext::UserQuestion;
use crossterm::event::KeyCode;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::bottom_pane::list_selection_view::SelectionItem;
use crate::key_hint;

use super::approval_overlay::ApprovalDecision;
use super::approval_overlay::ApprovalOption;

// ============================================================================
// State Types
// ============================================================================

/// State for tracking multi-question composite selection.
/// When answering multiple questions, this tracks which question we're on
/// and what answers have been collected so far.
#[derive(Clone, Debug)]
pub struct MultiQuestionState {
    /// Current question index (0-based)
    pub current_index: usize,
    /// Collected answers so far: header -> selected label(s)
    pub collected_answers: HashMap<String, String>,
    /// All questions being answered
    pub questions: Vec<UserQuestion>,
    /// Tool call ID for final submission
    pub tool_call_id: String,
}

/// State for tracking multiSelect toggle selections.
/// When a question has multi_select=true, this tracks which options are toggled.
#[derive(Clone, Debug, Default)]
pub struct MultiSelectState {
    /// Indices of currently selected options (0-based within current question's options)
    pub selected_indices: HashSet<usize>,
}

// ============================================================================
// Option Builders
// ============================================================================

/// Plan mode approval options (4 options aligned with Claude Code).
pub fn plan_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, bypass permissions".to_string(),
            decision: ApprovalDecision::PlanApproval {
                approved: true,
                permission_mode: Some(PlanExitPermissionMode::BypassPermissions),
            },
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('b'))],
        },
        ApprovalOption {
            label: "Yes, auto-accept edits".to_string(),
            decision: ApprovalDecision::PlanApproval {
                approved: true,
                permission_mode: Some(PlanExitPermissionMode::AcceptEdits),
            },
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('e'))],
        },
        ApprovalOption {
            label: "Yes, manual approval".to_string(),
            decision: ApprovalDecision::PlanApproval {
                approved: true,
                permission_mode: Some(PlanExitPermissionMode::Default),
            },
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
        },
        ApprovalOption {
            label: "No, keep planning".to_string(),
            decision: ApprovalDecision::PlanApproval {
                approved: false,
                permission_mode: None,
            },
            display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
        },
    ]
}

/// Enter plan mode options.
pub fn enter_plan_mode_options() -> Vec<ApprovalOption> {
    vec![
        ApprovalOption {
            label: "Yes, enter plan mode".to_string(),
            decision: ApprovalDecision::EnterPlanModeApproval { approved: true },
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('y'))],
        },
        ApprovalOption {
            label: "No, continue without planning".to_string(),
            decision: ApprovalDecision::EnterPlanModeApproval { approved: false },
            display_shortcut: Some(key_hint::plain(KeyCode::Esc)),
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char('n'))],
        },
    ]
}

/// Build options for a single user question.
/// For multi_select questions, options are shown with [ ] checkbox prefix.
/// For single-select questions, options are shown directly.
pub fn single_question_options(
    tool_call_id: String,
    question: UserQuestion,
    is_multi_select: bool,
) -> Vec<ApprovalOption> {
    let header = question.header.clone();
    let mut approval_options: Vec<ApprovalOption> = Vec::new();

    for (idx, opt) in question.options.iter().enumerate() {
        // For multi_select, prefix with [ ] to show as unchecked checkbox
        let label = if is_multi_select {
            format!("[ ] {}", opt.label)
        } else {
            opt.label.clone()
        };

        let mut answers = HashMap::new();
        answers.insert(header.clone(), opt.label.clone());

        // Assign shortcuts: 1-4 for up to 4 options
        let shortcut_char = char::from_digit((idx + 1) as u32, 10).unwrap_or('1');

        approval_options.push(ApprovalOption {
            label,
            decision: ApprovalDecision::UserQuestionAnswer {
                tool_call_id: tool_call_id.clone(),
                answers,
            },
            display_shortcut: None,
            additional_shortcuts: vec![key_hint::plain(KeyCode::Char(shortcut_char))],
        });
    }

    // For multi_select: add a "Confirm selections" option at the end
    if is_multi_select {
        approval_options.push(ApprovalOption {
            label: "Confirm selections".to_string(),
            decision: ApprovalDecision::UserQuestionConfirm {
                tool_call_id: tool_call_id.clone(),
                header: header.clone(),
            },
            display_shortcut: Some(key_hint::plain(KeyCode::Enter)),
            additional_shortcuts: vec![],
        });
    }

    // Add "Other" option for custom input
    let mut other_answers = HashMap::new();
    other_answers.insert(header, "Other".to_string());
    approval_options.push(ApprovalOption {
        label: "Other (provide custom answer)".to_string(),
        decision: ApprovalDecision::UserQuestionAnswer {
            tool_call_id,
            answers: other_answers,
        },
        display_shortcut: None,
        additional_shortcuts: vec![key_hint::plain(KeyCode::Char('o'))],
    });

    approval_options
}

// ============================================================================
// Multi-Select Helpers
// ============================================================================

/// Toggle checkbox label between [ ] and [x].
pub fn toggle_checkbox_label(label: &str, selected: bool) -> String {
    let text = label
        .strip_prefix("[x] ")
        .or_else(|| label.strip_prefix("[ ] "))
        .unwrap_or(label);
    if selected {
        format!("[x] {text}")
    } else {
        format!("[ ] {text}")
    }
}

/// Extract labels from selected options (strips checkbox prefix).
pub fn get_selected_labels(
    selected_indices: &HashSet<usize>,
    options: &[ApprovalOption],
) -> Vec<String> {
    let mut labels: Vec<String> = selected_indices
        .iter()
        .filter_map(|&idx| {
            options.get(idx).map(|opt| {
                opt.label
                    .strip_prefix("[x] ")
                    .or_else(|| opt.label.strip_prefix("[ ] "))
                    .unwrap_or(&opt.label)
                    .to_string()
            })
        })
        .collect();
    labels.sort();
    labels
}

// ============================================================================
// Header Builders
// ============================================================================

/// Build header paragraph for user question display.
pub fn build_user_question_header(questions: &[UserQuestion]) -> Paragraph<'static> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from("The LLM is asking for your input:".bold()));
    lines.push(Line::from(""));

    for (i, q) in questions.iter().enumerate() {
        lines.push(Line::from(format!("{}. {}", i + 1, q.question).bold()));
        lines.push(Line::from(format!("   [{}]", q.header).dim()));
        for opt in &q.options {
            lines.push(Line::from(format!(
                "   • {} - {}",
                opt.label, opt.description
            )));
        }
        if q.multi_select {
            lines.push(Line::from("   (Multiple selections allowed)".italic()));
        }
        lines.push(Line::from(""));
    }

    Paragraph::new(lines).wrap(Wrap { trim: false })
}

/// Build question title for multi-question flow.
pub fn build_question_title(
    current_index: usize,
    total: usize,
    header: &str,
    question: &str,
) -> String {
    format!("[{}/{}] {}: {}", current_index + 1, total, header, question)
}

/// Build question header lines with option descriptions.
pub fn build_question_header_lines(title: String, question: &UserQuestion) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(title.bold()));
    lines.push(Line::from(""));
    for opt in &question.options {
        lines.push(Line::from(format!("• {} - {}", opt.label, opt.description)));
    }
    if question.multi_select {
        lines.push(Line::from(""));
        lines.push(Line::from(
            "(Use number keys to toggle, Enter to confirm)".italic(),
        ));
    }
    lines
}

// ============================================================================
// Request Header Builders
// ============================================================================

/// Build header for Plan approval request (preview first 15 lines).
pub fn build_plan_request_header(plan_content: &str, plan_file_path: &str) -> Paragraph<'static> {
    let preview_lines: Vec<&str> = plan_content.lines().take(15).collect();
    let preview = preview_lines.join("\n");
    let truncated = if plan_content.lines().count() > 15 {
        format!("{preview}\n... (truncated)")
    } else {
        preview
    };

    Paragraph::new(vec![
        Line::from(vec![
            "Plan file: ".into(),
            plan_file_path.to_string().bold(),
        ]),
        Line::from(""),
        Line::from(truncated),
    ])
    .wrap(Wrap { trim: false })
}

/// Build header for EnterPlanMode request.
pub fn build_enter_plan_mode_header() -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(
            "The LLM is requesting to enter plan mode."
                .to_string()
                .bold(),
        ),
        Line::from(""),
        Line::from("In plan mode, the LLM will:"),
        Line::from("- Explore the codebase using read-only tools"),
        Line::from("- Design an implementation approach"),
        Line::from("- Write a plan file for your review"),
        Line::from("- Ask for approval before implementing"),
    ])
    .wrap(Wrap { trim: false })
}

// ============================================================================
// State Initialization
// ============================================================================

/// Initialize multi-question and multi-select state for UserQuestion requests.
/// Returns (multi_question_state, multi_select_state).
pub fn init_user_question_state(
    tool_call_id: &str,
    questions: &[UserQuestion],
) -> (Option<MultiQuestionState>, Option<MultiSelectState>) {
    let mq_state = if questions.len() > 1 {
        Some(MultiQuestionState {
            current_index: 0,
            collected_answers: HashMap::new(),
            questions: questions.to_vec(),
            tool_call_id: tool_call_id.to_string(),
        })
    } else {
        None
    };

    let current_q = if let Some(ref mq) = mq_state {
        questions.get(mq.current_index)
    } else {
        questions.first()
    };

    let ms_state = if current_q.is_some_and(|q| q.multi_select) {
        Some(MultiSelectState::default())
    } else {
        None
    };

    (mq_state, ms_state)
}

// ============================================================================
// UserQuestion Flow Actions
// ============================================================================

/// Action to take after processing a user question answer.
#[derive(Debug)]
pub enum UserQuestionFlowAction {
    /// Toggle multi-select option at given index.
    ToggleOption(usize),
    /// Advance to next question (rebuild UI).
    AdvanceQuestion,
    /// Complete the flow with final answers.
    Complete {
        tool_call_id: String,
        answers: HashMap<String, String>,
    },
}

/// Process a UserQuestionAnswer decision.
/// Returns the action to take.
pub fn process_user_question_answer(
    mq_state: &mut Option<MultiQuestionState>,
    ms_state: &Option<MultiSelectState>,
    answers: &HashMap<String, String>,
    actual_idx: usize,
    options_len: usize,
) -> UserQuestionFlowAction {
    // For multi-select mode: toggle instead of selecting (not "Confirm" or "Other")
    if ms_state.is_some() && actual_idx < options_len.saturating_sub(2) {
        return UserQuestionFlowAction::ToggleOption(actual_idx);
    }

    // For multi-question: collect answer and advance
    if let Some(mq) = mq_state {
        for (header, answer) in answers.iter() {
            mq.collected_answers.insert(header.clone(), answer.clone());
        }
        mq.current_index += 1;

        if mq.current_index < mq.questions.len() {
            UserQuestionFlowAction::AdvanceQuestion
        } else {
            let final_answers = mq.collected_answers.clone();
            let final_tool_call_id = mq.tool_call_id.clone();
            UserQuestionFlowAction::Complete {
                tool_call_id: final_tool_call_id,
                answers: final_answers,
            }
        }
    } else {
        // Single question: complete immediately
        // Note: tool_call_id comes from variant, caller fills in
        UserQuestionFlowAction::Complete {
            tool_call_id: String::new(),
            answers: answers.clone(),
        }
    }
}

/// Process a UserQuestionConfirm decision (multi-select confirm).
/// Returns the action to take.
pub fn process_user_question_confirm(
    mq_state: &mut Option<MultiQuestionState>,
    selected_labels: Vec<String>,
    header: &str,
) -> UserQuestionFlowAction {
    let answer = if selected_labels.is_empty() {
        "(no selection)".to_string()
    } else {
        selected_labels.join(", ")
    };

    if let Some(mq) = mq_state {
        mq.collected_answers.insert(header.to_string(), answer);
        mq.current_index += 1;

        if mq.current_index < mq.questions.len() {
            UserQuestionFlowAction::AdvanceQuestion
        } else {
            let final_answers = mq.collected_answers.clone();
            let final_tool_call_id = mq.tool_call_id.clone();
            UserQuestionFlowAction::Complete {
                tool_call_id: final_tool_call_id,
                answers: final_answers,
            }
        }
    } else {
        // Single question multi-select
        let mut answers = HashMap::new();
        answers.insert(header.to_string(), answer);
        UserQuestionFlowAction::Complete {
            tool_call_id: String::new(),
            answers,
        }
    }
}

// ============================================================================
// UI Building Helpers
// ============================================================================

/// Build SelectionItem list from ApprovalOptions.
pub fn build_selection_items(options: &[ApprovalOption]) -> Vec<SelectionItem> {
    options
        .iter()
        .map(|opt| SelectionItem {
            name: opt.label.clone(),
            display_shortcut: opt
                .display_shortcut
                .or_else(|| opt.additional_shortcuts.first().copied()),
            dismiss_on_select: false,
            ..Default::default()
        })
        .collect()
}

/// Build standard footer hint for approval dialogs.
pub fn build_approval_footer_hint() -> Line<'static> {
    Line::from(vec![
        "Press ".into(),
        key_hint::plain(KeyCode::Enter).into(),
        " to confirm or ".into(),
        key_hint::plain(KeyCode::Esc).into(),
        " to cancel".into(),
    ])
}

/// Build next question view components.
/// Returns (options, header_paragraph).
pub fn build_next_question_view(
    mq_state: &MultiQuestionState,
) -> (Vec<ApprovalOption>, Paragraph<'static>) {
    let current_q = &mq_state.questions[mq_state.current_index];
    let tool_call_id = mq_state.tool_call_id.clone();

    let options = single_question_options(tool_call_id, current_q.clone(), current_q.multi_select);

    let title = build_question_title(
        mq_state.current_index,
        mq_state.questions.len(),
        &current_q.header,
        &current_q.question,
    );
    let header_lines = build_question_header_lines(title, current_q);
    let header = Paragraph::new(header_lines).wrap(Wrap { trim: false });

    (options, header)
}

// ============================================================================
// Impl Block for ApprovalOverlay - Methods moved from approval_overlay.rs
// ============================================================================

use crate::app_event::AppEvent;
use crate::bottom_pane::list_selection_view::ListSelectionView;
use codex_core::protocol::Op;

use crate::render::renderable::Renderable;

use super::approval_overlay::ApprovalOverlay;
use super::approval_overlay::ApprovalRequest;
use super::approval_overlay::ApprovalRequestState;
use super::approval_overlay::ApprovalVariant;

impl ApprovalOverlay {
    /// Toggle a multi-select option and update the display.
    pub(super) fn toggle_multi_select_option(&mut self, option_index: usize) {
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
    pub(super) fn get_multi_select_labels(&self) -> Vec<String> {
        let Some(ref ms_state) = self.multi_select_state else {
            return Vec::new();
        };
        get_selected_labels(&ms_state.selected_indices, &self.options)
    }

    /// Rebuild list items after option labels change (for multi-select toggle).
    fn rebuild_list_items(&mut self) {
        self.list.update_items(build_selection_items(&self.options));
    }

    /// Advance to the next question in multi-question flow.
    pub(super) fn advance_to_next_question(&mut self) {
        let Some(ref mq_state) = self.multi_question_state else {
            return;
        };

        // Set up multi-select state if needed
        let current_q = &mq_state.questions[mq_state.current_index];
        if current_q.multi_select {
            self.multi_select_state = Some(MultiSelectState::default());
        } else {
            self.multi_select_state = None;
        }

        // Build view components using ext
        let (options, header) = build_next_question_view(mq_state);

        let params = super::list_selection_view::SelectionViewParams {
            footer_hint: Some(build_approval_footer_hint()),
            items: build_selection_items(&options),
            header: Box::new(header),
            ..Default::default()
        };

        self.options = options;
        self.list = ListSelectionView::new(params, self.app_event_tx.clone());
    }

    pub(super) fn handle_plan_decision(
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

    pub(super) fn handle_enter_plan_mode_decision(&self, approved: bool) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::EnterPlanModeApproval { approved }));
    }

    pub(super) fn handle_user_question_answer(
        &self,
        tool_call_id: &str,
        answers: HashMap<String, String>,
    ) {
        self.app_event_tx
            .send(AppEvent::CodexOp(Op::UserQuestionAnswer {
                tool_call_id: tool_call_id.to_string(),
                answers,
            }));
    }
}

// ============================================================================
// Delegation Functions - Called from approval_overlay.rs
// ============================================================================

/// Called from set_current() to initialize ext state for UserQuestion requests.
pub(super) fn init_ext_state(overlay: &mut ApprovalOverlay, request: &ApprovalRequest) {
    if let ApprovalRequest::UserQuestion {
        tool_call_id,
        questions,
    } = request
    {
        let (mq, ms) = init_user_question_state(tool_call_id, questions);
        overlay.multi_question_state = mq;
        overlay.multi_select_state = ms;
    } else {
        overlay.multi_question_state = None;
        overlay.multi_select_state = None;
    }
}

/// Called from build_options() for ext variants (Plan, EnterPlanMode, UserQuestion).
/// Returns (options, title) for the variant.
pub(super) fn build_ext_options(
    variant: &ApprovalVariant,
    multi_question_state: &Option<MultiQuestionState>,
) -> (Vec<ApprovalOption>, String) {
    match variant {
        ApprovalVariant::Plan => (
            plan_options(),
            "Would you like to approve this plan?".to_string(),
        ),
        ApprovalVariant::EnterPlanMode => (
            enter_plan_mode_options(),
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
                    single_question_options(
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
                    single_question_options(tool_call_id.clone(), q.clone(), q.multi_select),
                    format!("{}: {}", q.header, q.question),
                )
            }
        }
        // This shouldn't be called for non-ext variants
        _ => (Vec::new(), String::new()),
    }
}

/// Called from on_ctrl_c() to handle cancellation of ext variants.
pub(super) fn handle_ext_cancel(overlay: &ApprovalOverlay, variant: &ApprovalVariant) {
    match variant {
        ApprovalVariant::Plan => {
            overlay.handle_plan_decision(false, None);
        }
        ApprovalVariant::EnterPlanMode => {
            overlay.handle_enter_plan_mode_decision(false);
        }
        ApprovalVariant::UserQuestion { tool_call_id, .. } => {
            // On cancel, send empty answers
            overlay.handle_user_question_answer(tool_call_id, HashMap::new());
        }
        _ => {}
    }
}

/// Result of ext selection handling.
enum ExtSelectionResult {
    /// Selection was handled and completed (set current_complete, advance_queue)
    Handled,
    /// Early return (toggle, advance) - don't set complete
    EarlyReturn,
    /// Not an ext variant/decision combination - fall through to default
    NotHandled,
}

/// Called from apply_selection() catch-all arm for ext variants.
/// Returns true if caller should early return (handled or toggle/advance).
/// Returns false if not an ext variant (fall through to default handling).
pub(super) fn handle_ext_selection(
    overlay: &mut ApprovalOverlay,
    variant: &ApprovalVariant,
    decision: &ApprovalDecision,
    actual_idx: usize,
) -> bool {
    match apply_ext_selection(overlay, variant, decision, actual_idx) {
        ExtSelectionResult::Handled => {
            overlay.current_complete = true;
            overlay.advance_queue();
            true
        }
        ExtSelectionResult::EarlyReturn => true,
        ExtSelectionResult::NotHandled => false,
    }
}

/// Internal: Called from handle_ext_selection() to handle ext variant/decision combinations.
fn apply_ext_selection(
    overlay: &mut ApprovalOverlay,
    variant: &ApprovalVariant,
    decision: &ApprovalDecision,
    actual_idx: usize,
) -> ExtSelectionResult {
    match (variant, decision) {
        (
            ApprovalVariant::Plan,
            ApprovalDecision::PlanApproval {
                approved,
                permission_mode,
            },
        ) => {
            overlay.handle_plan_decision(*approved, permission_mode.clone());
            ExtSelectionResult::Handled
        }
        (ApprovalVariant::EnterPlanMode, ApprovalDecision::EnterPlanModeApproval { approved }) => {
            overlay.handle_enter_plan_mode_decision(*approved);
            ExtSelectionResult::Handled
        }
        (
            ApprovalVariant::UserQuestion { tool_call_id, .. },
            ApprovalDecision::UserQuestionAnswer { answers, .. },
        ) => {
            let action = process_user_question_answer(
                &mut overlay.multi_question_state,
                &overlay.multi_select_state,
                answers,
                actual_idx,
                overlay.options.len(),
            );
            match action {
                UserQuestionFlowAction::ToggleOption(idx) => {
                    overlay.toggle_multi_select_option(idx);
                    ExtSelectionResult::EarlyReturn
                }
                UserQuestionFlowAction::AdvanceQuestion => {
                    overlay.advance_to_next_question();
                    ExtSelectionResult::EarlyReturn
                }
                UserQuestionFlowAction::Complete {
                    tool_call_id: tid,
                    answers: ans,
                } => {
                    let final_id = if tid.is_empty() {
                        tool_call_id.clone()
                    } else {
                        tid
                    };
                    overlay.multi_question_state = None;
                    overlay.multi_select_state = None;
                    overlay.handle_user_question_answer(&final_id, ans);
                    ExtSelectionResult::Handled
                }
            }
        }
        (
            ApprovalVariant::UserQuestion { tool_call_id, .. },
            ApprovalDecision::UserQuestionConfirm { header, .. },
        ) => {
            let selected_labels = overlay.get_multi_select_labels();
            let action = process_user_question_confirm(
                &mut overlay.multi_question_state,
                selected_labels,
                header,
            );
            overlay.multi_select_state = None;
            match action {
                UserQuestionFlowAction::AdvanceQuestion => {
                    overlay.advance_to_next_question();
                    ExtSelectionResult::EarlyReturn
                }
                UserQuestionFlowAction::Complete {
                    tool_call_id: tid,
                    answers,
                } => {
                    let final_id = if tid.is_empty() {
                        tool_call_id.clone()
                    } else {
                        tid
                    };
                    overlay.multi_question_state = None;
                    overlay.handle_user_question_answer(&final_id, answers);
                    ExtSelectionResult::Handled
                }
                _ => ExtSelectionResult::EarlyReturn,
            }
        }
        _ => ExtSelectionResult::NotHandled,
    }
}

/// Called from `From<ApprovalRequest> for ApprovalRequestState` for ext request types.
/// Returns `Some(ApprovalRequestState)` if handled, `None` otherwise.
pub(super) fn build_ext_request_state(request: ApprovalRequest) -> Option<ApprovalRequestState> {
    match request {
        ApprovalRequest::Plan {
            plan_content,
            plan_file_path,
        } => {
            let header = build_plan_request_header(&plan_content, &plan_file_path);
            Some(ApprovalRequestState {
                variant: ApprovalVariant::Plan,
                header: Box::new(header) as Box<dyn Renderable>,
            })
        }
        ApprovalRequest::EnterPlanMode => {
            let header = build_enter_plan_mode_header();
            Some(ApprovalRequestState {
                variant: ApprovalVariant::EnterPlanMode,
                header: Box::new(header) as Box<dyn Renderable>,
            })
        }
        ApprovalRequest::UserQuestion {
            tool_call_id,
            questions,
        } => {
            let header = build_user_question_header(&questions);
            Some(ApprovalRequestState {
                variant: ApprovalVariant::UserQuestion {
                    tool_call_id,
                    questions,
                },
                header: Box::new(header) as Box<dyn Renderable>,
            })
        }
        _ => None,
    }
}
