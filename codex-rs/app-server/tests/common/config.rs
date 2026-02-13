use codex_core::features::FEATURES;
use codex_core::features::Feature;
use std::collections::BTreeMap;
use std::path::Path;

pub fn write_mock_responses_config_toml(
    codex_home: &Path,
    server_uri: &str,
    feature_flags: &BTreeMap<Feature, bool>,
    auto_compact_limit: i64,
    requires_openai_auth: Option<bool>,
    model_provider_id: &str,
    compact_prompt: &str,
) -> std::io::Result<()> {
    // Phase 1: build the features block for config.toml.
    let mut features = BTreeMap::new();
    for (feature, enabled) in feature_flags {
        features.insert(*feature, *enabled);
    }
    let feature_entries = features
        .into_iter()
        .map(|(feature, enabled)| {
            let key = FEATURES
                .iter()
                .find(|spec| spec.id == feature)
                .map(|spec| spec.key)
                .unwrap_or_else(|| panic!("missing feature key for {feature:?}"));
            format!("{key} = {enabled}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    // Phase 2: build provider-specific config bits.
    let requires_line = match requires_openai_auth {
        Some(true) => "requires_openai_auth = true\n".to_string(),
        Some(false) | None => String::new(),
    };
    let provider_block = if model_provider_id == "openai" {
        String::new()
    } else {
        format!(
            r#"
[model_providers.mock_provider]
name = "Mock provider for test"
base_url = "{server_uri}/v1"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
{requires_line}
"#
        )
    };
    // Phase 3: write the final config file.
    let config_toml = codex_home.join("config.toml");
    std::fs::write(
        config_toml,
        format!(
            r#"
model = "mock-model"
approval_policy = "never"
sandbox_mode = "read-only"
compact_prompt = "{compact_prompt}"
model_auto_compact_token_limit = {auto_compact_limit}

model_provider = "{model_provider_id}"

[features]
{feature_entries}
{provider_block}
"#
        ),
    )
}
