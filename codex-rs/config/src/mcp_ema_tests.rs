//! Enterprise registration provenance checks.

use super::*;
use crate::AbsolutePathBuf;
use crate::ConfigLayerEntry;
use crate::ConfigRequirements;
use crate::ConfigRequirementsToml;
use pretty_assertions::assert_eq;

fn stack(layers: Vec<(ConfigLayerSource, &str)>) -> ConfigLayerStack {
    ConfigLayerStack::new(
        layers
            .into_iter()
            .map(|(source, value)| ConfigLayerEntry::new(source, toml::from_str(value).unwrap()))
            .collect(),
        ConfigRequirements::default(),
        ConfigRequirementsToml::default(),
    )
    .unwrap()
}

fn local_sources(name: &str) -> (ConfigLayerSource, ConfigLayerSource, ConfigLayerSource) {
    let path = AbsolutePathBuf::from_absolute_path(std::env::temp_dir().join(name)).unwrap();
    (
        ConfigLayerSource::System { file: path.clone() },
        ConfigLayerSource::User {
            file: path.clone(),
            profile: None,
        },
        ConfigLayerSource::Project {
            dot_codex_folder: path,
        },
    )
}

#[test]
fn ema_profiles_and_auth_modes_preserve_non_project_authority() {
    let (system, user, project) = local_sources("ema-config");
    let trusted = "[mcp_enterprise_managed_auth.idp]\nissuer = 'https://idp.example'\nclient_id = 'enterprise'";
    let replacement =
        "[mcp_enterprise_managed_auth.idp]\nissuer = 'https://other.example'\nclient_id = 'other'";
    let expected = McpEnterpriseManagedAuthConfig {
        idp: McpServerIdpOAuthConfig {
            issuer: "https://idp.example".into(),
            client_id: "enterprise".into(),
        },
    };
    for (layers, selected) in [
        (
            vec![
                (system.clone(), trusted),
                (user.clone(), replacement),
                (project.clone(), replacement),
            ],
            Some(expected.clone()),
        ),
        (
            vec![(user.clone(), trusted), (project.clone(), replacement)],
            Some(expected.clone()),
        ),
        (vec![(project, trusted)], None),
        (
            vec![
                (system, trusted),
                (
                    user,
                    "[mcp_enterprise_managed_auth.idp]\nclient_id = 'partial'",
                ),
            ],
            Some(expected),
        ),
    ] {
        assert_eq!(
            McpEnterpriseManagedAuthConfig::from_config_layers(
                &stack(layers),
                /*fallback*/ None
            )
            .unwrap(),
            selected
        );
    }
    let incomplete = stack(vec![(
        ConfigLayerSource::SessionFlags,
        "[mcp_enterprise_managed_auth.idp]\nclient_id = 'partial'",
    )]);
    assert!(
        McpEnterpriseManagedAuthConfig::from_config_layers(&incomplete, /*fallback*/ None).is_err()
    );

    let server: McpServerConfig = toml::from_str("url='https://resource.example'\nauth='ema_auth'\n[oauth.idp]\nissuer='https://other.example'\nclient_id='other'").unwrap();
    assert_eq!(server.oauth_idp(), None);
}

#[test]
fn ema_rejects_alternate_credentials_and_executor_custody() {
    for extra in [
        "bearer_token_env_var='TOKEN'",
        "http_headers.Authorization='secret'",
        "http_headers.Accept='application/json'",
        "env_http_headers.X-Key='TOKEN'",
        "http_headers_helper='get-headers'",
        "environment_id='remote'",
    ] {
        let server: McpServerConfig = toml::from_str(&format!(
            "url='https://resource.example'\nauth='ema_auth'\n{extra}"
        ))
        .unwrap();
        assert!(server.validate_ema_auth_transport().is_err(), "{extra}");
    }
    let server: McpServerConfig =
        toml::from_str("url='https://resource.example'\nauth='ema_auth'\nhttp_headers={}").unwrap();
    assert_eq!(server.validate_ema_auth_transport(), Ok(()));
}
