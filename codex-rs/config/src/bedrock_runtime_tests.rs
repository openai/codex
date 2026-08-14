use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use pretty_assertions::assert_eq;

use super::ConfigToml;

#[test]
fn runtime_provider_accepts_aws_profile_and_region_overrides() {
    let config = toml::from_str::<ConfigToml>(
        r#"
[model_providers.amazon-bedrock-runtime.aws]
profile = "runtime-profile"
region = "us-west-2"
"#,
    )
    .expect("Bedrock Runtime AWS overrides should deserialize");

    assert_eq!(
        config
            .model_providers
            .get(AMAZON_BEDROCK_RUNTIME_PROVIDER_ID)
            .and_then(|provider| provider.aws.clone()),
        Some(ModelProviderAwsAuthInfo {
            profile: Some("runtime-profile".to_string()),
            region: Some("us-west-2".to_string()),
        })
    );
}

#[test]
fn custom_provider_still_rejects_aws_auth() {
    let error = toml::from_str::<ConfigToml>(
        r#"
[model_providers.custom]
name = "Custom"

[model_providers.custom.aws]
region = "us-west-2"
"#,
    )
    .expect_err("custom providers must not accept AWS auth");

    assert!(error.to_string().contains(
        "provider aws is only supported for `amazon-bedrock` or `amazon-bedrock-runtime`"
    ));
}
