use codex_apply_patch::ApplyPatchAction;
use codex_apply_patch::ApplyPatchFileChange;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::permissions::LocalFileSystemPolicyMatcher;
use codex_protocol::protocol::AskForApproval;
use codex_sandboxing::get_platform_sandbox;
use codex_utils_path_uri::PathUri;
use std::io;

const PATCH_REJECTED_OUTSIDE_PROJECT_REASON: &str =
    "writing outside of the project; rejected by user approval settings";
const PATCH_REJECTED_READ_ONLY_REASON: &str =
    "writing is blocked by read-only sandbox; rejected by user approval settings";

#[derive(Debug, PartialEq)]
pub enum SafetyCheck {
    AutoApprove,
    AskUser,
    Reject { reason: String },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PatchSandboxRoute {
    ExecutorManaged,
    Platform(WindowsSandboxLevel),
}

/// Keeps configured permissions and prepared local matching separate for one patch.
pub(crate) struct PatchPolicyMatcher<'a> {
    configured_policy: &'a FileSystemSandboxPolicy,
    pub(crate) context: FileSystemSandboxPolicyContext<'a>,
    pub(crate) sandbox_route: PatchSandboxRoute,
    local_matching: Option<LocalFileSystemPolicyMatcher<'a>>,
}

impl PatchSandboxRoute {
    pub(crate) fn prepare_matching<'a>(
        self,
        configured_policy: &'a FileSystemSandboxPolicy,
        context: &FileSystemSandboxPolicyContext<'a>,
    ) -> io::Result<PatchPolicyMatcher<'a>> {
        let local_matching = match self {
            Self::Platform(_) => Some(configured_policy.prepare_local_matching(context)?),
            Self::ExecutorManaged => None,
        };
        Ok(PatchPolicyMatcher {
            configured_policy,
            context: *context,
            sandbox_route: self,
            local_matching,
        })
    }
}

impl PatchPolicyMatcher<'_> {
    pub(crate) fn can_write_path(&self, path: &PathUri) -> io::Result<bool> {
        match &self.local_matching {
            Some(matching) => matching.can_write_path(path),
            None => Ok(self.configured_policy.can_write_path(path, &self.context)),
        }
    }
}

pub fn assess_patch_safety(
    action: &ApplyPatchAction,
    policy: AskForApproval,
    permission_profile: &PermissionProfile,
    matching: &PatchPolicyMatcher<'_>,
) -> io::Result<SafetyCheck> {
    if action.is_empty() {
        return Ok(SafetyCheck::Reject {
            reason: "empty patch".to_string(),
        });
    }

    match policy {
        AskForApproval::Never | AskForApproval::OnRequest | AskForApproval::Granular(_) => {
            // Continue to see if this can be auto-approved.
        }
        // TODO(ragona): I'm not sure this is actually correct? I believe in this case
        // we want to continue to the writable paths check before asking the user.
        AskForApproval::UnlessTrusted => {
            return Ok(SafetyCheck::AskUser);
        }
    }

    let rejects_sandbox_approval = matches!(policy, AskForApproval::Never)
        || matches!(
            policy,
            AskForApproval::Granular(granular_config) if !granular_config.sandbox_approval
        );
    let sandbox_available = match matching.sandbox_route {
        PatchSandboxRoute::ExecutorManaged => true,
        PatchSandboxRoute::Platform(windows_sandbox_level) => {
            get_platform_sandbox(windows_sandbox_level != WindowsSandboxLevel::Disabled).is_some()
        }
    };

    // Even though the patch appears to be constrained to writable paths, it is
    // possible that paths in the patch are hard links to files outside the
    // writable roots, so we should still run `apply_patch` in a sandbox in that case.
    // Disabled and External profiles intentionally do not apply an outer sandbox.
    if is_write_patch_constrained_to_writable_paths(action, matching)?
        && (matches!(
            permission_profile,
            PermissionProfile::Disabled | PermissionProfile::External { .. }
        ) || sandbox_available)
    {
        Ok(SafetyCheck::AutoApprove)
    } else if rejects_sandbox_approval {
        Ok(SafetyCheck::Reject {
            reason: patch_rejection_reason(
                permission_profile,
                matching.configured_policy,
                &matching.context,
            )
            .to_string(),
        })
    } else {
        Ok(SafetyCheck::AskUser)
    }
}

fn patch_rejection_reason(
    permission_profile: &PermissionProfile,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> &'static str {
    let has_no_writable_roots = !file_system_sandbox_policy.has_configured_writable_roots(context);
    match permission_profile {
        PermissionProfile::Managed { .. }
            if !file_system_sandbox_policy.has_full_disk_write_access_with_context(context)
                && has_no_writable_roots =>
        {
            PATCH_REJECTED_READ_ONLY_REASON
        }
        PermissionProfile::Managed { .. }
        | PermissionProfile::Disabled
        | PermissionProfile::External { .. } => PATCH_REJECTED_OUTSIDE_PROJECT_REASON,
    }
}

fn is_write_patch_constrained_to_writable_paths(
    action: &ApplyPatchAction,
    matching: &PatchPolicyMatcher<'_>,
) -> io::Result<bool> {
    // A full-disk policy permits every patch target, so no per-path writable-root check can
    // further constrain the result.
    if matching
        .configured_policy
        .has_full_disk_write_access_with_context(&matching.context)
    {
        return Ok(true);
    }

    for (path, change) in action.changes() {
        match change {
            ApplyPatchFileChange::Add { .. } | ApplyPatchFileChange::Delete { .. } => {
                if !matching.can_write_path(path)? {
                    return Ok(false);
                }
            }
            ApplyPatchFileChange::Update { move_path, .. } => {
                if !matching.can_write_path(path)? {
                    return Ok(false);
                }
                if let Some(dest) = move_path
                    && !matching.can_write_path(dest)?
                {
                    return Ok(false);
                }
            }
        }
    }

    Ok(true)
}

#[cfg(test)]
#[path = "safety_tests.rs"]
mod tests;
