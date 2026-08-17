use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_4_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_5_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID;
use codex_model_provider_info::AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID;
use codex_models_manager::bundled_models_response;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ModelsResponse;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::WebSearchToolType;

const GPT_5_BEDROCK_CONTEXT_WINDOW: i64 = 272_000;
const GPT_5_6_SOL_OPENAI_MODEL_ID: &str = "gpt-5.6-sol";
const GPT_5_6_TERRA_OPENAI_MODEL_ID: &str = "gpt-5.6-terra";
const GPT_5_6_LUNA_OPENAI_MODEL_ID: &str = "gpt-5.6-luna";
const GPT_5_5_OPENAI_MODEL_ID: &str = "gpt-5.5";
const GPT_5_4_OPENAI_MODEL_ID: &str = "gpt-5.4";

pub(crate) fn static_model_catalog() -> ModelsResponse {
    normalize_bedrock_catalog(ModelsResponse {
        models: vec![
            gpt_5_6_bedrock_model(
                GPT_5_6_SOL_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID,
                "GPT-5.6 Sol",
                /*priority*/ 0,
            ),
            gpt_5_6_bedrock_model(
                GPT_5_6_TERRA_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID,
                "GPT-5.6 Terra",
                /*priority*/ 1,
            ),
            gpt_5_6_bedrock_model(
                GPT_5_6_LUNA_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID,
                "GPT-5.6 Luna",
                /*priority*/ 2,
            ),
            gpt_5_bedrock_model(
                GPT_5_5_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_5_MODEL_ID,
                "GPT-5.5",
                /*priority*/ 3,
            ),
            gpt_5_bedrock_model(
                GPT_5_4_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_4_MODEL_ID,
                "GPT-5.4",
                /*priority*/ 4,
            ),
        ],
    })
}

pub(crate) fn normalize_bedrock_catalog(mut catalog: ModelsResponse) -> ModelsResponse {
    for model in &mut catalog.models {
        // Amazon Bedrock currently only supports the implicit "default" tier for GPT models.
        model.additional_speed_tiers.clear();
        model.service_tiers.clear();
        model.default_service_tier = None;
        // Bedrock rejects the `search_content_types` field used by multimodal search.
        model.web_search_tool_type = WebSearchToolType::Text;
    }
    catalog
}

fn gpt_5_bedrock_model(
    openai_slug: &str,
    bedrock_slug: &str,
    display_name: &str,
    priority: i32,
) -> ModelInfo {
    let mut model = bundled_openai_model(openai_slug);
    model.slug = bedrock_slug.to_string();
    model.display_name = display_name.to_string();
    model.priority = priority;
    model.context_window = Some(GPT_5_BEDROCK_CONTEXT_WINDOW);
    model.max_context_window = Some(GPT_5_BEDROCK_CONTEXT_WINDOW);
    model.visibility = ModelVisibility::List;
    model.availability_nux = None;
    model.upgrade = None;
    model
}

fn gpt_5_6_bedrock_model(
    openai_slug: &str,
    bedrock_slug: &str,
    display_name: &str,
    priority: i32,
) -> ModelInfo {
    let mut model = bundled_openai_model(openai_slug);
    model.slug = bedrock_slug.to_string();
    model.display_name = display_name.to_string();
    model.priority = priority;
    model.visibility = ModelVisibility::List;
    model.availability_nux = None;
    model.upgrade = None;
    model.use_responses_lite = false;
    model.tool_mode = None;
    model
        .supported_reasoning_levels
        .retain(|level| level.effort != ReasoningEffort::Ultra);
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
    fn catalog_uses_mantle_model_ids_in_priority_order() {
        let catalog = static_model_catalog();

        assert_eq!(
            catalog
                .models
                .iter()
                .map(|model| model.slug.as_str())
                .collect::<Vec<_>>(),
            vec![
                AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_5_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_4_MODEL_ID,
            ]
        );
    }

    #[test]
    fn gpt_5_bedrock_models_use_bedrock_context_window() {
        let catalog = static_model_catalog();

        assert_eq!(
            catalog
                .models
                .iter()
                .map(|model| (
                    model.slug.as_str(),
                    model.context_window,
                    model.max_context_window,
                    model.web_search_tool_type,
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID,
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    Some(872_000),
                    WebSearchToolType::Text,
                ),
                (
                    AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID,
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    Some(872_000),
                    WebSearchToolType::Text,
                ),
                (
                    AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID,
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    Some(872_000),
                    WebSearchToolType::Text,
                ),
                (
                    AMAZON_BEDROCK_GPT_5_5_MODEL_ID,
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    WebSearchToolType::Text,
                ),
                (
                    AMAZON_BEDROCK_GPT_5_4_MODEL_ID,
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    Some(GPT_5_BEDROCK_CONTEXT_WINDOW),
                    WebSearchToolType::Text,
                ),
            ]
        );
    }

    #[test]
    fn configured_bedrock_catalogs_normalize_unsupported_model_capabilities() {
        let model = bundled_openai_model(GPT_5_4_OPENAI_MODEL_ID);
        let mut expected = model.clone();
        expected.additional_speed_tiers.clear();
        expected.service_tiers.clear();
        expected.default_service_tier = None;
        expected.web_search_tool_type = WebSearchToolType::Text;

        assert_eq!(
            normalize_bedrock_catalog(ModelsResponse {
                models: vec![model],
            }),
            ModelsResponse {
                models: vec![expected],
            }
        );
    }

    #[test]
    fn gpt_5_bedrock_models_do_not_include_availability_nux_or_upgrade() {
        let catalog = static_model_catalog();

        for model in catalog.models {
            assert_eq!((model.availability_nux, model.upgrade), (None, None));
        }
    }

    #[test]
    fn gpt_5_bedrock_models_are_visible() {
        let catalog = static_model_catalog();

        for model in catalog.models {
            assert_eq!(model.visibility, ModelVisibility::List);
        }
    }

    #[test]
    fn gpt_5_6_bedrock_models_use_bundled_metadata_with_bedrock_overrides() {
        let catalog = static_model_catalog();

        for (openai_slug, slug, display_name, priority) in [
            (
                GPT_5_6_SOL_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_SOL_MODEL_ID,
                "GPT-5.6 Sol",
                0,
            ),
            (
                GPT_5_6_TERRA_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_TERRA_MODEL_ID,
                "GPT-5.6 Terra",
                1,
            ),
            (
                GPT_5_6_LUNA_OPENAI_MODEL_ID,
                AMAZON_BEDROCK_GPT_5_6_LUNA_MODEL_ID,
                "GPT-5.6 Luna",
                2,
            ),
        ] {
            let mut expected = bundled_openai_model(openai_slug);
            expected.slug = slug.to_string();
            expected.display_name = display_name.to_string();
            expected.priority = priority;
            expected.visibility = ModelVisibility::List;
            expected.availability_nux = None;
            expected.upgrade = None;
            expected.use_responses_lite = false;
            expected.tool_mode = None;
            expected
                .supported_reasoning_levels
                .retain(|level| level.effort != ReasoningEffort::Ultra);
            expected.additional_speed_tiers.clear();
            expected.service_tiers.clear();
            expected.default_service_tier = None;
            expected.web_search_tool_type = WebSearchToolType::Text;

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
