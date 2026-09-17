//! Discovers routing for the selected ChatGPT workspace without crossing auth owners.
//! Discovery is coordinated per routing key; a cache miss never proves independence.
//! Credential refreshes invalidate the cache, but do not cancel same-owner discovery.
//! Model sessions retain their original bootstrap scope across configuration changes.

use super::*;
use codex_app_server_protocol::AccountRoutingOverride;
use codex_backend_client::AccountEntry;
use codex_login::WorkspaceRouting;
use codex_login::WorkspaceRoutingRequest;
use codex_login::WorkspaceRoutingResolver;
use codex_model_provider::ProviderAccount;
use codex_model_provider::ProviderAccountError;
use codex_model_provider::ProviderAccountState;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Weak;
use tokio::sync::Semaphore;
use url::Url;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct WorkspaceRoutingKey {
    auth_generation: u64,
    chatgpt_account_id: String,
    effective_chatgpt_base_url: String,
    required_chatgpt_base_url: Option<String>,
}

// Results live through same-key waiters, even if another key replaces the cache.
// Weak entries are pruned on the next read after those requests finish.
pub(super) type WorkspaceRoutingFetches = HashMap<WorkspaceRoutingKey, Weak<WorkspaceRoutingFetch>>;

pub(super) struct WorkspaceRoutingFetch {
    discovery: Semaphore,
    routing: Mutex<Option<WorkspaceRouting>>,
}

pub(super) struct CachedWorkspaceRouting {
    key: WorkspaceRoutingKey,
    routing: WorkspaceRouting,
}

