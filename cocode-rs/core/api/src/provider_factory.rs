//! Factory for creating hyper-sdk providers from protocol config.
//!
//! This module bridges cocode-protocol's ProviderInfo to hyper-sdk providers.
//!
//! # Example
//!
//! ```ignore
//! use cocode_api::provider_factory::{create_provider, create_model};
//! use cocode_protocol::{ProviderInfo, ProviderType};
//!
//! let info = ProviderInfo::new("OpenAI", ProviderType::Openai, "https://api.openai.com/v1")
//!     .with_api_key("sk-xxx");
//!
//! let provider = create_provider(&info)?;
//! let model = create_model(&info, "gpt-4o")?;
//! ```

use crate::error::Result;
use cocode_protocol::ProviderInfo;
use cocode_protocol::ProviderType;
use hyper_sdk::AnthropicProvider;
use hyper_sdk::GeminiProvider;
use hyper_sdk::Model;
use hyper_sdk::OpenAICompatProvider;
use hyper_sdk::OpenAIProvider;
use hyper_sdk::Provider;
use hyper_sdk::VolcengineProvider;
use hyper_sdk::ZaiProvider;
use std::sync::Arc;

/// Create a provider from ProviderInfo configuration.
///
/// This function creates the appropriate hyper-sdk provider based on the
/// `provider_type` field in the ProviderInfo.
///
/// # Errors
///
/// Returns an error if:
/// - The provider type is not supported
/// - The provider configuration is invalid (e.g., missing API key)
pub fn create_provider(info: &ProviderInfo) -> Result<Arc<dyn Provider>> {
    let provider: Arc<dyn Provider> = match info.provider_type {
        ProviderType::Openai => {
            let mut builder = OpenAIProvider::builder()
                .api_key(&info.api_key)
                .base_url(&info.base_url)
                .timeout_secs(info.timeout_secs);

            // Handle organization ID from provider options
            if let Some(options) = &info.options {
                if let Some(org_id) = options.get("organization_id").and_then(|v| v.as_str()) {
                    builder = builder.organization_id(org_id);
                }
            }

            Arc::new(builder.build().map_err(|e| {
                crate::error::api_error::SdkSnafu {
                    message: e.to_string(),
                }
                .build()
            })?)
        }
        ProviderType::Anthropic => {
            let builder = AnthropicProvider::builder()
                .api_key(&info.api_key)
                .base_url(&info.base_url)
                .timeout_secs(info.timeout_secs);

            Arc::new(builder.build().map_err(|e| {
                crate::error::api_error::SdkSnafu {
                    message: e.to_string(),
                }
                .build()
            })?)
        }
        ProviderType::Gemini => {
            let builder = GeminiProvider::builder()
                .api_key(&info.api_key)
                .base_url(&info.base_url)
                .timeout_secs(info.timeout_secs);

            Arc::new(builder.build().map_err(|e| {
                crate::error::api_error::SdkSnafu {
                    message: e.to_string(),
                }
                .build()
            })?)
        }
        ProviderType::Volcengine => {
            let builder = VolcengineProvider::builder()
                .api_key(&info.api_key)
                .base_url(&info.base_url)
                .timeout_secs(info.timeout_secs);

            Arc::new(builder.build().map_err(|e| {
                crate::error::api_error::SdkSnafu {
                    message: e.to_string(),
                }
                .build()
            })?)
        }
        ProviderType::Zai => {
            let mut builder = ZaiProvider::builder()
                .api_key(&info.api_key)
                .base_url(&info.base_url)
                .timeout_secs(info.timeout_secs);

            // Handle use_zhipuai from provider options
            if let Some(options) = &info.options {
                if let Some(use_zhipuai) = options.get("use_zhipuai").and_then(|v| v.as_bool()) {
                    builder = builder.use_zhipuai(use_zhipuai);
                }
            }

            Arc::new(builder.build().map_err(|e| {
                crate::error::api_error::SdkSnafu {
                    message: e.to_string(),
                }
                .build()
            })?)
        }
        ProviderType::OpenaiCompat => {
            let builder = OpenAICompatProvider::builder(&info.name)
                .api_key(&info.api_key)
                .base_url(&info.base_url)
                .timeout_secs(info.timeout_secs);

            Arc::new(builder.build().map_err(|e| {
                crate::error::api_error::SdkSnafu {
                    message: e.to_string(),
                }
                .build()
            })?)
        }
    };
    Ok(provider)
}

