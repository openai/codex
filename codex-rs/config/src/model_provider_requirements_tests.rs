//! Tests exact provider requirements across layers and config projections.

use crate::ConfigLayerEntry;
use crate::ConfigLayerSource;
use crate::ConfigLayerStack;
use crate::ConfigRequirements;
use crate::ConfigRequirementsToml;
use crate::RequirementSource;
use crate::RequirementsLayerEntry;
use crate::compose_requirements;
use pretty_assertions::assert_eq;

const REQUIRED: &str = r#"
model_provider = "gateway"
[model_providers.gateway]
name = "Managed gateway"
base_url = "https://gateway.example.test/v1"
requires_openai_auth = true
[model_providers.gateway.http_headers]
X-Managed = "yes"
"#;

#[test]
fn higher_requirements_merge_provider_fields_and_nested_tables() -> anyhow::Result<()> {
    let cloud_source = RequirementSource::EnterpriseManaged {
        id: "req_1".into(),
        name: "Gateway".into(),
    };
    let requirements = compose_requirements([
        RequirementsLayerEntry::from_toml(
            RequirementSource::Unknown,
            r#"
model_provider = "other"
[model_providers.gateway]
name = "System gateway"
base_url = "https://old.example.test"
env_key = "OLD_KEY"
supports_websockets = true
[model_providers.gateway.http_headers]
X-Old = "no"
X-Managed = "old"
[model_providers.other]
name = "Other provider"
"#,
        ),
        RequirementsLayerEntry::from_toml(cloud_source.clone(), REQUIRED),
    ])?
    .expect("provider requirements must not be discarded");
    let normalized = ConfigRequirements::try_from(requirements.clone())?;
    assert_eq!(
        normalized
            .model_provider
            .as_ref()
            .map(|requirement| &requirement.source),
        Some(&cloud_source)
    );
    let requirements = requirements.into_toml();
    let expected: ConfigRequirementsToml = toml::from_str(
        r#"
model_provider = "gateway"
[model_providers.gateway]
name = "Managed gateway"
base_url = "https://gateway.example.test/v1"
requires_openai_auth = true
env_key = "OLD_KEY"
supports_websockets = true
[model_providers.gateway.http_headers]
X-Managed = "yes"
X-Old = "no"
[model_providers.other]
name = "Other provider"
"#,
    )?;
    assert_eq!(requirements, expected);
    assert_eq!(
        normalized
            .model_providers
            .map(|requirement| requirement.value),
        requirements.model_providers
    );
    Ok(())
}

#[test]
fn required_provider_definitions_are_validated_without_local_defaults() -> anyhow::Result<()> {
    for invalid in [
        "[model_providers.gateway]\nbase_url = 'https://example.test'",
        "[model_providers.openai]\nname = 'Reserved'",
        "[model_providers.gateway]\nname = 'Gateway'\n[model_providers.gateway.aws]\nregion = 'us-east-1'",
    ] {
        let requirements = toml::from_str(invalid)?;
        let local =
            ConfigLayerEntry::new(ConfigLayerSource::SessionFlags, toml::from_str(REQUIRED)?);
        let error = ConfigLayerStack::new(vec![local], ConfigRequirements::default(), requirements)
            .expect_err("local config cannot repair an invalid required provider");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }
    Ok(())
}
