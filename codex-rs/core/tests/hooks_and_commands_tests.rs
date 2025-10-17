// Comprehensive tests for hooks and custom commands
use codex_core::custom_commands::*;
use codex_core::hooks::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_hook_system_end_to_end() {
    let mut config = HookConfig::new();

    // Register hooks for different events
    config.add_hook(HookEvent::OnTaskStart, "echo 'Starting task'".to_string());
    config.add_hook(
        HookEvent::OnTaskComplete,
        "echo 'Task completed'".to_string(),
    );

    let executor = HookExecutor::new(config);

    // Test OnTaskStart
    let context = HookContext::new(HookEvent::OnTaskStart).with_task_id("task-123".to_string());

    let results = executor.execute(context).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].success);

    // Test OnTaskComplete
    let context = HookContext::new(HookEvent::OnTaskComplete).with_task_id("task-123".to_string());

    let results = executor.execute(context).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].success);
}

#[tokio::test]
async fn test_hook_with_environment_variables() {
    let mut config = HookConfig::new();

    let command = if cfg!(target_os = "windows") {
        "echo %CODEX_TASK_ID%"
    } else {
        "echo $CODEX_TASK_ID"
    };

    config.add_hook(HookEvent::OnTaskStart, command.to_string());

    let executor = HookExecutor::new(config);
    let context = HookContext::new(HookEvent::OnTaskStart)
        .with_task_id("test-task-456".to_string())
        .with_agent_type("CodeExpert".to_string());

    let results = executor.execute(context).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].stdout.contains("test-task-456"));
}

#[test]
fn test_custom_command_registry_defaults() {
    let registry = CustomCommandRegistry::default();

    // デフォルトで7つのコマンドが登録されているはず
    assert_eq!(registry.list_names().len(), 7);

    // 各コマンドの存在確認
    assert!(registry.has_command("analyze_code"));
    assert!(registry.has_command("security_review"));
    assert!(registry.has_command("generate_tests"));
    assert!(registry.has_command("deep_research"));
    assert!(registry.has_command("debug_issue"));
    assert!(registry.has_command("optimize_performance"));
    assert!(registry.has_command("generate_docs"));
}

#[test]
fn test_custom_command_details() {
    let registry = CustomCommandRegistry::default();

    // analyze_code コマンドの詳細確認
    let cmd = registry.get("analyze_code").unwrap();
    assert_eq!(cmd.name, "analyze_code");
    assert_eq!(cmd.subagent, Some("CodeExpert".to_string()));
    assert!(cmd.description.contains("bugs"));

    // security_review コマンドの詳細確認
    let cmd = registry.get("security_review").unwrap();
    assert_eq!(cmd.subagent, Some("SecurityExpert".to_string()));

    // deep_research コマンドの詳細確認
    let cmd = registry.get("deep_research").unwrap();
    assert_eq!(cmd.subagent, Some("DeepResearcher".to_string()));
    assert_eq!(cmd.parameters.get("depth"), Some(&"5".to_string()));
    assert_eq!(cmd.parameters.get("sources"), Some(&"20".to_string()));
}

#[tokio::test]
async fn test_custom_command_executor() {
    let executor = CustomCommandExecutor::default();

    // analyze_code コマンドを実行
    let result = executor
        .execute("analyze_code", "fn main() { println!(\"Hello\"); }")
        .await
        .unwrap();

    assert_eq!(result.command_name, "analyze_code");
    assert!(result.success);
    assert_eq!(result.subagent_used, Some("CodeExpert".to_string()));
    assert!(result.output.contains("CodeExpert"));

    // security_review コマンドを実行
    let result = executor
        .execute("security_review", "let password = \"admin123\";")
        .await
        .unwrap();

    assert_eq!(result.command_name, "security_review");
    assert_eq!(result.subagent_used, Some("SecurityExpert".to_string()));
}

