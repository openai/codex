use super::*;
use codex_config::CloudRequirementsFragment;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn cloud_fragments_combine_before_provider_validation() {
    let home = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(home.path()).expect("absolute path");
    let mut bundle = CloudConfigBundle::default();
    // Cloud fragments arrive highest-priority first.
    bundle.requirements_toml.enterprise_managed = vec![
        CloudRequirementsFragment {
            id: "high".to_string(),
            name: "URL".to_string(),
            contents: "[model_providers.gateway]\nbase_url = 'https://gateway.example/v1'\n[model_providers.gateway.auth]\ntimeout_ms = 10000\ncwd = 'auth'"
                .to_string(),
        },
        CloudRequirementsFragment {
            id: "low".to_string(),
            name: "Name".to_string(),
            contents: "[model_providers.gateway]\nname = 'Gateway'\n[model_providers.gateway.auth]\ncommand = 'get-token'".to_string(),
        },
    ];
    assert_eq!(validate_bundle(&bundle, &base_dir), Ok(()));
}

#[test]
fn cloud_bedrock_overrides_accept_supported_fields() {
    let home = tempdir().expect("tempdir");
    let base_dir = AbsolutePathBuf::from_absolute_path(home.path()).expect("absolute path");
    for provider in ["amazon-bedrock", "amazon-bedrock-runtime"] {
        let mut bundle = CloudConfigBundle::default();
        bundle.requirements_toml.enterprise_managed = vec![CloudRequirementsFragment {
            id: "bedrock".to_string(),
            name: "Bedrock".to_string(),
            contents: format!(
                r#"
[model_providers.{provider}]
base_url = "https://bedrock.example"
[model_providers.{provider}.http_headers]
X-Managed = "required"
[model_providers.{provider}.aws]
profile = "managed"
region = "us-east-1"
"#
            ),
        }];
        assert_eq!(validate_bundle(&bundle, &base_dir), Ok(()));
    }
}
