use codex_aws_auth::AwsAuthConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use pretty_assertions::assert_eq;

use super::aws_auth_config;
use super::base_url;

#[test]
fn base_url_uses_regional_runtime_endpoint() {
    assert_eq!(
        base_url("us-west-2"),
        "https://bedrock-runtime.us-west-2.amazonaws.com/openai/v1"
    );
}

#[test]
fn aws_auth_config_uses_bedrock_service() {
    assert_eq!(
        aws_auth_config(&ModelProviderAwsAuthInfo {
            profile: Some("runtime-profile".to_string()),
            region: Some(" eu-west-1 ".to_string()),
            auth_refresh: None,
        }),
        AwsAuthConfig {
            profile: Some("runtime-profile".to_string()),
            region: Some("eu-west-1".to_string()),
            service: "bedrock".to_string(),
        }
    );
}
