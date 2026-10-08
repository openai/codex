//! Ordinary refresh resolution and enterprise revocation without Session construction.

use super::*;
use crate::config::ConfigBuilder;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ConfigLayerSource;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_core_plugins::PluginLoadOutcome;
use pretty_assertions::assert_eq;

async fn base_config() -> (tempfile::TempDir, Config) {
    let home = tempfile::tempdir().unwrap();
    let config = ConfigBuilder::without_managed_config_for_tests()
        .codex_home(home.path().to_path_buf())
        .fallback_cwd(Some(home.path().to_path_buf()))
        .build()
        .await
        .unwrap();
    (home, config)
}

async fn layered_config(
    base: &Config,
    user: &str,
    managed: &str,
    requirements: &str,
    session: &str,
) -> Config {
    std::fs::write(base.codex_home.join(CONFIG_TOML_FILE), user).unwrap();
    let overrides: toml::Table = toml::from_str(session).unwrap();
    ConfigBuilder::without_managed_config_for_tests()
        .codex_home(base.codex_home.to_path_buf())
        .fallback_cwd(Some(base.cwd.to_path_buf()))
        .cli_overrides(overrides.into_iter().collect())
        .cloud_config_bundle(
            CloudConfigBundleFixture::enterprise_config(managed)
                .add_enterprise_requirement(format!("[features]\n{requirements}"))
                .into_loader(),
        )
        .build()
        .await
        .unwrap()
}

fn enterprise_config(issuer: &str, resource: &str) -> String {
    format!(
        "[features]\nuse_xaa = true\napps = false\nsecret_auth_storage = false\n\
         [mcp_servers.enterprise]\nurl = 'https://resource.example/mcp'\nauth = 'ema_auth'\n\
         oauth_resource = '{resource}'\n\
         [mcp_enterprise_managed_auth.idp]\nissuer = '{issuer}'\nclient_id = '{issuer}-client'\n"
    )
}

fn runtime_servers(config: &Config) -> HashMap<String, McpServerConfig> {
    codex_mcp::configured_mcp_servers(
        &config.to_mcp_config_with_loaded_plugins(&PluginLoadOutcome::default(), []),
    )
}

#[test_case::test_case(RuntimeConfigRefresh::User; "user refresh")]
#[test_case::test_case(RuntimeConfigRefresh::Mcp; "MCP refresh")]
#[tokio::test]
async fn refresh_honors_fresh_managed_denial(scope: RuntimeConfigRefresh) {
    let (_home, base) = base_config().await;
    let enterprise = enterprise_config("https://idp.example", "resource");
    let current = layered_config(&base, &enterprise, "", "use_xaa = true", "").await;
    let next = layered_config(&base, &enterprise, "", "use_xaa = false", "").await;
    let refreshed = current.resolve_runtime_refresh(&next, scope).unwrap();
    assert!(!runtime_servers(&refreshed)["enterprise"].enabled);
    assert!(refreshed.mcp_enterprise_managed_auth.is_none());
    assert_eq!(refreshed.model, current.model);
}

#[tokio::test]
async fn plugin_refresh_resolves_only_the_final_layer_combination() {
    let (_home, base) = base_config().await;
    let snapshot = |user: &str, project: &str| {
        let mut config = base.clone();
        config.config_layer_stack = codex_config::ConfigLayerStack::new(
            vec![
                codex_config::ConfigLayerEntry::new(
                    ConfigLayerSource::User {
                        file: base.codex_home.join(CONFIG_TOML_FILE),
                        profile: None,
                    },
                    toml::from_str(user).unwrap(),
                ),
                codex_config::ConfigLayerEntry::new(
                    ConfigLayerSource::Project {
                        dot_codex_folder: base.codex_home.join(".codex"),
                    },
                    toml::from_str(project).unwrap(),
                ),
            ],
            Default::default(),
            Default::default(),
        )
        .unwrap();
        config.plugins = config
            .config_layer_stack
            .effective_config()
            .try_into::<codex_config::config_toml::ConfigToml>()
            .unwrap()
            .plugins;
        config
    };
    let current = snapshot("[plugins._default]\nenabled = false", "");
    let incoming = snapshot(
        "[plugins._default]\nenabled = false\n[plugins.\"example@marketplace\"]\nenabled = 'invalid'",
        "[plugins.\"example@marketplace\"]\nenabled = false",
    );
    for (current, incoming, scope) in [
        (&current, &incoming, RuntimeConfigRefresh::User),
        (&current, &incoming, RuntimeConfigRefresh::UserFiles),
        (&incoming, &current, RuntimeConfigRefresh::Mcp),
    ] {
        assert!(current.resolve_runtime_refresh(incoming, scope).is_err());
        assert!(!current.plugins.allows_plugin("example@marketplace"));
    }
    let corrected = snapshot("[plugins._default]\nenabled = true", "");
    let refreshed = current
        .resolve_runtime_refresh(&corrected, RuntimeConfigRefresh::User)
        .unwrap();
    assert_eq!(refreshed.plugins, corrected.plugins);
    assert!(refreshed.plugins.allows_plugin("example@marketplace"));
}

#[tokio::test]
async fn refresh_honors_disabled_enterprise_host_load_result() {
    let (_home, base) = base_config().await;
    let current = layered_config(
        &base,
        &enterprise_config("https://idp.example", "resource"),
        "",
        "",
        "",
    )
    .await;
    let mut failed = current.clone();
    failed.disable_mcp_enterprise_auth();
    let refreshed = current
        .resolve_runtime_refresh(&failed, RuntimeConfigRefresh::Mcp)
        .unwrap();
    assert!(refreshed.mcp_enterprise_managed_auth.is_none());
    assert!(!runtime_servers(&refreshed)["enterprise"].enabled);
}