/// Create a model from ProviderInfo for a specific model slug.
///
/// This function creates a provider and retrieves a model instance from it.
/// It handles model aliases (e.g., endpoint IDs for Volcengine) by looking
/// up the API model name from the ProviderInfo.
///
/// # Arguments
///
/// * `info` - The provider configuration
/// * `model_slug` - The model identifier (e.g., "gpt-4o", "claude-sonnet-4")
///
/// # Errors
///
/// Returns an error if:
/// - Provider creation fails
/// - The model is not found or not supported by the provider
pub fn create_model(info: &ProviderInfo, model_slug: &str) -> Result<Arc<dyn Model>> {
    let provider = create_provider(info)?;

    // Get the API model name (handles aliases like endpoint IDs for Volcengine)
    let api_name = info.api_model_name(model_slug).unwrap_or(model_slug);

    provider.model(api_name).map_err(|e| {
        crate::error::api_error::SdkSnafu {
            message: e.to_string(),
        }
        .build()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cocode_protocol::ModelInfo;

    fn make_provider_info(provider_type: ProviderType, base_url: &str) -> ProviderInfo {
        ProviderInfo::new("Test", provider_type, base_url).with_api_key("test-api-key")
    }

    #[test]
    fn test_create_openai_provider() {
        let info = make_provider_info(ProviderType::Openai, "https://api.openai.com/v1");
        let result = create_provider(&info);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "openai");
    }

    #[test]
    fn test_create_anthropic_provider() {
        let info = make_provider_info(ProviderType::Anthropic, "https://api.anthropic.com");
        let result = create_provider(&info);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "anthropic");
    }

    #[test]
    fn test_create_gemini_provider() {
        let info = make_provider_info(
            ProviderType::Gemini,
            "https://generativelanguage.googleapis.com",
        );
        let result = create_provider(&info);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "gemini");
    }

    #[test]
    fn test_create_volcengine_provider() {
        let info = make_provider_info(
            ProviderType::Volcengine,
            "https://ark.cn-beijing.volces.com/api/v3",
        );
        let result = create_provider(&info);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "volcengine");
    }

    #[test]
    fn test_create_zai_provider() {
        let info = make_provider_info(ProviderType::Zai, "https://api.z.ai/api/paas/v4");
        let result = create_provider(&info);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "zhipuai");
    }

    #[test]
    fn test_create_openai_compat_provider() {
        let info = make_provider_info(ProviderType::OpenaiCompat, "https://custom.api.com/v1");
        let result = create_provider(&info);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name(), "Test");
    }

    #[test]
    fn test_create_model_with_slug() {
        let info = make_provider_info(ProviderType::Openai, "https://api.openai.com/v1");
        let result = create_model(&info, "gpt-4o");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().model_name(), "gpt-4o");
    }

    #[test]
    fn test_create_model_with_alias() {
        let model_info = ModelInfo {
            slug: "deepseek-r1".to_string(),
            ..Default::default()
        };

        let info = make_provider_info(
            ProviderType::Volcengine,
            "https://ark.cn-beijing.volces.com/api/v3",
        )
        .with_model_aliased("deepseek-r1", model_info, "ep-20250101-xxxxx");

        let result = create_model(&info, "deepseek-r1");
        assert!(result.is_ok());
        // The model name should be the alias (endpoint ID)
        assert_eq!(result.unwrap().model_name(), "ep-20250101-xxxxx");
    }

    #[test]
    fn test_missing_api_key() {
        let info = ProviderInfo::new("Test", ProviderType::Openai, "https://api.openai.com/v1");
        // API key is empty
        let result = create_provider(&info);
        assert!(result.is_err());
    }
}
