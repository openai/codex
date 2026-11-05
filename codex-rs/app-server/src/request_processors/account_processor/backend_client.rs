//! Build account API clients from the latest config and captured account authority.

use super::*;
use futures::future::BoxFuture;

impl AccountRequestProcessor {
    // Keep config loading and runtime bootstrap out of request-handler future types.
    #[inline(never)]
    pub(super) fn backend_client<'a>(
        &'a self,
        auth: &'a CodexAuth,
        http_client_factory: codex_http_client::HttpClientFactory,
    ) -> BoxFuture<'a, Result<BackendClient, JSONRPCErrorError>> {
        Box::pin(async move {
            let config = self.load_latest_config().await;
            let runtime = self
                .thread_manager
                .runtime_auth_for_config(&config)
                .map_err(|error| internal_error(error.to_string()))?;
            BackendClient::from_auth_with_runtime(
                config.chatgpt_base_url.clone(),
                auth,
                http_client_factory,
                runtime.as_ref(),
                /*headers*/ &Default::default(),
            )
            .await
            .map_err(|error| internal_error(error.to_string()))
        })
    }
}
