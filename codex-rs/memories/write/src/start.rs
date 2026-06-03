use crate::extensions::seed_extension_instructions;
use crate::guard;
use crate::memory_root;
use crate::metrics::MEMORY_STARTUP;
use crate::phase1;
use crate::phase2;
use crate::runtime::MemoryStartupContext;
use codex_core::CodexThread;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_state::GeneratedMemoryStore;
use std::sync::Arc;
use tracing::warn;

/// Starts the asynchronous startup memory pipeline for an eligible root session.
///
/// The pipeline is skipped for ephemeral sessions, disabled feature flags, and
/// subagent sessions.
pub fn start_memories_startup_task(
    thread_manager: Arc<ThreadManager>,
    auth_manager: Arc<AuthManager>,
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    config: Arc<Config>,
    source: &SessionSource,
) {
    if memories_startup_is_disabled(config.as_ref(), source) {
        return;
    }

    let context = memory_startup_context(
        thread_manager,
        Arc::clone(&auth_manager),
        thread_id,
        thread,
        config.as_ref(),
        source,
    );
    let Some(state_db) = context.state_db() else {
        warn!("state db unavailable for memories startup pipeline; skipping");
        return;
    };
    let store: Arc<dyn GeneratedMemoryStore> = Arc::new(state_db.memories().clone());
    spawn_memories_startup_task(context, auth_manager, config, store);
}

/// Starts startup memory generation with an injected generated-memory store.
pub fn start_memories_startup_task_with_store(
    thread_manager: Arc<ThreadManager>,
    auth_manager: Arc<AuthManager>,
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    config: Arc<Config>,
    source: &SessionSource,
    store: Arc<dyn GeneratedMemoryStore>,
) {
    if memories_startup_is_disabled(config.as_ref(), source) {
        return;
    }

    let context = memory_startup_context(
        thread_manager,
        Arc::clone(&auth_manager),
        thread_id,
        thread,
        config.as_ref(),
        source,
    );
    spawn_memories_startup_task(context, auth_manager, config, store);
}

fn spawn_memories_startup_task(
    context: Arc<MemoryStartupContext>,
    auth_manager: Arc<AuthManager>,
    config: Arc<Config>,
    store: Arc<dyn GeneratedMemoryStore>,
) {
    tokio::spawn(async move {
        let root = memory_root(&config.codex_home);
        if let Err(err) = tokio::fs::create_dir_all(&root).await {
            warn!("failed creating memories root: {err}");
            return;
        }
        if let Err(err) = seed_extension_instructions(&root).await {
            warn!("failed seeding memory extension instructions: {err}");
        }

        // Clean memories to make preserve DB size. This does not consume tokens so can be
        // done before the quota check.
        phase1::prune(store.as_ref(), &config).await;

        if !guard::rate_limits_ok(&auth_manager, &config).await {
            context.counter(
                MEMORY_STARTUP,
                /*inc*/ 1,
                &[("status", "skipped_rate_limit")],
            );
            return;
        }

        // Run phase 1.
        phase1::run(
            Arc::clone(&context),
            Arc::clone(&config),
            Arc::clone(&store),
        )
        .await;
        // Run phase 2.
        phase2::run(context, config, store).await;
    });
}

fn memory_startup_context(
    thread_manager: Arc<ThreadManager>,
    auth_manager: Arc<AuthManager>,
    thread_id: ThreadId,
    thread: Arc<CodexThread>,
    config: &Config,
    source: &SessionSource,
) -> Arc<MemoryStartupContext> {
    Arc::new(MemoryStartupContext::new(
        thread_manager,
        auth_manager,
        thread_id,
        thread,
        config,
        source.clone(),
    ))
}

fn memories_startup_is_disabled(config: &Config, source: &SessionSource) -> bool {
    config.ephemeral || !config.features.enabled(Feature::MemoryTool) || source.is_non_root_agent()
}
