use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use codex_utils_absolute_path::AbsolutePathBuf;
use toml::Value as TomlValue;
use tracing::info;
use tracing::warn;

use crate::config::Config;
use crate::config::types::SkillsConfig;
use crate::config_loader::CloudRequirementsLoader;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::load_config_layers_state;
use crate::plugins::PluginsManager;
use crate::skills::SkillLoadOutcome;
use crate::skills::build_implicit_skill_path_indexes;
use crate::skills::config_rules::SkillConfigRules;
use crate::skills::config_rules::resolve_disabled_skill_paths;
use crate::skills::config_rules::skill_config_rules_from_stack;
use crate::skills::loader::SkillRoot;
use crate::skills::loader::load_skills_from_roots;
use crate::skills::loader::skill_roots;
use crate::skills::system::install_system_skills;
use crate::skills::system::uninstall_system_skills;

pub struct SkillsManager {
    codex_home: PathBuf,
    plugins_manager: Arc<PluginsManager>,
    restriction_product: Option<Product>,
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
    cache_by_config: RwLock<HashMap<ConfigSkillsCacheKey, SkillLoadOutcome>>,
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
        let manager = Self {
            codex_home,
            plugins_manager,
            restriction_product,
            cache_by_cwd: RwLock::new(HashMap::new()),
            cache_by_config: RwLock::new(HashMap::new()),
        };
        if !bundled_skills_enabled {
            // The loader caches bundled skills under `skills/.system`. Clearing that directory is
            // best-effort cleanup; root selection still enforces the config even if removal fails.
            uninstall_system_skills(&manager.codex_home);
        } else if let Err(err) = install_system_skills(&manager.codex_home) {
            tracing::error!("failed to install system skills: {err}");
        }
        manager
    }

    /// Load skills for an already-constructed [`Config`], avoiding any additional config-layer
    /// loading.
    ///
    /// This path uses a cache keyed by the effective skill-relevant config state rather than just
    /// cwd so role-local and session-local skill overrides cannot bleed across sessions that happen
    /// to share a directory.
    pub fn skills_for_config(&self, config: &Config) -> SkillLoadOutcome {
        let roots = self.skill_roots_for_config(config);
        let skill_config_rules = skill_config_rules_from_stack(&config.config_layer_stack);
        let cache_key = config_skills_cache_key(&roots, &skill_config_rules);
        if let Some(outcome) = self.cached_outcome_for_config(&cache_key) {
            return outcome;
        }

        let outcome = self.build_skill_outcome(roots, &skill_config_rules);
        let mut cache = self
            .cache_by_config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.insert(cache_key, outcome.clone());
        outcome
    }

    pub(crate) fn skill_roots_for_config(&self, config: &Config) -> Vec<SkillRoot> {
        let loaded_plugins = self.plugins_manager.plugins_for_config(config);
        let mut roots = skill_roots(
            &config.config_layer_stack,
            &config.cwd,
            loaded_plugins.effective_skill_roots(),
        );
        if !config.bundled_skills_enabled() {
            roots.retain(|root| root.scope != SkillScope::System);
        }
        roots
    }

    pub async fn skills_for_cwd(
        &self,
        cwd: &Path,
        config: &Config,
        force_reload: bool,
    ) -> SkillLoadOutcome {
        if !force_reload && let Some(outcome) = self.cached_outcome_for_cwd(cwd) {
            return outcome;
        }

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
        if !force_reload && let Some(outcome) = self.cached_outcome_for_cwd(cwd) {
            return outcome;
        }
        let normalized_extra_user_roots = normalize_extra_user_roots(extra_user_roots);

        let cwd_abs = match AbsolutePathBuf::try_from(cwd) {
            Ok(cwd_abs) => cwd_abs,
            Err(err) => {
                return SkillLoadOutcome {
                    errors: vec![crate::skills::model::SkillError {
                        path: cwd.to_path_buf(),
                        message: err.to_string(),
                    }],
                    ..Default::default()
                };
            }
        };

        let cli_overrides: Vec<(String, TomlValue)> = Vec::new();
        let config_layer_stack = match load_config_layers_state(
            &self.codex_home,
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
                    errors: vec![crate::skills::model::SkillError {
                        path: cwd.to_path_buf(),
                        message: err.to_string(),
                    }],
                    ..Default::default()
                };
            }
        };

        let loaded_plugins = self
            .plugins_manager
            .plugins_for_config_with_force_reload(config, force_reload);
        let mut roots = skill_roots(
            &config_layer_stack,
            cwd,
            loaded_plugins.effective_skill_roots(),
        );
        if !bundled_skills_enabled_from_stack(&config_layer_stack) {
            roots.retain(|root| root.scope != SkillScope::System);
        }
        roots.extend(
            normalized_extra_user_roots
                .iter()
                .cloned()
                .map(|path| SkillRoot {
                    path,
                    scope: SkillScope::User,
                }),
        );
        let skill_config_rules = skill_config_rules_from_stack(&config_layer_stack);
        let outcome = self.build_skill_outcome(roots, &skill_config_rules);
        let mut cache = self
            .cache_by_cwd
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.insert(cwd.to_path_buf(), outcome.clone());
        outcome
    }

    fn build_skill_outcome(
        &self,
        roots: Vec<SkillRoot>,
        skill_config_rules: &SkillConfigRules,
    ) -> SkillLoadOutcome {
        let outcome = crate::skills::filter_skill_load_outcome_for_product(
            load_skills_from_roots(roots),
            self.restriction_product,
        );
        let disabled_paths = resolve_disabled_skill_paths(&outcome.skills, skill_config_rules);
        finalize_skill_outcome(outcome, disabled_paths)
    }

    pub fn clear_cache(&self) {
        let cleared_cwd = {
            let mut cache = self
                .cache_by_cwd
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let cleared = cache.len();
            cache.clear();
            cleared
        };
        let cleared_config = {
            let mut cache = self
                .cache_by_config
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let cleared = cache.len();
            cache.clear();
            cleared
        };
        let cleared = cleared_cwd + cleared_config;
        info!("skills cache cleared ({cleared} entries)");
    }

    fn cached_outcome_for_cwd(&self, cwd: &Path) -> Option<SkillLoadOutcome> {
        match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        }
    }

    fn cached_outcome_for_config(
        &self,
        cache_key: &ConfigSkillsCacheKey,
    ) -> Option<SkillLoadOutcome> {
        match self.cache_by_config.read() {
            Ok(cache) => cache.get(cache_key).cloned(),
            Err(err) => err.into_inner().get(cache_key).cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConfigSkillsCacheKey {
    roots: Vec<(PathBuf, u8)>,
    skill_config_rules: SkillConfigRules,
}

pub(crate) fn bundled_skills_enabled_from_stack(
    config_layer_stack: &crate::config_loader::ConfigLayerStack,
) -> bool {
    let effective_config = config_layer_stack.effective_config();
    let Some(skills_value) = effective_config
        .as_table()
        .and_then(|table| table.get("skills"))
    else {
        return true;
    };

    let skills: SkillsConfig = match skills_value.clone().try_into() {
        Ok(skills) => skills,
        Err(err) => {
            warn!("invalid skills config: {err}");
            return true;
        }
    };

    skills.bundled.unwrap_or_default().enabled
}

fn config_skills_cache_key(
    roots: &[SkillRoot],
    skill_config_rules: &SkillConfigRules,
) -> ConfigSkillsCacheKey {
    ConfigSkillsCacheKey {
        roots: roots
            .iter()
            .map(|root| {
                let scope_rank = match root.scope {
                    SkillScope::Repo => 0,
                    SkillScope::User => 1,
                    SkillScope::System => 2,
                    SkillScope::Admin => 3,
                };
                (root.path.clone(), scope_rank)
            })
            .collect(),
        skill_config_rules: skill_config_rules.clone(),
    }
}

fn finalize_skill_outcome(
    mut outcome: SkillLoadOutcome,
    disabled_paths: HashSet<PathBuf>,
) -> SkillLoadOutcome {
    outcome.disabled_paths = disabled_paths;
    let (by_scripts_dir, by_doc_path) =
        build_implicit_skill_path_indexes(outcome.allowed_skills_for_implicit_invocation());
    outcome.implicit_skills_by_scripts_dir = Arc::new(by_scripts_dir);
    outcome.implicit_skills_by_doc_path = Arc::new(by_doc_path);
    outcome
}

fn normalize_extra_user_roots(extra_user_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut normalized: Vec<PathBuf> = extra_user_roots
        .iter()
        .map(|path| dunce::canonicalize(path).unwrap_or_else(|_| path.clone()))
        .collect();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
