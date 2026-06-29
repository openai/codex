use crate::agents_md::LoadedAgentsMd;
use crate::agents_md::load_project_instructions_with_roots;
use crate::config::Config;
use crate::environment_selection::TurnEnvironmentSnapshot;
use codex_extension_api::UserInstructions;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_path_uri::PathUri;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Owns the inputs and cached result of AGENTS.md discovery for a session.
pub(crate) struct AgentsMdManager {
    user_instructions: Option<UserInstructions>,
    cache: Mutex<AgentsMdCache>,
}

#[derive(Default)]
struct AgentsMdCache {
    selections: Option<Vec<TurnEnvironmentSelection>>,
    loaded: Option<Arc<LoadedAgentsMd>>,
}

impl AgentsMdManager {
    pub(crate) fn new(user_instructions: Option<UserInstructions>) -> Self {
        Self {
            user_instructions: user_instructions
                .filter(|instructions| !instructions.text.trim().is_empty()),
            cache: Mutex::new(AgentsMdCache::default()),
        }
    }

    pub(crate) async fn refresh(
        &self,
        config: &Config,
        environments: &TurnEnvironmentSnapshot,
    ) -> Option<PathUri> {
        let selections = environments.to_selections();
        if self.cache.lock().await.selections.as_ref() == Some(&selections) {
            return None;
        }

        let mut outcome = load_project_instructions_with_roots(
            config,
            self.user_instructions.clone(),
            environments,
        )
        .await;
        let primary_project_root = environments
            .primary()
            .and_then(|primary| outcome.project_roots.remove(&primary.environment_id));
        let mut cache = self.cache.lock().await;
        cache.selections = Some(selections);
        cache.loaded = outcome.loaded.map(Arc::new);
        primary_project_root
    }

    pub(crate) async fn get_loaded(&self) -> Option<Arc<LoadedAgentsMd>> {
        self.cache.lock().await.loaded.clone()
    }

    pub(crate) fn user_instructions(&self) -> Option<UserInstructions> {
        self.user_instructions.clone()
    }
}
