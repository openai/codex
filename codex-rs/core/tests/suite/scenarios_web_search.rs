//! Model-visible context for standalone web search.

use super::*;
use codex_protocol::config_types::WebSearchMode;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_partial_json;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn literature_search_and_open() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let search =
        json!({"literature_search_query": [{"q": "FlashAttention-2", "domains": ["arxiv.org"]}]});
    let open = json!({"open": [{"ref_id": "turn0academia0"}]});
    for (commands, output) in [
        (&search, "[turn0academia0] FlashAttention-2 by Tri Dao"),
        (
            &open,
            "FlashAttention-2 reports a 2x speedup over FlashAttention.",
        ),
    ] {
        Mock::given(method("POST"))
            .and(path("/v1/alpha/search"))
            .and(body_partial_json(json!({"commands": commands})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"output": output})))
            .expect(1)
            .mount(&server)
            .await;
    }
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call_with_namespace("search-paper", "web", "run", &search.to_string()),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_function_call_with_namespace("open-paper", "web", "run", &open.to_string()),
                ev_completed("resp-2"),
            ]),
            sse(vec![
                ev_assistant_message("answer", "Tri Dao reports a 2x speedup."),
                ev_completed("resp-3"),
            ]),
        ],
    )
    .await;

    let auth = codex_core::test_support::auth_manager_from_auth(CodexAuth::from_api_key("dummy"));
    let mut extensions = ExtensionRegistryBuilder::new();
    codex_web_search_extension::install(&mut extensions, Arc::clone(&auth));
    let test = test_codex()
        .with_auth_manager(auth)
        .with_extensions(Arc::new(extensions.build()))
        .with_config(|config| {
            configure_scenario_catalog(config);
            config
                .features
                .enable(Feature::StandaloneWebSearch)
                .unwrap();
            config.web_search_mode.set(WebSearchMode::Live).unwrap();
        })
        .with_model_info_override("paper-model", |model| {
            model.tool_mode = Some(ToolMode::Direct);
            model
                .experimental_supported_tools
                .push("literature_search_query".to_string());
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_turn("Find the FlashAttention-2 paper and open it to check the reported speedup.")
        .await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    let web_run = requests[0]
        .tool_by_name("web", "run")
        .expect("advertised web.run");
    let mut parameters = web_run
        .get("parameters")
        .expect("web.run parameters")
        .clone();
    parameters.sort_all_objects();
    insta::assert_snapshot!(
        "literature_search_web_run_schema",
        serde_json::to_string_pretty(&parameters)?
    );
    insta::assert_snapshot!(context_snapshot::format_request_history_snapshot(
        "A model opted into literature search finds a paper, opens the returned reference, and reads the result before answering.",
        &requests,
        &ContextSnapshotOptions::default()
            .rewrite_known_segments()
            .include_request_settings(),
    ));
    Ok(())
}
