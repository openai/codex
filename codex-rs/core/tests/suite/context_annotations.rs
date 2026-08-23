use anyhow::Result;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_request_item_types_roles_and_content_annotations() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let response = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            config.developer_instructions = Some("Keep world-state annotations aligned.".into());
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_text_turn("inspect world state").await?;

    let items = response
        .single_request()
        .input()
        .into_iter()
        .map(|item| {
            let item_type = item["type"].as_str().expect("response item type");
            let role = item["role"].as_str().unwrap_or("-");
            let content_annotations =
                &item["internal_chat_message_metadata_passthrough"]["content_item_kinds"];
            format!("{item_type} {role} {content_annotations}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(items, @r#"
    message developer ["generic.developer_instructions","generic.permissions_instructions"]
    message user ["environments.environment_context"]
    message user null
    "#);

    Ok(())
}
