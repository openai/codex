#![cfg(test)]
//! Exercise an explicit request for the bundled statusline authoring skill.

use super::*;
use core_test_support::responses::mount_sse_once;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_statusline_mention_loads_the_bundled_skill() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let home = Arc::new(TempDir::new()?);
    fs::write(
        home.path().join("config.toml"),
        "[skills.bundled]\nenabled = true\n",
    )?;
    let mock = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("author"),
            ev_assistant_message("reply", "I'll show the current model in your footer."),
            ev_completed("author"),
        ]),
    )
    .await;
    let test = test_codex()
        .with_model("gpt-6-astra")
        .with_home(home)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_extensions(skills_extensions())
        .with_config(configure_scenario_catalog)
        .build_with_auto_env(&server)
        .await?;

    test.submit_text_turn("Use $create-statusline to make a footer showing my current model.")
        .await?;

    let request = mock.single_request();
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .all(|message| !message.contains("- create-statusline:"))
    );
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "A user requests the bundled statusline skill by name.",
        &mock.requests(),
        &ContextSnapshotOptions::default().rewrite_known_segments(),
    ));
    Ok(())
}
