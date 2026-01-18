use codex_core::features::Feature;
use core_test_support::load_sse_fixture_with_id;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;

const HIERARCHICAL_AGENTS_SNIPPET: &str =
    "Files called AGENTS.md commonly appear in many places inside a container";

fn sse_completed(id: &str) -> String {
    load_sse_fixture_with_id("../fixtures/completed_template.json", id)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hierarchical_agents_appends_to_project_doc_in_user_instructions() {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ChildAgentsMd);
        std::fs::write(config.cwd.join("AGENTS.md"), "be nice").expect("write AGENTS.md");
    });
    let test = builder.build(&server).await.expect("build test codex");

    test.submit_turn("hello").await.expect("submit turn");

    let request = resp_mock.single_request();
    let user_messages = request.message_input_texts("user");
    let instructions = user_messages
        .iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .expect("instructions message");
    assert!(
        instructions.contains("be nice"),
        "expected AGENTS.md text included: {instructions}"
    );
    let snippet_pos = instructions
        .find(HIERARCHICAL_AGENTS_SNIPPET)
        .expect("expected hierarchical agents snippet");
    let base_pos = instructions
        .find("be nice")
        .expect("expected AGENTS.md text");
    assert!(
        snippet_pos > base_pos,
        "expected hierarchical agents message appended after base instructions: {instructions}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hierarchical_agents_emits_when_no_project_doc() {
    let server = start_mock_server().await;
    let resp_mock = mount_sse_once(&server, sse_completed("resp1")).await;

    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ChildAgentsMd);
    });
    let test = builder.build(&server).await.expect("build test codex");

    test.submit_turn("hello").await.expect("submit turn");

    let request = resp_mock.single_request();
    let user_messages = request.message_input_texts("user");
    let instructions = user_messages
        .iter()
        .find(|text| text.starts_with("# AGENTS.md instructions for "))
        .expect("instructions message");
    assert!(
        instructions.contains(HIERARCHICAL_AGENTS_SNIPPET),
        "expected hierarchical agents message appended: {instructions}"
    );
}
