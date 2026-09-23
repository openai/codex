use codex_arg0::Arg0DispatchPaths;
use codex_cloud_config::cloud_config_bundle_loader;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigLayerStack;
use codex_config::LoaderOverrides;
use codex_config::ThreadConfigLoader;
use codex_config::loader::load_config_layers_state;
use codex_config::loader::load_managed_requirements_state;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_exec_server::LOCAL_FS;
use codex_features::feature_for_key;
use codex_login::AuthManager;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_model_provider_info::AMAZON_BEDROCK_PROVIDER_ID;
use codex_model_provider_info::AMAZON_BEDROCK_RUNTIME_PROVIDER_ID;
use codex_model_provider_info::built_in_model_providers;
use codex_model_provider_info::merge_configured_model_providers;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_json_to_toml::json_to_toml;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use toml::Value as TomlValue;
use tracing::instrument;
use tracing::warn;

#[path = "application_network.rs"]
pub(crate) mod application_network;

#[derive(Debug, thiserror::Error)]
#[error(
    "Your organization's required model provider settings changed. Restart Codex to apply them; this request was not sent"
)]
pub(crate) struct ModelProviderRequirementsChanged;

/// Shared app-server entry point for loading effective Codex configuration.
#[derive(Clone)]
pub(crate) struct ConfigManager {
    codex_home: PathBuf,
    cli_overrides: Arc<RwLock<Vec<(String, TomlValue)>>>,
    runtime_feature_enablement: Arc<RwLock<BTreeMap<String, bool>>>,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    cloud_config_bundle: Arc<RwLock<CloudConfigBundleLoader>>,
    arg0_paths: Arg0DispatchPaths,
    thread_config_loader: Arc<dyn ThreadConfigLoader>,
    network_policy: codex_http_client::NetworkPolicyController,
    local_network_policy: codex_http_client::NetworkPolicyController,
    network_policy_reload: Arc<tokio::sync::Semaphore>,
    network_policy_snapshot: Arc<RwLock<Option<Arc<ApplicationPolicySnapshot>>>>,
}

/// Configuration and policy must finish loading against the same account and cloud snapshot.
pub(crate) struct ApplicationPolicyLoad {
    cloud_config: CloudConfigBundleLoader,
    snapshot: Arc<ApplicationPolicySnapshot>,
}

#[derive(PartialEq, Eq)]
struct ApplicationPolicySnapshot {
    revision: codex_http_client::NetworkPolicyRevision,
    policy: codex_http_client::DestinationPolicy,
    cloud: Option<codex_config::CloudConfigBundle>,
}

impl ConfigManager {
    pub(crate) fn new(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
        strict_config: bool,
        cloud_config_bundle: CloudConfigBundleLoader,
        arg0_paths: Arg0DispatchPaths,
        thread_config_loader: Arc<dyn ThreadConfigLoader>,
    ) -> Self {
        let network_policy = codex_http_client::NetworkPolicyController::default();
        Self {
            codex_home,
            cli_overrides: Arc::new(RwLock::new(cli_overrides)),
            runtime_feature_enablement: Arc::new(RwLock::new(BTreeMap::new())),
            loader_overrides,
            strict_config,
            cloud_config_bundle: Arc::new(RwLock::new(cloud_config_bundle)),
            arg0_paths,
            thread_config_loader,
            network_policy,
            local_network_policy: Default::default(),
            network_policy_reload: Arc::new(tokio::sync::Semaphore::new(/*permits*/ 1)),
            network_policy_snapshot: Arc::default(),
        }
    }

    pub(crate) fn codex_home(&self) -> &Path {
        self.codex_home.as_path()
    }

    pub(crate) fn with_embedded_network_policy(
        mut self,
        policy: crate::in_process::EmbeddedNetworkPolicy,
    ) -> Self {
        self.network_policy = policy.effective;
        self.local_network_policy = policy.local;
        self
    }

    pub(crate) fn user_config_path(&self) -> std::io::Result<AbsolutePathBuf> {
        self.loader_overrides.user_config_path(self.codex_home())
    }

