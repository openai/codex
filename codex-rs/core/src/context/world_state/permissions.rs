//! Tracks model-visible permission instructions and approved-command prefix changes.

use super::PreviousSectionState;
use super::SectionTransition;
use super::WorldStateContextFragment;
use super::WorldStateHash;
use super::WorldStateSection;
use super::WorldStateUpdate;
use crate::context::ApprovedCommandPrefixSaved;
use crate::context::ContextualUserFragment;
use crate::context::PermissionsInstructions;
use crate::context::environment_context::push_xml_escaped_text;
use codex_execpolicy::Policy;
use codex_extension_api::RenderedWorldStateFragment;
use codex_prompts::ApprovalPromptContext;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ContentItemKind;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::format_allow_prefixes;
use codex_protocol::models::input_text;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::protocol::AskForApproval;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

/// Permission instructions currently visible to the model.
#[derive(Clone, Debug)]
pub(crate) struct PermissionsState {
    saved_prefix_heading: Option<String>,
    snapshot: PermissionsSnapshot,
    // None is unscoped text: the legacy description or the thread-default fallback.
    descriptions: BTreeMap<Option<String>, PermissionDescription>,
}

/// One permission description and its comparison value, excluding saved prefixes.
#[derive(Clone, Debug)]
pub(crate) struct PermissionDescription {
    instructions: PermissionsInstructions,
    fingerprint: WorldStateHash,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum PermissionsSnapshot {
    Scoped {
        environments: BTreeMap<String, WorldStateHash>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread_defaults: Option<WorldStateHash>,
        approved_command_prefixes: BTreeSet<Vec<String>>,
    },
    Current {
        instructions: WorldStateHash,
        approved_command_prefixes: BTreeSet<Vec<String>>,
    },
    Legacy(WorldStateHash),
}

impl PermissionDescription {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        permission_profile: &PermissionProfile,
        approval_policy: AskForApproval,
        approval_context: ApprovalPromptContext<'_>,
        exec_policy: &Policy,
        cwd: &Path,
        paths: Option<&FileSystemSandboxPolicyContext<'_>>,
        exec_permission_approvals_enabled: bool,
        request_permissions_tool_enabled: bool,
    ) -> Self {
        let build_instructions = |exec_policy| {
            PermissionsInstructions::from_permission_profile(
                permission_profile,
                approval_policy,
                approval_context,
                exec_policy,
                cwd,
                paths,
                exec_permission_approvals_enabled,
                request_permissions_tool_enabled,
            )
        };
        let instructions = build_instructions(exec_policy);
        let instructions_without_approved_prefixes = build_instructions(&Policy::empty());
        Self {
            instructions,
            fingerprint: WorldStateHash::from_fragment(&instructions_without_approved_prefixes),
        }
    }
}

impl PermissionsState {
    pub(crate) fn new(
        descriptions: BTreeMap<Option<String>, PermissionDescription>,
        exec_policy: &Policy,
        approval_context: ApprovalPromptContext<'_>,
        per_environment: bool,
    ) -> Self {
        let approved_command_prefixes = exec_policy.get_allowed_prefixes().into_iter().collect();
        let snapshot = if per_environment {
            PermissionsSnapshot::Scoped {
                environments: descriptions
                    .iter()
                    .filter_map(|(id, description)| {
                        Some((id.clone()?, description.fingerprint.clone()))
                    })
                    .collect(),
                thread_defaults: descriptions
                    .get(&None)
                    .map(|description| description.fingerprint.clone()),
                approved_command_prefixes,
            }
        } else {
            PermissionsSnapshot::Current {
                instructions: descriptions[&None].fingerprint.clone(),
                approved_command_prefixes,
            }
        };
        Self {
            saved_prefix_heading: approval_context
                .approved_command_prefix_saved_heading()
                .map(str::to_owned),
            snapshot,
            descriptions,
        }
    }
}

