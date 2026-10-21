//! Application requirements use managed precedence and deny unlisted destinations.

use super::*;
use crate::ConfigRequirements;
use crate::ConfigRequirementsToml;
use crate::ConfigRequirementsWithSources;
use crate::RequirementSource;
use crate::RequirementsLayerEntry;
use crate::Sourced;
use crate::compose_requirements_for_hostname;
use pretty_assertions::assert_eq;

const INSTALLATION: &str = r#"
[application.network.domains]
"shared.example.com" = "allow"
"installed.example.com" = "allow"
"blocked.example.com" = "deny"
"#;
const WORKSPACE: &str = r#"
[application.network.domains]
"shared.example.com" = "allow"
"workspace.example.com" = "allow"
"blocked.example.com" = "allow"
"#;
const MERGED: &str = r#"
[application.network.domains]
"shared.example.com" = "allow"
"installed.example.com" = "allow"
"workspace.example.com" = "allow"
"blocked.example.com" = "allow"
"#;

fn parse(contents: &str) -> ConfigRequirementsToml {
    toml::from_str(contents).expect("valid application requirements")
}

fn cloud_source() -> RequirementSource {
    RequirementSource::EnterpriseManaged {
        id: "workspace".to_string(),
        name: "Workspace policy".to_string(),
    }
}

fn managed_source() -> RequirementSource {
    RequirementSource::MdmManagedPreferences {
        domain: "com.openai.codex".to_string(),
        key: "requirements".to_string(),
    }
}

#[test]
fn application_network_defaults_to_enabled_and_denies_unlisted_domains() {
    assert!(parse("[application]").is_empty());
    let requirements = parse("[application.network]");
    assert!(!requirements.is_empty());
    assert_eq!(
        requirements.application,
        Some(ApplicationRequirementsToml {
            storage: None,
            network: Some(ApplicationNetworkRequirementsToml {
                enabled: true,
                domains: BTreeMap::new(),
            }),
        }),
    );
    assert_eq!(
        parse("[application.network.domains]\n\"EXAMPLE.com.\" = \"deny\""),
        parse("[application.network.domains]\n\"example.com\" = \"deny\""),
    );
}

#[test]
fn application_network_is_preserved_from_each_managed_source() {
    for source in [
        RequirementSource::SystemRequirementsToml {
            file: crate::AbsolutePathBuf::try_from(std::env::temp_dir().join("requirements.toml"))
                .expect("absolute requirements path"),
        },
        managed_source(),
        cloud_source(),
    ] {
        let requirements = compose_requirements_for_hostname(
            [RequirementsLayerEntry::from_toml(
                source.clone(),
                INSTALLATION,
            )],
            /*hostname*/ None,
        )
        .expect("compose application requirements")
        .expect("application requirements are not empty");
        assert_eq!(
            requirements,
            ConfigRequirementsWithSources {
                application: Some(Sourced::new(
                    parse(INSTALLATION).application.expect("application"),
                    source,
                )),
                ..Default::default()
            },
        );
        let normalized: ConfigRequirements =
            requirements.clone().try_into().expect("runtime policy");
        assert_eq!(normalized.application, requirements.application);
    }
}

#[test]
fn application_network_uses_regular_toml_precedence() {
    let installed_wins = MERGED.replace(
        "\"blocked.example.com\" = \"allow\"",
        "\"blocked.example.com\" = \"deny\"",
    );
    let disabled_installation = format!("[application.network]\nenabled = false\n{INSTALLATION}");
    for (low, high, expected) in [
        (INSTALLATION, WORKSPACE, MERGED),
        (WORKSPACE, INSTALLATION, installed_wins.as_str()),
        (
            INSTALLATION,
            "[application.network]\nenabled = false",
            disabled_installation.as_str(),
        ),
        (
            disabled_installation.as_str(),
            "[application.network]\nenabled = true",
            INSTALLATION,
        ),
        (
            "[application.network]\nenabled = false",
            INSTALLATION,
            disabled_installation.as_str(),
        ),
        (INSTALLATION, "[application.network]", INSTALLATION),
        (INSTALLATION, "", INSTALLATION),
        ("[application]", INSTALLATION, INSTALLATION),
        ("[application.network]", WORKSPACE, WORKSPACE),
        (
            "[application.network.domains]\n'example.com' = 'allow'",
            "[application.network.domains]\n'EXAMPLE.COM.' = 'deny'",
            "[application.network.domains]\n'example.com' = 'deny'",
        ),
        (
            "[application.network.domains]\n'EXAMPLE.COM.' = 'deny'",
            "[application.network.domains]\n'example.com' = 'allow'",
            "[application.network.domains]\n'example.com' = 'allow'",
        ),
    ] {
        let composed = compose_requirements_for_hostname(
            [
                RequirementsLayerEntry::from_toml(managed_source(), low),
                RequirementsLayerEntry::from_toml(cloud_source(), high),
            ],
            /*hostname*/ None,
        )
        .expect("compose application requirements")
        .expect("configured requirements");
        assert_eq!(
            composed.into_toml(),
            parse(expected),
            "low: {low}, high: {high}"
        );
    }
}

