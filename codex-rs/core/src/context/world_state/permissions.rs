use super::PreviousSectionState;
use super::WorldStateSection;
use crate::context::ApprovalPromptContext;
use crate::context::ContextualUserFragment;
use crate::context::PermissionsInstructions;
use crate::session::turn_context::TurnContext;
use codex_execpolicy::Policy;
use codex_features::Feature;
use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;

/// The permissions guidance currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct PermissionsState {
    instructions: Option<PermissionsInstructions>,
    snapshot: PermissionsSnapshot,
}

/// Persisted inputs that determine when permissions guidance must be rendered again.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub(crate) struct PermissionsSnapshot {
    enabled: bool,
    model_slug: Option<String>,
    instructions_fingerprint: Option<String>,
}

impl PermissionsState {
    pub(crate) fn from_turn_context(turn_context: &TurnContext, exec_policy: &Policy) -> Self {
        if !turn_context.config.include_permissions_instructions {
            return Self::disabled();
        }

        let approval_messages = turn_context
            .model_info
            .model_messages
            .as_ref()
            .and_then(|messages| messages.approvals.as_ref());
        let instructions = PermissionsInstructions::from_permission_profile(
            &turn_context.permission_profile,
            turn_context.approval_policy.value(),
            ApprovalPromptContext::new(turn_context.config.approvals_reviewer, approval_messages),
            exec_policy,
            #[allow(deprecated)]
            &turn_context.cwd,
            turn_context
                .config
                .features
                .enabled(Feature::ExecPermissionApprovals),
            turn_context
                .config
                .features
                .enabled(Feature::RequestPermissionsTool),
        );
        Self::enabled(instructions, turn_context.model_info.slug.as_str())
    }

    pub(crate) fn enabled(instructions: PermissionsInstructions, model_slug: &str) -> Self {
        let instructions_fingerprint = Some(format!(
            "sha1:{:x}",
            sha1::Sha1::digest(instructions.body().as_bytes())
        ));

        Self {
            instructions: Some(instructions),
            snapshot: PermissionsSnapshot {
                enabled: true,
                model_slug: Some(model_slug.to_string()),
                instructions_fingerprint,
            },
        }
    }

    pub(crate) fn disabled() -> Self {
        Self {
            instructions: None,
            snapshot: PermissionsSnapshot::default(),
        }
    }
}

impl WorldStateSection for PermissionsState {
    const ID: &'static str = "permissions";
    type Snapshot = PermissionsSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        self.snapshot.clone()
    }

    fn matches_legacy_fragment(role: &str, text: &str) -> bool {
        role == "developer" && PermissionsInstructions::matches_text(text)
    }

    fn has_retained_fragment_matcher() -> bool {
        true
    }

    fn matches_retained_fragment(role: &str, text: &str) -> bool {
        Self::matches_legacy_fragment(role, text)
    }

    fn render_diff(
        &self,
        previous: PreviousSectionState<'_, Self::Snapshot>,
    ) -> Option<Box<dyn ContextualUserFragment>> {
        if matches!(previous, PreviousSectionState::Known(previous) if previous == &self.snapshot) {
            return None;
        }

        self.instructions
            .clone()
            .map(|instructions| Box::new(instructions) as Box<dyn ContextualUserFragment>)
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
