use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;

use codex_app_server_protocol::ConfigLayerSource;
use codex_config::ConfigLayerStack;
use codex_config::ConfigLayerStackOrdering;
use codex_protocol::protocol::Product;
use codex_protocol::protocol::SkillScope;
use tracing::info;
use tracing::warn;

use crate::config_types::SkillsConfig;
use crate::skills::loader::SkillRoot;
use crate::skills::loader::load_skills_from_roots;
use crate::skills::loader::skill_roots;
use crate::skills::model::SkillLoadOutcome;
use crate::skills::model::SkillMetadata;
use crate::skills::runtime::SkillLoadRequest;
use crate::skills::system::install_system_skills;
use crate::skills::system::uninstall_system_skills;

pub struct SkillsManager {
    restriction_product: Option<Product>,
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
    cache_by_config: RwLock<HashMap<ConfigSkillsCacheKey, SkillLoadOutcome>>,
}

impl SkillsManager {
    pub fn new(codex_home: PathBuf, bundled_skills_enabled: bool) -> Self {
        Self::new_with_restriction_product(codex_home, bundled_skills_enabled, Some(Product::Codex))
    }

    pub fn new_with_restriction_product(
        codex_home: PathBuf,
        bundled_skills_enabled: bool,
        restriction_product: Option<Product>,
    ) -> Self {
        let manager = Self {
            restriction_product,
            cache_by_cwd: RwLock::new(HashMap::new()),
            cache_by_config: RwLock::new(HashMap::new()),
        };
        if !bundled_skills_enabled {
            uninstall_system_skills(&codex_home);
        } else if let Err(err) = install_system_skills(&codex_home) {
            tracing::error!("failed to install system skills: {err}");
        }
        manager
    }

    pub fn skills_for_request(&self, request: &SkillLoadRequest) -> SkillLoadOutcome {
        let roots = self.skill_roots_for_request(request);
        let cache_key = config_skills_cache_key(&roots, &request.config_layer_stack);
        if let Some(outcome) = self.cached_outcome_for_config(&cache_key) {
            return outcome;
        }

        let outcome = self.build_skill_outcome(roots, &request.config_layer_stack);
        let mut cache = self
            .cache_by_config
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.insert(cache_key, outcome.clone());
        outcome
    }

    pub fn skill_roots_for_request(&self, request: &SkillLoadRequest) -> Vec<SkillRoot> {
        let mut roots = skill_roots(
            &request.config_layer_stack,
            &request.cwd,
            request.plugin_skill_roots.clone(),
        );
        if !request.bundled_skills_enabled {
            roots.retain(|root| root.scope != SkillScope::System);
        }
        roots.extend(
            normalize_extra_user_roots(&request.extra_user_roots)
                .into_iter()
                .map(|path| SkillRoot {
                    path,
                    scope: SkillScope::User,
                }),
        );
        roots
    }

    pub fn skills_for_cwd_request(
        &self,
        request: &SkillLoadRequest,
        force_reload: bool,
    ) -> SkillLoadOutcome {
        if !force_reload && let Some(outcome) = self.cached_outcome_for_cwd(&request.cwd) {
            return outcome;
        }

        let outcome = self.build_skill_outcome(
            self.skill_roots_for_request(request),
            &request.config_layer_stack,
        );
        let mut cache = self
            .cache_by_cwd
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.insert(request.cwd.clone(), outcome.clone());
        outcome
    }

    fn build_skill_outcome(
        &self,
        roots: Vec<SkillRoot>,
        config_layer_stack: &ConfigLayerStack,
    ) -> SkillLoadOutcome {
        crate::skills::filter_skill_load_outcome_for_product(
            finalize_skill_outcome(load_skills_from_roots(roots), config_layer_stack),
            self.restriction_product,
        )
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
    disabled_paths: Vec<PathBuf>,
}

pub fn bundled_skills_enabled_from_stack(config_layer_stack: &ConfigLayerStack) -> bool {
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

fn disabled_paths_from_stack(config_layer_stack: &ConfigLayerStack) -> HashSet<PathBuf> {
    let mut configs = HashMap::new();
    for layer in config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ true,
    ) {
        if !matches!(
            layer.name,
            ConfigLayerSource::User { .. } | ConfigLayerSource::SessionFlags
        ) {
            continue;
        }

        let Some(skills_value) = layer.config.get("skills") else {
            continue;
        };
        let skills: SkillsConfig = match skills_value.clone().try_into() {
            Ok(skills) => skills,
            Err(err) => {
                warn!("invalid skills config: {err}");
                continue;
            }
        };

        for entry in skills.config {
            let path = normalize_override_path(entry.path.as_path());
            configs.insert(path, entry.enabled);
        }
    }

    configs
        .into_iter()
        .filter_map(|(path, enabled)| (!enabled).then_some(path))
        .collect()
}

fn config_skills_cache_key(
    roots: &[SkillRoot],
    config_layer_stack: &ConfigLayerStack,
) -> ConfigSkillsCacheKey {
    let mut disabled_paths: Vec<PathBuf> = disabled_paths_from_stack(config_layer_stack)
        .into_iter()
        .collect();
    disabled_paths.sort_unstable();

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
        disabled_paths,
    }
}

fn finalize_skill_outcome(
    mut outcome: SkillLoadOutcome,
    config_layer_stack: &ConfigLayerStack,
) -> SkillLoadOutcome {
    outcome.disabled_paths = disabled_paths_from_stack(config_layer_stack);
    let (by_scripts_dir, by_doc_path) =
        build_implicit_skill_path_indexes(outcome.allowed_skills_for_implicit_invocation());
    outcome.implicit_skills_by_scripts_dir = Arc::new(by_scripts_dir);
    outcome.implicit_skills_by_doc_path = Arc::new(by_doc_path);
    outcome
}

fn build_implicit_skill_path_indexes(
    skills: Vec<SkillMetadata>,
) -> (
    HashMap<PathBuf, SkillMetadata>,
    HashMap<PathBuf, SkillMetadata>,
) {
    let mut by_scripts_dir = HashMap::new();
    let mut by_skill_doc_path = HashMap::new();

    for skill in skills {
        let skill_doc_path = normalize_override_path(skill.path_to_skills_md.as_path());
        by_skill_doc_path.insert(skill_doc_path, skill.clone());

        if let Some(skill_dir) = skill.path_to_skills_md.parent() {
            let scripts_dir = normalize_override_path(&skill_dir.join("scripts"));
            by_scripts_dir.insert(scripts_dir, skill);
        }
    }

    (by_scripts_dir, by_skill_doc_path)
}

fn normalize_override_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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
#[path = "manager_request_tests.rs"]
mod tests;
