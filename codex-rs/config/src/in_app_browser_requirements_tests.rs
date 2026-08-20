use super::InAppBrowserRequirementsToml;
use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;
use crate::RequirementSource;
use crate::RequirementsLayerEntry;
use crate::Sourced;
use crate::compose_requirements_for_hostname;
use pretty_assertions::assert_eq;

const EMPTY: &str = "[in_app_browser]";
const ALLOW: &str = "[in_app_browser]\nallow_external_browser_settings_import = true";
const DENY: &str = "[in_app_browser]\nallow_external_browser_settings_import = false";

fn parse(contents: &str) -> ConfigRequirementsToml {
    toml::from_str(contents).expect("parse requirements")
}

fn source(name: &str) -> RequirementSource {
    RequirementSource::EnterpriseManaged {
        id: name.to_string(),
        name: name.to_string(),
    }
}

#[test]
fn import_requirement_preserves_unset_and_explicit_values() {
    for (contents, expected_table) in [
        ("", None),
        (EMPTY, Some(None)),
        (ALLOW, Some(Some(true))),
        (DENY, Some(Some(false))),
    ] {
        let requirements = parse(contents);
        assert_eq!(
            requirements,
            ConfigRequirementsToml {
                in_app_browser: expected_table.map(|value| InAppBrowserRequirementsToml {
                    allow_external_browser_settings_import: value,
                }),
                ..Default::default()
            },
        );
        let configured = requirements
            .in_app_browser
            .as_ref()
            .and_then(|policy| policy.allow_external_browser_settings_import);
        assert_eq!(requirements.is_empty(), configured.is_none());
    }
}

#[test]
fn import_requirement_uses_normal_managed_toml_precedence() {
    for (low, high, expected) in [
        ("", EMPTY, None),
        (DENY, "", Some(DENY)),
        (DENY, EMPTY, Some(DENY)),
        (DENY, ALLOW, Some(ALLOW)),
        (ALLOW, DENY, Some(DENY)),
    ] {
        let composed = compose_requirements_for_hostname(
            [
                RequirementsLayerEntry::from_toml(source("low"), low),
                RequirementsLayerEntry::from_toml(source("high"), high),
            ],
            /*hostname*/ None,
        )
        .expect("compose managed import policy")
        .map(ConfigRequirementsWithSources::into_toml);
        assert_eq!(composed, expected.map(parse));
    }

    let composed = compose_requirements_for_hostname(
        [RequirementsLayerEntry::from_toml(source("managed"), DENY)],
        /*hostname*/ None,
    )
    .expect("compose managed denial");
    assert_eq!(
        composed,
        Some(ConfigRequirementsWithSources {
            in_app_browser: Some(Sourced::new(
                InAppBrowserRequirementsToml {
                    allow_external_browser_settings_import: Some(false),
                },
                source("managed"),
            )),
            ..Default::default()
        }),
    );
}

#[test]
fn legacy_merge_uses_whole_table_precedence() {
    for (high, low, expected, expected_source) in [
        (EMPTY, DENY, EMPTY, "high"),
        ("", ALLOW, ALLOW, "low"),
        (DENY, ALLOW, DENY, "high"),
        (ALLOW, DENY, ALLOW, "high"),
    ] {
        let mut composed = ConfigRequirementsWithSources::default();
        composed.merge_unset_fields(source("high"), parse(high));
        composed.merge_unset_fields(source("low"), parse(low));
        assert_eq!(
            composed,
            ConfigRequirementsWithSources {
                in_app_browser: Some(Sourced::new(
                    parse(expected).in_app_browser.expect("requirements table"),
                    source(expected_source),
                )),
                ..Default::default()
            },
        );
        assert_eq!(composed.into_toml(), parse(expected));
    }
}

#[test]
fn invalid_import_requirement_is_rejected_even_when_overridden() {
    for invalid in ["\"false\"", "1", "[]", "{}"] {
        let contents =
            format!("[in_app_browser]\nallow_external_browser_settings_import = {invalid}");
        let error = toml::from_str::<ConfigRequirementsToml>(&contents)
            .expect_err("import requirement must be a boolean");
        assert!(
            error
                .to_string()
                .contains("allow_external_browser_settings_import"),
            "{error}",
        );
        let error = compose_requirements_for_hostname(
            [
                RequirementsLayerEntry::from_toml(source("invalid"), contents),
                RequirementsLayerEntry::from_toml(source("higher"), ALLOW),
            ],
            /*hostname*/ None,
        )
        .expect_err("invalid managed layers must not be silently ignored");
        assert!(
            error
                .to_string()
                .contains("allow_external_browser_settings_import"),
            "{error}",
        );
    }
}
