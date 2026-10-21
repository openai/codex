//! Trusted network extensions survive TOML resolution with origin-scoped header placeholders.

use super::*;
use pretty_assertions::assert_eq;

const HEADER_ACTION: &str = r#"
version = 2
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
action = ["substitute"]
"#;

#[test]
fn resolves_origin_scoped_header_substitution_from_toml() {
    let config: NetworkMitmToml = toml::from_str(HEADER_ACTION).unwrap();
    let hooks = config.to_runtime_hooks(config.actions.as_ref());
    assert_eq!(hooks[0].matcher.schemes, vec!["https"]);
    assert_eq!(hooks[0].matcher.ports, vec![443]);
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

    let unversioned = HEADER_ACTION.replacen("version = 2\n", "", 1);
    assert!(toml::from_str::<NetworkMitmToml>(&unversioned).is_err());
    assert!(toml::from_str::<LegacyMitm>(&unversioned).is_ok());
    assert!(toml::from_str::<LegacyMitm>(HEADER_ACTION).is_err());
}

#[test]
fn profile_and_managed_requirements_reject_misspelled_mitm_matchers() {
    let profile = HEADER_ACTION
        .replace("[[actions.", "[[permissions.test.network.mitm.actions.")
        .replace("[hooks.", "[permissions.test.network.mitm.hooks.");
    let profile = format!("[permissions.test.network.mitm]\n{profile}");
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
    let input = HEADER_ACTION.replacen("placeholder =", "unknown_placeholder =", 1);
    let error = toml::from_str::<NetworkMitmToml>(&input).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unknown field `unknown_placeholder`")
    );
}
