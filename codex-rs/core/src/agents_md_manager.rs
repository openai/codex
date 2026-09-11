//! Resolves global, thread, and repository instructions into one validated snapshot.
//! Refreshes are serialized; the state lock is never held while calling providers.

use crate::agents_md::LoadedAgentsMd;
use crate::agents_md::load_project_instructions;
use crate::config::Config;
use crate::environment_selection::TurnEnvironmentSnapshot;
use codex_extension_api::Instructions;
use codex_extension_api::ThreadInstructionsProvider;
use codex_extension_api::UserInstructionsProvider;
use codex_protocol::config_types::TrustLevel;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::TurnEnvironmentSelection;
use codex_utils_string::approx_bytes_for_tokens;
use codex_utils_string::approx_tokens_from_byte_count;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;

/// Owns instruction sources, refresh serialization, and the applied snapshot.
pub(crate) struct AgentsMdManager {
    refresh_lock: Semaphore,
    state: Mutex<AgentsMdState>,
}

/// Root runtimes retain providers; subagents inherit only their applied snapshots.
#[derive(Clone, Default)]
pub(crate) struct SessionInstructions {
    pub(crate) user: Option<Instructions>,
    pub(crate) thread: Option<Instructions>,
    pub(crate) user_provider: Option<Arc<dyn UserInstructionsProvider>>,
    pub(crate) thread_provider: Option<Arc<dyn ThreadInstructionsProvider>>,
}

struct AgentsMdState {
    instructions: SessionInstructions,
    cache: AgentsMdCache,
}

#[derive(Default)]
struct AgentsMdCache {
    selections: Option<Vec<TurnEnvironmentSelection>>,
    active_project_trust_level: Option<TrustLevel>,
    loaded: Option<Arc<LoadedAgentsMd>>,
}

impl AgentsMdManager {
    pub(crate) fn new(mut instructions: SessionInstructions) -> Self {
        instructions.user = normalize_instructions(instructions.user);
        instructions.thread = normalize_instructions(instructions.thread);
        Self {
            refresh_lock: Semaphore::new(/*permits*/ 1),
            state: Mutex::new(AgentsMdState {
                instructions,
                cache: AgentsMdCache::default(),
            }),
        }
    }

    /// Resolves and validates one coherent snapshot for startup or a request boundary.
    /// Providers own fetching and caching. Warnings survive validation failures;
    /// only validated snapshots replace the applied host instructions.
    #[tracing::instrument(name = "agents_md.refresh", skip_all)]
    pub(crate) async fn refresh(
        &self,
        config: &Config,
        environments: &TurnEnvironmentSnapshot,
    ) -> (CodexResult<Option<Arc<LoadedAgentsMd>>>, Vec<String>) {
        // Serialize overlapping captures without blocking reads of the applied snapshot.
        let Ok(_refresh_guard) = self.refresh_lock.acquire().await else {
            return (
                Err(CodexErr::Fatal(
                    "instruction refresh semaphore closed".to_string(),
                )),
                Vec::new(),
            );
        };
        let selections = environments
            .turn_environments()
            .map(|environment| environment.selection.clone())
            .collect::<Vec<_>>();
        let active_project_trust_level = config.active_project.trust_level;
        let (mut instructions, cached, refresh_repository) = {
            let mut state = self.state.lock().await;
            let refresh_repository = state.cache.selections.as_ref() != Some(&selections)
                || state.cache.active_project_trust_level != active_project_trust_level;
            if refresh_repository {
                // Tightened read permissions must not leave inaccessible instructions visible,
                // even if discovery fails or the caller cancels the refresh.
                state.cache = AgentsMdCache::default();
            }
            (
                state.instructions.clone(),
                state.cache.loaded.clone(),
                refresh_repository,
            )
        };
        let mut warnings = Vec::new();
        if let Some(provider) = &instructions.user_provider {
            let loaded = provider.load_user_instructions().await;
            instructions.user = normalize_instructions(loaded.instructions);
            warnings.extend(loaded.warnings);
        }
        if let Some(provider) = &instructions.thread_provider {
            let loaded = provider.load_thread_instructions().await;
            instructions.thread = normalize_instructions(loaded.instructions);
            warnings.extend(loaded.warnings);
        }

        let result = async {
            if let Some(input) = &instructions.thread {
                validate_thread_instruction_size(input.text.len())?;
            }
            if !refresh_repository {
                let state = self.state.lock().await;
                if state.instructions.user == instructions.user
                    && state.instructions.thread == instructions.thread
                {
                    return Ok(cached);
                }
            }

            let loaded = if refresh_repository {
                load_project_instructions(config, /*user_instructions*/ None, environments)
                    .await?
                    .unwrap_or_default()
            } else {
                cached.as_deref().cloned().unwrap_or_default()
            }
            .with_instructions(instructions.user.clone(), instructions.thread.clone())
            .map(Arc::new);
            let mut state = self.state.lock().await;
            state.instructions = instructions;
            state.cache = AgentsMdCache {
                selections: Some(selections),
                active_project_trust_level,
                loaded: loaded.clone(),
            };
            Ok(loaded)
        }
        .await;
        (result, warnings)
    }

    pub(crate) async fn get_loaded(&self) -> Option<Arc<LoadedAgentsMd>> {
        self.state.lock().await.cache.loaded.clone()
    }

    pub(crate) async fn inherited_instructions(&self) -> SessionInstructions {
        let state = self.state.lock().await;
        SessionInstructions {
            user: state.instructions.user.clone(),
            thread: state.instructions.thread.clone(),
            ..Default::default()
        }
    }
}

// Bound the new host-provided contribution independently of project_doc_max_bytes,
// which controls repository discovery. Existing global and combined instruction
// size policy is unchanged; reject oversized thread input rather than truncate it.
const MAX_THREAD_INSTRUCTIONS_TOKENS: usize = 10_000;

fn validate_thread_instruction_size(bytes: usize) -> CodexResult<()> {
    if bytes > approx_bytes_for_tokens(MAX_THREAD_INSTRUCTIONS_TOKENS) {
        let estimated_tokens = approx_tokens_from_byte_count(bytes);
        return Err(CodexErr::InvalidRequest(format!(
            "thread instructions exceed the limit of {MAX_THREAD_INSTRUCTIONS_TOKENS} estimated tokens ({estimated_tokens} estimated tokens provided)"
        )));
    }
    Ok(())
}

fn normalize_instructions(instructions: Option<Instructions>) -> Option<Instructions> {
    instructions.filter(|instructions| !instructions.text.trim().is_empty())
}
