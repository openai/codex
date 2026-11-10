//! Select host-owned shared service-tier state at existing runtime configuration boundaries.

use crate::agent::api::AgentConfigUpdate;
use crate::agent::host::AgentRuntimeServices;
use crate::session::session::Session;
use codex_protocol::SessionId;
use futures::FutureExt;
use futures::future::BoxFuture;
use futures::future::Shared;

pub(crate) type ServiceTierAcceptance = Shared<BoxFuture<'static, Result<(), String>>>;

/// The host's view of the service tier shared within an agent session.
///
/// Install an `Arc<dyn AgentServiceTier>` in thread extension data before startup. Host-owned
/// children with a provider reject explicit per-turn tier overrides. Host-owned runtimes without
/// a provider retain their configured tier and do not propagate shared tier updates.
/// Controller-owned runtimes use their controller.
/// Core awaits reads during preparation and step capture. A host may answer from a cache.
/// Hosts own cross-runtime propagation and recovery. Until an update reaches a runtime,
/// its `current` may return the prior tier.
/// Children read the latest value at the existing preparation and step boundaries;
/// updates do not change requests already in flight. `None` restores the default tier.
pub trait AgentServiceTier: Send + Sync {
    fn current(&self) -> BoxFuture<'_, Option<String>>;

    /// Hand off the committed tier in call order and return a receipt for local acceptance.
    /// Core calls this method under its commit locks: the handoff must be fast and nonblocking.
    /// Drive the update independently of the receipt: dropping the receipt only cancels the wait.
    /// Core waits outside the state and registry locks. `Ok(())` confirms local acceptance.
    /// `Err` leaves acceptance unconfirmed and carries a caller-safe message. Core surfaces it
    /// in an explicit thread settings reply. A later settings request with a reply can hand off
    /// the same tier again after Core observes a failed receipt. The host must preserve commit
    /// order and tolerate repeated handoffs.
    fn update(&self, service_tier: Option<String>) -> BoxFuture<'static, Result<(), String>>;
}

pub(crate) fn host_child_uses_shared_service_tier(session: &Session) -> bool {
    matches!(&session.agent_runtime, AgentRuntimeServices::Host(services) if services.service_tier.is_some())
        && session.session_id() != SessionId::from(session.thread_id())
}

pub(crate) async fn service_tier(
    session: &Session,
    configured_service_tier: Option<String>,
) -> Option<String> {
    match &session.agent_runtime {
        AgentRuntimeServices::Controller => session.services.agent_control.service_tier(),
        AgentRuntimeServices::Host(services) => match &services.service_tier {
            Some(provider) => provider.current().await,
            None => configured_service_tier,
        },
    }
}

pub(crate) fn propagate_config_update(
    session: &Session,
    update: AgentConfigUpdate,
) -> Option<ServiceTierAcceptance> {
    match &session.agent_runtime {
        AgentRuntimeServices::Controller => {
            session
                .services
                .agent_control
                .propagate_config_update(update);
            None
        }
        AgentRuntimeServices::Host(services) => {
            // Only the root can publish shared settings. Compare session and thread IDs
            // because a detached child may have no structural parent ID.
            if session.session_id() != SessionId::from(session.thread_id()) {
                return None;
            }
            let AgentConfigUpdate::ServiceTier(tier) = update;
            Some(services.service_tier.as_ref()?.update(tier).shared())
        }
    }
}
