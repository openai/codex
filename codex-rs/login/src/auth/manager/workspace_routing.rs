//! Connects account-owned routing discovery to ChatGPT request builders.
//! The resolver is weakly held so it cannot keep its app-server owner alive.

use codex_config::ConfigLayerStack;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Weak;

use crate::AuthManager;
use crate::CodexAuth;

/// A successful discovery for one selected workspace.
#[derive(Clone, Debug)]
pub struct WorkspaceRouting {
    pub chatgpt_account_id: String,
    pub backend_origin: String,
    pub account_routing_override: String,
}

/// Retained configuration layers and project location for one model session.
/// The account owner refreshes global settings without discarding session overrides.
pub struct WorkspaceRoutingSession {
    pub cwd: PathBuf,
    pub config_layer_stack: ConfigLayerStack,
}

impl std::fmt::Debug for WorkspaceRoutingSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceRoutingSession")
            .finish_non_exhaustive()
    }
}

/// The provider and bootstrap selected for one model session.
pub struct WorkspaceRoutingRequest {
    pub provider_base_url: String,
    pub chatgpt_base_url: String,
    pub previously_routed: bool,
    pub session: Option<Arc<WorkspaceRoutingSession>>,
}

/// Resolves routing through the account owner's cache and requirements loader.
/// Returns no routing for independent destinations or no selected workspace.
/// Destination scope is decided here, before making a discovery request.
/// Implementations must reject failed discovery and changes to the selected account.
pub trait WorkspaceRoutingResolver: Send + Sync {
    fn resolve(
        &self,
        request: WorkspaceRoutingRequest,
    ) -> Pin<Box<dyn Future<Output = io::Result<Option<WorkspaceRouting>>> + Send + '_>>;
}

impl AuthManager {
    /// Installs the app-server's routing owner before accepting model requests.
    pub fn set_workspace_routing_resolver(&self, resolver: Weak<dyn WorkspaceRoutingResolver>) {
        assert!(self.workspace_routing_resolver.set(resolver).is_ok());
    }

    /// CLI callers without a discovery owner retain their existing routing behavior.
    pub async fn workspace_routing(
        &self,
        auth: &CodexAuth,
        request: WorkspaceRoutingRequest,
    ) -> io::Result<Option<WorkspaceRouting>> {
        let Some(resolver) = self.workspace_routing_resolver.get() else {
            return Ok(None);
        };
        let auth_changes = self.auth_change_receiver();
        let revision = *auth_changes.borrow();
        let resolver = resolver
            .upgrade()
            .ok_or_else(|| io::Error::other("workspace routing owner is unavailable"))?;
        let routing = resolver.resolve(request).await?;
        if *auth_changes.borrow() != revision
            || routing.as_ref().is_some_and(|routing| {
                auth.get_account_id().as_deref() != Some(routing.chatgpt_account_id.as_str())
            })
        {
            return Err(io::Error::other(
                "account changed during workspace routing discovery",
            ));
        }
        Ok(routing)
    }
}
