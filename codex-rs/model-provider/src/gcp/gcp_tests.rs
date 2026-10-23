use codex_models_manager::ModelsManagerConfig;
use codex_protocol::config_types::ModelProviderAuthInfo;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::protocol::MultiAgentVersion;
use pretty_assertions::assert_eq;

use super::*;

#[test]
fn vertex_base_url_defaults_to_global_location() {
    assert_eq!(
        vertex_base_url(
            &ModelProviderInfo::default(),
            Some("example-project"),
            /*environment_location*/ None
        )
        .expect("project should resolve a Vertex endpoint"),
        "https://aiplatform.googleapis.com/v1/projects/example-project/locations/global/publishers/openai/v1"
    );
}

#[test]
fn vertex_base_url_uses_explicit_location() {
    assert_eq!(
        vertex_base_url(
            &ModelProviderInfo::default(),
            Some("example-project"),
            Some("us-central1")
        )
        .expect("project and location should resolve a Vertex endpoint"),
        "https://aiplatform.googleapis.com/v1/projects/example-project/locations/us-central1/publishers/openai/v1"
    );
}

#[test]
fn vertex_base_url_explains_missing_project() {
    let error = vertex_base_url(
        &ModelProviderInfo::default(),
        /*environment_project*/ None,
        /*environment_location*/ None,
    )
    .expect_err("missing project should be rejected");

    assert_eq!(
        error.to_string(),
        "Missing environment variable: `GOOGLE_CLOUD_PROJECT`. Set model_providers.gcp.project_id or GOOGLE_CLOUD_PROJECT to your Google Cloud project ID, or configure model_providers.gcp.base_url."
    );
}

#[test]
fn vertex_base_url_rejects_path_components() {
    let error = vertex_base_url(
        &ModelProviderInfo::default(),
        Some("example-project/other"),
        /*environment_location*/ None,
    )
    .expect_err("project must not add endpoint path components");

    assert_eq!(
        error.to_string(),
        "GOOGLE_CLOUD_PROJECT must be non-empty and contain only ASCII letters, digits, and hyphens"
    );
}

#[test]
fn configured_vertex_endpoint_resolves_each_field_independently() {
    for (project_id, location, expected) in [
        (
            Some("configured-project"),
            Some("us-east4"),
            "https://aiplatform.googleapis.com/v1/projects/configured-project/locations/us-east4/publishers/openai/v1",
        ),
        (
            Some("configured-project"),
            None,
            "https://aiplatform.googleapis.com/v1/projects/configured-project/locations/us-central1/publishers/openai/v1",
        ),
        (
            None,
            Some("us-east4"),
            "https://aiplatform.googleapis.com/v1/projects/environment-project/locations/us-east4/publishers/openai/v1",
        ),
    ] {
        let info = ModelProviderInfo {
            project_id: project_id.map(str::to_string),
            location: location.map(str::to_string),
            ..ModelProviderInfo::default()
        };
        assert_eq!(
            vertex_base_url(&info, Some("environment-project"), Some("us-central1"))
                .expect("configured fields should take precedence over environment values"),
            expected
        );
    }
}

#[test]
fn empty_configured_project_does_not_fall_back_to_environment() {
    let info = ModelProviderInfo {
        project_id: Some(String::new()),
        ..ModelProviderInfo::default()
    };
    let error = vertex_base_url(&info, Some("environment-project"), Some("us-central1"))
        .expect_err("invalid explicit config should not fall back to environment values");

    assert_eq!(
        error.to_string(),
        "model_providers.gcp.project_id must be non-empty and contain only ASCII letters, digits, and hyphens"
    );
}

#[tokio::test]
async fn configured_project_and_location_build_api_provider() {
    let provider = GcpModelProvider::new(
        ModelProviderInfo {
            project_id: Some("configured-project".to_string()),
            location: Some("us-east4".to_string()),
            ..ModelProviderInfo::create_gcp_provider()
        },
        /*auth_manager*/ None,
    );

    assert_eq!(
        provider
            .api_provider()
            .await
            .expect("configured endpoint should build")
            .base_url,
        "https://aiplatform.googleapis.com/v1/projects/configured-project/locations/us-east4/publishers/openai/v1"
    );
}

