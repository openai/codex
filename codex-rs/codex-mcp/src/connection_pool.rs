//! Shares MCP 2026-07-28 connections while keeping each chat's ownership separate.
//!
//! The protocol requires request-scoped context and explicit handles for application state.
//! Request continuations use the calling chat's elicitation authority. Legacy connections and
//! servers that request input during startup remain private.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::time::Duration;

use codex_async_utils::OrCancelExt;
use codex_connectors::ConnectorRuntimeContextKey;
use codex_login::AuthChangeState;
use codex_login::AuthManager;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::SendElicitation;
use futures::FutureExt;
use rmcp::model::ElicitationAction;
use rmcp::model::ProtocolVersion;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::client_tool_catalog::ClientToolCatalog;
use crate::client_tool_catalog::ToolCatalogSnapshot;
use crate::rmcp_client::ManagedClient;
use crate::rmcp_client::ManagedClientFuture;
use crate::rmcp_client::StartupOutcomeError;
use crate::rmcp_client::list_tools_for_client_uncached;
use crate::server::McpServerConnectionIdentity;
use crate::tool_catalog_cache::McpToolCatalogCacheContext;

/// Process-owned registry for compatible local stdio servers using MCP 2026-07-28.
#[derive(Clone, Default)]
pub struct McpConnectionPool {
    entries: Arc<Mutex<Vec<Weak<Entry>>>>,
}

#[derive(Clone)]
pub(crate) struct ConnectionKey {
    pub(crate) server_name: String,
    pub(crate) identity: McpServerConnectionIdentity,
    pub(crate) account: ConnectorRuntimeContextKey,
    pub(crate) auth_state: Option<(Arc<AuthManager>, AuthChangeState)>,
    pub(crate) startup_timeout: Duration,
    pub(crate) catalog_item_limit: usize,
}

impl PartialEq for ConnectionKey {
    fn eq(&self, other: &Self) -> bool {
        self.server_name == other.server_name
            && self.identity == other.identity
            && self.account == other.account
            && self.startup_timeout == other.startup_timeout
            && self.catalog_item_limit == other.catalog_item_limit
            && match (&self.auth_state, &other.auth_state) {
                (Some((left, left_state)), Some((right, right_state))) => {
                    Arc::ptr_eq(left, right)
                        && left_state.owner_generation == right_state.owner_generation
                        && left_state.source_generation == right_state.source_generation
                        && left_state.login_generation == right_state.login_generation
                }
                (None, None) => true,
                (Some(_), None) | (None, Some(_)) => false,
            }
    }
}

struct Entry {
    key: ConnectionKey,
    client: ManagedClientFuture,
    capabilities: Arc<Mutex<Option<serde_json::Value>>>,
    // Share immutable definitions; every chat owns its catalog revisions and refresh locks.
    catalog: Mutex<Option<ToolCatalogSnapshot>>,
    cancellation: CancellationToken,
    /// Wakes joiners without cancelling the creator's startup.
    sharing_declined: CancellationToken,
    owners: Weak<LeaseOwners>,
    pool: Weak<Mutex<Vec<Weak<Entry>>>>,
}

/// One strong reference per active chat lease, independent of retained client bindings.
/// Every strong reference must be consumed by `ConnectionLease::release`.
struct LeaseOwners;

#[derive(Clone, Copy, PartialEq, Eq)]
enum AcquisitionRole {
    Creator,
    Joiner,
}

impl Entry {
    fn remove_from_pool(self: &Arc<Self>) {
        if let Some(pool) = self.pool.upgrade() {
            pool.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .retain(|entry| !entry.ptr_eq(&Arc::downgrade(self)));
        }
    }

    async fn tool_catalog_for_chat(
        &self,
        client: &ManagedClient,
        role: AcquisitionRole,
        cache_context: Option<&McpToolCatalogCacheContext>,
    ) -> Result<Arc<ClientToolCatalog>, StartupOutcomeError> {
        // Listing must not update another chat's catalog or wait for its calls.
        let listed = if role == AcquisitionRole::Creator {
            client.tool_catalog.read(|snapshot| snapshot).await
        } else {
            let fetch_ticket = cache_context.map(|context| (context, context.begin_fetch()));
            let (tools, meta) = list_tools_for_client_uncached(
                &self.key.server_name,
                self.key.server_name == crate::CODEX_APPS_MCP_SERVER_NAME,
                "shared_connection",
                &client.client,
                Some(self.key.startup_timeout),
                self.key.catalog_item_limit,
                client.server_instructions.as_deref(),
            )
            .await?;
            if let Some((context, mut ticket)) = fetch_ticket {
                if context.track_tool_list_changes {
                    ticket.tool_list_revision = Some(client.client.tool_list_revision());
                }
                context.publish_if_newest(ticket, &tools, meta.clone());
            }
            ToolCatalogSnapshot {
                revision: 0,
                tools: tools.into(),
                meta,
            }
        };
        let mut catalog = self
            .catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let catalog = catalog.get_or_insert_with(|| listed.clone());
        if catalog.tools != listed.tools || catalog.meta != listed.meta {
            *catalog = listed;
        }
        Ok(Arc::new(ClientToolCatalog::new(
            Arc::clone(&catalog.tools),
            catalog.meta.clone(),
            /*updates*/ None,
        )))
    }
}

