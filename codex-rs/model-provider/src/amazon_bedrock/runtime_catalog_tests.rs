use codex_protocol::protocol::MultiAgentVersion;
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
            ("global.openai.gpt-6-astra", "GPT-6-Astra (Global)", 1),
            ("global.openai.gpt-5.6-terra", "GPT-5.6 Terra (Global)", 2),
            ("global.openai.gpt-5.6-luna", "GPT-5.6 Luna (Global)", 3),
            ("us.openai.gpt-5.6-sol", "GPT-5.6 Sol (US cross-region)", 4),
            ("us.openai.gpt-6-astra", "GPT-6-Astra (US cross-region)", 5),
            (
                "us.openai.gpt-5.6-terra",
                "GPT-5.6 Terra (US cross-region)",
                6,
            ),
            (
                "us.openai.gpt-5.6-luna",
                "GPT-5.6 Luna (US cross-region)",
                7
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
                model.multi_agent_version,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "global.openai.gpt-5.6-sol",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
            (
                "global.openai.gpt-6-astra",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
            (
                "global.openai.gpt-5.6-terra",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
            (
                "global.openai.gpt-5.6-luna",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
            (
                "us.openai.gpt-5.6-sol",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
            (
                "us.openai.gpt-6-astra",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
            (
                "us.openai.gpt-5.6-terra",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
            (
                "us.openai.gpt-5.6-luna",
                None,
                false,
                Some(MultiAgentVersion::V1),
            ),
        ]
    );
}
