use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

const CONFIG_TOML: &str = "config.toml";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_does_not_persist_when_config_exists() {
    let server = start_mock_server().await;
    let initial_contents = "model = \"gpt-4o\"\n";
    let mut builder = test_codex()
        .with_pre_build_hook(move |home| {
            let config_path = home.join(CONFIG_TOML);
            std::fs::write(config_path, initial_contents).expect("seed config.toml");
        })
        .with_config(|config| {
            config.model = Some("gpt-4o".to_string());
        });
    let test = builder.build(&server).await.expect("create conversation");
    let codex = test.codex.clone();
    let config_path = test.home.path().join(CONFIG_TOML);

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::High)),
            summary: None,
            collaboration_mode: None,
            personality: None,
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
    let mut builder = test_codex();
    let test = builder.build(&server).await.expect("create conversation");
    let codex = test.codex.clone();
    let config_path = test.home.path().join(CONFIG_TOML);
    assert!(
        !config_path.exists(),
        "test setup should start without config"
    );

    codex
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            sandbox_policy: None,
            windows_sandbox_level: None,
            model: Some("o3".to_string()),
            effort: Some(Some(ReasoningEffort::Medium)),
            summary: None,
            collaboration_mode: None,
            personality: None,
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
