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
