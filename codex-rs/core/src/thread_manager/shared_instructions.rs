//! Keeps one replaceable instruction provider per root while its agent tree is alive.

use codex_extension_api::Instructions;
use codex_extension_api::LoadInstructionsFuture;
use codex_extension_api::LoadedUserInstructions;
use codex_extension_api::ThreadInstructionsProvider;
use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Weak;

#[derive(Default)]
pub(super) struct SharedThreadInstructionsProviders {
    roots: Mutex<HashMap<ThreadId, Weak<SharedThreadInstructionsProvider>>>,
}

impl SharedThreadInstructionsProviders {
    pub(super) fn for_root(
        &self,
        root: ThreadId,
        provider: Option<Arc<dyn ThreadInstructionsProvider>>,
    ) -> Option<Arc<dyn ThreadInstructionsProvider>> {
        let mut roots = self
            .roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        roots.retain(|_, provider| provider.strong_count() > 0);
        let existing = roots.get(&root).and_then(Weak::upgrade);
        if provider
            .as_ref()
            .is_some_and(|provider| !provider.share_with_subagents())
        {
            if let Some(existing) = existing {
                existing.replace(/*provider*/ None);
            }
            return provider;
        }
        match (existing, provider) {
            (Some(existing), provider) => {
                if provider.is_some() {
                    existing.replace(provider);
                }
                Some(existing)
            }
            (None, Some(provider)) => {
                let shared = Arc::new(SharedThreadInstructionsProvider(Mutex::new(
                    ProviderState {
                        provider: Some(provider),
                        last: None,
                    },
                )));
                roots.insert(root, Arc::downgrade(&shared));
                Some(shared)
            }
            (None, None) => None,
        }
    }
}

struct SharedThreadInstructionsProvider(Mutex<ProviderState>);

struct ProviderState {
    provider: Option<Arc<dyn ThreadInstructionsProvider>>,
    last: Option<Instructions>,
}

impl SharedThreadInstructionsProvider {
    fn replace(&self, provider: Option<Arc<dyn ThreadInstructionsProvider>>) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .provider = provider;
    }
}

impl ThreadInstructionsProvider for SharedThreadInstructionsProvider {
    fn share_with_subagents(&self) -> bool {
        true
    }

    fn load_thread_instructions(&self) -> LoadInstructionsFuture<'_> {
        Box::pin(async move {
            let (provider, last) = {
                let state = self
                    .0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                (state.provider.clone(), state.last.clone())
            };
            let Some(provider) = provider else {
                return LoadedUserInstructions {
                    instructions: last,
                    warnings: Vec::new(),
                };
            };
            let loaded = provider.load_thread_instructions().await;
            let mut state = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state
                .provider
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &provider))
            {
                state.last = loaded.instructions.clone();
            }
            loaded
        })
    }
}
