//! Reviews terminal input against retained launch permissions and current policy.
//! Permission snapshots stay host-owned; neither an earlier approval nor a policy
//! change alters the sandbox of an already-running process. Native launches keep
//! their configured Windows sandbox selection; executor launches use executor defaults.

use super::ProcessEntry;
use super::UnifiedExecContext;
use super::UnifiedExecError;
use crate::config::NetworkProxySpec;
use crate::session::turn_context::TurnContext;
use crate::session::turn_context::TurnEnvironment;
use crate::tools::sandboxing::ApprovalAction;
use crate::tools::sandboxing::ToolError;
use crate::windows_sandbox::windows_sandbox_level_for_legacy_checks;
use codex_features::Feature;
use codex_file_system::FileSystemSandboxContext;
use codex_network_proxy::EnvironmentNetworkPolicy;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::PermissionProfile;
use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_sandboxing::policy_transforms::effective_permission_profile;
use codex_sandboxing::policy_transforms::merge_permission_profiles;

#[derive(Clone, Copy)]
pub(crate) enum TerminalSandboxSource {
    Native,
    Executor,
}

pub(crate) struct TerminalPermissions {
    policy: TerminalPolicy,
    sandbox_source: TerminalSandboxSource,
    launch_permissions: SandboxPermissions,
    additional_permissions: Option<AdditionalPermissionProfile>,
    internal_permissions: Option<AdditionalPermissionProfile>,
}

/// Host-owned launch settings. Never serialize these into approval messages.
#[derive(PartialEq, Eq)]
struct TerminalPolicy {
    sandbox: FileSystemSandboxContext,
    environment_network: Option<EnvironmentNetworkPolicy>,
    controller_network: Option<NetworkProxySpec>,
    controller_proxy: bool,
}

impl TerminalPolicy {
    fn capture(
        environment: &TurnEnvironment,
        turn: &TurnContext,
        sandbox_source: TerminalSandboxSource,
        additional_permissions: Option<AdditionalPermissionProfile>,
    ) -> Self {
        let mut sandbox = environment.sandbox_context(additional_permissions);
        if matches!(sandbox_source, TerminalSandboxSource::Native)
            && windows_sandbox_level_for_legacy_checks(
                environment.config().windows_sandbox_type,
                environment.config().windows_sandbox_level,
            ) == codex_protocol::config_types::WindowsSandboxLevel::Disabled
        {
            // The filesystem helper applies executor defaults, but native process
            // launches honor Disabled. Preserve it so later enablement is detected.
            sandbox.windows_sandbox_selection =
                codex_protocol::config_types::WindowsSandboxLevel::Disabled.into();
        }
        Self {
            sandbox,
            environment_network: environment.config().network_policy.clone(),
            controller_network: turn.config.permissions.network.clone(),
            controller_proxy: turn.network.is_some(),
        }
    }

    fn file_system_context(&self) -> FileSystemSandboxContext {
        let mut context = self.sandbox.clone();
        // Network changes require review, not rejection for denied-read drift.
        if let PermissionProfile::Managed { network, .. } = &mut context.permissions {
            *network = NetworkSandboxPolicy::Restricted;
        }
        context
    }
}

impl TerminalPermissions {
    pub(crate) fn for_launch(
        environment: &TurnEnvironment,
        turn: &TurnContext,
        sandbox_source: TerminalSandboxSource,
        launch_permissions: SandboxPermissions,
        additional_permissions: Option<&AdditionalPermissionProfile>,
        internal_permissions: Option<&AdditionalPermissionProfile>,
    ) -> Self {
        Self {
            policy: TerminalPolicy::capture(
                environment,
                turn,
                sandbox_source,
                merge_permission_profiles(additional_permissions, internal_permissions),
            ),
            sandbox_source,
            // A bypass is a property of the successful attempt, not a difference
            // between settings: a full-access environment can still bypass a proxy.
            launch_permissions,
            additional_permissions: additional_permissions.cloned(),
            internal_permissions: internal_permissions.cloned(),
        }
    }

