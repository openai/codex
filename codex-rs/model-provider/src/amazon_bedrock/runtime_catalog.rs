use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID;
use codex_protocol::openai_models::ModelsResponse;

use super::catalog::static_model_catalog;

const ROUTING_VARIANTS: [(&str, &str, i32); 2] =
    [("global.", "Global", 0), ("us.", "US cross-region", 1)];

pub(super) fn static_runtime_model_catalog() -> ModelsResponse {
    let base_models = static_model_catalog()
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
        .collect::<Vec<_>>();
    let mut models = Vec::with_capacity(ROUTING_VARIANTS.len() * base_models.len());
    for (prefix, routing_label, routing_priority) in ROUTING_VARIANTS {
        for model in &base_models {
            let mut variant = model.clone();
            variant.slug = format!("{prefix}{}", model.slug);
            variant.display_name = format!("{} ({routing_label})", model.display_name);
            variant.priority = routing_priority * base_models.len() as i32 + model.priority;
            variant.supports_search_tool = false;
            models.push(variant);
        }
    }
    ModelsResponse { models }
}

#[cfg(test)]
#[path = "runtime_catalog_tests.rs"]
mod tests;
