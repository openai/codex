//! Lifecycle ownership for a thread's MCP connections.
//!
//! A manager remains stable while complete connection generations are prepared
//! and published atomically. Startup cancellation and shutdown therefore stay
//! with the object that owns the live clients instead of being coordinated by
//! callers.

use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::RwLock as StdRwLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::connection_generation::ConnectionGeneration;
use crate::connection_generation::ConnectionGenerationLease;
use crate::connection_generation::McpConnectionStartParams;
use crate::elicitation::ElicitationRequestManager;
#[cfg(test)]
use crate::rmcp_client::AsyncManagedClient;
#[cfg(test)]
use crate::server::McpServerMetadata;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

struct McpConnectionLifecycle {
    active: StdRwLock<Arc<ConnectionGeneration>>,
    pending_startup: StdMutex<Option<CancellationToken>>,
    mutation: Mutex<()>,
    retirements: StdMutex<JoinSet<()>>,
    elicitation_requests: ElicitationRequestManager,
    closed: AtomicBool,
}

impl Drop for McpConnectionLifecycle {
    fn drop(&mut self) {
        if let Ok(pending_startup) = self.pending_startup.get_mut()
            && let Some(token) = pending_startup.take()
        {
            token.cancel();
        }
        if let Ok(active) = self.active.get_mut() {
            active.cancel_startup();
        }
    }
}

/// Owns and atomically replaces all live MCP connections for one thread.
#[derive(Clone)]
pub struct McpConnectionManager {
    lifecycle: Arc<McpConnectionLifecycle>,
}

impl McpConnectionManager {
    /// Creates a valid manager with no live servers.
    pub fn empty(
        approval_policy: AskForApproval,
        permission_profile: PermissionProfile,
        prefix_mcp_tool_names: bool,
    ) -> Self {
        Self {
            lifecycle: Arc::new(McpConnectionLifecycle {
                active: StdRwLock::new(Arc::new(ConnectionGeneration::empty(
                    prefix_mcp_tool_names,
                ))),
                pending_startup: StdMutex::new(None),
                mutation: Mutex::new(()),
                retirements: StdMutex::new(JoinSet::new()),
                elicitation_requests: ElicitationRequestManager::new(
                    approval_policy,
                    permission_profile,
                    /*reviewer*/ None,
                ),
                closed: AtomicBool::new(false),
            }),
        }
    }

    /// Creates a manager and publishes its first connection generation.
    pub async fn start(params: McpConnectionStartParams) -> Self {
        let manager = Self::empty(
            params.approval_policy,
            params.permission_profile.clone(),
            params.prefix_mcp_tool_names,
        );
        manager.initialize(params).await;
        manager
    }

    /// Publishes the initial connection generation for an empty manager.
    pub async fn initialize(&self, params: McpConnectionStartParams) {
        self.replace_generation(params).await;
    }

    /// Prepares and atomically publishes a replacement connection generation.
    pub async fn reconfigure(&self, params: McpConnectionStartParams) {
        self.replace_generation(params).await;
    }

    async fn replace_generation(&self, params: McpConnectionStartParams) {
        let _mutation_guard = self.lifecycle.mutation.lock().await;
        if self.lifecycle.closed.load(Ordering::Acquire) {
            return;
        }
        self.set_approval_policy(params.approval_policy);
        self.set_permission_profile(params.permission_profile.clone());
        let elicitation_requests = self
            .lifecycle
            .elicitation_requests
            .for_generation(params.elicitation_reviewer.clone());

        let startup_cancellation_token = CancellationToken::new();
        let previous_pending = self
            .lifecycle
            .pending_startup
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(startup_cancellation_token.clone());
        if let Some(previous) = previous_pending {
            previous.cancel();
        }
        let generation = Arc::new(
            ConnectionGeneration::start(params, elicitation_requests, startup_cancellation_token)
                .await,
        );
        let previous = match self.lifecycle.active.write() {
            Ok(mut active) => std::mem::replace(&mut *active, generation),
            Err(poisoned) => {
                let mut active = poisoned.into_inner();
                std::mem::replace(&mut *active, generation)
            }
        };
        self.lifecycle
            .pending_startup
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        previous.cancel_startup();
        let mut retirements = self
            .lifecycle
            .retirements
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while retirements.try_join_next().is_some() {}
        retirements.spawn(async move {
            previous.wait_until_idle().await;
            previous.shutdown().await;
        });
    }

    pub fn cancel_startup(&self) {
        let pending_startup = self
            .lifecycle
            .pending_startup
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(pending_startup) = pending_startup {
            pending_startup.cancel();
        } else {
            self.generation().cancel_startup();
        }
    }

    /// Atomically removes the active generation and stops all owned clients.
    pub async fn shutdown(&self) {
        self.lifecycle.closed.store(true, Ordering::Release);
        self.cancel_startup();
        let _mutation_guard = self.lifecycle.mutation.lock().await;
        let prefix_mcp_tool_names = self.generation().prefix_mcp_tool_names;
        let empty = Arc::new(ConnectionGeneration::empty(prefix_mcp_tool_names));
        let previous = match self.lifecycle.active.write() {
            Ok(mut active) => std::mem::replace(&mut *active, empty),
            Err(poisoned) => {
                let mut active = poisoned.into_inner();
                std::mem::replace(&mut *active, empty)
            }
        };
        previous.wait_until_idle().await;
        previous.shutdown().await;
        let mut retirements = std::mem::take(
            &mut *self
                .lifecycle
                .retirements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        while retirements.join_next().await.is_some() {}
    }

    pub(crate) fn generation(&self) -> ConnectionGenerationLease {
        let active = self
            .lifecycle
            .active
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        active.acquire()
    }

    pub(crate) fn elicitation_requests(&self) -> &ElicitationRequestManager {
        &self.lifecycle.elicitation_requests
    }

    #[cfg(test)]
    pub(crate) fn insert_client_for_test(&self, server_name: String, client: AsyncManagedClient) {
        let mut active = self
            .lifecycle
            .active
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::make_mut(&mut active)
            .clients
            .insert(server_name, client);
    }

    #[cfg(test)]
    pub(crate) fn insert_server_metadata_for_test(
        &self,
        server_name: String,
        metadata: McpServerMetadata,
    ) {
        let mut active = self
            .lifecycle
            .active
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Arc::make_mut(&mut active)
            .server_metadata
            .insert(server_name, metadata);
    }
}
