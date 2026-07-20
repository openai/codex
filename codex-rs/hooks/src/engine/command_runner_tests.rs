use std::collections::HashMap;
use std::fs;

use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

use super::CommandShell;
use super::ConfiguredHandler;
use super::run_command;

#[tokio::test]
async fn cmd_shell_runs_quoted_hook_command_path() {
    let temp = tempdir().expect("create temp dir");
    let hook_dir = temp.path().join("hook with spaces");
    fs::create_dir(&hook_dir).expect("create hook dir");
    let hook_path = hook_dir.join("hook.cmd");
    fs::write(
        &hook_path,
        "@echo off\r\nif not \"%~1\"==\"notify\" exit /B 7\r\necho hook-ran\r\n",
    )
    .expect("write hook command");
    let source_path =
        AbsolutePathBuf::try_from(hook_path.clone()).expect("absolute hook command path");
    let handler = ConfiguredHandler {
        event_name: HookEventName::SessionStart,
        matcher: None,
        command: format!(r#""{}" notify"#, hook_path.display()),
        timeout_sec: 10,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path,
        source: HookSource::User,
        display_order: 0,
        env: HashMap::new(),
    };
    let shells = [
        CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        CommandShell {
            program: std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()),
            args: vec!["/c".to_string()],
        },
    ];

    for shell in shells {
        let result = run_command(
            &shell,
            &handler,
            /*configured_order*/ 0,
            "{}",
            temp.path(),
        )
        .await;

        assert_eq!(result.exit_code, Some(0), "stderr: {}", result.stderr);
        assert_eq!(result.stdout.trim(), "hook-ran");
        assert!(result.error.is_none());
    }
}