/// A chat releases its lease explicitly on shutdown, even while bindings retain its client.
pub(crate) struct ConnectionLease {
    entry: Arc<Entry>,
    owner: Mutex<Option<Arc<LeaseOwners>>>,
    cancellation: CancellationToken,
    slot: Arc<ConnectionSlot>,
}

/// Tracks an acquisition without retaining its lease, including before startup completes.
#[derive(Default)]
pub(crate) struct ConnectionSlot {
    state: Mutex<SlotState>,
    shutdown_tasks: OnceLock<TaskTracker>,
}

#[derive(Default)]
enum SlotState {
    #[default]
    Available,
    Acquired(Weak<Entry>),
    Retired,
}

impl ConnectionSlot {
    pub(crate) fn retire(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let SlotState::Acquired(entry) = std::mem::replace(&mut *state, SlotState::Retired)
            && let Some(entry) = entry.upgrade()
        {
            entry.remove_from_pool();
        }
    }

    pub(crate) async fn wait_for_shutdown(&self) {
        if let Some(tasks) = self.shutdown_tasks.get() {
            tasks.close();
            tasks.wait().await;
        }
    }
}

#[derive(Clone, Default)]
pub(crate) enum ConnectionSharing {
    #[default]
    Private,
    Negotiated,
    /// The server declined sharing, but its creator still owns the startup entry.
    Owned(Arc<ConnectionLease>),
    Shared(Arc<ConnectionLease>),
}

impl McpConnectionPool {
    pub(crate) async fn connect(
        &self,
        slot: Arc<ConnectionSlot>,
        key: ConnectionKey,
        capabilities: Arc<Mutex<Option<serde_json::Value>>>,
        tool_catalog_cache_context: Option<McpToolCatalogCacheContext>,
        cancellation: CancellationToken,
        start: impl Fn(CancellationToken, Option<CancellationToken>) -> ManagedClientFuture + Sync,
    ) -> Result<ManagedClient, StartupOutcomeError> {
        let (lease, role) = self
            .acquire(key, &slot, Arc::clone(&capabilities), &cancellation, &start)
            .or_cancel(&cancellation)
            .await
            .unwrap_or(Err(StartupOutcomeError::Cancelled))?;
        let outcome = tokio::select! {
            outcome = lease.entry.client.clone() => Some(outcome),
            () = lease.entry.sharing_declined.cancelled(), if role == AcquisitionRole::Joiner => None,
            () = cancellation.cancelled() => return Err(StartupOutcomeError::Cancelled),
        };
        match outcome {
            Some(Ok(mut client))
                if role == AcquisitionRole::Creator
                    || matches!(client.connection_sharing, ConnectionSharing::Negotiated) =>
            {
                let shared_capabilities = lease
                    .entry
                    .capabilities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                *capabilities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = shared_capabilities;
                client.connection_sharing =
                    if matches!(client.connection_sharing, ConnectionSharing::Negotiated) {
                        client.client.enable_request_cancellation();
                        client.tool_catalog = lease
                            .entry
                            .tool_catalog_for_chat(
                                &client,
                                role,
                                tool_catalog_cache_context.as_ref(),
                            )
                            .or_cancel(&cancellation)
                            .await
                            .unwrap_or(Err(StartupOutcomeError::Cancelled))?;
                        ConnectionSharing::Shared(lease)
                    } else {
                        lease.entry.remove_from_pool();
                        ConnectionSharing::Owned(lease)
                    };
                Ok(client)
            }
            Some(Err(error)) if role == AcquisitionRole::Creator => Err(error),
            _ => {
                // Older or failed servers leave other chats to initialize privately
                // with their own elicitation authority.
                drop(lease);
                start(cancellation, /*sharing_declined*/ None).await
            }
        }
    }

