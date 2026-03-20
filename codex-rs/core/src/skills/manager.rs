use crate::config::Config;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::load_config_layers_state;
use crate::plugins::PluginsManager;
use crate::skills::SkillError;
use crate::skills::SkillLoadOutcome;
use crate::skills::loader::SkillRoot;
use codex_capabilities::skills::SkillLoadRequest;
use codex_protocol::protocol::Product;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use toml::Value as TomlValue;

pub struct SkillsManager {
    plugins_manager: Arc<PluginsManager>,
    inner: codex_capabilities::skills::SkillsManager,
}

impl SkillsManager {
    pub fn new(
        codex_home: PathBuf,
        plugins_manager: Arc<PluginsManager>,
        bundled_skills_enabled: bool,
    ) -> Self {
        Self::new_with_restriction_product(
            codex_home,
            plugins_manager,
            bundled_skills_enabled,
            Some(Product::Codex),
        )
    }

    pub fn new_with_restriction_product(
        codex_home: PathBuf,
        plugins_manager: Arc<PluginsManager>,
        bundled_skills_enabled: bool,
        restriction_product: Option<Product>,
    ) -> Self {
        Self {
            plugins_manager,
            inner: codex_capabilities::skills::SkillsManager::new_with_restriction_product(
                codex_home,
                bundled_skills_enabled,
                restriction_product,
            ),
        }
    }

    pub fn skills_for_config(&self, config: &Config) -> SkillLoadOutcome {
        self.inner.skills_for_request(
            &self.skill_load_request(
                config.cwd.clone(),
                config.config_layer_stack.clone(),
                self.plugins_manager
                    .plugins_for_config(config)
                    .effective_skill_roots(),
                Vec::new(),
            ),
        )
    }

    pub(crate) fn skill_roots_for_config(&self, config: &Config) -> Vec<SkillRoot> {
        self.inner.skill_roots_for_request(
            &self.skill_load_request(
                config.cwd.clone(),
                config.config_layer_stack.clone(),
                self.plugins_manager
                    .plugins_for_config(config)
                    .effective_skill_roots(),
                Vec::new(),
            ),
        )
    }

    pub async fn skills_for_cwd(
        &self,
        cwd: &Path,
        config: &Config,
        force_reload: bool,
    ) -> SkillLoadOutcome {
        self.skills_for_cwd_with_extra_user_roots(cwd, config, force_reload, &[])
            .await
    }

    pub async fn skills_for_cwd_with_extra_user_roots(
        &self,
        cwd: &Path,
        config: &Config,
        force_reload: bool,
        extra_user_roots: &[PathBuf],
    ) -> SkillLoadOutcome {
        let cwd_abs = match AbsolutePathBuf::try_from(cwd) {
            Ok(cwd_abs) => cwd_abs,
            Err(err) => {
                return SkillLoadOutcome {
                    errors: vec![SkillError {
                        path: cwd.to_path_buf(),
                        message: err.to_string(),
                    }],
                    ..Default::default()
                };
            }
        };

        let cli_overrides: Vec<(String, TomlValue)> = Vec::new();
        let config_layer_stack = match load_config_layers_state(
            &config.codex_home,
            Some(cwd_abs),
            &cli_overrides,
            LoaderOverrides::default(),
            CloudRequirementsLoader::default(),
        )
        .await
        {
            Ok(config_layer_stack) => config_layer_stack,
            Err(err) => {
                return SkillLoadOutcome {
                    errors: vec![SkillError {
                        path: cwd.to_path_buf(),
                        message: err.to_string(),
                    }],
                    ..Default::default()
                };
            }
        };

        let plugin_skill_roots = self
            .plugins_manager
            .plugins_for_config_with_force_reload(config, force_reload)
            .effective_skill_roots();
        self.inner.skills_for_cwd_request(
            &self.skill_load_request(
                cwd.to_path_buf(),
                config_layer_stack,
                plugin_skill_roots,
                extra_user_roots.to_vec(),
            ),
            force_reload,
        )
    }

    pub fn clear_cache(&self) {
        self.inner.clear_cache();
    }

    fn skill_load_request(
        &self,
        cwd: PathBuf,
        config_layer_stack: crate::config_loader::ConfigLayerStack,
        plugin_skill_roots: Vec<PathBuf>,
        extra_user_roots: Vec<PathBuf>,
    ) -> SkillLoadRequest {
        let bundled_skills_enabled =
            codex_capabilities::skills::bundled_skills_enabled_from_stack(&config_layer_stack);
        SkillLoadRequest {
            cwd,
            config_layer_stack,
            bundled_skills_enabled,
            plugin_skill_roots,
            extra_user_roots,
        }
    }
}

pub(crate) fn bundled_skills_enabled_from_stack(
    config_layer_stack: &crate::config_loader::ConfigLayerStack,
) -> bool {
    codex_capabilities::skills::bundled_skills_enabled_from_stack(config_layer_stack)
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