    /// Compares launch policy with current policy including retained grants,
    /// without reading turn state or touching a process.
    fn review_requirement(
        &self,
        current: &TerminalPolicy,
        baseline: &PermissionProfile,
    ) -> Result<SandboxPermissions, &'static str> {
        let bypassed = self.launch_permissions.requires_escalated_permissions();
        if current.environment_network.is_some()
            && (bypassed || self.policy.environment_network != current.environment_network)
        {
            return Err(
                "this terminal cannot enforce the current environment-owned network restrictions; start a new terminal",
            );
        }
        // Approval cannot retrofit denied reads onto a running process. Unless
        // its sandbox still matches, start a new terminal under the current policy.
        if baseline
            .file_system_sandbox_policy()
            .has_denied_read_restrictions()
            && (bypassed || self.policy.file_system_context() != current.file_system_context())
        {
            return Err(
                "this terminal cannot enforce the current denied-read restrictions; start a new terminal",
            );
        }
        // Once the retained settings match, only the baseline permissions can differ.
        Ok(if bypassed || &self.policy != current {
            SandboxPermissions::RequireEscalated
        } else if self.policy.sandbox.permissions
            == effective_permission_profile(baseline, /*additional_permissions*/ None)
        {
            SandboxPermissions::UseDefault
        } else {
            SandboxPermissions::WithAdditionalPermissions
        })
    }

    fn approval_reason(
        &self,
        sandbox_permissions: SandboxPermissions,
    ) -> Result<String, serde_json::Error> {
        let authority = if self.launch_permissions.requires_escalated_permissions() {
            "This terminal was launched outside the sandbox, bypassing any managed network proxy."
        } else if self.policy.sandbox.permissions == PermissionProfile::Disabled {
            "This terminal runs without a filesystem sandbox."
        } else {
            match sandbox_permissions {
                SandboxPermissions::UseDefault => "This terminal uses the current permissions.",
                SandboxPermissions::WithAdditionalPermissions => {
                    "This terminal retains additional permissions."
                }
                SandboxPermissions::RequireEscalated => {
                    "This terminal retains sandbox or network settings that differ from the current permissions."
                }
            }
        };
        let mut reason = format!("Send input to an existing terminal. {authority}");
        if self.internal_permissions.is_some() {
            reason.push_str(" It also has an internal filesystem grant.");
        }
        reason.push_str(" The cwd is its launch directory; the terminal's current directory and state may have changed.");
        if let Some(grants) = &self.additional_permissions {
            // Stable reason text also reaches clients that strip the experimental
            // additionalPermissions field. Internal paths never enter this text.
            reason.push_str(&format!(
                " Retained grants: {}.",
                serde_json::to_string(grants)?
            ));
        }
        Ok(reason)
    }
}

impl ProcessEntry {
    pub(super) fn stdin_approval(
        &self,
        context: &UnifiedExecContext,
        input: &str,
        strict_auto_review: bool,
    ) -> Result<Option<(ApprovalAction, String)>, UnifiedExecError> {
        if input.is_empty()
            || (!self.tty && input == "\u{3}")
            || !context
                .session
                .features()
                .enabled(Feature::WriteStdinApproval)
        {
            return Ok(None);
        }
        let environment = context
            .step_context
            .environments
            .turn_environments()
            .find(|environment| environment.selection.environment_id == self.environment_id)
            .ok_or_else(|| {
                approval_error(
                    "cannot access the terminal's original environment; select it before retrying",
                )
            })?;
        let permissions = &self.permissions;
        let current = TerminalPolicy::capture(
            environment,
            &context.step_context.turn,
            permissions.sandbox_source,
            merge_permission_profiles(
                permissions.additional_permissions.as_ref(),
                permissions.internal_permissions.as_ref(),
            ),
        );
        let sandbox_permissions = permissions
            .review_requirement(&current, environment.permission_profile())
            .map_err(approval_error)?;
        if sandbox_permissions == SandboxPermissions::UseDefault && !strict_auto_review {
            return Ok(None);
        }
        // Manual approvals shell-quote the input, which cannot preserve NUL bytes.
        if input.contains('\0') {
            return Err(approval_error(
                "terminal input contains a NUL byte and cannot be reviewed safely",
            ));
        }
        let reason = permissions
            .approval_reason(sandbox_permissions)
            .map_err(approval_error)?;
        let action = ApprovalAction::WriteStdin {
            id: self.call_id.clone(),
            approval_id: context.call_id.clone(),
            environment_id: self.environment_id.clone(),
            process_id: self.process_id,
            input: input.to_string(),
            cwd: self.cwd.clone(),
            tty: self.tty,
            sandbox_permissions,
            additional_permissions: permissions.additional_permissions.clone(),
        };
        Ok(Some((action, reason)))
    }
}

fn approval_error(reason: impl std::fmt::Display) -> UnifiedExecError {
    UnifiedExecError::StdinApproval(ToolError::Rejected(reason.to_string()))
}

#[cfg(test)]
#[path = "stdin_approval_tests.rs"]
mod tests;
