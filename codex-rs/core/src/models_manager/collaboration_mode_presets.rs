use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::config_types::ModeKind;
use codex_protocol::openai_models::ReasoningEffort;

const COLLABORATION_MODE_PLAN: &str = include_str!("../../templates/collaboration_mode/plan.md");
const COLLABORATION_MODE_CODE: &str = include_str!("../../templates/collaboration_mode/code.md");
const COLLABORATION_MODE_PAIR_PROGRAMMING: &str =
    include_str!("../../templates/collaboration_mode/pair_programming.md");
const COLLABORATION_MODE_EXECUTE: &str =
    include_str!("../../templates/collaboration_mode/execute.md");

pub(super) fn builtin_collaboration_mode_presets() -> Vec<CollaborationModeMask> {
    vec![
        plan_preset(),
        code_preset(),
        pair_programming_preset(),
        execute_preset(),
    ]
}

#[cfg(any(test, feature = "test-support"))]
pub fn test_builtin_collaboration_mode_presets() -> Vec<CollaborationModeMask> {
    builtin_collaboration_mode_presets()
}

fn plan_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Plan".to_string(),
        mode: Some(ModeKind::Plan),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffort::Medium)),
        developer_instructions: Some(Some(COLLABORATION_MODE_PLAN.to_string())),
    }
}

fn code_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Code".to_string(),
        mode: Some(ModeKind::Code),
        model: None,
        reasoning_effort: None,
        developer_instructions: Some(Some(COLLABORATION_MODE_CODE.to_string())),
    }
}

fn pair_programming_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Pair Programming".to_string(),
        mode: Some(ModeKind::PairProgramming),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffort::Medium)),
        developer_instructions: Some(Some(COLLABORATION_MODE_PAIR_PROGRAMMING.to_string())),
    }
}

fn execute_preset() -> CollaborationModeMask {
    CollaborationModeMask {
        name: "Execute".to_string(),
        mode: Some(ModeKind::Execute),
        model: None,
        reasoning_effort: Some(Some(ReasoningEffort::High)),
        developer_instructions: Some(Some(COLLABORATION_MODE_EXECUTE.to_string())),
    }
}
