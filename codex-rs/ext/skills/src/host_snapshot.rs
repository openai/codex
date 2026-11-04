use std::io;
use std::sync::Arc;

use crate::SkillLoadOutcome;
use codex_exec_server::EnvironmentAccess;
use codex_extension_api::ExtensionData;
use codex_skills::SkillMetadata;

/// Immutable snapshot of host-owned skills and their source filesystems.
#[derive(Debug, Clone)]
pub struct HostSkillsSnapshot {
    outcome: Arc<SkillLoadOutcome>,
}

impl HostSkillsSnapshot {
    pub fn new(outcome: Arc<SkillLoadOutcome>) -> Self {
        Self { outcome }
    }

    /// Prefer the snapshot prepared with this step's permissions when the host supplies one.
    pub fn for_step(step_store: &ExtensionData, turn_store: &ExtensionData) -> Option<Arc<Self>> {
        step_store
            .get::<Self>()
            .or_else(|| turn_store.get::<Self>())
    }

    pub fn outcome(&self) -> &SkillLoadOutcome {
        self.outcome.as_ref()
    }

    pub async fn read_skill_text(
        &self,
        skill: &SkillMetadata,
        accessors: &[&dyn EnvironmentAccess],
    ) -> io::Result<String> {
        self.outcome.read_skill_text(skill, accessors).await
    }
}
