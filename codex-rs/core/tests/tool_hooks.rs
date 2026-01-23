//! Tests for tool hooks (before/after tool calls and exec events).
//!
//! This test module covers:
//! - Tool before/after hooks
//! - File before/after write hooks
//! - Exec event hooks

use codex_core::config::types::ProjectHookCommand;
use codex_core::config::types::ProjectHookConfig;
use codex_core::config::types::ProjectHookEvent;
use codex_core::project_hooks::ProjectHooks;
use std::path::PathBuf;

#[test]
fn test_tool_before_hook() {
    let project_root = PathBuf::from("/project");
    let configs = vec![ProjectHookConfig {
        name: Some("tool-before".to_string()),
        event: ProjectHookEvent::PreToolUse,
        run: ProjectHookCommand::String("./pre-tool.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].event, ProjectHookEvent::PreToolUse);
    assert_eq!(hook_list[0].name, Some("tool-before".to_string()));
}

#[test]
fn test_tool_after_hook() {
    let project_root = PathBuf::from("/project");
    let configs = vec![ProjectHookConfig {
        name: Some("tool-after".to_string()),
        event: ProjectHookEvent::PostToolUse,
        run: ProjectHookCommand::String("./post-tool.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].event, ProjectHookEvent::PostToolUse);
    assert_eq!(hook_list[0].name, Some("tool-after".to_string()));
}

#[test]
fn test_file_before_write_hook() {
    let project_root = PathBuf::from("/project");
    let configs = vec![ProjectHookConfig {
        name: Some("file-before-write".to_string()),
        event: ProjectHookEvent::FileBeforeWrite,
        run: ProjectHookCommand::String("./file-before.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].event, ProjectHookEvent::FileBeforeWrite);
    assert_eq!(hook_list[0].name, Some("file-before-write".to_string()));
}

#[test]
fn test_file_after_write_hook() {
    let project_root = PathBuf::from("/project");
    let configs = vec![ProjectHookConfig {
        name: Some("file-after-write".to_string()),
        event: ProjectHookEvent::FileAfterWrite,
        run: ProjectHookCommand::String("./file-after.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].event, ProjectHookEvent::FileAfterWrite);
    assert_eq!(hook_list[0].name, Some("file-after-write".to_string()));
}

#[test]
fn test_tool_hooks_with_list_command() {
    let project_root = PathBuf::from("/project");
    let configs = vec![ProjectHookConfig {
        name: Some("tool-hook-list".to_string()),
        event: ProjectHookEvent::PreToolUse,
        run: ProjectHookCommand::List(vec![
            "python3".to_string(),
            "-c".to_string(),
            "print('hook')".to_string(),
        ]),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    match &hook_list[0].run {
        ProjectHookCommand::List(args) => {
            assert_eq!(args.len(), 3);
            assert_eq!(args[0], "python3");
        }
        ProjectHookCommand::String(_) => panic!("Expected List command"),
    }
}

#[test]
fn test_multiple_tool_hooks() {
    let project_root = PathBuf::from("/project");
    let configs = vec![
        ProjectHookConfig {
            name: Some("pre-tool-1".to_string()),
            event: ProjectHookEvent::PreToolUse,
            run: ProjectHookCommand::String("./pre-tool-1.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
        ProjectHookConfig {
            name: Some("pre-tool-2".to_string()),
            event: ProjectHookEvent::PreToolUse,
            run: ProjectHookCommand::String("./pre-tool-2.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
        ProjectHookConfig {
            name: Some("post-tool-1".to_string()),
            event: ProjectHookEvent::PostToolUse,
            run: ProjectHookCommand::String("./post-tool-1.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
    ];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 3);

    // Filter by event type
    assert_eq!(
        hook_list
            .iter()
            .filter(|h| h.event == ProjectHookEvent::PreToolUse)
            .count(),
        2
    );

    assert_eq!(
        hook_list
            .iter()
            .filter(|h| h.event == ProjectHookEvent::PostToolUse)
            .count(),
        1
    );
}

#[test]
fn test_tool_hooks_with_env_and_timeout() {
    let project_root = PathBuf::from("/project");
    let mut env = std::collections::HashMap::new();
    env.insert("HOOK_TYPE".to_string(), "tool".to_string());
    env.insert("DEBUG".to_string(), "true".to_string());

    let configs = vec![ProjectHookConfig {
        name: Some("tool-with-config".to_string()),
        event: ProjectHookEvent::PreToolUse,
        run: ProjectHookCommand::String("./tool-hook.sh".into()),
        cwd: Some(PathBuf::from("./hooks")),
        env: Some(env.clone()),
        timeout_ms: Some(30000),
        run_in_background: false,
    }];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 1);
    assert_eq!(hook_list[0].env, Some(env));
    assert_eq!(hook_list[0].timeout_ms, Some(30000));
    assert_eq!(hook_list[0].cwd, Some(PathBuf::from("./hooks")));
}

#[test]
fn test_file_hooks_global_and_project_merge() {
    let global_root = PathBuf::from("/home/user/.codex");
    let project_root = PathBuf::from("/project");

    let global_configs = vec![ProjectHookConfig {
        name: Some("global-file-before".to_string()),
        event: ProjectHookEvent::FileBeforeWrite,
        run: ProjectHookCommand::String("./global-file-hook.sh".into()),
        cwd: None,
        env: None,
        timeout_ms: None,
        run_in_background: false,
    }];

    let project_configs = vec![ProjectHookConfig {
        name: Some("project-file-after".to_string()),
        event: ProjectHookEvent::FileAfterWrite,
        run: ProjectHookCommand::String("./project-file-hook.sh".into()),
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

    assert_eq!(hook_list.len(), 2);

    // Global hook should be first
    assert_eq!(hook_list[0].name, Some("global-file-before".to_string()));
    assert_eq!(hook_list[0].event, ProjectHookEvent::FileBeforeWrite);

    // Project hook should be second
    assert_eq!(hook_list[1].name, Some("project-file-after".to_string()));
    assert_eq!(hook_list[1].event, ProjectHookEvent::FileAfterWrite);
}

#[test]
fn test_tool_hook_background_execution() {
    let project_root = PathBuf::from("/project");
    let configs = vec![
        ProjectHookConfig {
            name: Some("background-hook".to_string()),
            event: ProjectHookEvent::PostToolUse,
            run: ProjectHookCommand::String("./log-tool-use.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: true,
        },
        ProjectHookConfig {
            name: Some("foreground-hook".to_string()),
            event: ProjectHookEvent::PostToolUse,
            run: ProjectHookCommand::String("./validate-tool-use.sh".into()),
            cwd: None,
            env: None,
            timeout_ms: None,
            run_in_background: false,
        },
    ];

    let hooks = ProjectHooks::from_configs(Some(&configs), &project_root);
    let hook_list = hooks.hooks();

    assert_eq!(hook_list.len(), 2);
    assert!(hook_list[0].run_in_background);
    assert!(!hook_list[1].run_in_background);
}