#[tokio::test]
async fn test_custom_command_with_hooks() {
    let mut registry = CustomCommandRegistry::new();

    let command = CustomCommand::new(
        "test_with_hooks".to_string(),
        "Test command with hooks".to_string(),
    )
    .with_subagent("General".to_string())
    .with_pre_hook("echo 'Before'".to_string())
    .with_post_hook("echo 'After'".to_string());

    registry.register(command);

    let executor = CustomCommandExecutor::new(registry);
    let result = executor
        .execute("test_with_hooks", "test context")
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.pre_hook_results.len(), 1);
    assert_eq!(result.post_hook_results.len(), 1);
    assert!(result.output.contains("Before"));
    assert!(result.output.contains("After"));
}

#[tokio::test]
async fn test_hook_error_handling() {
    let mut config = HookConfig::new();

    // 失敗するコマンド
    let command = if cfg!(target_os = "windows") {
        "exit 1"
    } else {
        "exit 1"
    };

    config.add_hook(HookEvent::OnError, command.to_string());

    let executor = HookExecutor::new(config);
    let context = HookContext::new(HookEvent::OnError).with_error("Test error".to_string());

    let results = executor.execute(context).await.unwrap();
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert_eq!(results[0].exit_code, 1);
}

#[tokio::test]
async fn test_multiple_hooks_sequential() {
    let mut config = HookConfig::new();
    config.async_execution = false; // Sequential execution

    config.add_hook(HookEvent::OnTaskComplete, "echo 'First'".to_string());
    config.add_hook(HookEvent::OnTaskComplete, "echo 'Second'".to_string());
    config.add_hook(HookEvent::OnTaskComplete, "echo 'Third'".to_string());

    let executor = HookExecutor::new(config);
    let context = HookContext::new(HookEvent::OnTaskComplete);

    let results = executor.execute(context).await.unwrap();

    assert_eq!(results.len(), 3);
    for (i, result) in results.iter().enumerate() {
        assert!(result.success, "Hook {} failed", i + 1);
    }
}

#[test]
fn test_hook_event_types() {
    let events = vec![
        HookEvent::OnTaskStart,
        HookEvent::OnTaskComplete,
        HookEvent::OnError,
        HookEvent::OnTaskAbort,
        HookEvent::OnSubAgentStart,
        HookEvent::OnSubAgentComplete,
        HookEvent::OnSessionStart,
        HookEvent::OnSessionEnd,
        HookEvent::OnPatchApply,
        HookEvent::OnCommandExec,
    ];

    // All events should have unique string representations
    let mut seen = std::collections::HashSet::new();
    for event in events {
        let s = event.to_string();
        assert!(!s.is_empty());
        assert!(seen.insert(s), "Duplicate event string: {}", event);
    }
}

#[tokio::test]
async fn test_hook_context_metadata() {
    let context = HookContext::new(HookEvent::OnTaskStart)
        .with_task_id("task-1".to_string())
        .with_agent_type("CodeExpert".to_string())
        .with_metadata("priority".to_string(), "high".to_string())
        .with_metadata("timeout".to_string(), "30".to_string());

    assert_eq!(context.metadata.len(), 2);
    assert_eq!(context.metadata.get("priority"), Some(&"high".to_string()));
    assert_eq!(context.metadata.get("timeout"), Some(&"30".to_string()));
}

#[test]
fn test_custom_command_builder() {
    let command = CustomCommand::new("test".to_string(), "Test cmd".to_string())
        .with_subagent("TestAgent".to_string())
        .with_parameter("p1".to_string(), "v1".to_string())
        .with_parameter("p2".to_string(), "v2".to_string())
        .with_pre_hook("pre1".to_string())
        .with_pre_hook("pre2".to_string())
        .with_post_hook("post1".to_string());

    assert_eq!(command.parameters.len(), 2);
    assert_eq!(command.pre_hooks.len(), 2);
    assert_eq!(command.post_hooks.len(), 1);
}

#[tokio::test]
async fn test_all_default_commands_executable() {
    let executor = CustomCommandExecutor::default();
    let commands = executor.list_commands();

    for command_name in commands {
        let result = executor.execute(&command_name, "test context").await;
        assert!(
            result.is_ok(),
            "Failed to execute command: {}",
            command_name
        );

        let res = result.unwrap();
        assert!(res.success, "Command failed: {}", command_name);
    }
}
