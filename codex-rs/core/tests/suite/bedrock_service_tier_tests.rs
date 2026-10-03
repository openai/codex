//! Exercises Bedrock service-tier selection through provider catalogs and HTTP requests.

use codex_login::CodexAuth;
use codex_login::auth::BedrockApiKeyAuth;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::openai_models::ModelServiceTier;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;

#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-6-astra"; "mantle")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-6-astra"; "runtime global")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "us.openai.gpt-6-astra"; "runtime us")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bedrock_ultrafast_can_be_configured_and_toggled(
    provider_id: &'static str,
    model: &'static str,
) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_sequence(
        &server,
        ["configured", "standard", "ultrafast"]
            .into_iter()
            .map(|id| responses::sse(vec![responses::ev_completed(id)]))
            .collect(),
    )
    .await;
    let test = test_codex()
        .with_auth(CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "dummy".to_string(),
            region: "us-west-2".to_string(),
        }))
        .with_model(model)
        .with_config(move |config| {
            let base_url = config.model_provider.base_url.clone();
            config.model_provider_id = provider_id.to_string();
            config.model_provider = built_in_model_providers(/*openai_base_url*/ None)
                .remove(provider_id)
                .expect("built-in Bedrock provider");
            config.model_provider.base_url = base_url;
            config.service_tier = Some("ultrafast".to_string());
        })
        .build_with_auto_env(&server)
        .await?;
    assert!(test.config.model_catalog.is_none());

    test.submit_turn("configured ultrafast").await?;
    test.submit_turn_with_service_tier("standard", Some("default"))
        .await?;
    test.submit_turn_with_service_tier("ultrafast again", Some("ultrafast"))
        .await?;

    assert_eq!(
        mock.requests()
            .iter()
            .map(|request| request.body_json().get("service_tier").cloned())
            .collect::<Vec<_>>(),
        vec![
            Some(serde_json::json!("ultrafast")),
            None,
            Some(serde_json::json!("ultrafast"))
        ]
    );
    Ok(())
}

#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-6-astra", &["ultrafast"], Some("ultrafast"), Some("ultrafast"); "mantle astra")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-6-astra", &["ultrafast"], Some("ultrafast"), Some("ultrafast"); "runtime global astra")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "us.openai.gpt-6-astra", &["ultrafast"], Some("ultrafast"), Some("ultrafast"); "runtime us astra")]
#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-6-astra", &["priority"], Some("priority"), Some("priority"); "mantle priority")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-6-astra", &["flex"], Some("flex"), Some("flex"); "runtime flex")]
#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-6-sol", &["custom-tier"], Some("custom-tier"), Some("custom-tier"); "mantle custom tier")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-6-sol", &["custom-tier"], Some("custom-tier"), Some("custom-tier"); "runtime custom tier")]
#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-6-astra", &[], Some("ultrafast"), None; "mantle empty tiers")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-6-astra", &[], Some("ultrafast"), None; "runtime empty tiers")]
#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-6-astra", &["ultrafast"], Some("priority"), None; "mantle unadvertised tier")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-6-astra", &["ultrafast"], Some("flex"), None; "runtime unadvertised tier")]
#[test_case::test_case(AMAZON_BEDROCK_PROVIDER_ID, "openai.gpt-6-astra", &["ultrafast"], None, None; "mantle core requires explicit selection")]
#[test_case::test_case(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID, "global.openai.gpt-6-astra", &["ultrafast"], None, None; "runtime core requires explicit selection")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bedrock_custom_catalog_controls_service_tiers(
    provider_id: &'static str,
    model: &'static str,
    advertised_tiers: &'static [&'static str],
    tier: Option<&'static str>,
    expected_tier: Option<&'static str>,
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
            region: "us-west-2".to_string(),
        }))
        .with_model(model)
        .with_model_info_override(model, move |info| {
            info.service_tiers = advertised_tiers
                .iter()
                .map(|id| ModelServiceTier {
                    id: id.to_string(),
                    name: id.to_string(),
                    description: id.to_string(),
                })
                .collect();
            info.default_service_tier = advertised_tiers.first().map(ToString::to_string);
        })
        .with_config(move |config| {
            let base_url = config.model_provider.base_url.clone();
            config.model_provider_id = provider_id.to_string();
            config.model_provider = built_in_model_providers(/*openai_base_url*/ None)
                .remove(provider_id)
                .expect("built-in Bedrock provider");
            config.model_provider.base_url = base_url;
            config.service_tier = tier.map(str::to_string);
        })
        .build_with_auto_env(&server)
        .await?;

    test.submit_turn("hello").await?;

    assert_eq!(
        mock.single_request()
            .body_json()
            .get("service_tier")
            .cloned(),
        expected_tier.map(serde_json::Value::from)
    );
    Ok(())
}
