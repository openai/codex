use codex_aws_auth::AwsAuthConfig;
use codex_model_provider_info::ModelProviderAwsAuthInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;

use super::auth::BedrockAuthMethod;
use super::auth::resolve_auth_method;

const BEDROCK_MANTLE_SERVICE_NAME: &str = "bedrock-mantle";
const BEDROCK_MANTLE_SUPPORTED_REGIONS: [&str; 12] = [
    "us-east-2",
    "us-east-1",
    "us-west-2",
    "ap-southeast-3",
    "ap-south-1",
    "ap-northeast-1",
    "eu-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-south-1",
    "eu-north-1",
    "sa-east-1",
];

pub(super) fn aws_auth_config(aws: &ModelProviderAwsAuthInfo) -> AwsAuthConfig {
    AwsAuthConfig {
        profile: aws.profile.clone(),
        region: region_from_config(aws),
        service: BEDROCK_MANTLE_SERVICE_NAME.to_string(),
    }
}

pub(super) fn region_from_config(aws: &ModelProviderAwsAuthInfo) -> Option<String> {
    aws.region
        .as_deref()
        .map(str::trim)
        .filter(|region| !region.is_empty())
        .map(str::to_string)
}

pub(super) fn base_url(region: &str) -> Result<String> {
    if BEDROCK_MANTLE_SUPPORTED_REGIONS.contains(&region) {
        Ok(format!("https://bedrock-mantle.{region}.api.aws/openai/v1"))
    } else {
        Err(CodexErr::Fatal(format!(
            "Amazon Bedrock Mantle does not support region `{region}`"
        )))
    }
}

pub(super) async fn runtime_base_url(aws: &ModelProviderAwsAuthInfo) -> Result<String> {
    let region = resolve_region(aws).await?;
    base_url(&region)
}

async fn resolve_region(aws: &ModelProviderAwsAuthInfo) -> Result<String> {
    match resolve_auth_method(aws).await? {
        BedrockAuthMethod::EnvBearerToken { region, .. } => Ok(region),
        BedrockAuthMethod::AwsSdkAuth { context } => Ok(context.region().to_string()),
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn base_url_uses_region_endpoint() {
        assert_eq!(
            base_url("ap-northeast-1").expect("supported region"),
            "https://bedrock-mantle.ap-northeast-1.api.aws/openai/v1"
        );
    }

    #[test]
    fn base_url_rejects_unsupported_region() {
        let err = base_url("us-west-1").expect_err("unsupported region");

        assert_eq!(
            err.to_string(),
            "Fatal error: Amazon Bedrock Mantle does not support region `us-west-1`"
        );
    }

    #[test]
    fn aws_auth_config_uses_profile_and_mantle_service() {
        assert_eq!(
            aws_auth_config(&ModelProviderAwsAuthInfo {
                profile: Some("codex-bedrock".to_string()),
                region: None,
            }),
            AwsAuthConfig {
                profile: Some("codex-bedrock".to_string()),
                region: None,
                service: "bedrock-mantle".to_string(),
            }
        );
    }

    #[test]
    fn aws_auth_config_uses_configured_region() {
        assert_eq!(
            aws_auth_config(&ModelProviderAwsAuthInfo {
                profile: None,
                region: Some(" us-west-2 ".to_string()),
            }),
            AwsAuthConfig {
                profile: None,
                region: Some("us-west-2".to_string()),
                service: "bedrock-mantle".to_string(),
            }
        );
    }
}
