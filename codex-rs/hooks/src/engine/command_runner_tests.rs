use std::collections::HashMap;
use std::ffi::OsStr;

use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookSource;
use codex_protocol::shell_environment::OPENAI_IDENTITY_TOKEN_ENV_VAR;
use codex_utils_absolute_path::AbsolutePathBuf;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn build_command_removes_process_only_variables_after_handler_overrides() {
    let token_name = OPENAI_IDENTITY_TOKEN_ENV_VAR.to_ascii_lowercase();
    let token_file_name = "OpenAI_Identity_Token_File".to_string();
    let handler = ConfiguredHandler {
        event_name: HookEventName::SessionStart,
        matcher: None,
        command: "true".to_string(),
        timeout_sec: 1,
        status_message: None,
        source_path: AbsolutePathBuf::from_absolute_path("/tmp/hooks.json")
            .expect("absolute source path"),
        source: HookSource::User,
        display_order: 0,
        env: HashMap::from([
            (token_name.clone(), "configured-assertion".to_string()),
            (
                token_file_name.clone(),
                "/configured/identity-token".to_string(),
            ),
        ]),
    };

    let command = build_command(
        &CommandShell {
            program: String::new(),
            args: Vec::new(),
        },
        &handler,
    );
    let configured_env = command
        .as_std()
        .get_envs()
        .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
        .collect::<HashMap<_, _>>();

    assert_eq!(configured_env.get(OsStr::new(&token_name)), Some(&None));
    assert_eq!(
        configured_env.get(OsStr::new(&token_file_name)),
        Some(&None)
    );
    assert!(configured_env.values().all(Option::is_none));
}
