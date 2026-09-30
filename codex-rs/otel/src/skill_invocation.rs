//! Exports skill-use metadata without skill contents, filesystem paths, or tool payloads.
//! Detection belongs to the caller; an invocation does not imply successful task completion.

use crate::SessionTelemetry;
use crate::events::shared::timestamp;
use crate::targets::OTEL_LOG_ONLY_TARGET;
use codex_protocol::protocol::SkillScope;

/// How Codex detected the skill's use.
pub enum SkillInvocationType {
    Explicit,
    Implicit,
}

/// Metadata for a detected skill invocation. Never include skill contents or resource paths.
pub struct SkillInvocationEvent<'a> {
    pub turn_id: &'a str,
    pub skill_name: &'a str,
    pub scope: Option<SkillScope>,
    pub plugin_id: Option<&'a str>,
    pub invocation_type: SkillInvocationType,
}

impl SessionTelemetry {
    pub fn with_user_id(mut self, user_id: Option<String>) -> Self {
        self.metadata.user_id = user_id;
        self
    }

    pub fn skill_invocation(&self, event: SkillInvocationEvent<'_>) {
        let invocation_type = match event.invocation_type {
            SkillInvocationType::Explicit => "explicit",
            SkillInvocationType::Implicit => "implicit",
        };
        let scope = event.scope.map(|scope| match scope {
            SkillScope::User => "user",
            SkillScope::Repo => "repo",
            SkillScope::System => "system",
            SkillScope::Admin => "admin",
        });
        tracing::event!(
            name: "codex.skill_invocation",
            target: OTEL_LOG_ONLY_TARGET,
            tracing::Level::INFO,
            event.name = "codex.skill_invocation",
            event.timestamp = %timestamp(),
            conversation.id = %self.metadata.conversation_id,
            turn.id = event.turn_id,
            user.id = self.metadata.user_id.as_deref(),
            user.account_id = self.metadata.account_id.as_deref(),
            skill.name = event.skill_name,
            skill.scope = scope,
            skill.plugin_id = event.plugin_id,
            skill.invocation_type = invocation_type,
            model = %self.metadata.model,
            app.version = self.metadata.app_version,
            originator = %self.metadata.originator,
            auth_mode = self.metadata.auth_mode.as_deref(),
        );
    }
}
