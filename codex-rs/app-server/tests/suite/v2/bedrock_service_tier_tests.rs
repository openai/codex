//! Verifies Bedrock service-tier discovery through the public model/list API.

use app_test_support::TestAppServer;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::ModelServiceTier;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[test_case::test_case("amazon-bedrock", vec!["openai.gpt-6-astra"]; "mantle")]
#[test_case::test_case("amazon-bedrock-runtime", vec!["global.openai.gpt-6-astra", "us.openai.gpt-6-astra"]; "runtime")]
#[tokio::test]
async fn bedrock_model_list_advertises_ultrafast_without_changing_default(
    provider_id: &str,
    expected_models: Vec<&str>,
) -> anyhow::Result<()> {
    let home = TempDir::new()?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"model_provider = "{provider_id}"
[model_providers.{provider_id}.aws]
region = "us-west-2"
"#
        ),
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let response: ModelListResponse = app_server
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams {
                cursor: None,
                limit: Some(100),
                include_hidden: Some(true),
            },
        })
        .await?;

    assert_eq!(
        response
            .data
            .iter()
            .filter(|model| !model.service_tiers.is_empty())
            .map(|model| (
                model.model.as_str(),
                model.service_tiers.clone(),
                model.default_service_tier.clone()
            ))
            .collect::<Vec<_>>(),
        expected_models
            .into_iter()
            .map(|model| (
                model,
                vec![ModelServiceTier {
                    id: "ultrafast".to_string(),
                    name: "Ultrafast".to_string(),
                    description: "The fastest available responses for latency-sensitive work."
                        .to_string(),
                }],
                None
            ))
            .collect::<Vec<_>>()
    );
    Ok(())
}

#[test_case::test_case("amazon-bedrock", "openai.gpt-6-astra"; "mantle")]
#[test_case::test_case("amazon-bedrock-runtime", "global.openai.gpt-6-astra"; "runtime")]
#[tokio::test]
async fn bedrock_model_list_preserves_custom_service_tiers(
    provider_id: &str,
    model_id: &str,
) -> anyhow::Result<()> {
    let home = TempDir::new()?;
    let mut model = codex_models_manager::bundled_models_response()?
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-6-astra")
        .expect("bundled Astra model");
    model.slug = model_id.to_string();
    model.additional_speed_tiers = vec!["fast".to_string()];
    model.service_tiers = vec![codex_protocol::openai_models::ModelServiceTier {
        id: "ultrafast".to_string(),
        name: "Express".to_string(),
        description: "My custom Bedrock tier.".to_string(),
    }];
    model.default_service_tier = Some("ultrafast".to_string());
    let catalog_path = home.path().join("models.json");
    std::fs::write(
        &catalog_path,
        serde_json::to_vec(&codex_protocol::openai_models::ModelsResponse {
            models: vec![model],
        })?,
    )?;
    let catalog_path = serde_json::to_string(&catalog_path)?;
    std::fs::write(
        home.path().join("config.toml"),
        format!(
            r#"model_provider = "{provider_id}"
model_catalog_json = {catalog_path}
[model_providers.{provider_id}.aws]
region = "us-west-2"
"#
        ),
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(home.path())
        .build_initialized()
        .await?;
    let response: ModelListResponse = app_server
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams {
                cursor: None,
                limit: Some(100),
                include_hidden: Some(true),
            },
        })
        .await?;

    assert_eq!(
        response
            .data
            .into_iter()
            .map(|model| (
                model.model,
                model.additional_speed_tiers,
                model.service_tiers,
                model.default_service_tier,
            ))
            .collect::<Vec<_>>(),
        vec![(
            model_id.to_string(),
            vec!["fast".to_string()],
            vec![ModelServiceTier {
                id: "ultrafast".to_string(),
                name: "Express".to_string(),
                description: "My custom Bedrock tier.".to_string(),
            }],
            Some("ultrafast".to_string()),
        )]
    );
    Ok(())
}
