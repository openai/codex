use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol_config_types::ReasoningEffort;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

const CONFIG_TOML: &str = "config.toml";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_does_not_persist_when_config_exists() {
    let server = start_mock_server().await;
    let test = test_codex()
        .with_model("gpt-4o")
        .build(&server)
        .await
        .expect("build test codex");
    let codex = test.codex.clone();

    let config_path = test.home.path().join(CONFIG_TOML);
    let initial_contents = "model = \"gpt-4o\"\n";
    tokio::fs::write(&config_path, initial_contents)
        .await
        .expect("seed config.toml");

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: None,
        })
        .await
        .expect("submit override");

    codex.submit(Op::Shutdown).await.expect("request shutdown");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    let contents = tokio::fs::read_to_string(&config_path)
        .await
        .expect("read config.toml after override");
    assert_eq!(contents, initial_contents);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_does_not_create_config_file() {
    let server = start_mock_server().await;
    let test = test_codex().build(&server).await.expect("build test codex");
    let codex = test.codex.clone();

    let config_path = test.home.path().join(CONFIG_TOML);
    if config_path.exists() {
        tokio::fs::remove_file(&config_path)
            .await
            .expect("remove existing config.toml");
    }
    assert!(
        !config_path.exists(),
        "test setup should start without config"
    );

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::Medium)),
            summary: None,
        })
        .await
        .expect("submit override");

    codex.submit(Op::Shutdown).await.expect("request shutdown");
    wait_for_event(&codex, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    assert!(
        !config_path.exists(),
        "override should not create config.toml"
    );
}