pub(super) struct AccountRead {
    account_state: ProviderAccountState,
    pub(super) workspace_routing: Option<WorkspaceRouting>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum AccountReadError {
    #[error(transparent)]
    InvalidAccount(#[from] ProviderAccountError),
    #[error(transparent)]
    Routing(#[from] WorkspaceRoutingError),
}

/// Routing failure categories carry no account, URL, or backend data.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum WorkspaceRoutingError {
    #[error("failed to load workspace requirements")]
    RequirementsLoad,
    #[error("workspace routing requires a ChatGPT account id")]
    MissingAccountId,
    #[error("workspace routing discovery cancelled")]
    DiscoveryCancelled,
    #[error("account changed during workspace routing discovery")]
    AccountChanged,
    #[error("workspace routing discovery unauthorized (401)")]
    DiscoveryUnauthorized,
    #[error("workspace routing discovery failed")]
    DiscoveryFailed,
    #[error("selected workspace missing from routing discovery")]
    MissingWorkspace,
    #[error("duplicate workspace in routing discovery")]
    DuplicateWorkspace,
    #[error("failed to reload workspace requirements")]
    RequirementsReload,
    #[error("configuration changed during workspace routing discovery; retry account/read")]
    ConfigurationChanged,
    #[error("invalid model provider URL")]
    InvalidProviderUrl,
    #[error("invalid ChatGPT backend URL")]
    InvalidBootstrapUrl,
    #[error("invalid session ChatGPT backend URL")]
    InvalidSessionBootstrapUrl,
    #[error("ChatGPT backend changed; start a new thread before sending more content")]
    SessionBackendChanged,
    #[error("workspace routing discovery cancelled during shutdown")]
    Shutdown,
    #[error("workspace routing discovery timed out")]
    DiscoveryTimeout,
    #[error("workspace routing discovery missing backend origin")]
    MissingBackendOrigin,
    #[error("workspace routing discovery has invalid account routing override")]
    InvalidRoutingOverride,
    #[error("workspace routing discovery must return an origin")]
    BackendIsNotOrigin,
    #[error("required ChatGPT backend conflicts with workspace routing")]
    BackendConflict,
    #[error("invalid workspace backend URL")]
    InvalidBackendUrl,
    #[error("workspace backend must use an HTTPS origin without credentials")]
    InvalidBackendOrigin,
}

impl WorkspaceRoutingResolver for AccountRequestProcessor {
    fn resolve(
        &self,
        request: WorkspaceRoutingRequest,
    ) -> Pin<Box<dyn Future<Output = io::Result<Option<WorkspaceRouting>>> + Send + '_>> {
        Box::pin(async move {
            self.read_account(Some(&request))
                .await
                .map(|account| account.workspace_routing)
                .map_err(|error| match error {
                    AccountReadError::InvalidAccount(error) => io::Error::other(error),
                    AccountReadError::Routing(error) => io::Error::new(
                        if error == WorkspaceRoutingError::DiscoveryUnauthorized {
                            io::ErrorKind::PermissionDenied
                        } else {
                            io::ErrorKind::Other
                        },
                        error,
                    ),
                })
        })
    }
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
            if let Ok(response) = processor.read_account(/*request*/ None).await
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

    pub(crate) async fn get_account(
        &self,
        params: GetAccountParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.refresh_token_if_requested(params.refresh_token).await;
        let read = self
            .read_account(/*request*/ None)
            .await
            .map_err(|error| match error {
                AccountReadError::InvalidAccount(error) => invalid_request(error.to_string()),
                AccountReadError::Routing(error) => internal_error(error.to_string()),
            })?;
        Ok(Some(
            GetAccountResponse {
                account: read.account_state.account.map(Account::from),
                requires_openai_auth: read.account_state.requires_openai_auth,
                workspace_routing: read.workspace_routing.map(|routing| {
                    codex_app_server_protocol::WorkspaceRouting {
                        chatgpt_account_id: routing.chatgpt_account_id,
                        backend_origin: routing.backend_origin,
                        account_routing_override: match routing.account_routing_override.as_str() {
                            "NO_CONSTRAINT" => AccountRoutingOverride::NoConstraint,
                            "us" => AccountRoutingOverride::Us,
                            "us_cr" => AccountRoutingOverride::UsCr,
                            _ => unreachable!("routing discovery validates the override"),
                        },
                    }
                }),
            }
            .into(),
        ))
    }

    pub(super) async fn read_account(
        &self,
        request: Option<&WorkspaceRoutingRequest>,
    ) -> Result<AccountRead, AccountReadError> {
        let mut auth_changes = self.auth_manager.auth_change_state_receiver();
        let auth_state = *auth_changes.borrow_and_update();
        let current_auth_changes = auth_changes.clone();
        let read = Box::pin(async {
            let load_config = async || {
                match request.and_then(|request| request.session.as_deref()) {
                    Some(session) => {
                        self.config_manager
                            .load_retained_session_config(&session.config_layer_stack, &session.cwd)
                            .await
                    }
                    None => {
                        self.config_manager
                            .load_latest_config(/*fallback_cwd*/ None)
                            .await
                    }
                }
            };
            let config = match load_config().await {
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
                Err(_) => return Err(WorkspaceRoutingError::RequirementsLoad.into()),
            };
            let auth = self.auth_manager.auth_cached();
            let provider = create_model_provider(
                config.model_provider.clone(),
                Some(self.auth_manager.clone()),
            );
            let account_state = provider
                .account_state()
                .map_err(AccountReadError::InvalidAccount)?;
            let mut workspace_routing = if let Some((auth, account_id)) = auth
                .as_ref()
                .filter(|auth| {
                    auth.is_chatgpt_auth()
                        && (request.is_some()
                            || matches!(
                                account_state.account,
                                Some(ProviderAccount::Chatgpt { .. })
                            ))
                })
                .and_then(|auth| auth.get_account_id().map(|account_id| (auth, account_id)))
            {
                if account_id.is_empty() {
                    return Err(WorkspaceRoutingError::MissingAccountId.into());
                }
                let required_chatgpt_base_url = config
                    .config_layer_stack
                    .requirements_toml()
                    .chatgpt_base_url
                    .clone();
                let key = WorkspaceRoutingKey {
                    auth_generation: auth_state.generation,
                    chatgpt_account_id: account_id.clone(),
                    effective_chatgpt_base_url: config.chatgpt_base_url.clone(),
                    required_chatgpt_base_url: required_chatgpt_base_url.clone(),
                };
                let fetch = {
                    let mut fetches = self.workspace_routing_fetches.lock().await;
                    fetches.retain(|_, fetch| fetch.strong_count() > 0);
                    if let Some(fetch) = fetches.get(&key).and_then(Weak::upgrade) {
                        fetch
                    } else {
                        let fetch = Arc::new(WorkspaceRoutingFetch {
                            discovery: Semaphore::new(/*permits*/ 1),
                            routing: Mutex::new(/*t*/ None),
                        });
                        fetches.insert(key.clone(), Arc::downgrade(&fetch));
                        fetch
                    }
                };
                let _permit = fetch
                    .discovery
                    .acquire()
                    .await
                    .map_err(|_| WorkspaceRoutingError::DiscoveryCancelled)?;
                let fetched = fetch.routing.lock().await.clone();
                let cached = match fetched {
                    Some(routing) => Some(routing),
                    None => self
                        .workspace_routing
                        .lock()
                        .await
                        .as_ref()
                        .filter(|cached| cached.key == key)
                        .map(|cached| cached.routing.clone()),
                };
                let routing = if let Some(cached) = cached {
                    cached
                } else {
                    let mut recovery = self.auth_manager.unauthorized_recovery();
                    let mut discovery_auth = auth.clone();
                    let mut discovery_generation = auth_state.generation;
                    let response = loop {
                        let client = BackendClient::from_auth(
                            &config.chatgpt_base_url,
                            &discovery_auth,
                            config.http_client_factory(),
                        );
                        match client.get_accounts_check().await {
                            Ok(response) => break response,
                            Err(error) => {
                                let unauthorized = error.is_unauthorized();
                                if unauthorized
                                    && recovery.has_next()
                                    && recovery.next().await.is_ok()
                                {
                                    let refreshed = *current_auth_changes.borrow();
                                    if refreshed.owner_generation != auth_state.owner_generation {
                                        return Err(WorkspaceRoutingError::AccountChanged.into());
                                    }
                                    discovery_generation = refreshed.generation;
                                    discovery_auth = self
                                        .auth_manager
                                        .auth_cached()
                                        .ok_or(WorkspaceRoutingError::AccountChanged)?;
                                    continue;
                                }
                                return Err(if unauthorized {
                                    WorkspaceRoutingError::DiscoveryUnauthorized
                                } else {
                                    WorkspaceRoutingError::DiscoveryFailed
                                }
                                .into());
                            }
                        }
                    };
                    let mut accounts = response
                        .accounts
                        .into_iter()
                        .filter(|account| account.id == account_id);
                    let entry = accounts
                        .next()
                        .ok_or(WorkspaceRoutingError::MissingWorkspace)?;
                    if accounts.next().is_some() {
                        return Err(WorkspaceRoutingError::DuplicateWorkspace.into());
                    }
                    let latest_config = load_config()
                        .await
                        .map_err(|_| WorkspaceRoutingError::RequirementsReload)?;
                    if latest_config.chatgpt_base_url != config.chatgpt_base_url
                        || latest_config.model_provider != config.model_provider
                        || latest_config
                            .config_layer_stack
                            .requirements_toml()
                            .chatgpt_base_url
                            != required_chatgpt_base_url
                    {
                        return Err(WorkspaceRoutingError::ConfigurationChanged.into());
                    }
                    let routing = resolve_routing(
                        entry,
                        required_chatgpt_base_url.as_deref(),
                        &config.chatgpt_base_url,
                    )?;
                    let mut cached = self.workspace_routing.lock().await;
                    if current_auth_changes.borrow().owner_generation != auth_state.owner_generation
                    {
                        return Err(WorkspaceRoutingError::AccountChanged.into());
                    }
                    // Same-owner refresh can complete a newer discovery first.
                    if current_auth_changes.borrow().generation == discovery_generation {
                        *cached = Some(CachedWorkspaceRouting {
                            key: WorkspaceRoutingKey {
                                auth_generation: discovery_generation,
                                ..key
                            },
                            routing: routing.clone(),
                        });
                    }
                    routing
                };
                *fetch.routing.lock().await = Some(routing.clone());
                Some(routing)
            } else {
                *self.workspace_routing.lock().await = None;
                None
            };
            if let Some(request) = request.filter(|_| workspace_routing.is_some()) {
                let origin = Url::parse(&request.provider_base_url)
                    .map_err(|_| WorkspaceRoutingError::InvalidProviderUrl)?
                    .origin();
                let bootstrap_origin = Url::parse(&config.chatgpt_base_url)
                    .map_err(|_| WorkspaceRoutingError::InvalidBootstrapUrl)?
                    .origin();
                let session_bootstrap_origin = Url::parse(&request.chatgpt_base_url)
                    .map_err(|_| WorkspaceRoutingError::InvalidSessionBootstrapUrl)?
                    .origin();
                let selected_backend = workspace_routing
                    .as_ref()
                    .is_some_and(|routing| routing.backend_origin == origin.ascii_serialization());
                let workspace_bound = request.previously_routed
                    || request.provider_base_url == codex_model_provider::CHATGPT_CODEX_BASE_URL
                    || origin == session_bootstrap_origin
                    || (session_bootstrap_origin == bootstrap_origin && selected_backend);
                if !workspace_bound {
                    workspace_routing = None;
                } else if session_bootstrap_origin != bootstrap_origin {
                    return Err(WorkspaceRoutingError::SessionBackendChanged.into());
                }
            }
            Ok(AccountRead {
                account_state,
                workspace_routing,
            })
        });
        let result = tokio::select! {
            biased;
            _ = self.workspace_routing_shutdown.cancelled() => {
                return Err(WorkspaceRoutingError::Shutdown.into());
            }
            _ = auth_changes.wait_for(|state| state.owner_generation != auth_state.owner_generation) => {
                return Err(WorkspaceRoutingError::AccountChanged.into());
            }
            result = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), read) => {
                result.map_err(|_| WorkspaceRoutingError::DiscoveryTimeout)?
            }
        };
        if auth_changes.borrow().owner_generation != auth_state.owner_generation {
            return Err(WorkspaceRoutingError::AccountChanged.into());
        }
        result
    }
}

