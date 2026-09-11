use codex_core::test_support::all_model_presets;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::default_input_modalities;
use std::path::Path;

/// Convert a ModelPreset to ModelInfo for cache storage.
fn preset_to_info(preset: &ModelPreset, priority: i32) -> ModelInfo {
    ModelInfo {
        slug: preset.id.clone(),
        display_name: preset.display_name.clone(),
        description: Some(preset.description.clone()),
        default_reasoning_level: Some(preset.default_reasoning_effort.clone()),
        supported_reasoning_levels: preset.supported_reasoning_efforts.clone(),
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: if preset.show_in_picker {
            ModelVisibility::List
        } else {
            ModelVisibility::Hide
        },
        supported_in_api: preset.supported_in_api,
        priority,
        additional_speed_tiers: preset.additional_speed_tiers.clone(),
        service_tiers: preset.service_tiers.clone(),
        default_service_tier: preset.default_service_tier.clone(),
        available_access_programs: preset.available_access_programs.clone(),
        upgrade: preset.upgrade.as_ref().map(Into::into),
        model_messages: Some(ModelMessages {
            persistent_instructions: None,
            tools: None,
            instructions_template: Some("base instructions".to_string()),
            instructions_variables: None,
            approvals: None,
            collaboration_modes: None,
            auto_review: None,
            permissions: None,
            multi_agent: None,
            token_budget: None,
            confirmation_policies: None,
            guardian_v2: None,
        }),
        include_skills_usage_instructions: false,
        include_plugin_usage_instructions: false,
        include_apps_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        availability_nux: preset.availability_nux.clone(),
        apply_patch_tool_type: None,
        web_search_tool_type: Default::default(),
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_image_detail_original: false,
        context_window: Some(272_000),
        max_context_window: None,
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        used_fallback_model_metadata: false,
        supports_search_tool: false,
        supports_experimental_context: false,
        use_responses_lite: false,
        guardian: None,
        node_repl_auto_review_required: false,
        node_repl_disabled: false,
        auto_review_model_override: None,
        model_specialty: None,
        tool_mode: None,
        multi_agent_version: preset.multi_agent_version,
        multi_agent_reasoning_effort: None,
    }
}

/// Write a models_cache.json file to the codex home directory.
/// This prevents ModelsManager from making network requests to refresh models.
/// The cache will be treated as fresh (within TTL) and used instead of fetching from the network.
/// Uses bundled-catalog-derived presets, converted to ModelInfo format.
pub async fn write_models_cache(codex_home: &Path) -> std::io::Result<()> {
    // Get a stable bundled-catalog-derived preset list and filter for picker-visible entries.
    let presets: Vec<&ModelPreset> = all_model_presets()
        .iter()
        .filter(|preset| preset.show_in_picker)
        .collect();
    // Convert presets to ModelInfo, assigning priorities (lower = earlier in list).
    // Priority is used for sorting, so the first model gets the lowest priority.
    let models: Vec<ModelInfo> = presets
        .iter()
        .enumerate()
        .map(|(idx, preset)| {
            // Lower priority = earlier in list.
            let priority = idx as i32;
            preset_to_info(preset, priority)
        })
        .collect();

    write_models_cache_with_models(codex_home, models).await
}

/// Write a models_cache.json file with specific models.
/// Useful when tests need specific models to be available.
pub async fn write_models_cache_with_models(
    codex_home: &Path,
    models: Vec<ModelInfo>,
) -> std::io::Result<()> {
    let config = codex_core::config::ConfigBuilder::default()
        .loader_overrides(codex_config::LoaderOverrides::without_managed_config_for_tests())
        .codex_home(codex_home.to_path_buf())
        .build()
        .await?;
    let auth = codex_login::CodexAuth::from_auth_storage(
        codex_home,
        config.cli_auth_credentials_store_mode,
        Some(&config.chatgpt_base_url),
        config.auth_keyring_backend_kind(),
        &codex_login::test_support::transport_default_auth_route_config(),
    )
    .await?;
    let cache = codex_model_provider::test_support::models_cache_entry(
        &config.model_provider,
        auth.as_ref(),
        models,
    );
    let cache_path = codex_home.join("models_cache.json");
    std::fs::write(cache_path, serde_json::to_string_pretty(&cache)?)
}
