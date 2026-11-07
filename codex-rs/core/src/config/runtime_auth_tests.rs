//! Verifies managed runtime-auth defaults and rejection of conflicting user overrides.

#![cfg(test)]

use super::*;
use codex_config::RequirementSource;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test]
async fn managed_runtime_mode_pins_defaults_and_rejects_overrides() -> anyhow::Result<()> {
    for (name, mode) in [
        ("dual_header", RuntimeAuthMode::DualHeader),
        ("assertion_only", RuntimeAuthMode::AssertionOnly),
    ] {
        let requirement: codex_config::ConfigRequirementsToml = toml::from_str(&format!(
            "[features.runtime_agent_instances]\nenabled = true\nmode = '{name}'"
        ))?;
        for local in ["", "[features]\nruntime_agent_instances = true"] {
            let mut sourced = codex_config::ConfigRequirementsWithSources::default();
            sourced.merge_unset_fields(RequirementSource::Unknown, requirement.clone());
            let stack = codex_config::ConfigLayerStack::new(
                Vec::new(),
                sourced.try_into()?,
                requirement.clone(),
            )?;
            let home = tempdir()?;
            let config = Config::load_config_with_layer_stack(
                LOCAL_FS.as_ref(),
                toml::from_str(local)?,
                ConfigOverrides::default(),
                AbsolutePathBuf::try_from(home.path())?,
                stack,
            )
            .await?;
            assert_eq!(config.runtime_auth_mode, mode);
            assert!(config.features.enabled(Feature::RuntimeAgentInstances));
        }
        let different = if name == "dual_header" {
            "assertion_only"
        } else {
            "dual_header"
        };
        let configured = toml::from_str(&format!(
            "[features.runtime_agent_instances]\nmode = '{different}'"
        ))?;
        let requirements = Sourced::new(
            requirement.feature_requirements.clone().unwrap(),
            RequirementSource::Unknown,
        );
        assert!(
            managed_features::resolve_runtime_auth_mode(&configured, Some(&requirements)).is_err()
        );
        assert!(
            managed_features::validate_explicit_feature_settings_in_config_toml(
                &configured,
                Some(&requirements)
            )
            .is_err()
        );
    }
    Ok(())
}

#[test]
fn unmanaged_runtime_modes_and_legacy_boolean_requirements_work() -> anyhow::Result<()> {
    for (text, expected) in [
        ("", RuntimeAuthMode::DualHeader),
        (
            "[features]\nruntime_agent_instances = true",
            RuntimeAuthMode::DualHeader,
        ),
        (
            "[features.runtime_agent_instances]\nmode = 'assertion_only'",
            RuntimeAuthMode::AssertionOnly,
        ),
    ] {
        assert_eq!(
            managed_features::resolve_runtime_auth_mode(
                &toml::from_str(text)?,
                /*requirements*/ None
            )?,
            expected
        );
    }
    let requirement: codex_config::ConfigRequirementsToml =
        toml::from_str("[features]\nruntime_agent_instances = false")?;
    let requirement = requirement.feature_requirements.unwrap();
    assert_eq!(
        requirement.entries.get("runtime_agent_instances"),
        Some(&false)
    );
    assert_eq!(requirement.runtime_auth_mode, None);
    assert!(
        toml::from_str::<codex_config::ConfigRequirementsToml>(
            "[features.runtime_agent_instances]\nmode = 'unknown'"
        )
        .is_err()
    );
    Ok(())
}
