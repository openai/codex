use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use std::collections::HashMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dependency_env_is_propagated_to_user_shell() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex();
    // We need to access the session to set dependency_env, but building the test_codex
    // returns a fixture that contains it.
    let test = builder.build(&server).await?;

    // Set a dependency environment variable.
    let mut deps = HashMap::new();
    deps.insert("MY_TEST_TOKEN".to_string(), "secret_value_123".to_string());
    test.codex.set_dependency_env(deps).await;

    // Run a command that prints this variable.
    #[cfg(windows)]
    let command = "[System.Console]::Write($env:MY_TEST_TOKEN)".to_string();
    #[cfg(not(windows))]
    let command = "printf '%s' \"$MY_TEST_TOKEN\"".to_string();

    test.codex
        .submit(Op::RunUserShellCommand { command })
        .await?;

    // Wait for the end event and check stdout.
    let end_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::ExecCommandEnd(event) => Some(event.clone()),
        _ => None,
    })
    .await;

    assert_eq!(end_event.exit_code, 0);
    assert_eq!(end_event.stdout.trim(), "secret_value_123");

    Ok(())
}
