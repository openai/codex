//! Captures configuration-derived sandbox labels and writes them to diagnostics.
//! Labels never inspect the filesystem and must not be used for authorization.

use crate::responses_metadata::CodexResponsesMetadata;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxType;
use codex_sandboxing::get_platform_sandbox;
use codex_sandboxing::policy_transforms::should_require_platform_sandbox;
use std::path::Path;

/// Diagnostic labels captured with a turn's permission configuration.
/// These labels must never be used to authorize filesystem access.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SandboxTags {
    sandbox: &'static str,
    policy: &'static str,
}

impl SandboxTags {
    pub(crate) fn new(
        profile: &PermissionProfile,
        cwd: &Path,
        windows_sandbox_level: WindowsSandboxLevel,
        enforce_managed_network: bool,
    ) -> Self {
        Self {
            sandbox: permission_profile_sandbox_tag(
                profile,
                windows_sandbox_level,
                enforce_managed_network,
            ),
            policy: permission_profile_policy_tag(profile, cwd),
        }
    }

    /// Adds the captured labels to a tool's metric attributes.
    pub(crate) fn append_metric_tags(&self, tags: &mut Vec<(&str, &str)>) {
        tags.extend([("sandbox", self.sandbox), ("sandbox_policy", self.policy)]);
    }

    /// Records the same captured labels in model and MCP request metadata.
    pub(crate) fn record_metadata(&self, metadata: &mut CodexResponsesMetadata) {
        metadata.sandbox = Some(self.sandbox.to_string());
        metadata.sandbox_mode = Some(self.policy.to_string());
    }
}

/// Records policy metadata for detached requests without selecting a sandbox backend.
pub(crate) fn record_policy_metadata(
    profile: &PermissionProfile,
    cwd: &Path,
    metadata: &mut CodexResponsesMetadata,
) {
    metadata.sandbox_mode = Some(permission_profile_policy_tag(profile, cwd).to_string());
}

fn permission_profile_sandbox_tag(
    profile: &PermissionProfile,
    windows_sandbox_level: WindowsSandboxLevel,
    enforce_managed_network: bool,
) -> &'static str {
    match profile {
        PermissionProfile::Disabled => return "none",
        PermissionProfile::External { .. } => return "external",
        PermissionProfile::Managed {
            file_system,
            network,
        } => {
            let file_system_policy = file_system.to_sandbox_policy();
            if !should_require_platform_sandbox(
                &file_system_policy,
                *network,
                enforce_managed_network,
            ) {
                return "none";
            }
        }
    }
    if cfg!(target_os = "windows") {
        match windows_sandbox_level {
            WindowsSandboxLevel::Elevated => return "windows_elevated",
            WindowsSandboxLevel::Disabled | WindowsSandboxLevel::RestrictedToken => {}
        }
    }

    get_platform_sandbox(windows_sandbox_level != WindowsSandboxLevel::Disabled)
        .map(SandboxType::as_metric_tag)
        .unwrap_or("none")
}

fn permission_profile_policy_tag(profile: &PermissionProfile, cwd: &Path) -> &'static str {
    match profile {
        PermissionProfile::Disabled => "danger-full-access",
        PermissionProfile::External { .. } => "external-sandbox",
        PermissionProfile::Managed { .. } => {
            let file_system_policy = profile.file_system_sandbox_policy();
            if file_system_policy.has_full_disk_write_access() {
                "danger-full-access"
            } else if !file_system_policy.has_configured_writable_roots_with_cwd(cwd) {
                "read-only"
            } else {
                "workspace-write"
            }
        }
    }
}

#[cfg(test)]
#[path = "sandbox_tags_tests.rs"]
mod tests;
