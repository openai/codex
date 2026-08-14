use pretty_assertions::assert_eq;

use super::static_runtime_model_catalog;

#[test]
fn runtime_catalog_includes_supported_cross_region_models_in_priority_order() {
    let catalog = static_runtime_model_catalog();

    assert_eq!(
        catalog
            .models
            .iter()
            .map(|model| (
                model.slug.as_str(),
                model.display_name.as_str(),
                model.priority,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("global.openai.gpt-5.6-sol", "GPT-5.6 Sol (Global)", 0),
            ("global.openai.gpt-5.6-terra", "GPT-5.6 Terra (Global)", 1),
            ("global.openai.gpt-5.6-luna", "GPT-5.6 Luna (Global)", 2),
            ("us.openai.gpt-5.6-sol", "GPT-5.6 Sol (US cross-region)", 3),
            (
                "us.openai.gpt-5.6-terra",
                "GPT-5.6 Terra (US cross-region)",
                4,
            ),
            (
                "us.openai.gpt-5.6-luna",
                "GPT-5.6 Luna (US cross-region)",
                5
            ),
        ]
    );
}

#[test]
fn runtime_catalog_disables_web_search_without_overriding_review_models() {
    let catalog = static_runtime_model_catalog();

    assert_eq!(
        catalog
            .models
            .iter()
            .map(|model| (
                model.slug.as_str(),
                model.auto_review_model_override.as_deref(),
                model.supports_search_tool,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("global.openai.gpt-5.6-sol", None, false),
            ("global.openai.gpt-5.6-terra", None, false),
            ("global.openai.gpt-5.6-luna", None, false),
            ("us.openai.gpt-5.6-sol", None, false),
            ("us.openai.gpt-5.6-terra", None, false),
            ("us.openai.gpt-5.6-luna", None, false),
        ]
    );
}
