//! Explicit ThreadManager host for core's context-adapter unit tests.
//! Application-level tests install the real Guardian extension instead.

use std::sync::Arc;

use codex_home::CodexHomeUserInstructionsProvider;
use codex_protocol::protocol::SessionSource;

use crate::config::Config;
use crate::session::session::Session;

pub(crate) fn install(session: &Session, config: &Config) {
    let manager = Arc::new(crate::ThreadManager::new(
        config,
        Arc::clone(&session.services.auth_manager),
        Arc::clone(&session.services.models_manager),
        crate::CodexAppsToolsCache::default(),
        SessionSource::Exec,
        session.services.turn_environments.environment_manager(),
        codex_extension_api::empty_extension_registry(),
        Arc::new(CodexHomeUserInstructionsProvider::new(
            config.codex_home.clone(),
        )),
        /*analytics_events_client*/ None,
        crate::passthrough_image_store(),
        Arc::clone(&session.services.thread_store),
        /*agent_graph_store*/ None,
        session.installation_id.clone(),
        /*attestation_provider*/ None,
        /*external_time_provider*/ None,
    ));
    let host = super::GuardianReviewSessionHost::with_thread_manager(Arc::downgrade(&manager));
    session.services.thread_extension_data.insert(host);
    session.services.thread_extension_data.insert(manager);
}
