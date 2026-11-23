//! Exercises collaboration tools with Bedrock model catalogs.

use codex_features::Feature;
use codex_login::CodexAuth;
use codex_login::auth::BedrockApiKeyAuth;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::protocol::MultiAgentVersion;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-5.6-sol"; "mantle")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-5.6-sol"; "runtime")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bedrock_catalog_selects_v2_and_rejects_unadvertised_flex(
    provider_id: &'static str,
    model: &'static str,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    const CALL_ID: &str = "spawn-worker";
    let mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("spawn"),
                responses::ev_function_call_with_namespace(
                    CALL_ID,
                    "collaboration",
                    "spawn_agent",
                    r#"{"message":"Do the task","task_name":"worker","service_tier":"flex"}"#,
                ),
                responses::ev_completed("spawn"),
            ]),
            responses::sse(vec![responses::ev_completed("done")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_auth(CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "dummy".to_string(),
            region: "us-east-1".to_string(),
        }))
        .with_model(model)
        .with_config(move |config| {
            config.multi_agent_v2.expose_spawn_agent_service_tier = true;
            let base_url = config.model_provider.base_url.clone();
            config.model_provider_id = provider_id.to_string();
            config.model_provider = built_in_model_providers(/*openai_base_url*/ None)
                .remove(provider_id)
                .expect("built-in Bedrock provider");
            config.model_provider.base_url = base_url;
        })
        .build_with_auto_env(&server)
        .await?;
    assert!(!test.config.features.enabled(Feature::MultiAgentV2));
    assert!(test.config.model_catalog.is_none());

    test.submit_text_turn("Spawn a worker on Flex").await?;

    assert_eq!(
        test.codex.multi_agent_version(),
        Some(MultiAgentVersion::V2)
    );
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    let declarations = requests[0].inputs_of_type("additional_tools");
    for tool in ["spawn_agent", "followup_task"] {
        assert!(
            declarations
                .iter()
                .any(|item| responses::namespace_child_tool(item, "collaboration", tool).is_some()),
            "default Bedrock catalog should expose collaboration.{tool}",
        );
    }
    let (output, _) = requests[1]
        .function_call_output_content_and_success(CALL_ID)
        .expect("spawn_agent error output");
    assert_eq!(
        output,
        Some(format!(
            "Service tier `flex` is unavailable for model `{model}`. Omit service_tier to inherit the usual tier."
        ))
    );
    Ok(())
}
