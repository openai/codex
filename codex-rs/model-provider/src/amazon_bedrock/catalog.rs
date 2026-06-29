use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_4_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_5_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID;
use codex_models_manager::bundled_models_response;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::ReasoningEffortPreset;

const GPT_5_BEDROCK_CONTEXT_WINDOW: i64 = 272_000;
const GPT_5_5_OPENAI_MODEL_ID: &str = "gpt-5.5";
const GPT_5_4_OPENAI_MODEL_ID: &str = "gpt-5.4";

pub(crate) fn static_model_catalog() -> ModelsResponse {
    with_default_only_service_tier(ModelsResponse {
        models: vec![
            gpt_5_bedrock_model(
                GPT_5_5_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_5_MODEL_ID,
                /*priority*/ 0,
            ),
            gpt_5_bedrock_model(
                GPT_5_4_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_4_MODEL_ID,
                /*priority*/ 1,
            ),
            gpt_5_6_bedrock_model(
                AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID,
                "Sol",
                /*priority*/ 2,
            ),
            gpt_5_6_bedrock_model(
                AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID,
                "Terra",
                /*priority*/ 3,
            ),
            gpt_5_6_bedrock_model(
                AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID,
                "Luna",
                /*priority*/ 4,
            ),
        ],
    })
}

pub(crate) fn with_default_only_service_tier(mut catalog: ModelsResponse) -> ModelsResponse {
    for model in &mut catalog.models {
        // Amazon Bedrock currently only supports the implicit "default" tier for GPT models.
        model.additional_speed_tiers.clear();
        model.service_tiers.clear();
        model.default_service_tier = None;
    }
    catalog
}

fn gpt_5_bedrock_model(openai_slug: &str, bedrock_slug: &str, priority: i32) -> ModelInfo {
    let mut model = bundled_openai_model(openai_slug);
    model.slug = bedrock_slug.to_string();
    model.priority = priority;
    model.context_window = Some(GPT_5_BEDROCK_CONTEXT_WINDOW);
    model.max_context_window = Some(GPT_5_BEDROCK_CONTEXT_WINDOW);
    model
}

fn gpt_5_6_bedrock_model(bedrock_slug: &str, display_name: &str, priority: i32) -> ModelInfo {
    let mut model = gpt_5_bedrock_model(GPT_5_5_OPENAI_MODEL_ID, bedrock_slug, priority);
    model.display_name = display_name.to_string();
    model
        .supported_reasoning_levels
        .push(ReasoningEffortPreset {
            effort: ReasoningEffort::Max,
            description: "Maximum reasoning depth for the hardest problems".to_string(),
        });
    model
}

fn bundled_openai_model(slug: &str) -> ModelInfo {
    bundled_models_response()
        .unwrap_or_else(|err| panic!("bundled models.json should parse: {err}"))
        .models
        .into_iter()
        .find(|model| model.slug == slug)
        .unwrap_or_else(|| panic!("bundled models.json should include {slug}"))
}

#[cfg(test)]
mod tests {
    use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn catalog_uses_mantle_model_ids_as_slugs() {
        let catalog = static_model_catalog();

        assert_eq!(
            catalog
                .models
                .iter()
                .map(|model| model.slug.as_str())
                .collect::<Vec<_>>(),
            vec![
                AMAZON_BEDROCK_GPT_5_5_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_4_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID,
            ]
        );
    }

    #[test]
    fn gpt_5_bedrock_models_use_bedrock_context_window() {
        let catalog = static_model_catalog();

        for model in catalog.models {
            assert_eq!(
                (model.context_window, model.max_context_window),
                (
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW)
                )
            );
        }
    }

    #[test]
    fn gpt_5_6_bedrock_models_clone_gpt_5_5_config_with_max_reasoning_effort() {
        let catalog = static_model_catalog();
        let gpt_5_5 = catalog
            .models
            .iter()
            .find(|model| model.slug == AMAZON_BEDROCK_GPT_5_5_MODEL_ID)
            .expect("Bedrock catalog should include GPT-5.5");

        for (slug, display_name, priority) in [
            (AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID, "Sol", 2),
            (AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID, "Terra", 3),
            (AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID, "Luna", 4),
        ] {
            let mut expected = gpt_5_5.clone();
            expected.slug = slug.to_string();
            expected.display_name = display_name.to_string();
            expected.priority = priority;
            expected
                .supported_reasoning_levels
                .push(ReasoningEffortPreset {
                    effort: ReasoningEffort::Max,
                    description: "Maximum reasoning depth for the hardest problems".to_string(),
                });

            assert_eq!(
                catalog.models.iter().find(|model| model.slug == slug),
                Some(&expected)
            );
        }
    }

    #[test]
    fn gpt_5_bedrock_models_only_allow_default_service_tier() {
        let catalog = static_model_catalog();

        for model in catalog.models {
            assert_eq!(model.additional_speed_tiers, Vec::<String>::new());
            assert_eq!(model.service_tiers, Vec::new());
            assert_eq!(model.default_service_tier, None);
            assert_eq!(
                model.service_tier_for_request(Some("priority".to_string())),
                None
            );
            assert_eq!(
                model
                    .service_tier_for_request(Some(SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string())),
                None
            );
        }
    }
}
