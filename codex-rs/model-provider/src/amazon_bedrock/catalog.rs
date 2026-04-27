use codex_models_manager::model_info::BASE_INSTRUCTIONS;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::config_types::Verbosity;
use codex_protocol::openai_models::ApplyPatchToolType;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::InputModality;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::WebSearchToolType;

const GPT_OSS_CONTEXT_WINDOW: i64 = 128_000;
const GPT_5_4_CONTEXT_WINDOW: i64 = 272_000;
const GPT_5_4_MAX_CONTEXT_WINDOW: i64 = 1_000_000;
const GPT_5_4_CMB_MODEL_ID: &str = "openai.gpt-5.4-cmb";

pub(crate) fn static_model_catalog() -> ModelsResponse {
    ModelsResponse {
        models: vec![
            gpt_5_4_cmb_bedrock_model(/*priority*/ 0),
            bedrock_oss_model(
                "openai.gpt-oss-120b",
                "GPT OSS 120B on Bedrock",
                /*priority*/ 1,
            ),
            bedrock_oss_model(
                "openai.gpt-oss-20b",
                "GPT OSS 20B on Bedrock",
                /*priority*/ 2,
            ),
        ],
    }
}

fn gpt_5_4_cmb_bedrock_model(priority: i32) -> ModelInfo {
    ModelInfo {
        slug: GPT_5_4_CMB_MODEL_ID.to_string(),
        display_name: "gpt-5.4".to_string(),
        description: Some("Strong model for everyday coding.".to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: gpt_5_4_cmb_reasoning_levels(),
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority,
        additional_speed_tiers: vec!["fast".to_string()],
        availability_nux: None,
        upgrade: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        model_messages: None,
        supports_reasoning_summaries: true,
        default_reasoning_summary: ReasoningSummary::None,
        support_verbosity: true,
        default_verbosity: Some(Verbosity::Medium),
        apply_patch_tool_type: Some(ApplyPatchToolType::Function),
        web_search_tool_type: WebSearchToolType::TextAndImage,
        truncation_policy: TruncationPolicyConfig::tokens(/*limit*/ 10_000),
        supports_parallel_tool_calls: true,
        supports_image_detail_original: true,
        context_window: Some(GPT_5_4_CONTEXT_WINDOW),
        max_context_window: Some(GPT_5_4_MAX_CONTEXT_WINDOW),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: vec![InputModality::Text, InputModality::Image],
        used_fallback_model_metadata: false,
        supports_search_tool: true,
    }
}

fn bedrock_oss_model(slug: &str, display_name: &str, priority: i32) -> ModelInfo {
    ModelInfo {
        slug: slug.to_string(),
        display_name: display_name.to_string(),
        description: Some(display_name.to_string()),
        default_reasoning_level: Some(ReasoningEffort::Medium),
        supported_reasoning_levels: vec![
            reasoning_effort_preset(ReasoningEffort::Low),
            reasoning_effort_preset(ReasoningEffort::Medium),
            reasoning_effort_preset(ReasoningEffort::High),
        ],
        shell_type: ConfigShellToolType::ShellCommand,
        visibility: ModelVisibility::List,
        supported_in_api: true,
        priority,
        additional_speed_tiers: Vec::new(),
        availability_nux: None,
        upgrade: None,
        base_instructions: BASE_INSTRUCTIONS.to_string(),
        model_messages: None,
        supports_reasoning_summaries: true,
        default_reasoning_summary: ReasoningSummary::None,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        web_search_tool_type: WebSearchToolType::Text,
        truncation_policy: TruncationPolicyConfig::tokens(/*limit*/ 10_000),
        supports_parallel_tool_calls: true,
        supports_image_detail_original: false,
        context_window: Some(GPT_OSS_CONTEXT_WINDOW),
        max_context_window: Some(GPT_OSS_CONTEXT_WINDOW),
        auto_compact_token_limit: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: vec![InputModality::Text],
        used_fallback_model_metadata: false,
        supports_search_tool: false,
    }
}

fn gpt_5_4_cmb_reasoning_levels() -> Vec<ReasoningEffortPreset> {
    vec![
        reasoning_effort_preset(ReasoningEffort::Minimal),
        reasoning_effort_preset(ReasoningEffort::Low),
        reasoning_effort_preset(ReasoningEffort::Medium),
        reasoning_effort_preset(ReasoningEffort::High),
    ]
}

fn reasoning_effort_preset(effort: ReasoningEffort) -> ReasoningEffortPreset {
    ReasoningEffortPreset {
        effort,
        description: match effort {
            ReasoningEffort::None => "No reasoning",
            ReasoningEffort::Minimal => "Minimal reasoning",
            ReasoningEffort::Low => "Fast responses with lighter reasoning",
            ReasoningEffort::Medium => "Balances speed and reasoning depth for everyday tasks",
            ReasoningEffort::High => "Greater reasoning depth for complex problems",
            ReasoningEffort::XHigh => "Extra high reasoning depth for complex problems",
        }
        .to_string(),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn catalog_uses_mantle_model_ids_as_slugs() {
        let catalog = static_model_catalog();

        assert_eq!(catalog.models.len(), 3);
        assert_eq!(catalog.models[0].slug, GPT_5_4_CMB_MODEL_ID);
        assert_eq!(catalog.models[1].slug, "openai.gpt-oss-120b");
        assert_eq!(catalog.models[2].slug, "openai.gpt-oss-20b");
    }

    #[test]
    fn gpt_5_4_cmb_advertises_only_bedrock_supported_reasoning_levels() {
        let catalog = static_model_catalog();
        let cmb_model = catalog
            .models
            .iter()
            .find(|model| model.slug == GPT_5_4_CMB_MODEL_ID)
            .expect("Bedrock catalog should include GPT-5.4 CMB");

        assert_eq!(
            cmb_model.supported_reasoning_levels,
            gpt_5_4_cmb_reasoning_levels()
        );
    }
}
