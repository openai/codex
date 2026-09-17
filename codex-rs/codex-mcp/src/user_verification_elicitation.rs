//! Routes device verification directly to the app, outside automated approval policy.

use super::*;

pub(super) async fn route(
    router: ElicitationRequestRouter,
    events: Option<Sender<Event>>,
    authority: Option<ElicitationAuthority>,
    server_name: String,
    request: ElicitationRequest,
) -> Result<ElicitationResponse> {
    let plugin_service = authority.as_ref().is_some_and(|authority| {
        authority
            .config
            .mcp_server_catalog
            .server(&server_name)
            .is_some_and(|server| {
                server
                    .source()
                    .is_host_owned_apps(&server_name, server.config())
            })
    });
    let Some((events, authority)) = events
        .zip(authority)
        .filter(|_| plugin_service && !router.auto_deny())
    else {
        return Ok(ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: None,
        });
    };
    router
        .request_user_interaction(Some(events), &authority, server_name, request)
        .await
}
