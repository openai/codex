use std::collections::HashMap;
use std::ffi::OsStr;

use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::*;

#[test]
fn excluded_environment_variable_overrides_handler_env() {
    let variable = "CODEX_WIF_HOOK_ASSERTION";
    let shell = CommandShell {
        program: String::new(),
        args: Vec::new(),
        excluded_environment_variables: vec![variable.to_string()],
    };
    let handler = ConfiguredHandler {
        event_name: HookEventName::PreToolUse,
        matcher: None,
        command: "true".to_string(),
        timeout_sec: 1,
        status_message: None,
        source_path: AbsolutePathBuf::current_dir().expect("current dir"),
        source: HookSource::Unknown,
        display_order: 0,
        env: HashMap::from([(variable.to_string(), "secret.assertion".to_string())]),
    };

    let command = build_command(&shell, &handler);
    assert!(
        command
            .as_std()
            .get_envs()
            .any(|(name, value)| { name == OsStr::new(variable) && value.is_none() })
    );
}
