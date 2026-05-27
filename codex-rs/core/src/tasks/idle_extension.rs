use std::sync::Arc;

use crate::session::session::Session;

pub(super) fn schedule_turn(session: &Arc<Session>) {
    if session
        .services
        .extensions
        .idle_turn_contributors()
        .is_empty()
    {
        return;
    }

    let session = Arc::clone(session);
    let _handle = tokio::spawn(async move {
        session.maybe_start_idle_extension_turn().await;
    });
}
