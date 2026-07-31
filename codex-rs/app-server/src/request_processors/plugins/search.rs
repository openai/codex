use super::*;
use crate::error_code::method_not_found;
use codex_app_server_protocol::PluginSearchParams;

impl PluginRequestProcessor {
    pub(crate) async fn plugin_search(
        &self,
        _params: PluginSearchParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        Err(method_not_found("plugin/search is not implemented"))
    }
}
