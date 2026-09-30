//! Embedded startup preserves caller configuration and bootstrap access across authentication.

use super::EmbeddedNetworkPolicy;
use super::configure;
use crate::config_manager::ConfigManager;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use codex_arg0::Arg0DispatchPaths;
use codex_config::CloudConfigBundleLoader;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::set_project_trust_level;
use codex_http_client::DestinationPolicy;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPermit;
use codex_http_client::NetworkPolicyDenied;
use codex_login::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_login::ExternalAuth;
use codex_login::ExternalAuthFuture;
use codex_login::ExternalAuthRefreshContext;
use codex_protocol::config_types::TrustLevel;
use pretty_assertions::assert_eq;
use std::sync::Arc;

struct BootstrapExternalAuth(CodexAuth);

impl ExternalAuth for BootstrapExternalAuth {
    fn resolve(&self) -> ExternalAuthFuture<'_, CodexAuth> {
        Box::pin(async { Ok(self.0.clone()) })
    }

    fn refresh(&self, _context: ExternalAuthRefreshContext) -> ExternalAuthFuture<'_, CodexAuth> {
        self.resolve()
    }
}

#[tokio::test]
async fn bootstrap_discovery_survives_identity_installation_but_account_access_is_revoked()
-> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let overrides = LoaderOverrides::without_managed_config_for_tests();
    let config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .loader_overrides(overrides.clone())
        .build()
        .await?;
    let policy = EmbeddedNetworkPolicy::load(&overrides).await;
    let auth_config = policy.bind_bootstrap_auth(config.auth_config());
    let bootstrap = auth_config.auth_route_config.http_client_factory().clone();
    let bundle_url =
        codex_backend_client::Client::new(config.chatgpt_base_url.clone(), bootstrap.clone())
            .config_bundle_url()
            .parse()?;
    let content_url = "https://chatgpt.com/backend-api/codex/responses".parse()?;
    let manager = AuthManager::new(
        home.path().to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        AuthCredentialsStoreMode::Ephemeral,
        /*forced_chatgpt_workspace_id*/ None,
        Some(config.chatgpt_base_url.clone()),
        AuthKeyringBackendKind::default(),
        auth_config.auth_route_config,
    )
    .await;
    assert!(manager.auth_cached().is_none());

    bootstrap.network_policy().acquire(&bundle_url)?;
    let mut previous_access: Option<(HttpClientFactory, NetworkPermit)> = None;
    for identity in ["bootstrap-account-one", "bootstrap-account-two"] {
        let token = format!(
            "e30.{}.signature",
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&serde_json::json!({
                "sub": "bootstrap-user",
                "https://api.openai.com/auth": {
                    "chatgpt_user_id": "bootstrap-user",
                    "chatgpt_account_id": identity,
                },
            }))?),
        );
        let auth = CodexAuth::from_external_chatgpt_tokens(&token, identity, Some("enterprise"))?;
        manager
            .set_external_auth(Arc::new(BootstrapExternalAuth(auth)))
            .await?;

        if let Some((client, permit)) = previous_access.take() {
            assert_eq!(permit.check(), Err(NetworkPolicyDenied::Revoked));
            assert_eq!(
                client.network_policy().acquire(&content_url).err(),
                Some(NetworkPolicyDenied::Revoked)
            );
        }

        // Discovery must remain possible after both initial login and a workspace change.
        bootstrap.network_policy().acquire(&bundle_url)?;
        assert_eq!(
            bootstrap.network_policy().acquire(&content_url).err(),
            Some(NetworkPolicyDenied::Destination)
        );
        assert_eq!(
            manager
                .application_network_policy()
                .acquire(&content_url)
                .err(),
            Some(NetworkPolicyDenied::Unavailable)
        );

        assert!(policy.effective.publish(
            policy.effective.policy().revision(),
            DestinationPolicy::Unrestricted,
        ));
        let client = manager.http_client_factory();
        let permit = client.network_policy().acquire(&content_url)?;
        previous_access = Some((client, permit));
    }

    let (client, permit) = previous_access.expect("the last identity has active account access");
    manager.clear_external_auth();
    assert_eq!(permit.check(), Err(NetworkPolicyDenied::Revoked));
    assert_eq!(
        client.network_policy().acquire(&content_url).err(),
        Some(NetworkPolicyDenied::Revoked)
    );
    assert_eq!(
        manager
            .application_network_policy()
            .acquire(&content_url)
            .err(),
        Some(NetworkPolicyDenied::Unavailable)
    );
    bootstrap.network_policy().acquire(&bundle_url)?;

    // Losing local requirements must still close the bootstrap route.
    policy.local.unavailable(policy.local.policy().revision());
    assert_eq!(
        bootstrap.network_policy().acquire(&bundle_url).err(),
        Some(NetworkPolicyDenied::Unavailable)
    );
    Ok(())
}

#[tokio::test]
async fn startup_reloads_the_callers_selected_project() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let home = root.join("home");
    let selected = root.join("selected");
    std::fs::create_dir(&home)?;
    std::fs::create_dir_all(selected.join(".codex"))?;
    std::fs::create_dir(selected.join(".git"))?;
    set_project_trust_level(&home, &selected, TrustLevel::Trusted)?;
    let selected_url = "https://selected.example/backend-api/";
    let user_config = home.join("config.toml");
    let trust = std::fs::read_to_string(&user_config)?;
    std::fs::write(
        user_config,
        format!("chatgpt_base_url = '{selected_url}'\n{trust}"),
    )?;
    let project_config = selected.join(".codex/config.toml");
    std::fs::write(&project_config, "model = 'selected-model'")?;
    let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
    let mut config = Arc::new(
        ConfigBuilder::default()
            .codex_home(home.clone())
            .harness_overrides(ConfigOverrides {
                cwd: Some(selected.clone()),
                ..Default::default()
            })
            .loader_overrides(loader_overrides.clone())
            .build()
            .await?,
    );
    assert_eq!(config.chatgpt_base_url, selected_url);
    assert_eq!(config.model.as_deref(), Some("selected-model"));
    let manager = ConfigManager::new(
        home,
        Vec::new(),
        loader_overrides,
        /*strict_config*/ true,
        CloudConfigBundleLoader::default(),
        Arg0DispatchPaths::default(),
        Arc::new(NoopThreadConfigLoader),
    );
    let _auth = configure(
        &manager,
        &mut config,
        /*enable_codex_api_key_env*/ false,
    )
    .await?;
    assert_eq!(
        config.cwd.as_path().canonicalize()?,
        selected.canonicalize()?
    );
    assert_eq!(config.chatgpt_base_url, selected_url);

    std::fs::write(&project_config, "[malformed")?;
    let error = configure(
        &manager,
        &mut config,
        /*enable_codex_api_key_env*/ false,
    )
    .await
    .err()
    .ok_or_else(|| anyhow::anyhow!("embedded startup ignored the selected project"))?;
    assert!(
        error
            .to_string()
            .contains("Error parsing project config file")
    );
    Ok(())
}