    async fn acquire(
        &self,
        key: ConnectionKey,
        slot: &Arc<ConnectionSlot>,
        capabilities: Arc<Mutex<Option<serde_json::Value>>>,
        cancellation: &CancellationToken,
        start: impl Fn(CancellationToken, Option<CancellationToken>) -> ManagedClientFuture,
    ) -> Result<(Arc<ConnectionLease>, AcquisitionRole), StartupOutcomeError> {
        loop {
            let (lease, role) = {
                let mut state = slot
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if matches!(&*state, SlotState::Retired) {
                    return Err(StartupOutcomeError::Cancelled);
                }
                let mut entries = self
                    .entries
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                entries.retain(|entry| entry.strong_count() > 0);
                let lease_cancellation = cancellation.child_token();
                let existing = entries
                    .iter()
                    .filter_map(Weak::upgrade)
                    .filter(|entry| {
                        entry.key == key
                            && !entry.sharing_declined.is_cancelled()
                            && entry.client.peek().is_none_or(|result| {
                                result.as_ref().is_ok_and(|client| {
                                    matches!(
                                        client.connection_sharing,
                                        ConnectionSharing::Negotiated
                                    )
                                })
                            })
                    })
                    .find_map(|entry| entry.owners.upgrade().map(|owner| (entry, owner)));
                let (entry, owner, role) = if let Some((entry, owner)) = existing {
                    (entry, owner, AcquisitionRole::Joiner)
                } else {
                    let startup_cancellation = CancellationToken::new();
                    let sharing_declined = CancellationToken::new();
                    let client =
                        start(startup_cancellation.clone(), Some(sharing_declined.clone()));
                    // Startup belongs to the connection, including if its first chat is cancelled.
                    tokio::spawn(client.clone());
                    let owner = Arc::new(LeaseOwners);
                    let entry = Arc::new(Entry {
                        key: key.clone(),
                        client,
                        capabilities: Arc::clone(&capabilities),
                        catalog: Mutex::new(None),
                        cancellation: startup_cancellation,
                        sharing_declined,
                        owners: Arc::downgrade(&owner),
                        pool: Arc::downgrade(&self.entries),
                    });
                    entries.push(Arc::downgrade(&entry));
                    (entry, owner, AcquisitionRole::Creator)
                };
                let lease = Arc::new(ConnectionLease {
                    entry,
                    owner: Mutex::new(Some(owner)),
                    cancellation: lease_cancellation,
                    slot: Arc::clone(slot),
                });
                *state = SlotState::Acquired(Arc::downgrade(&lease.entry));
                (lease, role)
            };
            if let Some(Ok(client)) = lease.entry.client.peek()
                && client.client.is_closed().await
            {
                lease.entry.remove_from_pool();
                lease.release();
                continue;
            }
            return Ok((lease, role));
        }
    }
}

impl ConnectionLease {
    pub(crate) fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    /// Returns the slot tracking shutdown when this chat releases the last lease.
    pub(crate) fn release(&self) -> Option<&ConnectionSlot> {
        self.cancellation.cancel();
        let mut owner = self
            .owner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::into_inner(owner.take()?)?;
        self.entry.cancellation.cancel();
        self.entry.remove_from_pool();
        let client = self.entry.client.clone();
        let runtime = tokio::runtime::Handle::try_current().ok()?;
        // The chat must be able to await its final release even when cancellation drops the lease.
        let tasks = self.slot.shutdown_tasks.get_or_init(TaskTracker::new);
        drop(tasks.spawn_on(
            async move {
                match client.await {
                    Ok(client) => client.client.shutdown().await,
                    Err(error) => error.wait_for_shutdown().await,
                }
            },
            &runtime,
        ));
        Some(&self.slot)
    }
}

impl Drop for ConnectionLease {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

/// Releases waiting chats when startup becomes private and detaches callbacks after sharing.
pub(crate) struct SharingNegotiation {
    callback: Arc<Mutex<Option<SendElicitation>>>,
    sharing_declined: CancellationToken,
}

impl SharingNegotiation {
    pub(crate) fn new(
        sender: SendElicitation,
        sharing_declined: CancellationToken,
    ) -> (Self, SendElicitation) {
        let callback = Arc::new(Mutex::new(Some(sender)));
        let negotiation = Self {
            callback: Arc::clone(&callback),
            sharing_declined: sharing_declined.clone(),
        };
        let sender: SendElicitation = Box::new(move |id, request| {
            let callback = callback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(callback) = callback.as_ref() {
                // Client input binds this startup to its original chat.
                sharing_declined.cancel();
                callback(id, request)
            } else {
                futures::future::ready(Ok(ElicitationResponse {
                    action: ElicitationAction::Cancel,
                    content: None,
                    meta: None,
                }))
                .boxed()
            }
        });
        (negotiation, sender)
    }

    pub(crate) fn finish(self, protocol_version: &ProtocolVersion) -> ConnectionSharing {
        let mut callback = self
            .callback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *protocol_version == ProtocolVersion::V_2026_07_28
            && !self.sharing_declined.is_cancelled()
        {
            callback.take();
            ConnectionSharing::Negotiated
        } else {
            self.sharing_declined.cancel();
            ConnectionSharing::Private
        }
    }
}

#[cfg(test)]
#[path = "connection_pool_tests.rs"]
pub(crate) mod tests;
