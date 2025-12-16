use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::skills::SkillLoadOutcome;
use crate::skills::loader::load_skills_from_roots;
use crate::skills::loader::public_skills_root;
use crate::skills::loader::repo_skills_root;
use crate::skills::loader::user_skills_root;
use crate::skills::public::refresh_public_skills;
use async_channel::Sender;

pub struct SkillsManager {
    codex_home: PathBuf,
    cache_by_cwd: RwLock<HashMap<PathBuf, SkillLoadOutcome>>,
    attempted_public_refresh: AtomicBool,
    skills_update_listeners: Arc<Mutex<Vec<Sender<Event>>>>,
}

impl SkillsManager {
    pub fn new(codex_home: PathBuf) -> Self {
        Self {
            codex_home,
            cache_by_cwd: RwLock::new(HashMap::new()),
            attempted_public_refresh: AtomicBool::new(false),
            skills_update_listeners: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn register_skills_update_listener(&self, tx_event: Sender<Event>) {
        if let Ok(mut listeners) = self.skills_update_listeners.lock() {
            listeners.push(tx_event);
        }
    }

    pub fn skills_for_cwd(&self, cwd: &Path) -> SkillLoadOutcome {
        self.skills_for_cwd_with_options(cwd, false)
    }

    pub(crate) fn skills_for_cwd_with_options(
        &self,
        cwd: &Path,
        force_reload: bool,
    ) -> SkillLoadOutcome {
        // Best-effort refresh: attempt at most once per manager instance.
        if self
            .attempted_public_refresh
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let codex_home = self.codex_home.clone();
            let skills_update_listeners = Arc::clone(&self.skills_update_listeners);
            std::thread::spawn(move || match refresh_public_skills(&codex_home) {
                Ok(outcome) => {
                    if outcome.updated()
                        && let Ok(mut listeners) = skills_update_listeners.lock()
                    {
                        listeners.retain(|tx| {
                            tx.try_send(Event {
                                id: "".to_owned(),
                                msg: EventMsg::SkillsUpdateAvailable,
                            })
                            .is_ok()
                        });
                    }
                }
                Err(err) => {
                    tracing::error!("failed to refresh public skills: {err}");
                }
            });
        }

        let cached = match self.cache_by_cwd.read() {
            Ok(cache) => cache.get(cwd).cloned(),
            Err(err) => err.into_inner().get(cwd).cloned(),
        };
        if !force_reload && let Some(outcome) = cached {
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
