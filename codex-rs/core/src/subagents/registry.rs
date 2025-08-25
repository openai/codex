use super::default_agents;
use super::definition::SubagentDefinition;
use super::definition::SubagentSource;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Default, Clone)]
pub struct SubagentRegistry {
    /// Directory under the project (cwd/.codex/agents).
    project_dir: Option<PathBuf>,
    /// Directory under CODEX_HOME (~/.codex/agents).
    user_dir: Option<PathBuf>,
    /// Merged map: project definitions override user ones.
    map: HashMap<String, SubagentDefinition>,
}

impl SubagentRegistry {
    pub fn new(project_dir: Option<PathBuf>, user_dir: Option<PathBuf>) -> Self {
        Self {
            project_dir,
            user_dir,
            map: HashMap::new(),
        }
    }

    /// Loads JSON files from user_dir then project_dir (project wins on conflict).
    pub fn load(&mut self) {
        let mut map: HashMap<String, SubagentDefinition> = HashMap::new();

        // Start with embedded defaults (lowest precedence).
        for def in default_agents::embedded_defs() {
            map.insert(def.name.clone(), def);
        }

        // Load user definitions first
        if let Some(dir) = &self.user_dir {
            Self::load_from_dir_into(dir, &mut map, SubagentSource::User);
        }
        // Then load project definitions which override on conflicts
        if let Some(dir) = &self.project_dir {
            Self::load_from_dir_into(dir, &mut map, SubagentSource::Project);
        }

        // No ad-hoc fallback here; embedded defaults already include "hello".

        self.map = map;
    }

    pub fn get(&self, name: &str) -> Option<&SubagentDefinition> {
        self.map.get(name)
    }

    pub fn all_names(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }

    fn load_from_dir_into(
        dir: &Path,
        out: &mut HashMap<String, SubagentDefinition>,
        source: SubagentSource,
    ) {
        let Ok(iter) = fs::read_dir(dir) else {
            return;
        };
        for entry in iter.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            {
                match SubagentDefinition::from_file(&path) {
                    Ok(mut def) => {
                        def.source = source;
                        out.insert(def.name.clone(), def);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to load subagent from {}: {}", path.display(), e);
                    }
                }
            }
        }
    }
}
