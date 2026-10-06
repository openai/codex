//! Deferred namespace summaries keep names discoverable when descriptions exceed the budget.

use anyhow::Result;
use codex_core::StartThreadOptions;
use codex_features::Feature;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use core_test_support::apps_test_server::configure_search_capable_model;
use core_test_support::context_snapshot;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn descriptions_share_space_without_hiding_late_namespaces() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let description = "Find records & retrieve <details> from this service. Search by project, owner, date, or status, then read the matching record to answer questions with its title, summary, source link, and relevant activity from the connected workspace.";
    let namespaces = (0..19)
        .map(|index| format!("service_{index:02}"))
        .chain(std::iter::once("zulu_travel".to_string()))
        .collect::<Vec<_>>();
    let input_schema = json!({"type": "object", "properties": {}, "additionalProperties": false});
    let dynamic_tools = namespaces
        .iter()
        .map(|name| {
            DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
                name: name.clone(),
                description: description.to_string(),
                tools: vec![DynamicToolNamespaceTool::Function(
                    DynamicToolFunctionSpec {
                        name: "search_records".to_string(),
                        description: "Search records in this service.".to_string(),
                        input_schema: input_schema.clone(),
                        defer_loading: true,
                    },
                )],
            })
        })
        .collect();
    let mut test = test_codex()
        .with_config(|config| {
            super::configure_scenario_catalog(config);
            configure_search_capable_model(config);
            config.agents_enabled = false;
            config
                .features
                .enable(Feature::DeferredToolWorldState)
                .expect("enable deferred namespace summaries");
        })
        .build_with_auto_env(&server)
        .await?;
    let thread = test
        .thread_manager
        .start_thread(StartThreadOptions {
            dynamic_tools,
            environments: Some(vec![test.executor_environment().request()]),
            ..StartThreadOptions::new(test.config.clone())
        })
        .await?;
    test.codex = thread.thread;
    test.session_configured = thread.session_configured;

    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_tool_search_call(
                    "find-travel",
                    &json!({"query": "zulu_travel", "limit": 1}),
                ),
                responses::ev_completed("search"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message(
                    "found",
                    "Zulu Travel provides a record search tool.",
                ),
                responses::ev_completed("found"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message("follow-up", "The same services are available."),
                responses::ev_completed("follow-up"),
            ]),
        ],
    )
    .await;
    test.submit_turn("Find the record search tool for Zulu Travel.")
        .await?;
    test.submit_turn("Are the same services still available?")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    let tools_sections = requests
        .iter()
        .map(|request| {
            request
                .message_input_texts("developer")
                .into_iter()
                .filter(|text| text.starts_with("<tools>"))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let [fragment] = tools_sections[0].as_slice() else {
        panic!("expected one initial tools fragment");
    };
    assert!(fragment.len() <= 4096);
    let advertised = fragment
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(|line| line.split_once(": ").expect("nonempty description"))
        .collect::<Vec<_>>();
    assert_eq!(
        advertised.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        namespaces.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    assert!(advertised.iter().all(|(_, shortened)| {
        shortened.strip_suffix("...").is_some_and(|prefix| {
            !prefix.is_empty()
                && prefix.len() < description.len()
                && description.starts_with(prefix)
        })
    }));
    assert_eq!(tools_sections, vec![vec![fragment.clone()]; 3]);
    assert_eq!(
        requests[1].tool_search_output("find-travel")["tools"],
        json!([{
            "type": "namespace",
            "name": "zulu_travel",
            "description": description,
            "tools": [{
                "type": "function",
                "name": "search_records",
                "description": "Search records in this service.",
                "strict": false,
                "defer_loading": true,
                "parameters": input_schema,
            }],
        }]),
    );
    insta::assert_snapshot!(
        "names_before_descriptions",
        context_snapshot::format_request_history_snapshot(
            "A crowded tool catalog retains every namespace, discovers a late namespace, and stays unchanged on follow-up.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}