#[tokio::test]
async fn ordinary_transport_transitions_use_the_resolved_incoming_map() {
    let (_home, base) = base_config().await;
    let command = "[mcp_servers.ordinary]\ncommand = 'ordinary-server'";
    let url = "[mcp_servers.ordinary]\nurl = 'https://ordinary.example/mcp'";
    for scope in [RuntimeConfigRefresh::User, RuntimeConfigRefresh::Mcp] {
        let (current, incoming) = match scope {
            RuntimeConfigRefresh::User => (
                layered_config(&base, "", "", "", url).await,
                layered_config(&base, command, "", "", "").await,
            ),
            RuntimeConfigRefresh::Mcp => (
                layered_config(&base, command, "", "", "").await,
                layered_config(&base, "", url, "", "").await,
            ),
            RuntimeConfigRefresh::UserFiles => unreachable!(),
        };
        let refreshed = current.resolve_runtime_refresh(&incoming, scope).unwrap();
        assert_eq!(refreshed.mcp_servers.get(), incoming.mcp_servers.get());
    }
}

#[tokio::test]
async fn mcp_refresh_preserves_current_user_and_session_layers_without_reopening_ema() {
    let (_home, base) = base_config().await;
    let current = layered_config(
        &base,
        &enterprise_config("https://current-idp.example", "current-resource"),
        "",
        "",
        "[mcp_servers.enterprise]\nenabled = false",
    )
    .await;
    let next = layered_config(
        &base,
        &enterprise_config("https://old-idp.example", "old-resource"),
        "",
        "",
        "",
    )
    .await;
    let refreshed = current
        .resolve_runtime_refresh(&next, RuntimeConfigRefresh::Mcp)
        .unwrap();
    let preserved_layers = |config: &Config| {
        config
            .config_layer_stack
            .all_layers_low_to_high()
            .filter(|layer| {
                matches!(
                    layer.name,
                    ConfigLayerSource::User { .. } | ConfigLayerSource::SessionFlags
                )
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(preserved_layers(&refreshed), preserved_layers(&current));
    let server = &runtime_servers(&refreshed)["enterprise"];
    assert!(!server.enabled);
    assert_eq!(
        server.ema_registration(),
        runtime_servers(&current)["enterprise"].ema_registration()
    );
}

#[test_case::test_case("removed"; "removed")]
#[test_case::test_case("disabled"; "disabled")]
#[test_case::test_case("resource"; "resource changed")]
#[test_case::test_case("transport"; "transport changed")]
#[test_case::test_case("scopes"; "scopes changed")]
#[test_case::test_case("oauth"; "OAuth client changed")]
#[test_case::test_case("idp"; "IdP changed")]
#[tokio::test]
async fn refresh_revokes_changed_enterprise_admission(change: &str) {
    let (_home, base) = base_config().await;
    let current = layered_config(
        &base,
        &enterprise_config("https://idp.example", "resource"),
        "",
        "",
        "",
    )
    .await;
    let mut incoming = current.clone();
    let mut servers = incoming.mcp_servers.get().clone();
    match change {
        "removed" => {
            servers.remove("enterprise");
        }
        "disabled" => servers.get_mut("enterprise").unwrap().enabled = false,
        "resource" => servers.get_mut("enterprise").unwrap().oauth_resource = Some("other".into()),
        "scopes" => servers.get_mut("enterprise").unwrap().scopes = Some(vec!["other".into()]),
        "oauth" => {
            servers
                .get_mut("enterprise")
                .unwrap()
                .oauth
                .get_or_insert_default()
                .client_id = Some("other-client".into())
        }
        "idp" => {
            incoming
                .mcp_enterprise_managed_auth
                .as_mut()
                .unwrap()
                .idp
                .client_id = "other-client".into()
        }
        "transport" => {
            let codex_config::McpServerTransportConfig::StreamableHttp { url, .. } =
                &mut servers.get_mut("enterprise").unwrap().transport
            else {
                unreachable!()
            };
            *url = "https://other.example/mcp".into();
        }
        _ => unreachable!(),
    }
    incoming.mcp_servers.set(servers).unwrap();
    for scope in [RuntimeConfigRefresh::User, RuntimeConfigRefresh::Mcp] {
        let revoked = current.resolve_runtime_refresh(&incoming, scope).unwrap();
        assert!(revoked.mcp_enterprise_managed_auth.is_none());
        assert!(!runtime_servers(&revoked)["enterprise"].enabled);
        let restored = revoked.resolve_runtime_refresh(&current, scope).unwrap();
        assert!(restored.mcp_enterprise_managed_auth.is_none());
        assert!(!runtime_servers(&restored)["enterprise"].enabled);
    }
}

#[tokio::test]
async fn mcp_refresh_preserves_apps_until_new_session() {
    let (_home, base) = base_config().await;
    let current = layered_config(&base, "[features]\napps = true", "", "", "").await;
    let next = layered_config(&base, "[features]\napps = true", "", "apps = false", "").await;
    let refreshed = current
        .resolve_runtime_refresh(&next, RuntimeConfigRefresh::Mcp)
        .unwrap();
    assert!(refreshed.features.enabled(Feature::Apps));
    let reloaded = refreshed
        .resolve_runtime_refresh(&next, RuntimeConfigRefresh::User)
        .unwrap();
    assert!(reloaded.features.enabled(Feature::Apps));
    assert!(!next.features.enabled(Feature::Apps));
}
