//! Tests for Plan Mode module.

use super::*;
use codex_protocol::ThreadId;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn test_plan_mode_state_default() {
    let state = PlanModeState::new();
    assert!(!state.is_active);
    assert!(state.plan_file_path.is_none());
    assert!(!state.has_approved);
    assert!(state.conversation_id.is_none());
}

#[test]
fn test_plan_mode_enter() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path();
    let mut state = PlanModeState::new();
    let conv_id = ThreadId::new();

    let path = state
        .enter(codex_home, conv_id)
        .expect("should enter plan mode");

    assert!(state.is_active);
    assert!(state.plan_file_path.is_some());
    // Plan files use slug-based names (e.g., "rippling-wiggling-shamir.md")
    assert!(path.to_string_lossy().ends_with(".md"));
    assert!(path.to_string_lossy().contains("plans"));
}

#[test]
fn test_plan_mode_exit_approved() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path();
    let mut state = PlanModeState::new();
    let conv_id = ThreadId::new();
    let _ = state
        .enter(codex_home, conv_id)
        .expect("should enter plan mode");

    state.exit(true);

    assert!(!state.is_active);
    assert!(state.has_approved);
    // plan_file_path preserved
    assert!(state.plan_file_path.is_some());
}

#[test]
fn test_plan_mode_exit_rejected() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path();
    let mut state = PlanModeState::new();
    let conv_id = ThreadId::new();
    let _ = state
        .enter(codex_home, conv_id)
        .expect("should enter plan mode");

    state.exit(false);

    assert!(!state.is_active);
    assert!(!state.has_approved); // Not set when rejected
}

#[test]
fn test_is_reentry_false_when_not_exited() {
    let state = PlanModeState::new();
    assert!(!state.is_reentry());
}

#[test]
fn test_is_reentry_true_when_file_exists() {
    let temp = tempdir().unwrap();
    let mut state = PlanModeState::new();

    // Simulate: entered, exited with approval, file exists
    state.has_approved = true;
    let plan_path = temp.path().join("test_plan.md");
    std::fs::write(&plan_path, "test plan content").unwrap();
    state.plan_file_path = Some(plan_path);

    assert!(state.is_reentry());
}

#[test]
fn test_is_reentry_false_when_file_not_exists() {
    let mut state = PlanModeState::new();
    state.has_approved = true;
    state.plan_file_path = Some(PathBuf::from("/nonexistent/path.md"));

    assert!(!state.is_reentry());
}

#[test]
fn test_clear_reentry() {
    let mut state = PlanModeState::new();
    state.has_approved = true;

    state.clear_reentry();

    assert!(!state.has_approved);
}

#[test]
fn test_get_plan_file_path() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path();
    let conv_id = ThreadId::new();
    let path = get_plan_file_path(codex_home, &conv_id).expect("should get path");

    assert!(path.to_string_lossy().ends_with(".md"));
    assert!(path.to_string_lossy().contains("plans"));
}

#[test]
fn test_get_plans_directory() {
    let temp = tempdir().unwrap();
    let codex_home = temp.path();
    let plans_dir = get_plans_directory(codex_home).expect("should get plans directory");

    assert!(plans_dir.ends_with("plans"));
}

#[test]
fn test_read_plan_file() {
    let temp = tempdir().unwrap();
    let plan_path = temp.path().join("test.md");

    // File doesn't exist
    assert!(read_plan_file(&plan_path).is_none());

    // Write content
    let content = "# My Plan\n\n1. Step 1\n2. Step 2";
    std::fs::write(&plan_path, content).unwrap();

    // Read back
    let read_content = read_plan_file(&plan_path);
    assert_eq!(read_content, Some(content.to_string()));
}

#[test]
fn test_plan_file_exists() {
    let temp = tempdir().unwrap();
    let plan_path = temp.path().join("test.md");

    assert!(!plan_file_exists(&plan_path));

    std::fs::write(&plan_path, "test").unwrap();

    assert!(plan_file_exists(&plan_path));
}
