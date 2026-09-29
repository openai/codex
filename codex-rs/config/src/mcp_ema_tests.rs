//! Enterprise registration provenance and credential-boundary checks.

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
    let registration = r#"[mcp_servers.enterprise]
url='https://resource.example/mcp'
auth='ema_auth'
scopes=['tools']
oauth_resource='https://resource.example'
oauth.client_id='resource-client'
oauth.authorization_server_issuer='https://as.example'"#;
    for (source, allowed) in [
        (system.clone(), true),
        (user.clone(), true),
        (project.clone(), false),
    ] {
        assert_eq!(valid_ema_layers(vec![(source, registration)]), allowed);
    }
    for change in [
        "oauth_resource='https://other.example'",
        "oauth.client_id='other-client'",
        "oauth.authorization_server_issuer='https://other-as.example'",
        "url='https://other.example/mcp'",
        "url='https://resource.example/mcp/other'",
        "scopes=['admin']",
    ] {
        let overlay = format!("[mcp_servers.enterprise]\n{change}");
        assert!(!valid_ema_layers(vec![
            (system.clone(), registration),
            (project.clone(), &overlay),
        ]));
    }
    let higher = registration.replace("tools", "managed-tools");
    assert!(!valid_ema_layers(vec![
        (system.clone(), registration),
        (user, &higher),
        (project.clone(), registration),
    ]));
    assert!(valid_ema_layers(vec![
        (system.clone(), registration),
        (
            project.clone(),
            "[mcp_servers.enterprise]\nenabled=false\n[mcp_servers.enterprise.tools.read]\napproval_mode='prompt'"
        ),
    ]));
    assert!(valid_ema_layers(vec![
        (system, registration),
        (project, "[mcp_servers.enterprise]\nauth='ema_auth'"),
    ]));
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
fn ema_credential_names_bind_user_workspace_issuer_and_client() {
    let mut names = std::collections::HashSet::new();
    for user in ["a", "b"] {
        for workspace in ["a", "b"] {
            for issuer in ["https://one.example", "https://two.example"] {
                for client in ["a", "b"] {
                    let scope = McpEmaAuthScope::new(user.into(), workspace.into()).unwrap();
                    let idp = McpServerIdpOAuthConfig {
                        issuer: issuer.into(),
                        client_id: client.into(),
                    };
                    assert!(names.insert(idp.credential_name(&scope)));
                }
            }
        }
    }
    assert_eq!(McpEmaAuthScope::new(" ".into(), "workspace".into()), None);

    let identifiers = [
        "sensitive-user@example.com",
        "sensitive-workspace-id",
        "https://sensitive-idp.example",
        "sensitive-oauth-client",
    ];
    let scope = McpEmaAuthScope::new(identifiers[0].into(), identifiers[1].into()).unwrap();
    let name = McpServerIdpOAuthConfig {
        issuer: identifiers[2].into(),
        client_id: identifiers[3].into(),
    }
    .credential_name(&scope);
    assert!(name.starts_with("ema-idp:"));
    for identifier in identifiers {
        assert!(!name.contains(identifier));
        assert!(!name.contains(&URL_SAFE_NO_PAD.encode(identifier)));
    }
}

#[test]
fn ema_rejects_alternate_credentials_and_executor_custody() {
    for extra in [
        "bearer_token_env_var='TOKEN'",
        "http_headers.Authorization='secret'",
        "http_headers.Accept='application/json'",
        "env_http_headers.X-Key='TOKEN'",
        "http_headers_helper='get-headers'",
        "oauth={client_id='client', client_secret='secret'}",
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