impl WorldStateSection for PermissionsState {
    const ID: &'static str = "permissions";
    type Snapshot = PermissionsSnapshot;

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
    ) -> SectionTransition<Self::Snapshot> {
        if let PreviousSectionState::Known(previous) = &previous
            && *previous == &self.snapshot
        {
            return (None, Vec::new());
        }
        let current = self.snapshot.clone();
        if let PermissionsSnapshot::Scoped {
            approved_command_prefixes,
            ..
        } = &self.snapshot
        {
            let mut updates = Vec::new();
            let mut previous_descriptions = BTreeMap::new();
            let mut previous_prefixes = None;
            if let PreviousSectionState::Known(PermissionsSnapshot::Scoped {
                environments,
                thread_defaults,
                approved_command_prefixes,
            }) = previous
            {
                previous_descriptions.extend(
                    environments
                        .iter()
                        .map(|(id, hash)| (Some(id.clone()), hash)),
                );
                previous_descriptions.extend(thread_defaults.as_ref().map(|hash| (None, hash)));
                previous_prefixes = Some(approved_command_prefixes);
            }
            // An environment can remain selected but stop being ready. Withdraw its old
            // description even when the separate environment-context section is disabled.
            for id in previous_descriptions
                .keys()
                .filter(|id| !self.descriptions.contains_key(*id))
            {
                updates.push(scoped_permissions_update(
                    id.as_deref(),
                    /*instructions*/ None,
                ));
            }
            // Switching models can make saved command approvals stop applying.
            // We track these separately from each environment's other rules.
            // Additions get a short notice; removals require the full current rules.
            let prefixes_removed = previous_prefixes
                .is_some_and(|prefixes| !prefixes.is_subset(approved_command_prefixes));
            for (id, description) in &self.descriptions {
                if prefixes_removed
                    || previous_descriptions.get(id).copied() != Some(&description.fingerprint)
                {
                    updates.push(scoped_permissions_update(
                        id.as_deref(),
                        Some(&description.instructions),
                    ));
                }
            }
            // Saved prefixes apply to every environment, including those whose own
            // description did not change. A removal instead refreshes the full text.
            if !prefixes_removed
                && let Some(previous_prefixes) = previous_prefixes
                && previous_prefixes != approved_command_prefixes
            {
                let added = approved_command_prefixes
                    .difference(previous_prefixes)
                    .cloned()
                    .collect();
                if let Some(prefixes) = format_allow_prefixes(added) {
                    updates.push(WorldStateUpdate::fragment(ApprovedCommandPrefixSaved::new(
                        prefixes,
                        self.saved_prefix_heading.as_deref(),
                    )));
                }
            }
            return (Some(current), updates);
        }
        let instructions = &self.descriptions[&None].instructions;
        if let PreviousSectionState::Known(PermissionsSnapshot::Scoped {
            environments,
            thread_defaults,
            ..
        }) = &previous
        {
            let mut updates = environments
                .keys()
                .map(|id| Some(id.as_str()))
                .chain(thread_defaults.as_ref().map(|_| None))
                .map(|id| scoped_permissions_update(id, /*instructions*/ None))
                .collect::<Vec<_>>();
            updates.push(WorldStateUpdate::fragment(instructions.clone()));
            return (Some(current), updates);
        }
        match (previous, &self.snapshot) {
            (
                PreviousSectionState::Known(PermissionsSnapshot::Current {
                    instructions: previous_instructions,
                    approved_command_prefixes: previous_prefixes,
                }),
                PermissionsSnapshot::Current {
                    instructions,
                    approved_command_prefixes,
                },
            ) if previous_instructions == instructions => {
                if previous_prefixes.is_subset(approved_command_prefixes) {
                    let added_prefixes = approved_command_prefixes
                        .difference(previous_prefixes)
                        .cloned()
                        .collect();
                    if let Some(prefixes) = format_allow_prefixes(added_prefixes) {
                        return (
                            Some(current),
                            vec![WorldStateUpdate::fragment(ApprovedCommandPrefixSaved::new(
                                prefixes,
                                self.saved_prefix_heading.as_deref(),
                            ))],
                        );
                    }
                }
            }
            (
                PreviousSectionState::Known(PermissionsSnapshot::Legacy(previous)),
                PermissionsSnapshot::Current { .. },
            ) if previous == &WorldStateHash::from_fragment(instructions) => {
                return (Some(current), Vec::new());
            }
            _ => {}
        }

        (
            Some(current),
            vec![WorldStateUpdate::fragment(instructions.clone())],
        )
    }
}

/// Labels an environment's rules, or thread defaults when no environment ID is supplied.
/// Missing instructions withdraw the previous description for that scope.
fn scoped_permissions_update(
    environment_id: Option<&str>,
    instructions: Option<&PermissionsInstructions>,
) -> WorldStateUpdate {
    let mut text = String::new();
    let end = if let Some(id) = environment_id {
        text.push_str("<environment id=\"");
        push_xml_escaped_text(&mut text, id);
        text.push_str("\">\n");
        "</environment>"
    } else {
        text.push_str("<thread_defaults>\n");
        "</thread_defaults>"
    };
    if let Some(instructions) = instructions {
        let ContentItem::InputText { text: body } = instructions.body() else {
            unreachable!("permission instructions are text")
        };
        text.push_str(&body);
    } else {
        text.push_str("The previous permission description for this scope is no longer current.\n");
    }
    text.push_str(end);
    WorldStateUpdate::fragment(WorldStateContextFragment {
        fragment: RenderedWorldStateFragment::new(
            "developer",
            PermissionsInstructions::type_markers(),
            input_text(text),
        ),
        content_kind: ContentItemKind("permissions.instructions".to_string()),
    })
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
