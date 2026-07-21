use super::*;
use pretty_assertions::assert_eq;

#[test]
fn shell_environment_policy_accepts_legacy_lists_or_filters() {
    let legacy: ShellEnvironmentPolicyToml = toml::from_str(
        r#"
exclude = ["LEGACY_*", "SHARED_*"]
include_only = ["PATH", "HOME"]
"#,
    )
    .expect("legacy arrays should remain valid in config.toml");
    assert_eq!(
        legacy,
        ShellEnvironmentPolicyToml {
            exclude: Some(vec!["LEGACY_*".to_string(), "SHARED_*".to_string()]),
            include_only: Some(vec!["PATH".to_string(), "HOME".to_string()]),
            ..Default::default()
        }
    );

    let filtered: ShellEnvironmentPolicyToml = toml::from_str(
        r#"
[filters]
"FLIP_TO_EXCLUDE" = "exclude"
"FLIP_TO_INCLUDE" = "include"
"#,
    )
    .expect("filters should be valid in config.toml");
    assert_eq!(
        filtered,
        ShellEnvironmentPolicyToml {
            filters: Some(BTreeMap::from([
                (
                    "FLIP_TO_EXCLUDE".to_string(),
                    ShellEnvironmentPolicyFilter::Exclude,
                ),
                (
                    "FLIP_TO_INCLUDE".to_string(),
                    ShellEnvironmentPolicyFilter::Include,
                ),
            ])),
            ..Default::default()
        }
    );
    assert_eq!(
        ShellEnvironmentPolicy::from(filtered),
        ShellEnvironmentPolicy::from(ShellEnvironmentPolicyToml {
            exclude: Some(vec!["FLIP_TO_EXCLUDE".to_string()]),
            include_only: Some(vec!["FLIP_TO_INCLUDE".to_string()]),
            ..Default::default()
        })
    );
}

#[test]
fn shell_environment_policy_rejects_mixed_legacy_lists_and_filters() {
    let error = toml::from_str::<ShellEnvironmentPolicyToml>(
        r#"
exclude = ["LEGACY_*"]

[filters]
"CANONICAL_*" = "include"
"#,
    )
    .expect_err("one config layer must not mix legacy lists and filters");

    assert!(
        error
            .to_string()
            .contains("cannot mix `filters` with legacy `exclude` or `include_only`")
    );
}

#[test]
fn shell_environment_policy_rejects_case_variant_filters_within_layer() {
    let error = toml::from_str::<ShellEnvironmentPolicyToml>(
        r#"
[filters]
"AWS_*" = "exclude"
"aws_*" = "include"
"#,
    )
    .expect_err("case-variant filters in one layer should be rejected");

    assert!(
        error
            .to_string()
            .contains("duplicate shell environment filter")
    );
}

#[test]
fn shell_environment_policy_rejects_unicode_case_variant_filters_within_layer() {
    let error = toml::from_str::<ShellEnvironmentPolicyToml>(
        r#"
[filters]
"СЕКРЕТ_*" = "exclude"
"секрет_*" = "include"
"#,
    )
    .expect_err("Unicode case-variant filters in one layer should be rejected");

    assert!(
        error
            .to_string()
            .contains("duplicate shell environment filter")
    );
}
