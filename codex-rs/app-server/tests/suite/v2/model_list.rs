use std::time::Duration;

use anyhow::Error;
use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use app_test_support::write_models_cache;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetParams;
use codex_app_server_protocol::ExperimentalFeatureEnablementSetResponse;
use codex_app_server_protocol::JSONRPCError;
use codex_app_server_protocol::Model;
use codex_app_server_protocol::ModelListParams;
use codex_app_server_protocol::ModelListResponse;
use codex_app_server_protocol::ModelServiceTier;
use codex_app_server_protocol::ModelUpgradeInfo;
use codex_app_server_protocol::ReasoningEffortOption;
use codex_app_server_protocol::RequestId;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::login_with_api_key;
use codex_protocol::openai_models::MODEL_SPECIALTY_CYBER;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelsResponse;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const INVALID_REQUEST_ERROR_CODE: i64 = -32600;

#[test_case(None, false; "default off")]
#[test_case(None, true; "app rollout")]
#[test_case(Some(false), true; "user opt out")]
#[test_case(Some(true), false; "user opt in")]
#[tokio::test]
async fn api_key_model_discovery_startup_enablement_respects_user_config(
    user_enablement: Option<bool>,
    app_enablement: bool,
) -> Result<()> {
    let server = MockServer::start().await;
    let mut remote_model = codex_models_manager::bundled_models_response()?
        .models
        .remove(0);
    remote_model.slug = "rollout-model".into();
    remote_model.visibility = codex_protocol::openai_models::ModelVisibility::List;
    remote_model.supported_in_api = true;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(ModelsResponse {
                models: vec![remote_model.clone()],
            }),
        )
        .mount(&server)
        .await;
    let codex_home = TempDir::new()?;
    let server_uri = server.uri();
    let feature_config = user_enablement
        .map(|enabled| format!("[features]\napi_key_model_discovery = {enabled}\n"))
        .unwrap_or_default();
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!("openai_base_url = \"{server_uri}/v1\"\n{feature_config}"),
    )?;
    login_with_api_key(
        codex_home.path(),
        "test-key",
        AuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
    )?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .with_env_overrides(&[("OPENAI_API_KEY", None), ("CODEX_API_KEY", None)])
        .build_initialized()
        .await?;
    let mut bundled = codex_models_manager::bundled_models_response()?.models;
    bundled.sort_by_key(|model| model.priority);
    let mut bundled = ModelPreset::filter_by_auth(
        bundled.into_iter().map(Into::into).collect(),
        /*chatgpt_mode*/ false,
    );
    ModelPreset::mark_default_by_picker_visibility(&mut bundled);
    let mut remote = vec![ModelPreset::from(remote_model)];
    ModelPreset::mark_default_by_picker_visibility(&mut remote);
    let _: ExperimentalFeatureEnablementSetResponse = mcp
        .request(
            |request_id| ClientRequest::ExperimentalFeatureEnablementSet {
                request_id,
                params: ExperimentalFeatureEnablementSetParams {
                    enablement: [("api_key_model_discovery".to_string(), app_enablement)].into(),
                },
            },
        )
        .await?;
    let response: ModelListResponse = mcp
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams {
                limit: Some(100),
                include_hidden: Some(true),
                cursor: None,
            },
        })
        .await?;
    let enabled = user_enablement.unwrap_or(app_enablement);
    let expected = if enabled { &remote } else { &bundled };
    assert_eq!(
        response,
        ModelListResponse {
            data: expected
                .iter()
                .map(|preset| Model {
                    // These catalogs retain personality metadata; the cache fixture does not.
                    supports_personality: preset.supports_personality,
                    ..model_from_preset(preset)
                })
                .collect(),
            next_cursor: None,
        }
    );
    if !enabled {
        assert!(
            server
                .received_requests()
                .await
                .expect("request recording is enabled")
                .is_empty()
        );
    }
    Ok(())
}

