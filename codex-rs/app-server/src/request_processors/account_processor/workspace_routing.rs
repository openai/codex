//! Discovers routing for the selected ChatGPT workspace without crossing auth owners.
//! Credential refreshes invalidate the cache, but do not cancel same-owner discovery.

use super::*;
use codex_app_server_protocol::AccountRoutingOverride;
use codex_app_server_protocol::WorkspaceRouting;
use codex_backend_client::AccountEntry;
use url::Url;

pub(super) struct CachedWorkspaceRouting {
    auth_generation: u64,
    effective_chatgpt_base_url: String,
    required_chatgpt_base_url: Option<String>,
    routing: WorkspaceRouting,
}

impl AccountRequestProcessor {
    pub(crate) fn notify_workspace_routing_to_connection(&self, connection_id: ConnectionId) {
        let processor = self.clone();
        let auth_changes = self.auth_manager.auth_change_state_receiver();
        let owner_generation = auth_changes.borrow().owner_generation;
        tokio::spawn(async move {
            if auth_changes.borrow().owner_generation != owner_generation {
                return;
            }
            if let Ok(response) = processor
                .get_account_response(GetAccountParams {
                    refresh_token: false,
                })
                .await
                && response.workspace_routing.is_some()
                && auth_changes.borrow().owner_generation == owner_generation
            {
                let notification = processor.current_account_updated_notification();
                processor
                    .outgoing
                    .send_account_notification(
                        Some(connection_id),
                        &auth_changes,
                        owner_generation,
                        AccountNotification::Updated(notification),
                    )
                    .await;
            }
        });
    }

    pub(super) async fn get_account_response(
        &self,
        params: GetAccountParams,
    ) -> Result<GetAccountResponse, JSONRPCErrorError> {
        self.refresh_token_if_requested(params.refresh_token).await;
        let mut auth_changes = self.auth_manager.auth_change_state_receiver();
        let auth_state = *auth_changes.borrow_and_update();
        let current_auth_changes = auth_changes.clone();
        let read = Box::pin(async {
            let _fetch_permit = self
                .workspace_routing_fetch
                .acquire()
                .await
                .map_err(|_| internal_error("workspace routing discovery cancelled"))?;
            let config = match self
                .config_manager
                .load_latest_config(/*fallback_cwd*/ None)
                .await
            {
                Ok(config) => config,
                Err(_)
                    if !self
                        .auth_manager
                        .auth_cached()
                        .as_ref()
                        .is_some_and(CodexAuth::is_chatgpt_auth) =>
                {
                    self.config.as_ref().clone()
                }
                Err(_) => return Err(internal_error("failed to load workspace requirements")),
            };
            let auth = self.auth_manager.auth_cached();
            let provider = create_model_provider(
                config.model_provider.clone(),
                Some(self.auth_manager.clone()),
            );
            let account_state = provider
                .account_state()
                .map_err(|err| invalid_request(err.to_string()))?;
            let account = account_state.account.map(Account::from);
            let workspace_routing = if let Some((auth, account_id)) = auth
                .as_ref()
                .filter(|auth| {
                    auth.is_chatgpt_auth() && matches!(account, Some(Account::Chatgpt { .. }))
                })
                .and_then(|auth| auth.get_account_id().map(|account_id| (auth, account_id)))
            {
                if account_id.is_empty() {
                    return Err(internal_error(
                        "workspace routing requires a ChatGPT account id",
                    ));
                }
                let required_chatgpt_base_url = config
                    .config_layer_stack
                    .requirements_toml()
                    .chatgpt_base_url
                    .clone();
                let cached = self
                    .workspace_routing
                    .lock()
                    .await
                    .as_ref()
                    .filter(|cached| {
                        cached.auth_generation == auth_state.generation
                            && cached.effective_chatgpt_base_url == config.chatgpt_base_url
                            && cached.required_chatgpt_base_url == required_chatgpt_base_url
                            && cached.routing.chatgpt_account_id == account_id
                    })
                    .map(|cached| cached.routing.clone());
                if let Some(cached) = cached {
                    Some(cached)
                } else {
                    *self.workspace_routing.lock().await = None;
                    let client = BackendClient::from_auth(
                        &config.chatgpt_base_url,
                        auth,
                        config.http_client_factory(),
                    );
                    let response = client
                        .get_accounts_check()
                        .await
                        .map_err(|_| internal_error("workspace routing discovery failed"))?;
                    let mut accounts = response
                        .accounts
                        .into_iter()
                        .filter(|account| account.id == account_id);
                    let entry = accounts.next().ok_or_else(|| {
                        internal_error("selected workspace missing from routing discovery")
                    })?;
                    if accounts.next().is_some() {
                        return Err(internal_error("duplicate workspace in routing discovery"));
                    }
                    let latest_config = self
                        .config_manager
                        .load_latest_config(/*fallback_cwd*/ None)
                        .await
                        .map_err(|_| internal_error("failed to reload workspace requirements"))?;
                    if latest_config.chatgpt_base_url != config.chatgpt_base_url
                        || latest_config.model_provider != config.model_provider
                        || latest_config
                            .config_layer_stack
                            .requirements_toml()
                            .chatgpt_base_url
                            != required_chatgpt_base_url
                    {
                        return Err(internal_error(
                            "configuration changed during workspace routing discovery; retry account/read",
                        ));
                    }
                    let routing = resolve_routing(
                        entry,
                        required_chatgpt_base_url.as_deref(),
                        &config.chatgpt_base_url,
                    )?;
                    let mut cached = self.workspace_routing.lock().await;
                    if current_auth_changes.borrow().owner_generation != auth_state.owner_generation
                    {
                        return Err(internal_error(
                            "account changed during workspace routing discovery",
                        ));
                    }
                    *cached = Some(CachedWorkspaceRouting {
                        auth_generation: auth_state.generation,
                        effective_chatgpt_base_url: config.chatgpt_base_url.clone(),
                        required_chatgpt_base_url,
                        routing: routing.clone(),
                    });
                    Some(routing)
                }
            } else {
                *self.workspace_routing.lock().await = None;
                None
            };
            Ok(GetAccountResponse {
                account,
                requires_openai_auth: account_state.requires_openai_auth,
                workspace_routing,
            })
        });
        let result = tokio::select! {
            biased;
            _ = self.workspace_routing_shutdown.cancelled() => {
                return Err(internal_error("workspace routing discovery cancelled during shutdown"));
            }
            _ = auth_changes.wait_for(|state| state.owner_generation != auth_state.owner_generation) => {
                return Err(internal_error("account changed during workspace routing discovery"));
            }
            result = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), read) => {
                result.map_err(|_| internal_error("workspace routing discovery timed out"))?
            }
        };
        if auth_changes.borrow().owner_generation != auth_state.owner_generation {
            return Err(internal_error(
                "account changed during workspace routing discovery",
            ));
        }
        result
    }
}