#[tokio::test]
async fn configured_base_url_bypasses_project_discovery() {
    let mut provider_info = ModelProviderInfo::create_gcp_provider();
    provider_info.base_url = Some("https://vertex.example.com/v1".to_string());
    provider_info.auth = Some(ModelProviderAuthInfo {
        command: "print-token".to_string(),
        args: Vec::new(),
        timeout_ms: std::num::NonZeroU64::new(5_000).expect("timeout should be non-zero"),
        refresh_interval_ms: 300_000,
        cwd: std::env::current_dir()
            .expect("current directory should be available")
            .try_into()
            .expect("current directory should be absolute"),
    });
    provider_info.project_id = Some("configured-project".to_string());
    provider_info.location = Some("us-east4".to_string());
    let provider = GcpModelProvider::new(provider_info, /*auth_manager*/ None);

    assert_eq!(
        provider
            .api_provider()
            .await
            .expect("configured endpoint should build")
            .base_url,
        "https://vertex.example.com/v1"
    );
    assert!(
        provider
            .auth_manager()
            .expect("GCP should create a command-backed auth manager")
            .has_external_auth()
    );
}

#[tokio::test]
async fn bundled_vertex_models_filter_catalog_and_normalize_unsupported_tool_metadata() {
    let provider = GcpModelProvider::new(
        ModelProviderInfo::create_gcp_provider(),
        /*auth_manager*/ None,
    );
    let manager = provider.models_manager(
        PathBuf::from("unused-codex-home"),
        /*config_model_catalog*/ None,
    );
    let bundled_models = codex_models_manager::bundled_models_response()
        .expect("bundled model catalog should deserialize")
        .models;
    let mut expected: Vec<_> = ["gpt-6-astra", "gpt-6.1-sol", "gpt-6-luna"]
        .into_iter()
        .map(|slug| {
            bundled_models
                .iter()
                .find(|model| model.slug == slug)
                .expect("bundled catalog should contain the supported Vertex model")
                .clone()
        })
        .collect();
    for model in &mut expected {
        model.use_responses_lite = false;
        model.tool_mode = Some(ToolMode::Direct);
        model.apply_patch_tool_type = None;
    }

    assert_eq!(manager.get_remote_models().await, expected);
    assert_eq!(
        manager
            .try_list_models()
            .expect("static models should be available")
            .into_iter()
            .map(|model| (model.model, model.show_in_picker))
            .collect::<Vec<_>>(),
        vec![
            ("gpt-6.1-sol".to_string(), true),
            ("gpt-6-astra".to_string(), true),
            ("gpt-6-luna".to_string(), true),
        ]
    );
}

#[tokio::test]
async fn configured_vertex_models_filter_catalog_and_normalize_tool_metadata_without_cache() {
    let provider = GcpModelProvider::new(
        ModelProviderInfo::create_gcp_provider(),
        /*auth_manager*/ None,
    );
    let mut configured_model = codex_models_manager::bundled_models_response()
        .expect("bundled model catalog should deserialize")
        .models
        .into_iter()
        .find(|model| model.slug == "gpt-6.1-sol")
        .expect("bundled model catalog should contain gpt-6.1-sol");
    configured_model.use_responses_lite = true;
    configured_model.tool_mode = Some(ToolMode::CodeModeOnly);
    configured_model.apply_patch_tool_type = Some(ApplyPatchToolType::Freeform);
    configured_model.multi_agent_version = Some(MultiAgentVersion::V2);
    let mut expected_model = configured_model.clone();
    expected_model.use_responses_lite = false;
    expected_model.tool_mode = Some(ToolMode::Direct);
    expected_model.apply_patch_tool_type = None;
    let mut unsupported_model = configured_model.clone();
    unsupported_model.slug = "gpt-6-sol".to_string();
    let mut unsupported_variant = configured_model.clone();
    unsupported_variant.slug = "gpt-6.1-sol-extra".to_string();
    let manager = provider.models_manager_without_cache(Some(ModelsResponse {
        models: vec![unsupported_model, configured_model, unsupported_variant],
    }));

    assert_eq!(manager.get_remote_models().await, vec![expected_model]);
}

#[tokio::test]
async fn unknown_vertex_models_use_static_catalog_fallback() {
    let provider = GcpModelProvider::new(
        ModelProviderInfo::create_gcp_provider(),
        /*auth_manager*/ None,
    );
    let manager = provider.models_manager(
        PathBuf::from("unused-codex-home"),
        Some(ModelsResponse::default()),
    );

    let model = manager
        .get_model_info("vertex-test-model", &ModelsManagerConfig::default())
        .await;

    assert_eq!(
        (model.slug, model.used_fallback_model_metadata),
        ("vertex-test-model".to_string(), true)
    );
}
