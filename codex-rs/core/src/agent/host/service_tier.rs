//! Select host-owned shared service-tier state at existing runtime configuration boundaries.

use crate::agent::api::AgentConfigUpdate;
use crate::agent::host::AgentRuntimeServices;
use crate::session::session::Session;
use codex_protocol::SessionId;

/// The host's local view of the service tier shared within an agent session.
///
/// Install an `Arc<dyn AgentServiceTier>` in thread extension data before startup. Host-owned
/// children with a provider reject explicit per-turn tier overrides. Host-owned runtimes without
/// a provider retain their configured tier and do not propagate shared tier updates.
/// Controller-owned runtimes use their controller.
/// Both methods must be cheap, synchronous and non-reentrant: Core may call them while
/// holding configuration or registration locks. They must perform no network I/O. The host publishes
/// committed root settings into its cache and arranges propagation to other runtimes.
/// Children read the latest cached value at the existing preparation and step boundaries;
/// updates do not change requests already in flight. `None` restores the default tier.
pub trait AgentServiceTier: Send + Sync {
    fn current(&self) -> Option<String>;
    fn update(&self, service_tier: Option<String>);
}

pub(crate) fn host_child_uses_shared_service_tier(session: &Session) -> bool {
    matches!(&session.agent_runtime, AgentRuntimeServices::Host(services) if services.service_tier.is_some())
        && session.session_id() != SessionId::from(session.thread_id())
}

pub(crate) fn service_tier(
    session: &Session,
    configured_service_tier: Option<String>,
) -> Option<String> {
    match &session.agent_runtime {
        AgentRuntimeServices::Controller => session.services.agent_control.service_tier(),
        AgentRuntimeServices::Host(services) => services
            .service_tier
            .as_ref()
            .map_or(configured_service_tier, |provider| provider.current()),
    }
}

pub(crate) fn propagate_config_update(session: &Session, update: AgentConfigUpdate) {
    match &session.agent_runtime {
        AgentRuntimeServices::Controller => session
            .services
            .agent_control
            .propagate_config_update(update),
        AgentRuntimeServices::Host(services) => {
            // Only the root can publish shared settings. Compare session and thread IDs
            // because a detached child may have no structural parent ID.
            if session.session_id() == SessionId::from(session.thread_id())
                && let Some(provider) = &services.service_tier
            {
                let AgentConfigUpdate::ServiceTier(tier) = update;
                provider.update(tier);
            }
        }
    }
}