    pub(crate) fn current_cli_overrides(&self) -> Vec<(String, TomlValue)> {
        self.cli_overrides
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub(crate) fn current_cloud_config_bundle(&self) -> CloudConfigBundleLoader {
        self.cloud_config_bundle
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub(crate) fn extend_runtime_feature_enablement<I>(&self, enablement: I) -> Result<(), ()>
    where
        I: IntoIterator<Item = (String, bool)>,
    {
        let mut runtime_feature_enablement =
            self.runtime_feature_enablement.write().map_err(|_| ())?;
        runtime_feature_enablement.extend(enablement);
        Ok(())
    }

    pub(crate) fn replace_cloud_config_bundle_loader(
        &self,
        auth_manager: Arc<AuthManager>,
        chatgpt_base_url: String,
        http_client_factory: codex_http_client::HttpClientFactory,
    ) {
        let endpoint = codex_backend_client::Client::new(
            chatgpt_base_url.clone(),
            http_client_factory.clone(),
        )
        .config_bundle_url();
        let http_client_factory = http_client_factory.with_network_policy(
            self.local_network_policy
                .policy()
                .restrict_to_endpoints(endpoint.parse().into_iter().collect()),
        );
        let loader = cloud_config_bundle_loader(
            auth_manager,
            chatgpt_base_url,
            self.codex_home.clone(),
            http_client_factory,
        );
        if let Ok(mut guard) = self.cloud_config_bundle.write() {
            *guard = loader;
        } else {
            warn!("failed to update cloud config bundle loader");
        }
    }

    pub(crate) fn clear_cloud_config_bundle_loader(&self) {
        if let Ok(mut guard) = self.cloud_config_bundle.write() {
            *guard = CloudConfigBundleLoader::default();
        } else {
            warn!("failed to clear cloud config bundle loader");
        }
    }

    pub(crate) async fn sync_default_client_residency_requirement(&self) {
        match self.load_latest_config(/*fallback_cwd*/ None).await {
            Ok(config) => {
                set_default_client_residency_requirement(config.enforce_residency.value());
            }
            Err(err) => warn!(
                error = %err,
                "failed to sync default client residency requirement after auth refresh"
            ),
        }
    }

    pub(crate) async fn load_latest_config(
        &self,
        fallback_cwd: Option<PathBuf>,
    ) -> std::io::Result<Config> {
        self.load_with_cli_overrides(
            &self.current_cli_overrides(),
            /*request_overrides*/ None,
            ConfigOverrides::default(),
            fallback_cwd,
        )
        .await
    }

    /// Loads system, user, and runtime settings without discovering a project
    /// from the app-server process's working directory.
    pub(crate) async fn load_non_project_config(&self) -> std::io::Result<Config> {
        let mut manager = self.clone();
        manager.loader_overrides.ignore_project_config = true;
        manager.load_latest_config(/*fallback_cwd*/ None).await
    }

    pub(crate) async fn load_latest_config_with_session_layers(
        &self,
        session_layers: &ConfigLayerStack,
        cwd: &Path,
    ) -> std::io::Result<Config> {
        let refreshed_config = self.load_latest_config(Some(cwd.to_path_buf())).await?;
        let mut config = Config::rebuild_with_session_layers(
            session_layers,
            cwd.to_path_buf(),
            &refreshed_config.config_layer_stack,
            refreshed_config.codex_home.clone(),
            refreshed_config
                .zsh_path
                .clone()
                .map(AbsolutePathBuf::try_from)
                .transpose()?,
        )
        .await?;
        config.application_network_policy = refreshed_config.application_network_policy;
        config.application_auth_route_config = refreshed_config.application_auth_route_config;
        self.apply_runtime_feature_enablement(&mut config);
        self.apply_arg0_paths(&mut config);
        Ok(config)
    }

    /// Refreshes global settings and managed requirements using already fetched session layers.
    pub(crate) async fn load_retained_session_config(
        &self,
        session_layers: &ConfigLayerStack,
        cwd: &Path,
    ) -> std::io::Result<Config> {
        let mut manager = self.clone();
        manager.thread_config_loader = Arc::new(codex_config::NoopThreadConfigLoader);
        let refreshed_layers = manager
            .load_config_layers_for_cwd(AbsolutePathBuf::from_absolute_path(cwd)?)
            .await?;
        // Merge the retained provider definitions before resolving managed provider selection.
        let mut config = Config::rebuild_with_session_layers(
            session_layers,
            cwd.to_path_buf(),
            &refreshed_layers,
            AbsolutePathBuf::from_absolute_path(&self.codex_home)?,
            /*default_zsh_path*/ None,
        )
        .await?;
        self.apply_network_policy(&mut config);
        self.apply_runtime_feature_enablement(&mut config);
        self.apply_arg0_paths(&mut config);
        Ok(config)
    }

    /// Checks the retained session route against current managed provider requirements.
    pub(crate) async fn check_thread_model_provider(
        &self,
        current: &Config,
    ) -> std::io::Result<()> {
        let policy_load = self.refresh_application_network_policy().await?;
        // Existing threads retain their session route; only managed
        // requirements can invalidate it.
        let requirements = load_managed_requirements_state(
            LOCAL_FS.as_ref(),
            &self.codex_home,
            codex_config::ConfigLoadOptions {
                loader_overrides: self.loader_overrides.clone(),
                strict_config: self.strict_config,
                cloud_config_bundle: policy_load.cloud_config.clone(),
            },
        )
        .await?;
        self.check_application_policy_load(&policy_load)?;
        let selection_changed = requirements
            .model_provider
            .as_ref()
            .is_some_and(|provider_id| provider_id != &current.model_provider_id);
        let required_provider = requirements
            .model_providers
            .as_ref()
            .and_then(|providers| providers.get(&current.model_provider_id))
            .cloned();
        let required_provider = match required_provider {
            Some(provider)
                if matches!(
                    current.model_provider_id.as_str(),
                    AMAZON_BEDROCK_PROVIDER_ID | AMAZON_BEDROCK_RUNTIME_PROVIDER_ID
                ) =>
            {
                // Bedrock requirements contain overrides of the built-in provider.
                merge_configured_model_providers(
                    built_in_model_providers(/*openai_base_url*/ None),
                    HashMap::from([(current.model_provider_id.clone(), provider)]),
                )
                .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidData, message))?
                .remove(&current.model_provider_id)
            }
            Some(provider) => Some(provider),
            None => None,
        };
        let definition_changed = required_provider
            .as_ref()
            .is_some_and(|provider| provider != &current.model_provider);
        if selection_changed || definition_changed {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                ModelProviderRequirementsChanged,
            ));
        }
        Ok(())
    }

    /// Installs local policy before cloud access; failed policy loads block traffic while
    /// non-strict startup remains available for configuration repair.
    pub(crate) async fn load_startup_config(
        &self,
        fallback_cwd: Option<PathBuf>,
    ) -> std::io::Result<Config> {
        match self.load_latest_config(fallback_cwd).await {
            Ok(config) => Ok(config),
            Err(error)
                if self.strict_config
                    || crate::is_unsupported_untrusted_approval_policy_error(&error) =>
            {
                Err(error)
            }
            Err(error) => {
                warn!(%error, "configuration is unavailable; using default settings");
                self.load_default_config().await
            }
        }
    }

    pub(crate) async fn load_default_config(&self) -> std::io::Result<Config> {
        let mut loader_overrides = self.loader_overrides.clone();
        loader_overrides.ignore_user_config = true;
        loader_overrides.ignore_project_config = true;
        loader_overrides.ignore_managed_requirements |=
            self.refresh_local_network_policy().await.is_err();
        let mut config = ConfigBuilder::default()
            .codex_home(self.codex_home.clone())
            .cli_overrides(self.current_cli_overrides())
            .loader_overrides(loader_overrides)
            .fallback_cwd(Some(self.codex_home.clone()))
            .cloud_config_bundle(CloudConfigBundleLoader::default())
            .build()
            .await?;
        self.apply_network_policy(&mut config);
        self.apply_runtime_feature_enablement(&mut config);
        self.apply_arg0_paths(&mut config);
        Ok(config)
    }

    pub(crate) async fn load_with_overrides(
        &self,
        request_overrides: Option<HashMap<String, serde_json::Value>>,
        typesafe_overrides: ConfigOverrides,
    ) -> std::io::Result<Config> {
        self.load_with_cli_overrides(
            &self.current_cli_overrides(),
            request_overrides,
            typesafe_overrides,
            /*fallback_cwd*/ None,
        )
        .await
    }

    pub(crate) async fn load_for_cwd(
        &self,
        request_overrides: Option<HashMap<String, serde_json::Value>>,
        typesafe_overrides: ConfigOverrides,
        cwd: Option<PathBuf>,
    ) -> std::io::Result<Config> {
        self.load_with_cli_overrides(
            &self.current_cli_overrides(),
            request_overrides,
            typesafe_overrides,
            cwd,
        )
        .await
    }

    /// Reload sources using the task's session flags before materializing config.
    pub(crate) async fn load_permission_config_for_thread(
        &self,
        thread_config: &Config,
        cwd: AbsolutePathBuf,
        permission_profile: String,
    ) -> std::io::Result<Config> {
        let mut session_flags = TomlValue::Table(Default::default());
        for layer in thread_config.config_layer_stack.layers_low_to_high() {
            if matches!(layer.name, codex_config::ConfigLayerSource::SessionFlags) {
                codex_config::merge_toml_values(&mut session_flags, &layer.config);
            }
        }
        let overrides = session_flags
            .as_table()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "session flags must be a table",
                )
            })?
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        self.load_with_cli_overrides(
            &overrides,
            /*request_overrides*/ None,
            ConfigOverrides {
                cwd: Some(cwd.to_path_buf()),
                default_permissions: Some(permission_profile),
                codex_linux_sandbox_exe: self.arg0_paths.codex_linux_sandbox_exe.clone(),
                main_execve_wrapper_exe: self.arg0_paths.main_execve_wrapper_exe.clone(),
                ..Default::default()
            },
            Some(cwd.to_path_buf()),
        )
        .await
    }

    #[instrument(level = "trace", skip_all)]
    pub(crate) async fn load_with_cli_overrides(
        &self,
        cli_overrides: &[(String, TomlValue)],
        request_overrides: Option<HashMap<String, serde_json::Value>>,
        mut typesafe_overrides: ConfigOverrides,
        fallback_cwd: Option<PathBuf>,
    ) -> std::io::Result<Config> {
        let policy_load = self.refresh_application_network_policy().await?;
        let mut request_overrides = request_overrides.unwrap_or_default();
        if let Some(value) = request_overrides.remove("bypass_hook_trust") {
            typesafe_overrides.bypass_hook_trust = Some(value.as_bool().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "`bypass_hook_trust` override must be a boolean",
                )
            })?);
        }
        let merged_cli_overrides = cli_overrides
            .iter()
            .cloned()
            .chain(
                request_overrides
                    .into_iter()
                    .map(|(key, value)| (key, json_to_toml(value))),
            )
            .collect::<Vec<_>>();
        let result = codex_core::config::ConfigBuilder::default()
            .codex_home(self.codex_home.clone())
            .cli_overrides(merged_cli_overrides)
            .loader_overrides(self.loader_overrides.clone())
            .strict_config(self.strict_config)
            .harness_overrides(typesafe_overrides)
            .fallback_cwd(fallback_cwd)
            .cloud_config_bundle(policy_load.cloud_config.clone())
            .thread_config_loader(Arc::clone(&self.thread_config_loader))
            .build()
            .await;
        let mut config = result?;
        self.check_application_policy_load(&policy_load)?;
        self.apply_network_policy(&mut config);
        self.apply_runtime_feature_enablement(&mut config);
        self.apply_arg0_paths(&mut config);
        Ok(config)
    }

    pub(crate) async fn load_config_layers_for_cwd(
        &self,
        cwd: AbsolutePathBuf,
    ) -> std::io::Result<ConfigLayerStack> {
        self.load_config_layers(Some(cwd)).await
    }

    pub(crate) async fn load_config_layers(
        &self,
        cwd: Option<AbsolutePathBuf>,
    ) -> std::io::Result<ConfigLayerStack> {
        let policy_load = self.refresh_application_network_policy().await?;
        let result = load_config_layers_state(
            LOCAL_FS.as_ref(),
            &self.codex_home,
            cwd,
            &self.current_cli_overrides(),
            codex_config::ConfigLoadOptions {
                loader_overrides: self.loader_overrides.clone(),
                strict_config: self.strict_config,
                cloud_config_bundle: policy_load.cloud_config.clone(),
            },
            self.thread_config_loader.as_ref(),
        )
        .await;
        let layers = result?;
        self.check_application_policy_load(&policy_load)?;
        Ok(layers)
    }

    fn apply_runtime_feature_enablement(&self, config: &mut Config) {
        apply_runtime_feature_enablement(config, &self.current_runtime_feature_enablement());
    }

    fn current_runtime_feature_enablement(&self) -> BTreeMap<String, bool> {
        self.runtime_feature_enablement
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    fn apply_network_policy(&self, config: &mut Config) {
        config.application_network_policy = self.network_policy.policy();
        config.application_auth_route_config = Some(
            codex_login::AuthRouteConfig::from_http_client_factory(
                config
                    .http_client_factory()
                    .with_network_policy(self.network_policy.policy()),
            )
            .with_local_bootstrap_factory(
                config
                    .http_client_factory()
                    .with_network_policy(self.local_network_policy.policy()),
            ),
        );
    }

    fn apply_arg0_paths(&self, config: &mut Config) {
        config.codex_self_exe = self.arg0_paths.codex_self_exe.clone();
        config.codex_linux_sandbox_exe = self.arg0_paths.codex_linux_sandbox_exe.clone();
        config.main_execve_wrapper_exe = self.arg0_paths.main_execve_wrapper_exe.clone();
    }

    #[cfg(test)]
    pub(crate) fn new_for_tests(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
        cloud_config_bundle: CloudConfigBundleLoader,
    ) -> Self {
        Self::new(
            codex_home,
            cli_overrides,
            loader_overrides,
            /*strict_config*/ false,
            cloud_config_bundle,
            Arg0DispatchPaths::default(),
            Arc::new(codex_config::NoopThreadConfigLoader),
        )
    }

    #[cfg(test)]
    pub(crate) fn without_managed_config_for_tests(codex_home: PathBuf) -> Self {
        Self::new_for_tests(
            codex_home,
            Vec::new(),
            LoaderOverrides::without_managed_config_for_tests(),
            CloudConfigBundleLoader::default(),
        )
    }
}

