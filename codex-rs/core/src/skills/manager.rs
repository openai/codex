use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::skills::SkillLoadOutcome;
use crate::skills::loader::load_skills_from_roots;
use crate::skills::loader::repo_skills_root;
use crate::skills::loader::user_skills_root;

pub struct SkillsManager {
    codex_home: PathBuf,
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
}

impl SkillsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            codex_home,
            cache_by_cwd: RwLock::new(HashMap::new()),
        }
    }

    pub fn skills_for_cwd(&self, cwd: &Path) -> SkillLoadOutcome {
        let cached = match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        };
        if let Some(outcome) = cached {
            return outcome;
        }

        let mut roots = vec![user_skills_root(&self.codex_home)];
        if let Some(repo_root) = repo_skills_root(cwd) {
            roots.push(repo_root);
        }
        let outcome = load_skills_from_roots(roots);
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
}
