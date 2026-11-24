//! Adapts harness configuration and credentials to the process-local Statsig service.

use std::sync::Arc;

use codex_features::Feature;
use codex_login::AuthManager;
use codex_login::is_workload_identity_selected;
use codex_model_provider::ProviderAuthContext;
use codex_protocol::auth::AuthMode;
pub use codex_statsig::DynamicConfig;
pub use codex_statsig::Gate;
use codex_statsig::StatsigBootstrapContext;
use codex_statsig::StatsigIdentity;
pub use codex_statsig::StatsigRuntime;

use crate::config::Config;

/// Harness launch surfaces sent to Statsig as targeting values.
pub enum StatsigSurface {
    Cli,
    Exec,
    AppServer,
    Tui,
    Sandbox,
    ExecServer,
    RemoteExecutor,
}

/// The credential source selected by the launch path.
pub enum HostIdentity {
    ServingAccount {
        auth: Arc<AuthManager>,
        default_analytics_enabled: bool,
    },
    LocalAccount,
    Installation,
}

impl Config {
    /// Returns whether this launch opted in and has a client key.
    pub fn statsig_enabled(&self) -> bool {
        self.statsig_key().is_some()
    }

    fn statsig_key(&self) -> Option<String> {
        if !self.features.enabled(Feature::StatsigGates) {
            return None;
        }
        option_env!("CODEX_BUNDLED_STATSIG_CLIENT_KEY")
            .filter(|key| !key.is_empty())
            .map(str::to_owned)
    }

    /// Installs the flag store for the lifetime of the harness process.
    pub async fn start_statsig(
        &self,
        surface: StatsigSurface,
        identity: HostIdentity,
    ) -> Option<Arc<StatsigRuntime>> {
        let sdk_key = self.statsig_key()?;
        if let Some(runtime) = StatsigRuntime::current() {
            return Some(runtime);
        }
        let stable_id = crate::resolve_installation_id(&self.codex_home)
            .await
            .ok()?;
        let account_identity = |auth: Arc<AuthManager>| {
            StatsigIdentity::Account(ProviderAuthContext {
                runtime_auth: crate::build_runtime_auth_manager(
                    self,
                    Arc::clone(&auth),
                    &stable_id,
                )
                .map(|runtime| (runtime, self.runtime_auth_mode)),
                credentials: auth,
            })
        };
        let (identity, default_analytics_enabled) = match identity {
            HostIdentity::ServingAccount {
                auth,
                default_analytics_enabled,
            } => (account_identity(auth), Some(default_analytics_enabled)),
            HostIdentity::LocalAccount => {
                // Workload identity requires a token exchange, even when the auth modes are filtered.
                if !is_workload_identity_selected()
                    && let Ok(auth) = AuthManager::shared_from_config_with_auth_modes(
                        self,
                        /*enable_codex_api_key_env*/ false,
                        vec![
                            AuthMode::Chatgpt,
                            AuthMode::ChatgptAuthTokens,
                            AuthMode::ApiKey,
                        ],
                    )
                    .await
                    && auth.auth_cached().is_some()
                {
                    (
                        account_identity(auth),
                        (!matches!(surface, StatsigSurface::ExecServer)).then_some(true),
                    )
                } else {
                    (
                        StatsigIdentity::Installation(self.http_client_factory()),
                        None,
                    )
                }
            }
            HostIdentity::Installation => (
                StatsigIdentity::Installation(self.http_client_factory()),
                None,
            ),
        };
        let exposure_logging_enabled = default_analytics_enabled
            .is_some_and(|default_enabled| self.analytics_enabled.unwrap_or(default_enabled));
        Some(StatsigRuntime::start_process(
            identity,
            self.chatgpt_base_url.clone(),
            sdk_key,
            StatsigBootstrapContext {
                target_app: "PUBLIC-EVERYTHING-LEAKS-codex-cli",
                stable_id,
                app_version: env!("CARGO_PKG_VERSION").into(),
                locale: sys_locale::get_locale(),
                system_name: std::env::consts::OS.into(),
                system_version: os_info::get().version().to_string(),
                window_type: match surface {
                    StatsigSurface::Cli => "cli",
                    StatsigSurface::Exec => "exec",
                    StatsigSurface::AppServer => "app-server",
                    StatsigSurface::Tui => "tui",
                    StatsigSurface::Sandbox => "sandbox",
                    StatsigSurface::ExecServer => "exec-server",
                    StatsigSurface::RemoteExecutor => "remote-executor",
                },
                brand_name: "codex",
            },
            exposure_logging_enabled,
        ))
    }
}
