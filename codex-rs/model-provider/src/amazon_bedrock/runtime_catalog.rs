use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID;
use codex_protocol::openai_models::ModelsResponse;

use super::catalog::static_model_catalog;

const ROUTING_VARIANTS: [(&str, &str, i32); 2] =
    [("global.", "Global", 0), ("us.", "US cross-region", 1)];

pub(super) fn static_runtime_model_catalog() -> ModelsResponse {
    let models = static_model_catalog()
        .models
        .into_iter()
        .filter(|model| {
            matches!(
                model.slug.as_str(),
                AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID
                    | AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID
                    | AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID
            )
        })
        .flat_map(|model| {
            ROUTING_VARIANTS
                .into_iter()
                .map(move |(prefix, routing_label, routing_priority)| {
                    let mut variant = model.clone();
                    variant.slug = format!("{prefix}{}", model.slug);
                    variant.display_name = format!("{} ({routing_label})", model.display_name);
                    variant.priority = model.priority * 2 + routing_priority;
                    variant.supports_search_tool = false;
                    variant
                })
        })
        .collect();
    ModelsResponse { models }
}

#[cfg(test)]
#[path = "runtime_catalog_tests.rs"]
mod tests;
