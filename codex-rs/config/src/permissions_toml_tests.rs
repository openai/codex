//! Trusted network extensions survive TOML resolution without ambiguous signing authority.

use super::*;
use codex_network_proxy::ProxySecretSource;
use pretty_assertions::assert_eq;

const SIGNING: &str = r#"
version = 2
[actions.aws.aws_sigv4]
placeholder_access_key_id = "MASKEDACCESSKEY"
region = "us-east-1"
service = "s3"
access_key_id = { env_var = "PROXY_ACCESS_KEY" }
secret_access_key = { file = "/trusted/aws-secret" }
session_token = { env_var = "PROXY_SESSION_TOKEN" }

[[actions.substitute.inject_request_headers]]
name = "x-api-key"
secret_env_var = "PROXY_API_KEY"
placeholder = "dummy-key"

[hooks.bucket]
host = "bucket.s3.amazonaws.com"
schemes = ["https"]
ports = [443]
methods = ["PUT"]
path_prefixes = ["/workspace/"]
action = ["substitute", "aws"]
"#;

#[test]
fn resolves_native_signing_and_substitution_actions_from_toml() {
    let config: NetworkMitmToml = toml::from_str(SIGNING).unwrap();
    let hooks = config.to_runtime_hooks(config.actions.as_ref());
    assert_eq!(hooks[0].matcher.schemes, vec!["https"]);
    assert_eq!(hooks[0].matcher.ports, vec![443]);
    let signer = hooks[0].actions.aws_sigv4.as_ref().unwrap();
    assert_eq!(
        signer.access_key_id,
        ProxySecretSource::EnvVar("PROXY_ACCESS_KEY".into())
    );
    assert_eq!(signer.port, 443);
    assert_eq!(
        hooks[0].actions.inject_request_headers[0]
            .placeholder
            .as_deref(),
        Some("dummy-key")
    );
    let wire = serde_json::to_value(&hooks).unwrap();
    let decoded: Vec<MitmHookConfig> = serde_json::from_value(wire).unwrap();
    assert_eq!(decoded, hooks);
}

#[test]
fn scoped_rules_reject_rollback_to_the_legacy_format() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyMitm {
        #[serde(rename = "hooks")]
        _hooks: toml::Value,
        #[serde(rename = "actions")]
        _actions: toml::Value,
    }

    let unversioned = SIGNING.replacen("version = 2\n", "", 1);
    assert!(toml::from_str::<NetworkMitmToml>(&unversioned).is_err());
    assert!(toml::from_str::<LegacyMitm>(&unversioned).is_ok());
    assert!(toml::from_str::<LegacyMitm>(SIGNING).is_err());
}

#[test]
fn profile_and_managed_requirements_reject_misspelled_mitm_matchers() {
    let profile = SIGNING
        .replace("[actions.", "[permissions.test.network.mitm.actions.")
        .replace("[hooks.", "[permissions.test.network.mitm.hooks.");
    let profile = format!("[permissions.test.network.mitm]\n{profile}");
    toml::from_str::<crate::config_toml::ConfigToml>(&profile).unwrap();
    toml::from_str::<crate::ConfigRequirementsToml>(&profile).unwrap();
    for (key, value) in [
        ("header", r#"{ x-tenant = ["allowed"] }"#),
        ("port", "[443]"),
        ("scheme", r#"["https"]"#),
    ] {
        let input = format!("{profile}\n{key} = {value}");
        for error in [
            toml::from_str::<crate::config_toml::ConfigToml>(&input)
                .unwrap_err()
                .to_string(),
            toml::from_str::<crate::ConfigRequirementsToml>(&input)
                .unwrap_err()
                .to_string(),
        ] {
            assert!(error.contains(&format!("unknown field `{key}`")), "{error}");
        }
    }
}

#[test]
fn rejects_misspelled_header_placeholder() {
    let input = SIGNING.replacen("placeholder =", "unknown_placeholder =", 1);
    let error = toml::from_str::<NetworkMitmToml>(&input).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unknown field `unknown_placeholder`")
    );
}

#[test]
fn rejects_multiple_signing_actions_and_unknown_signing_parameters() {
    assert!(
        toml::from_str::<NetworkMitmToml>(&SIGNING.replace(
            "action = [\"substitute\", \"aws\"]",
            "action = [\"aws\", \"aws\"]"
        ))
        .is_err()
    );
    assert!(
        toml::from_str::<NetworkMitmToml>(&SIGNING.replace(
            "region = \"us-east-1\"",
            "region = \"us-east-1\"\nallow_unsigned_payload = true"
        ))
        .is_err()
    );
}
