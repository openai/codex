use codex_features::FEATURES;
use codex_features::Feature;
use std::collections::BTreeMap;
use std::path::Path;

/// Composes the standard mock Responses provider with test-specific configuration.
pub struct MockResponsesConfig {
    provider_id: String,
    provider_name: String,
    provider_base_url: String,
    model: String,
    approval_policy: String,
    sandbox_mode: String,
    features: BTreeMap<Feature, bool>,
    root_config: Vec<String>,
    provider_config: Vec<String>,
    extra_config: Vec<String>,
}

impl MockResponsesConfig {
    pub fn new(server_uri: &str) -> Self {
        Self {
            provider_id: "mock_provider".to_string(),
            provider_name: "Mock provider for test".to_string(),
            provider_base_url: format!("{server_uri}/v1"),
            model: "mock-model".to_string(),
            approval_policy: "never".to_string(),
            sandbox_mode: "read-only".to_string(),
            features: BTreeMap::new(),
            root_config: Vec::new(),
            provider_config: Vec::new(),
            extra_config: Vec::new(),
        }
    }

    pub fn with_model_provider(mut self, provider_id: &str) -> Self {
        self.provider_id = provider_id.to_string();
        self
    }

    pub fn with_provider_name(mut self, provider_name: &str) -> Self {
        self.provider_name = provider_name.to_string();
        self
    }

    pub fn with_provider_base_url(mut self, provider_base_url: &str) -> Self {
        self.provider_base_url = provider_base_url.to_string();
        self
    }

    pub fn with_model(mut self, model: &str) -> Self {
        self.model = model.to_string();
        self
    }

    pub fn with_approval_policy(mut self, approval_policy: &str) -> Self {
        self.approval_policy = approval_policy.to_string();
        self
    }

    pub fn with_sandbox_mode(mut self, sandbox_mode: &str) -> Self {
        self.sandbox_mode = sandbox_mode.to_string();
        self
    }

    pub fn enable_feature(mut self, feature: Feature) -> Self {
        self.features.insert(feature, true);
        self
    }

    pub fn disable_feature(mut self, feature: Feature) -> Self {
        self.features.insert(feature, false);
        self
    }

    pub fn with_features(mut self, features: &BTreeMap<Feature, bool>) -> Self {
        self.features.extend(
            features
                .iter()
                .map(|(&feature, &enabled)| (feature, enabled)),
        );
        self
    }

    pub fn with_root_config(mut self, config: &str) -> Self {
        self.root_config.push(config.to_string());
        self
    }

    pub fn with_provider_config(mut self, config: &str) -> Self {
        self.provider_config.push(config.to_string());
        self
    }

    pub fn with_extra_config(mut self, config: &str) -> Self {
        self.extra_config.push(config.to_string());
        self
    }

    pub fn write(self, codex_home: &Path) -> std::io::Result<()> {
        let Self {
            provider_id,
            provider_name,
            provider_base_url,
            model,
            approval_policy,
            sandbox_mode,
            features,
            root_config,
            provider_config,
            extra_config,
        } = self;
        let root_config = root_config.join("\n");
        let provider_config = provider_config.join("\n");
        let extra_config = extra_config.join("\n");
        let feature_entries = features
            .into_iter()
            .map(|(feature, enabled)| {
                let key = FEATURES
                    .iter()
                    .find(|spec| spec.id == feature)
                    .map(|spec| spec.key)
                    .expect("feature should have a config key");
                format!("{key} = {enabled}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        let feature_config = if feature_entries.is_empty() {
            String::new()
        } else {
            format!("[features]\n{feature_entries}\n\n")
        };

        std::fs::write(
            codex_home.join("config.toml"),
            format!(
                r#"
model = "{model}"
approval_policy = "{approval_policy}"
sandbox_mode = "{sandbox_mode}"
{root_config}
model_provider = "{provider_id}"

{feature_config}[model_providers.{provider_id}]
name = "{provider_name}"
base_url = "{provider_base_url}"
wire_api = "responses"
request_max_retries = 0
stream_max_retries = 0
{provider_config}

{extra_config}
"#
            ),
        )
    }
}

pub fn write_mock_responses_config_toml(
    codex_home: &Path,
    server_uri: &str,
    feature_flags: &BTreeMap<Feature, bool>,
    auto_compact_limit: i64,
    requires_openai_auth: Option<bool>,
    model_provider_id: &str,
    compact_prompt: &str,
) -> std::io::Result<()> {
    let mut config = MockResponsesConfig::new(server_uri)
        .with_model_provider(model_provider_id)
        .with_features(feature_flags)
        .with_root_config(&format!(
            "compact_prompt = \"{compact_prompt}\"\nmodel_auto_compact_token_limit = {auto_compact_limit}"
        ))
        .with_provider_config("supports_websockets = false");

    if model_provider_id == "openai" {
        config = config.with_root_config(&format!("openai_base_url = \"{server_uri}/v1\""));
    }
    if matches!(requires_openai_auth, Some(true)) {
        config = config
            .with_provider_name("OpenAI")
            .with_provider_config("requires_openai_auth = true");
    }

    config.write(codex_home)
}

pub fn write_mock_responses_config_toml_with_chatgpt_base_url(
    codex_home: &Path,
    server_uri: &str,
    chatgpt_base_url: &str,
) -> std::io::Result<()> {
    MockResponsesConfig::new(server_uri)
        .with_root_config(&format!("chatgpt_base_url = \"{chatgpt_base_url}\""))
        .write(codex_home)
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