fn model_from_preset(preset: &ModelPreset) -> Model {
    Model {
        id: preset.id.clone(),
        model: preset.model.clone(),
        upgrade: preset.upgrade.as_ref().map(|upgrade| upgrade.id.clone()),
        upgrade_info: preset.upgrade.as_ref().map(|upgrade| ModelUpgradeInfo {
            model: upgrade.id.clone(),
            upgrade_copy: upgrade.upgrade_copy.clone(),
            model_link: upgrade.model_link.clone(),
            migration_markdown: upgrade.migration_markdown.clone(),
            retirement_at: upgrade
                .retirement_at
                .as_ref()
                .map(chrono::DateTime::timestamp),
        }),
        availability_nux: preset.availability_nux.clone().map(Into::into),
        display_name: preset.display_name.clone(),
        description: preset.description.clone(),
        model_specialty: preset.model_specialty.clone(),
        hidden: !preset.show_in_picker,
        supported_reasoning_efforts: preset
            .supported_reasoning_efforts
            .iter()
            .map(|preset| ReasoningEffortOption {
                reasoning_effort: preset.effort.clone(),
                description: preset.description.clone(),
            })
            .collect(),
        default_reasoning_effort: preset.default_reasoning_effort.clone(),
        input_modalities: preset.input_modalities.clone(),
        // `write_models_cache().await` round-trips through a simplified ModelInfo fixture that does not
        // preserve personality placeholders in base instructions, so app-server list results from
        // cache report `supports_personality = false`.
        // todo(sayan): fix, maybe make roundtrip use ModelInfo only
        supports_personality: false,
        multi_agent_version: preset.multi_agent_version.map(Into::into),
        additional_speed_tiers: preset.additional_speed_tiers.clone(),
        service_tiers: preset
            .service_tiers
            .iter()
            .map(|service_tier| ModelServiceTier {
                id: service_tier.id.clone(),
                name: service_tier.name.clone(),
                description: service_tier.description.clone(),
            })
            .collect(),
        default_service_tier: preset.default_service_tier.clone(),
        available_access_programs: preset.available_access_programs.clone().map(Into::into),
        is_default: preset.is_default,
    }
}

fn expected_visible_models() -> Vec<Model> {
    // Filter by supported_in_api to support testing with both ChatGPT and non-ChatGPT auth modes.
    let mut presets = ModelPreset::filter_by_auth(
        codex_core::test_support::all_model_presets().clone(),
        /*chatgpt_mode*/ false,
    );

    // Mirror `ModelsManager::build_available_models()` default selection after auth filtering.
    ModelPreset::mark_default_by_picker_visibility(&mut presets);

    presets
        .iter()
        .filter(|preset| preset.show_in_picker)
        .map(model_from_preset)
        .collect()
}

#[tokio::test]
async fn list_models_returns_all_models_with_large_limit() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path()).await?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let ModelListResponse {
        data: items,
        next_cursor,
    } = mcp
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams {
                limit: Some(100),
                cursor: None,
                include_hidden: None,
            },
        })
        .await?;

    let expected_models = expected_visible_models();

    assert_eq!(items, expected_models);
    assert!(next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn list_models_includes_hidden_models() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path()).await?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;
    let ModelListResponse {
        data: items,
        next_cursor,
    } = mcp
        .request(|request_id| ClientRequest::ModelList {
            request_id,
            params: ModelListParams {
                limit: Some(100),
                cursor: None,
                include_hidden: Some(true),
            },
        })
        .await?;

    assert!(items.iter().any(|item| item.hidden));
    assert!(next_cursor.is_none());
    Ok(())
}

