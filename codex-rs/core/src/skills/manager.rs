use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::skills::SkillLoadOutcome;
use crate::skills::loader::load_skills_from_roots;
use crate::skills::loader::public_skills_root;
use crate::skills::loader::repo_skills_root;
use crate::skills::loader::user_skills_root;
use crate::skills::public::refresh_public_skills_blocking;

pub struct SkillsManager {
    codex_home: PathBuf,
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
    attempted_public_refresh: AtomicBool,
}

impl SkillsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            codex_home,
            cache_by_cwd: RwLock::new(HashMap::new()),
            attempted_public_refresh: AtomicBool::new(false),
        }
    }

    pub fn skills_for_cwd(&self, cwd: &Path) -> SkillLoadOutcome {
        // Best-effort refresh: attempt at most once per manager instance.
        if self
            .attempted_public_refresh
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
            && let Err(err) = refresh_public_skills_blocking(&self.codex_home)
        {
            tracing::error!("failed to refresh public skills: {err:#}");
        }

        let cached = match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        };
        if let Some(outcome) = cached {
            return outcome;
        }

        let mut roots = Vec::new();
        if let Some(repo_root) = repo_skills_root(cwd) {
            roots.push(repo_root);
        }
        roots.push(user_skills_root(&self.codex_home));
        roots.push(public_skills_root(&self.codex_home));
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
