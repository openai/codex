use codex_core::features::Feature;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;

const HIERARCHICAL_AGENTS_SNIPPET: &str =
    "Files called AGENTS.md commonly appear in many places inside a container";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hierarchical_agents_appends_to_project_doc_in_user_instructions() {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::ChildAgentsMd)
            .expect("test config should allow feature update");
        std::fs::write(config.cwd.join("AGENTS.md"), "be nice").expect("write AGENTS.md");
    });
    let test = builder.build(&server).await.expect("build test codex");

    test.submit_turn("hello").await.expect("submit turn");

    let request = resp_mock.single_request();
    let user_messages = request.message_input_texts("user");
    let agents_instructions = user_messages
        .iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .expect("AGENTS instructions message");
    assert!(
        agents_instructions.contains("be nice"),
        "expected AGENTS.md text included: {agents_instructions}"
    );
    let child_agents_instructions = user_messages
        .iter()
        .find(|text| text.contains(HIERARCHICAL_AGENTS_SNIPPET))
        .expect("child agents instructions message");
    let agents_pos = user_messages
        .iter()
        .position(|text| std::ptr::eq(text, agents_instructions))
        .expect("AGENTS instructions position");
    let child_agents_pos = user_messages
        .iter()
        .position(|text| std::ptr::eq(text, child_agents_instructions))
        .expect("child agents instructions position");
    assert!(
        child_agents_pos > agents_pos,
        "expected child-agents instructions after AGENTS fragment: {user_messages:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hierarchical_agents_emits_when_no_project_doc() {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp1"), ev_completed("resp1")]),
    )
    .await;

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::ChildAgentsMd)
            .expect("test config should allow feature update");
    });
    let test = builder.build(&server).await.expect("build test codex");

    test.submit_turn("hello").await.expect("submit turn");

    let request = resp_mock.single_request();
    let user_messages = request.message_input_texts("user");
    assert!(
        user_messages
            .iter()
            .any(|text| text.contains(HIERARCHICAL_AGENTS_SNIPPET)),
        "expected hierarchical agents instructions fragment: {user_messages:?}"
    );
}