#[test_case("chatgpt-access-token", None; "chatgpt")]
#[test_case("test-api-key", Some("test-api-key"); "api key")]
#[tokio::test]
async fn list_models_uses_remote_catalog_as_source_of_truth(
    bearer_token: &str,
    api_key: Option<&str>,
) -> Result<()> {
    let server = MockServer::start().await;
    let remote_models = [
        (
            json!("2030-01-01T00:00:00Z"),
            json!({ "cyber": ["standard", "daybreak_blue"] }),
        ),
        (json!(null), json!({ "cyber": ["daybreak_red"] })),
        (json!(null), json!({ "cyber": [] })),
        (json!(null), json!(null)),
    ]
    .into_iter()
    .enumerate()
    .map(|(priority, (retirement_at, access_programs))| {
        serde_json::from_value::<ModelInfo>(json!({
            "slug": format!("remote-only-{priority}"),
            "display_name": "Remote Only",
            "description": "Remote-only model for app-server model/list coverage",
            "model_specialty": MODEL_SPECIALTY_CYBER,
            "available_access_programs": access_programs,
            "default_reasoning_level": "max",
            "supported_reasoning_levels": [
                {"effort": "max", "description": "Maximum"},
                {"effort": "low", "description": "Low"},
                {"effort": "focused", "description": "Focused"}
            ],
            "shell_type": "shell_command",
            "visibility": "list",
            "minimal_client_version": [0, 1, 0],
            "supported_in_api": true,
            "priority": priority,
            "upgrade": {
                "model": "replacement-model",
                "migration_markdown": "Use the replacement model.",
                "retirement_at": retirement_at,
            },
            "support_verbosity": false,
            "default_verbosity": null,
            "apply_patch_tool_type": null,
            "truncation_policy": {"mode": "bytes", "limit": 10_000},
            "supports_image_detail_original": false,
            "multi_agent_version": "v2",
            "context_window": 272_000,
            "max_context_window": 272_000,
            "experimental_supported_tools": [],
        }))
    })
    .collect::<Result<Vec<_>, _>>()?;
    // The startup refresh worker and model/list can both fetch before the cache is populated.
    let _models_mock = Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("authorization", format!("Bearer {bearer_token}")))
        .respond_with(
            ResponseTemplate::new(/*s*/ 200).set_body_json(ModelsResponse {
                models: remote_models.clone(),
            }),
        )
        .expect(1..)
        .mount_as_scoped(&server)
        .await;

    let codex_home = TempDir::new()?;
    let server_uri = server.uri();
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
openai_base_url = "{server_uri}/v1"
[features]
api_key_model_discovery = true
"#
        ),
    )?;
    if let Some(api_key) = api_key {
        login_with_api_key(
            codex_home.path(),
            api_key,
            AuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )?;
    } else {
        write_chatgpt_auth(
            codex_home.path(),
            ChatGptAuthFixture::new(bearer_token).plan_type("pro"),
            AuthCredentialsStoreMode::File,
        )?;
    }

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .with_env_overrides(&[("OPENAI_API_KEY", None), ("CODEX_API_KEY", None)])
        .build_initialized()
        .await?;
    let request_id = mcp
        .send_list_models_request(ModelListParams {
            limit: Some(100),
            cursor: None,
            include_hidden: None,
        })
        .await?;
    let response = mcp
        .read_stream_until_response_message(RequestId::Integer(request_id))
        .await?;
    assert_eq!(
        response.result["data"][0]["upgradeInfo"]["retirementAt"],
        json!(1_893_456_000)
    );
    assert_eq!(
        response.result["data"][1]["upgradeInfo"]["retirementAt"],
        serde_json::Value::Null
    );
    assert_eq!(
        response.result["data"]
            .as_array()
            .expect("model/list data should be an array")
            .iter()
            .map(|model| model["availableAccessPrograms"].clone())
            .collect::<Vec<_>>(),
        vec![
            json!({ "cyber": ["standard", "daybreakBlue"] }),
            json!({ "cyber": ["daybreakRed"] }),
            json!({ "cyber": [] }),
            json!(null),
        ]
    );
    let ModelListResponse {
        data: items,
        next_cursor,
    } = serde_json::from_value(response.result)?;
    let mut expected_presets: Vec<ModelPreset> =
        remote_models.into_iter().map(Into::into).collect();
    ModelPreset::mark_default_by_picker_visibility(&mut expected_presets);
    let mut expected_items = expected_presets
        .iter()
        .map(model_from_preset)
        .collect::<Vec<_>>();
    expected_items[0].supported_reasoning_efforts = vec![
        ReasoningEffortOption {
            reasoning_effort: "max".parse().map_err(Error::msg)?,
            description: "Maximum".to_string(),
        },
        ReasoningEffortOption {
            reasoning_effort: "low".parse().map_err(Error::msg)?,
            description: "Low".to_string(),
        },
        ReasoningEffortOption {
            reasoning_effort: "focused".parse().map_err(Error::msg)?,
            description: "Focused".to_string(),
        },
    ];

    assert_eq!(items, expected_items);
    assert!(next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn list_models_pagination_works() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path()).await?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let expected_models = expected_visible_models();
    let mut cursor = None;
    let mut items = Vec::new();

    for _ in 0..expected_models.len() {
        let ModelListResponse {
            data: page_items,
            next_cursor,
        } = mcp
            .request(|request_id| ClientRequest::ModelList {
                request_id,
                params: ModelListParams {
                    limit: Some(1),
                    cursor: cursor.clone(),
                    include_hidden: None,
                },
            })
            .await?;

        assert_eq!(page_items.len(), 1);
        items.extend(page_items);

        if let Some(next_cursor) = next_cursor {
            cursor = Some(next_cursor);
        } else {
            assert_eq!(items, expected_models);
            return Ok(());
        }
    }

    panic!(
        "model pagination did not terminate after {} pages",
        expected_models.len()
    );
}

#[tokio::test]
async fn list_models_rejects_invalid_cursor() -> Result<()> {
    let codex_home = TempDir::new()?;
    write_models_cache(codex_home.path()).await?;
    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized()
        .await?;

    let request_id = mcp
        .send_list_models_request(ModelListParams {
            limit: None,
            cursor: Some("invalid".to_string()),
            include_hidden: None,
        })
        .await?;

    let error: JSONRPCError = timeout(
        DEFAULT_TIMEOUT,
        mcp.read_stream_until_error_message(RequestId::Integer(request_id)),
    )
    .await??;

    assert_eq!(error.id, RequestId::Integer(request_id));
    assert_eq!(error.error.code, INVALID_REQUEST_ERROR_CODE);
    assert_eq!(error.error.message, "invalid cursor: invalid");
    Ok(())
}
