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

fn validate_effective_ema(stack: &ConfigLayerStack) -> std::io::Result<()> {
    let servers = stack
        .effective_config()
        .get("mcp_servers")
        .cloned()
        .unwrap_or_else(|| toml::Value::Table(Default::default()))
        .try_into::<std::collections::HashMap<String, McpServerConfig>>()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
    validate_ema_auth_sources(stack, &servers)
}

fn valid_ema_layers(layers: Vec<(ConfigLayerSource, &str)>) -> bool {
    validate_effective_ema(&stack(layers)).is_ok()
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
fn ema_registrations_require_one_atomic_non_project_source() {
    let (system, user, project) = local_sources("ema-registration-sources");
    for (section, authorization, protected_changes) in [
        (
            "mcp_servers.enterprise",
            r#"
            auth='ema_auth'
            oauth_resource='https://resource.example'
            oauth.client_id='resource-client'
            oauth.authorization_server_issuer='https://as.example'
        "#,
            &[
                "oauth_resource='https://other.example'",
                "oauth.client_id='other-client'",
                "oauth.authorization_server_issuer='https://other-as.example'",
            ] as &[&str],
        ),
        (
            "plugins.\"sample@test\".mcp_servers.enterprise.ema_auth",
            r#"
            resource='https://resource.example'
            client_id='resource-client'
            authorization_server_issuer='https://as.example'
        "#,
            &[
                "resource='https://other.example'",
                "client_id='other-client'",
                "authorization_server_issuer='https://other-as.example'",
            ],
        ),
    ] {
        let registration = format!(
            "[{section}]\nurl='https://resource.example/mcp'\nscopes=['tools']\n{authorization}"
        );
        for (source, allowed) in [
            (system.clone(), true),
            (user.clone(), true),
            (project.clone(), false),
        ] {
            assert_eq!(
                valid_ema_layers(vec![(source, registration.as_str())]),
                allowed,
                "{section}"
            );
        }
        for change in protected_changes.iter().copied().chain([
            "url='https://other.example/mcp'",
            "url='https://resource.example/mcp/other'",
            "scopes=['admin']",
        ]) {
            let overlay = format!("[{section}]\n{change}");
            assert!(
                !valid_ema_layers(vec![
                    (system.clone(), &registration),
                    (project.clone(), &overlay)
                ]),
                "{section}: {change}"
            );
        }
        let higher = registration.replace("tools", "managed-tools");
        assert!(
            !valid_ema_layers(vec![
                (system.clone(), &registration),
                (user.clone(), &higher),
                (project.clone(), &registration)
            ]),
            "project rollback: {section}"
        );
        let policy_section = section.strip_suffix(".ema_auth").unwrap_or(section);
        let policy = format!(
            "[{policy_section}]\nenabled=false\n[{policy_section}.tools.read]\napproval_mode='prompt'"
        );
        assert!(valid_ema_layers(vec![
            (system.clone(), &registration),
            (project.clone(), &policy)
        ]));
        if section.ends_with(".ema_auth") {
            for required in [
                "url='https://resource.example/mcp'\n",
                "resource='https://resource.example'\n",
            ] {
                assert!(
                    !valid_ema_layers(vec![(system.clone(), &registration.replace(required, ""))]),
                    "missing {required}"
                );
            }
        } else {
            assert!(valid_ema_layers(vec![
                (system.clone(), &registration),
                (project.clone(), "[mcp_servers.enterprise]\nauth='ema_auth'")
            ]));
        }
    }
}

#[test]
fn project_cannot_reenable_trusted_disabled_ema_registration() {
    let (system, user, project) = local_sources("ema-disabled-source");
    let trusted = "[mcp_servers.enterprise]\nurl='https://resource.example/mcp'\nauth='ema_auth'\nenabled=false";
    let project_enable = "[mcp_servers.enterprise]\nenabled=true";
    assert!(!valid_ema_layers(vec![
        (system.clone(), trusted),
        (project.clone(), project_enable)
    ]));
    assert!(valid_ema_layers(vec![
        (system.clone(), trusted),
        (user, project_enable)
    ]));
    let enabled = trusted.replace("enabled=false", "enabled=true");
    assert!(valid_ema_layers(vec![
        (system, &enabled),
        (project, "[mcp_servers.enterprise]\nenabled=false")
    ]));
}

#[test]
fn project_cannot_reenable_trusted_disabled_ema_plugin() {
    let (system, _, project) = local_sources("ema-disabled-plugin");
    let trusted = r#"
        [plugins."sample@test".mcp_servers.enterprise]
        enabled=false
        [plugins."sample@test".mcp_servers.enterprise.ema_auth]
        url='https://resource.example/mcp'
        client_id='resource-client'
        authorization_server_issuer='https://as.example'
        resource='https://resource.example'
    "#;
    let project_enable = "[plugins.\"sample@test\".mcp_servers.enterprise]\nenabled=true";
    assert!(!valid_ema_layers(vec![
        (system.clone(), trusted),
        (project.clone(), project_enable)
    ]));
    assert!(valid_ema_layers(vec![
        (system, trusted),
        (
            project,
            "[plugins.\"sample@test\".mcp_servers.enterprise]\nenabled=false"
        )
    ]));
}

#[test]
fn non_project_auth_changes_and_ordinary_oauth_remain_allowed() {
    let (system, user, project) = local_sources("ema-auth-downgrade");
    for (base_auth, source) in [("ema_auth", user), ("oauth", project)] {
        let base = format!(
            "[mcp_servers.enterprise]\nurl='https://resource.example/mcp'\nauth='{base_auth}'"
        );
        assert!(
            validate_effective_ema(&stack(vec![
                (system.clone(), &base),
                (
                    source,
                    "[mcp_servers.enterprise]\nauth='oauth'\nenabled=false"
                ),
            ]))
            .is_ok()
        );
    }
}

#[test]
fn xaa_opt_in_requires_a_non_project_source() {
    let (_, user, project) = local_sources("xaa-sources");
    let session = ConfigLayerSource::SessionFlags;
    let enabled = "[features]\nuse_xaa=true";

    for (source, allowed) in [(user, true), (session, true), (project.clone(), false)] {
        assert_eq!(
            validate_xaa_opt_in_source(&stack(vec![(source, enabled)]), /*xaa_enabled*/ true,)
                .is_ok(),
            allowed
        );
    }
    assert!(
        validate_xaa_opt_in_source(&stack(vec![(project, enabled)]), /*xaa_enabled*/ false,)
            .is_ok()
    );
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
