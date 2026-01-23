//! Tests for hook event types and their payloads.
//!
//! This test module covers:
//! - Session start/end hooks
//! - User prompt submit hooks
//! - Stop hooks
//! - Notification hooks
//! - Pre/post compact hooks
//! - Subagent stop hooks

use codex_core::config::types::ProjectHookCommand;
use codex_core::config::types::ProjectHookConfig;
use codex_core::config::types::ProjectHookEvent;
use codex_core::project_hooks::ProjectHooks;
use std::path::PathBuf;

#[test]
fn test_project_hook_event_variants() {
    // Test that all expected hook event variants exist
    let variants = [
        ProjectHookEvent::SessionStart,
        ProjectHookEvent::SessionEnd,
        ProjectHookEvent::UserPromptSubmit,
        ProjectHookEvent::Stop,
        ProjectHookEvent::SubagentStop,
        ProjectHookEvent::PreCompact,
        ProjectHookEvent::PostCompact,
        ProjectHookEvent::Notification,
        ProjectHookEvent::PreToolUse,
        ProjectHookEvent::PostToolUse,
        ProjectHookEvent::FileBeforeWrite,
        ProjectHookEvent::FileAfterWrite,
    ];

    // Verify we can create a string representation
    for event in &variants {
        let s = format!("{event:?}");
        assert!(!s.is_empty(), "Event should have a debug representation");
    }
}

#[test]
fn test_hook_config_creation() {
    let config = ProjectHookConfig {
        name: Some("test-hook".to_string()),
        event: ProjectHookEvent::SessionStart,
        run: ProjectHookCommand::String("./hook.sh".into()),
        cwd: Some(PathBuf::from("/tmp")),
        env: None,
        timeout_ms: Some(5000),
        run_in_background: false,
    };

    assert_eq!(config.name, Some("test-hook".to_string()));
    assert_eq!(config.event, ProjectHookEvent::SessionStart);
    assert_eq!(config.timeout_ms, Some(5000));
}

#[test]
fn test_hooks_from_configs() {
    let project_root = PathBuf::from("/project");
    let configs = vec![
        ProjectHookConfig {
            name: Some("session-start".to_string()),
            event: ProjectHookEvent::SessionStart,
            run: ProjectHookCommand::String("./start.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
        ProjectHookConfig {
            name: Some("session-end".to_string()),
            event: ProjectHookEvent::SessionEnd,
            run: ProjectHookCommand::String("./end.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
    ];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 2);
    assert_eq!(hook_list[0].event, ProjectHookEvent::SessionStart);
    assert_eq!(hook_list[1].event, ProjectHookEvent::SessionEnd);
}

#[test]
fn test_global_and_project_hooks_merge() {
    let global_root = PathBuf::from("/home/user/.codex");
    let project_root = PathBuf::from("/project");

    let global_configs = vec![ProjectHookConfig {
        name: Some("global-hook".to_string()),
        event: ProjectHookEvent::UserPromptSubmit,
        run: ProjectHookCommand::String("./global.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let project_configs = vec![ProjectHookConfig {
        name: Some("project-hook".to_string()),
        event: ProjectHookEvent::UserPromptSubmit,
        run: ProjectHookCommand::String("./project.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_global_and_project_configs(
        Some(&global_configs),
        &global_root,
        Some(&project_configs),
        &project_root,
    );
    let hook_list = hooks.hooks();

    // Global hooks should come first, then project hooks
    assert_eq!(hook_list.len(), 2);
    assert_eq!(hook_list[0].name, Some("global-hook".to_string()));
    assert_eq!(hook_list[1].name, Some("project-hook".to_string()));
}

#[test]
fn test_hooks_with_only_global() {
    let global_root = PathBuf::from("/home/user/.codex");
    let project_root = PathBuf::from("/project");

    let global_configs = vec![ProjectHookConfig {
        name: Some("global-only".to_string()),
        event: ProjectHookEvent::Stop,
        run: ProjectHookCommand::String("./stop.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_global_and_project_configs(
        Some(&global_configs),
        &global_root,
        None,
        &project_root,
    );
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].name, Some("global-only".to_string()));
}

#[test]
fn test_hooks_with_only_project() {
    let global_root = PathBuf::from("/home/user/.codex");
    let project_root = PathBuf::from("/project");

    let project_configs = vec![ProjectHookConfig {
        name: Some("project-only".to_string()),
        event: ProjectHookEvent::Notification,
        run: ProjectHookCommand::String("./notify.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_global_and_project_configs(
        None,
        &global_root,
        Some(&project_configs),
        &project_root,
    );
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].name, Some("project-only".to_string()));
}

#[test]
fn test_pre_post_compact_hooks() {
    let project_root = PathBuf::from("/project");
    let configs = vec![
        ProjectHookConfig {
            name: Some("pre-compact".to_string()),
            event: ProjectHookEvent::PreCompact,
            run: ProjectHookCommand::String("./pre-compact.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
        ProjectHookConfig {
            name: Some("post-compact".to_string()),
            event: ProjectHookEvent::PostCompact,
            run: ProjectHookCommand::String("./post-compact.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
    ];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 2);
    assert_eq!(hook_list[0].event, ProjectHookEvent::PreCompact);
    assert_eq!(hook_list[1].event, ProjectHookEvent::PostCompact);
}

#[test]
fn test_subagent_stop_hook() {
    let project_root = PathBuf::from("/project");
    let configs = vec![ProjectHookConfig {
        name: Some("subagent-stop".to_string()),
        event: ProjectHookEvent::SubagentStop,
        run: ProjectHookCommand::String("./subagent-stop.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].event, ProjectHookEvent::SubagentStop);
    assert_eq!(hook_list[0].name, Some("subagent-stop".to_string()));
}

#[test]
fn test_hook_with_environment_variables() {
    let project_root = PathBuf::from("/project");
    let mut env = std::collections::HashMap::new();
    env.insert("MY_VAR".to_string(), "my_value".to_string());

    let configs = vec![ProjectHookConfig {
        name: Some("env-hook".to_string()),
        event: ProjectHookEvent::UserPromptSubmit,
        run: ProjectHookCommand::String("./env.sh".into()),
        cwd: Some(PathBuf::from("./hooks")),
        env: Some(env.clone()),
        timeout_ms: Some(10000),
        run_in_background: true,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].env, Some(env));
    assert_eq!(hook_list[0].timeout_ms, Some(10000));
    assert!(hook_list[0].run_in_background);
}

#[test]
fn test_empty_hooks() {
    let global_root = PathBuf::from("/home/user/.codex");
    let project_root = PathBuf::from("/project");

    let hooks =
        ProjectHooks::from_global_and_project_configs(None, &global_root, None, &project_root);
    let hook_list = hooks.hooks();

    assert!(hook_list.is_empty());
}
