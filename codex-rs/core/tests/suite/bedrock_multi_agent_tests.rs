//! Verifies default Bedrock catalogs select V2 and expose its collaboration tools.

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
async fn bedrock_catalog_selects_v2_by_default(
    provider_id: &'static str,
    model: &'static str,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![responses::ev_completed("done")]),
    )
    .await;
    let test = test_codex()
        .with_auth(CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "dummy".to_string(),
            region: "us-east-1".to_string(),
        }))
        .with_model(model)
        .with_config(move |config| {
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

    test.submit_text_turn("Hello").await?;

    assert_eq!(
        test.codex.multi_agent_version(),
        Some(MultiAgentVersion::V2)
    );
    let request = mock.single_request();
    for tool in ["spawn_agent", "followup_task"] {
        assert!(
            request.tool_by_name("collaboration", tool).is_some(),
            "default Bedrock catalog should expose collaboration.{tool}",
        );
    }
    Ok(())
}