#[test]
fn legacy_application_merge_uses_whole_table_precedence() {
    for (high, low) in [
        (INSTALLATION, WORKSPACE),
        (WORKSPACE, INSTALLATION),
        ("[application.network]\nenabled = false", INSTALLATION),
        ("[application]", INSTALLATION),
    ] {
        let mut composed = ConfigRequirementsWithSources::default();
        composed.merge_unset_fields(managed_source(), parse(high));
        composed.merge_unset_fields(cloud_source(), parse(low));
        assert_eq!(
            composed,
            ConfigRequirementsWithSources {
                application: Some(Sourced::new(
                    parse(high).application.expect("application"),
                    managed_source(),
                )),
                ..Default::default()
            },
        );
    }
    let mut composed = ConfigRequirementsWithSources::default();
    composed.merge_unset_fields(managed_source(), parse(""));
    composed.merge_unset_fields(cloud_source(), parse(INSTALLATION));
    assert_eq!(composed.into_toml(), parse(INSTALLATION));
}

#[test]
fn invalid_application_policy_is_rejected_before_layer_overrides() {
    for invalid in [
        "[application.network]\nenabled = 'true'",
        "[application.network]\nallowed_domains = ['example.com']",
        "[application.network.domains]\n'example.com' = 'prompt'",
        "[application.network.domains]\n'*.example.com' = 'allow'",
        "[application.network.domains]\n'https://example.com' = 'allow'",
        "[application.network.domains]\n'example.com:443' = 'allow'",
        "[application.network.domains]\n'example..com' = 'allow'",
        "[application.network.domains]\n' example.com' = 'allow'",
        "[application.network.domains]\n'EXAMPLE.com' = 'allow'\n'example.com.' = 'deny'",
    ] {
        assert!(
            toml::from_str::<ConfigRequirementsToml>(invalid).is_err(),
            "{invalid}"
        );
        assert!(
            compose_requirements_for_hostname(
                [
                    RequirementsLayerEntry::from_toml(cloud_source(), invalid),
                    RequirementsLayerEntry::from_toml(managed_source(), INSTALLATION),
                ],
                /*hostname*/ None,
            )
            .is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn cloud_bundle_application_network_uses_managed_precedence() {
    let base_dir =
        crate::AbsolutePathBuf::try_from(std::env::temp_dir()).expect("absolute base directory");
    let bundle = crate::CloudConfigBundle {
        requirements_toml: crate::CloudRequirementsTomlBundle {
            enterprise_managed: vec![crate::CloudRequirementsFragment {
                id: "workspace".to_string(),
                name: "Workspace".to_string(),
                contents: WORKSPACE.to_string(),
            }],
        },
        ..Default::default()
    };
    let mut layers = crate::CloudConfigBundleLayers::from_bundle(bundle, &base_dir)
        .expect("load cloud bundle")
        .enterprise_managed_requirements;
    layers.push(RequirementsLayerEntry::from_toml(
        managed_source(),
        INSTALLATION,
    ));
    let composed = compose_requirements_for_hostname(layers, /*hostname*/ None)
        .expect("compose cloud and installation requirements")
        .expect("requirements");
    let expected = MERGED.replace(
        "\"blocked.example.com\" = \"allow\"",
        "\"blocked.example.com\" = \"deny\"",
    );
    assert_eq!(composed.into_toml(), parse(&expected));
}
