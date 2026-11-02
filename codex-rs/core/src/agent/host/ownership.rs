//! Captures coordination providers once, after the thread's startup contributors finish.

use super::AgentTurnAdmission;
use codex_extension_api::ExtensionData;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use std::sync::Arc;

/// Selects the runtime's coordination owner at startup. Installing providers alone
/// does not select host ownership.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentRuntimeOwnership {
    #[default]
    Controller,
    /// Requires an [`AgentTurnAdmission`] provider. Core skips controller rollout-budget
    /// accounting, budget reminders, and subagent roster context for this runtime.
    Host,
}

pub(crate) enum AgentRuntimeServices {
    Controller,
    Host(HostAgentServices),
}

pub(crate) struct HostAgentServices {
    pub(crate) turn_admission: Arc<dyn AgentTurnAdmission>,
}

impl AgentRuntimeServices {
    pub(crate) fn from_extensions(data: &ExtensionData) -> Result<Self> {
        if data.get::<AgentRuntimeOwnership>().as_deref() != Some(&AgentRuntimeOwnership::Host) {
            return Ok(Self::Controller);
        }
        let turn_admission = data.get::<Arc<dyn AgentTurnAdmission>>().ok_or_else(|| {
            CodexErr::InvalidRequest(
                "host coordination requires AgentTurnAdmission before startup".into(),
            )
        })?;
        Ok(Self::Host(HostAgentServices {
            turn_admission: Arc::clone(turn_admission.as_ref()),
        }))
    }
}
