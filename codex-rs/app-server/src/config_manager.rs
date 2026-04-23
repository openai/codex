use codex_arg0::Arg0DispatchPaths;
use codex_cloud_requirements::cloud_requirements_loader;
use codex_config::CONFIG_TOML_FILE;
use codex_config::ThreadConfigLoader;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigLayerStack;
use codex_core::config_loader::LoaderOverrides;
use codex_core::config_loader::load_config_layers_state;
use codex_exec_server::LOCAL_FS;
use codex_features::feature_for_key;
use codex_login::AuthManager;
use codex_login::default_client::set_default_client_residency_requirement;
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
use tracing::warn;

/// Shared app-server entry point for loading effective Codex configuration.
#[derive(Clone)]
pub(crate) struct ConfigManager {
    codex_home: PathBuf,
    cli_overrides: Arc<RwLock<Vec<(String, TomlValue)>>>,
    runtime_feature_enablement: Arc<RwLock<BTreeMap<String, bool>>>,
    loader_overrides: LoaderOverrides,
    cloud_requirements: Arc<RwLock<CloudRequirementsLoader>>,
    arg0_paths: Arg0DispatchPaths,
    thread_config_loader: Arc<dyn ThreadConfigLoader>,
    host_name: Option<String>,
}

impl ConfigManager {
    pub(crate) fn new(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
        cloud_requirements: CloudRequirementsLoader,
        arg0_paths: Arg0DispatchPaths,
        thread_config_loader: Arc<dyn ThreadConfigLoader>,
    ) -> Self {
        Self::new_with_host_name(
            codex_home,
            cli_overrides,
            loader_overrides,
            cloud_requirements,
            arg0_paths,
            thread_config_loader,
            codex_config::host_name(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_host_name(
        codex_home: PathBuf,
        cli_overrides: Vec<(String, TomlValue)>,
        loader_overrides: LoaderOverrides,
        cloud_requirements: CloudRequirementsLoader,
        arg0_paths: Arg0DispatchPaths,
        thread_config_loader: Arc<dyn ThreadConfigLoader>,
        host_name: Option<String>,
    ) -> Self {
        Self {
            codex_home,
            cli_overrides: Arc::new(RwLock::new(cli_overrides)),
            runtime_feature_enablement: Arc::new(RwLock::new(BTreeMap::new())),
            loader_overrides,
            cloud_requirements: Arc::new(RwLock::new(cloud_requirements)),
            arg0_paths,
            thread_config_loader,
            host_name,
        }
    }

    pub(crate) fn codex_home(&self) -> &Path {
        self.codex_home.as_path()
    }

    pub(crate) fn user_config_path(&self) -> std::io::Result<AbsolutePathBuf> {
        match self.loader_overrides.user_config_path.as_ref() {
            Some(path) => AbsolutePathBuf::from_absolute_path(path.clone()),
            None => Ok(AbsolutePathBuf::resolve_path_against_base(
                CONFIG_TOML_FILE,
                self.codex_home(),
            )),
        }
    }

    pub(crate) fn current_cli_overrides(&self) -> Vec<(String, TomlValue)> {
        self.cli_overrides
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub(crate) fn current_cloud_requirements(&self) -> CloudRequirementsLoader {
        self.cloud_requirements
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

    pub(crate) fn replace_cloud_requirements_loader(
        &self,
        auth_manager: Arc<AuthManager>,
        chatgpt_base_url: String,
    ) {
        let loader =
            cloud_requirements_loader(auth_manager, chatgpt_base_url, self.codex_home.clone());
        if let Ok(mut guard) = self.cloud_requirements.write() {
            *guard = loader;
        } else {
            warn!("failed to update cloud requirements loader");
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

    pub(crate) async fn load_default_config(&self) -> std::io::Result<Config> {
        let mut config = Config::load_default_with_cli_overrides_for_codex_home(
            self.codex_home.clone(),
            self.current_cli_overrides(),
        )
        .await?;
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

    pub(crate) async fn load_with_cli_overrides(
        &self,
        cli_overrides: &[(String, TomlValue)],
        request_overrides: Option<HashMap<String, serde_json::Value>>,
        typesafe_overrides: ConfigOverrides,
        fallback_cwd: Option<PathBuf>,
    ) -> std::io::Result<Config> {
        let merged_cli_overrides = cli_overrides
            .iter()
            .cloned()
            .chain(
                request_overrides
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(key, value)| (key, json_to_toml(value))),
            )
            .collect::<Vec<_>>();

        let mut config = codex_core::config::ConfigBuilder::default()
            .codex_home(self.codex_home.clone())
            .cli_overrides(merged_cli_overrides)
            .loader_overrides(self.loader_overrides.clone())
            .harness_overrides(typesafe_overrides)
            .fallback_cwd(fallback_cwd)
            .cloud_requirements(self.current_cloud_requirements())
            .thread_config_loader(Arc::clone(&self.thread_config_loader))
            .host_name(self.host_name.clone())
            .build()
            .await?;
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
        load_config_layers_state(
            LOCAL_FS.as_ref(),
            &self.codex_home,
            cwd,
            &self.current_cli_overrides(),
            self.loader_overrides.clone(),
            self.current_cloud_requirements(),
            self.thread_config_loader.as_ref(),
            self.host_name.as_deref(),
        )
        .await
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
        cloud_requirements: CloudRequirementsLoader,
        host_name: Option<String>,
    ) -> Self {
        Self::new_with_host_name(
            codex_home,
            cli_overrides,
            loader_overrides,
            cloud_requirements,
            Arg0DispatchPaths::default(),
            Arc::new(codex_config::NoopThreadConfigLoader),
            host_name,
        )
    }

    #[cfg(test)]
    pub(crate) fn without_managed_config_for_tests(codex_home: PathBuf) -> Self {
        Self::new_for_tests(
            codex_home,
            Vec::new(),
            LoaderOverrides::without_managed_config_for_tests(),
            CloudRequirementsLoader::default(),
            /*host_name*/ None,
        )
    }
}

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
