//! Request-history coverage for opt-in incremental tools on Responses Lite.

use anyhow::Result;
use codex_features::Feature;
use codex_history::RolloutItem;
use codex_protocol::openai_models::CodeModeToolMessages;
use codex_protocol::openai_models::ToolMessage;
use codex_protocol::openai_models::ToolMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

#[test_case(true; "responses_lite")]
#[test_case(false; "responses_api")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn incremental_tools_append_changed_catalog_without_rewriting_history(
    use_responses_lite: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        (1..=3)
            .map(|index| responses::sse(vec![responses::ev_completed(&format!("resp-{index}"))]))
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_model_info_override("gpt-5.4", move |model| {
            model.use_responses_lite = use_responses_lite;
            model.tool_mode = Some(ToolMode::CodeMode);
        })
        .with_config(|config| {
            config
                .features
                .enable(Feature::IncrementalTools)
                .expect("enable incremental tools");
            config.base_instructions =
                Some("Use the available tools to help the user.".to_string());
            config.code_mode.disable_in_process_fallback = true;
            let catalog = config.model_catalog.as_mut().expect("model catalog");
            let mut updated = catalog
                .models
                .iter()
                .find(|model| model.slug == "gpt-5.4")
                .expect("source model")
                .clone();
            updated.slug = "updated-tools-model".to_string();
            updated
                .model_messages
                .get_or_insert_default()
                .tools
                .get_or_insert_default()
                .code_mode = Some(CodeModeToolMessages {
                exec: Some(ToolMessage {
                    description: Some("Updated execution instructions.".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            });
            catalog.models.push(updated);
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Begin work.").await?;
    test.submit_turn("Continue with the same tools.").await?;
    core_test_support::submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            model: Some("updated-tools-model".to_string()),
            ..Default::default()
        },
    )
    .await?;
    test.submit_text_turn("Continue with the updated execution instructions.")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    if !use_responses_lite {
        for request in &requests {
            let body = request.body_json();
            assert!(
                body["tools"]
                    .as_array()
                    .is_some_and(|tools| !tools.is_empty())
            );
            assert_eq!(
                body["instructions"],
                "Use the available tools to help the user."
            );
        }
        assert_eq!(
            requests[0].body_json()["tools"],
            requests[1].body_json()["tools"]
        );
        assert_ne!(
            requests[1].body_json()["tools"],
            requests[2].body_json()["tools"]
        );
        return Ok(());
    }
    assert!(requests[1].input().starts_with(&requests[0].input()));
    assert!(requests[2].input().starts_with(&requests[1].input()));
    assert!(
        requests
            .iter()
            .all(|request| request.body_json().get("tools").is_none())
    );
    let initial = requests[0].inputs_of_type("additional_tools");
    assert_eq!(initial.len(), 1);
    assert_eq!(
        initial[0]["tools"]
            .as_array()
            .expect("initial tool declarations")
            .iter()
            .map(|tool| tool["type"].clone())
            .collect::<Vec<_>>(),
        vec![json!("namespace"), json!("tool_search")]
    );
    assert_eq!(requests[1].inputs_of_type("additional_tools"), initial);
    let changed = requests[2].inputs_of_type("additional_tools");
    assert_eq!(requests[2].body_json()["model"], "updated-tools-model");
    assert_eq!(&changed[..initial.len()], &initial);
    assert_eq!(changed.len(), initial.len() + 1);
    let diff = changed.last().expect("changed tool declarations")["tools"]
        .as_array()
        .expect("changed tool array");
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0]["name"], "functions");
    let changed_tools = diff[0]["tools"].as_array().expect("namespace members");
    assert_eq!(changed_tools.len(), 1);
    assert_eq!(changed_tools[0]["name"], "exec");
    assert!(
        changed
            .last()
            .expect("changed tool declarations")
            .to_string()
            .contains("Updated execution instructions.")
    );

    test.codex.ensure_rollout_materialized().await;
    test.codex.flush_rollout().await?;
    let rollout =
        tokio::fs::read_to_string(test.codex.rollout_path().expect("rollout path")).await?;
    let catalogs = rollout
        .lines()
        .map(codex_rollout::parse_rollout_line)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(|line| match line.item {
            RolloutItem::WorldState(item) => item.state.get("top_level_tools").cloned(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(catalogs.len(), 2);
    assert_ne!(catalogs[0], catalogs[1]);
    assert_eq!(
        catalogs[1]
            .as_object()
            .expect("updated tool catalog")
            .iter()
            .filter(|(key, hash)| catalogs[0].get(*key) != Some(*hash))
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        vec!["functions.exec"]
    );
    assert!(catalogs.iter().all(|catalog| {
        catalog
            .as_object()
            .expect("tool catalog")
            .values()
            .all(|hash| hash.as_str().is_some_and(|hash| hash.len() == 40))
    }));
    insta::assert_snapshot!(
        "incremental_tools",
        context_snapshot::format_request_history_snapshot(
            "Tool definitions enter history in one batch; a catalog change appends only the changed exec definition.",
            &requests,
            &ContextSnapshotOptions::default()
                .rewrite_known_segments()
                .include_request_settings(),
        )
    );
    Ok(())
}
