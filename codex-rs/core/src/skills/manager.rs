use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;

use codex_utils_absolute_path::AbsolutePathBuf;
use toml::Value as TomlValue;
use tracing::warn;

use crate::config::Config;
use crate::config::types::SkillsConfig;
use crate::config_loader::LoaderOverrides;
use crate::config_loader::load_config_layers_state;
use crate::skills::SkillLoadOutcome;
use crate::skills::loader::load_skills_from_roots;
use crate::skills::loader::skill_roots_from_layer_stack;
use crate::skills::system::install_system_skills;

pub struct SkillsManager {
    codex_home: PathBuf,
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
}

impl SkillsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        if let Err(err) = install_system_skills(&codex_home) {
            tracing::error!("failed to install system skills: {err}");
        }

        Self {
            codex_home,
            cache_by_cwd: RwLock::new(HashMap::new()),
        }
    }

    /// Load skills for an already-constructed [`Config`], avoiding any additional config-layer
    /// loading. This also seeds the per-cwd cache for subsequent lookups.
    pub fn skills_for_config(&self, config: &Config) -> SkillLoadOutcome {
        let cwd = &config.cwd;
        let cached = match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        };
        if let Some(outcome) = cached {
            return outcome;
        }

        let roots = skill_roots_from_layer_stack(&config.config_layer_stack);
        let mut outcome = load_skills_from_roots(roots);
        outcome.disabled_paths = disabled_paths_from_stack(&config.config_layer_stack);
        match self.cache_by_cwd.write() {
            Ok(mut cache) => {
                cache.insert(cwd.to_path_buf(), outcome.clone());
            }
            Err(err) => {
                err.into_inner().insert(cwd.to_path_buf(), outcome.clone());
            }
        }
        outcome
    }

    pub async fn skills_for_cwd(&self, cwd: &Path, force_reload: bool) -> SkillLoadOutcome {
        let cached = match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        };
        if !force_reload && let Some(outcome) = cached {
            return outcome;
        }

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

        let roots = skill_roots_from_layer_stack(&config_layer_stack);
        let mut outcome = load_skills_from_roots(roots);
        outcome.disabled_paths = disabled_paths_from_stack(&config_layer_stack);
        match self.cache_by_cwd.write() {
            Ok(mut cache) => {
                cache.insert(cwd.to_path_buf(), outcome.clone());
            }
            Err(err) => {
                err.into_inner().insert(cwd.to_path_buf(), outcome.clone());
            }
        }
        outcome
    }

    pub fn clear_cache(&self) {
        match self.cache_by_cwd.write() {
            Ok(mut cache) => cache.clear(),
            Err(err) => err.into_inner().clear(),
        }
    }
}

fn disabled_paths_from_stack(
    config_layer_stack: &crate::config_loader::ConfigLayerStack,
) -> HashSet<PathBuf> {
    let mut disabled = HashSet::new();
    let mut configs = HashMap::new();
    // Skills config is user-layer only for now; higher-precedence layers are ignored.
    let Some(user_layer) = config_layer_stack.get_user_layer() else {
        return disabled;
    };
    let Some(skills_value) = user_layer.config.get("skills") else {
        return disabled;
    };
    let skills: SkillsConfig = match skills_value.clone().try_into() {
        Ok(skills) => skills,
        Err(err) => {
            warn!("invalid skills config: {err}");
            return disabled;
        }
    };

    for entry in skills.config {
        let path = normalize_override_path(entry.path.as_path());
        configs.insert(path, entry.enabled);
    }

    for (path, enabled) in configs {
        if !enabled {
            disabled.insert(path);
        }
    }

    disabled
}

fn normalize_override_path(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use crate::config::ConfigOverrides;
    use pretty_assertions::assert_eq;
    use std::fs;
    use tempfile::TempDir;

    fn write_user_skill(codex_home: &TempDir, dir: &str, name: &str, description: &str) {
        let skill_dir = codex_home.path().join("skills").join(dir);
        fs::create_dir_all(&skill_dir).unwrap();
        let content = format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n");
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }

    #[tokio::test]
    async fn skills_for_config_seeds_cache_by_cwd() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let cwd = tempfile::tempdir().expect("tempdir");

        let cfg = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .harness_overrides(ConfigOverrides {
                cwd: Some(cwd.path().to_path_buf()),
                ..Default::default()
            })
            .build()
            .await
            .expect("defaults for test should always succeed");

        let skills_manager = SkillsManager::new(codex_home.path().to_path_buf());

        write_user_skill(&codex_home, "a", "skill-a", "from a");
        let outcome1 = skills_manager.skills_for_config(&cfg);
        assert!(
            outcome1.skills.iter().any(|s| s.name == "skill-a"),
            "expected skill-a to be discovered"
        );

        // Write a new skill after the first call; the second call should hit the cache and not
        // reflect the new file.
        write_user_skill(&codex_home, "b", "skill-b", "from b");
        let outcome2 = skills_manager.skills_for_config(&cfg);
        assert_eq!(outcome2.errors, outcome1.errors);
        assert_eq!(outcome2.skills, outcome1.skills);
    }
}