fn resolve_routing(
    entry: AccountEntry,
    required_chatgpt_base_url: Option<&str>,
    effective_base_url: &str,
) -> Result<WorkspaceRouting, AccountReadError> {
    let backend = entry
        .workspace_backend_origin
        .ok_or(WorkspaceRoutingError::MissingBackendOrigin)?;
    let account_routing_override = match entry.account_routing_override.as_deref() {
        Some(value @ ("NO_CONSTRAINT" | "us" | "us_cr")) => value.to_owned(),
        _ => {
            return Err(WorkspaceRoutingError::InvalidRoutingOverride.into());
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
            return Err(WorkspaceRoutingError::BackendIsNotOrigin.into());
        }
        Some(url)
    };
    let origin = match (required, discovered) {
        (Some(required), Some(discovered)) => {
            if required.origin() != discovered.origin() {
                return Err(WorkspaceRoutingError::BackendConflict.into());
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

fn parse_backend_url(value: &str) -> Result<Url, AccountReadError> {
    let url = Url::parse(value).map_err(|_| WorkspaceRoutingError::InvalidBackendUrl)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || value.trim() != value
    {
        return Err(WorkspaceRoutingError::InvalidBackendOrigin.into());
    }
    Ok(url)
}

#[cfg(test)]
#[path = "workspace_routing_tests.rs"]
mod tests;
