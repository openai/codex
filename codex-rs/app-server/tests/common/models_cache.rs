use chrono::DateTime;
use chrono::Utc;
use codex_core::openai_models::model_presets::all_model_presets;
use codex_protocol::openai_models::ClientVersion;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelPreset;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ReasoningSummaryFormat;
use codex_protocol::openai_models::TruncationPolicyConfig;
use serde_json::json;
use std::path::Path;

/// Convert a ModelPreset to ModelInfo for cache storage.
fn preset_to_info(preset: &ModelPreset, priority: i32) -> ModelInfo {
    ModelInfo {
        slug: preset.id.clone(),
        display_name: preset.display_name.clone(),
        description: Some(preset.description.clone()),
        default_reasoning_level: preset.default_reasoning_effort,
        supported_reasoning_levels: preset.supported_reasoning_efforts.clone(),
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: if preset.show_in_picker {
            ModelVisibility::List
        } else {
            ModelVisibility::Hide
        },
        minimal_client_version: ClientVersion(0, 1, 0),
        supported_in_api: true,
        priority,
        upgrade: preset.upgrade.as_ref().map(|u| u.id.clone()),
        base_instructions: None,
        supports_reasoning_summaries: false,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        truncation_policy: TruncationPolicyConfig::bytes(10_000),
        supports_parallel_tool_calls: false,
        context_window: None,
        reasoning_summary_format: ReasoningSummaryFormat::None,
        experimental_supported_tools: Vec::new(),
    }
}

/// Write a models_cache.json file to the codex home directory.
/// This prevents ModelsManager from making network requests to refresh models.
/// The cache will be treated as fresh (within TTL) and used instead of fetching from the network.
/// Uses the built-in model presets from ModelsManager, converted to ModelInfo format.
pub fn write_models_cache(codex_home: &Path) -> std::io::Result<()> {
    // Get all presets and filter for show_in_picker (same as builtin_model_presets does)
    let presets: Vec<&ModelPreset> = all_model_presets()
        .iter()
        .filter(|preset| preset.show_in_picker)
        .collect();
    // Convert presets to ModelInfo, assigning priorities (higher = earlier in list)
    // Priority is used for sorting, so first model gets highest priority
    let models: Vec<ModelInfo> = presets
        .iter()
        .enumerate()
        .map(|(idx, preset)| {
            // Higher priority = earlier in list, so reverse the index
            let priority = (presets.len() - idx) as i32;
            preset_to_info(preset, priority)
        })
        .collect();

    write_models_cache_with_models(codex_home, models)
}

/// Write a models_cache.json file with specific models.
/// Useful when tests need specific models to be available.
pub fn write_models_cache_with_models(
    codex_home: &Path,
    models: Vec<ModelInfo>,
) -> std::io::Result<()> {
    let cache_path = codex_home.join("models_cache.json");
    // DateTime<Utc> serializes to RFC3339 format by default with serde
    let fetched_at: DateTime<Utc> = Utc::now();
    let cache = json!({
        "fetched_at": fetched_at,
        "etag": null,
        "models": models
    });
    std::fs::write(cache_path, serde_json::to_string_pretty(&cache)?)
}