#[cfg(test)]
#[path = "application_network_tests.rs"]
mod application_network_tests;

#[cfg(test)]
#[path = "config_manager_provider_tests.rs"]
mod provider_tests;

pub(crate) fn protected_feature_keys(config_layer_stack: &ConfigLayerStack) -> BTreeSet<String> {
    let mut protected_features = config_layer_stack
        .effective_config()
        .get("features")
        .and_then(toml::Value::as_table)
        .map(|features| features.keys().cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();

    if let Some(feature_requirements) = config_layer_stack
        .requirements_toml()
        .feature_requirements
        .as_ref()
    {
        protected_features.extend(feature_requirements.entries.keys().cloned());
    }

    protected_features
}

pub(crate) fn apply_runtime_feature_enablement(
    config: &mut Config,
    runtime_feature_enablement: &BTreeMap<String, bool>,
) {
    let protected_features = protected_feature_keys(&config.config_layer_stack);
    for (name, enabled) in runtime_feature_enablement {
        if protected_features.contains(name) {
            continue;
        }
        let Some(feature) = feature_for_key(name) else {
            continue;
        };
        if let Err(err) = config.features.set_enabled(feature, *enabled) {
            warn!(
                feature = name,
                error = %err,
                "failed to apply runtime feature enablement"
            );
        }
    }
}