fn resolve_routing(
    entry: AccountEntry,
    required_chatgpt_base_url: Option<&str>,
    effective_base_url: &str,
) -> Result<WorkspaceRouting, JSONRPCErrorError> {
    let backend = entry
        .workspace_backend_origin
        .ok_or_else(|| internal_error("workspace routing discovery missing backend origin"))?;
    let account_routing_override = match entry.account_routing_override.as_deref() {
        Some("NO_CONSTRAINT") => AccountRoutingOverride::NoConstraint,
        Some("us") => AccountRoutingOverride::Us,
        Some("us_cr") => AccountRoutingOverride::UsCr,
        _ => {
            return Err(internal_error(
                "workspace routing discovery has invalid account routing override",
            ));
        }
    };
    let required = required_chatgpt_base_url
        .map(parse_backend_url)
        .transpose()?;
    let discovered = if backend == "NO_CONSTRAINT" {
        None
    } else {
        let url = parse_backend_url(&backend)?;
        if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
            return Err(internal_error(
                "workspace routing discovery must return an origin",
            ));
        }
        Some(url)
    };
    let origin = match (required, discovered) {
        (Some(required), Some(discovered)) => {
            if required.origin() != discovered.origin() {
                return Err(internal_error(
                    "required ChatGPT backend conflicts with workspace routing",
                ));
            }
            required.origin()
        }
        (Some(url), None) | (None, Some(url)) => url.origin(),
        (None, None) => parse_backend_url(effective_base_url)?.origin(),
    };
    Ok(WorkspaceRouting {
        chatgpt_account_id: entry.id,
        backend_origin: origin.ascii_serialization(),
        account_routing_override,
    })
}

fn parse_backend_url(value: &str) -> Result<Url, JSONRPCErrorError> {
    let url = Url::parse(value).map_err(|_| internal_error("invalid workspace backend URL"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value.trim() != value
    {
        return Err(internal_error(
            "workspace backend must use an HTTPS origin without credentials",
        ));
    }
    Ok(url)
}

#[cfg(test)]
#[path = "workspace_routing_tests.rs"]
mod tests;
