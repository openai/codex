/*
Module: sandboxing

Core-owned adapter types for exec/runtime plumbing. Policy selection and
command transformation live in the codex-sandboxing crate; this module keeps
the exec-only metadata and translates transformed sandbox commands back into
ExecRequest for execution.
*/

use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::exec::StdoutStream;
use crate::exec::WindowsSandboxFilesystemOverrides;
use crate::exec::execute_exec_request;
#[cfg(target_os = "macos")]
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::ShellEnvironmentPolicy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::models::PermissionProfile;
pub use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxExecRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::compatibility_sandbox_policy_for_permission_profile;
use codex_utils_absolute_path::AbsolutePathBuf;
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct ExecOptions {
    pub(crate) expiration: ExecExpiration,
    pub(crate) capture_policy: ExecCapturePolicy,
}

#[derive(Clone, Debug)]
pub(crate) struct ExecServerEnvConfig {
    pub(crate) policy: codex_exec_server::ExecEnvPolicy,
    pub(crate) local_policy_env: HashMap<String, String>,
}

impl ExecServerEnvConfig {
    pub(crate) fn from_shell_policy(
        policy: &ShellEnvironmentPolicy,
        local_policy_env: HashMap<String, String>,
    ) -> Self {
        Self {
            policy: codex_exec_server::ExecEnvPolicy {
                inherit: policy.inherit.clone(),
                ignore_default_excludes: policy.ignore_default_excludes,
                exclude: policy
                    .exclude
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
                r#set: policy.r#set.clone(),
                include_only: policy
                    .include_only
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
            },
            local_policy_env,
        }
    }

    pub(crate) fn env_overlay(
        &self,
        request_env: &HashMap<String, String>,
    ) -> HashMap<String, String> {
        request_env
            .iter()
            .filter(|(key, value)| self.local_policy_env.get(*key) != Some(*value))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }
}

#[derive(Debug)]
pub struct ExecRequest {
    pub command: Vec<String>,
    pub cwd: AbsolutePathBuf,
    pub env: HashMap<String, String>,
    pub(crate) exec_server_env_config: Option<ExecServerEnvConfig>,
    pub network: Option<NetworkProxy>,
    pub expiration: ExecExpiration,
    pub capture_policy: ExecCapturePolicy,
    pub sandbox: SandboxType,
    pub windows_sandbox_policy_cwd: AbsolutePathBuf,
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub windows_sandbox_private_desktop: bool,
    pub permission_profile: PermissionProfile,
    pub file_system_sandbox_policy: FileSystemSandboxPolicy,
    pub network_sandbox_policy: NetworkSandboxPolicy,
    pub(crate) windows_sandbox_filesystem_overrides: Option<WindowsSandboxFilesystemOverrides>,
    pub arg0: Option<String>,
}

impl ExecRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command: Vec<String>,
        cwd: AbsolutePathBuf,
        env: HashMap<String, String>,
        network: Option<NetworkProxy>,
        expiration: ExecExpiration,
        capture_policy: ExecCapturePolicy,
        sandbox: SandboxType,
        windows_sandbox_level: WindowsSandboxLevel,
        windows_sandbox_private_desktop: bool,
        permission_profile: PermissionProfile,
        arg0: Option<String>,
    ) -> Self {
        let windows_sandbox_policy_cwd = cwd.clone();
        let (file_system_sandbox_policy, network_sandbox_policy) =
            permission_profile.to_runtime_permissions();
        Self {
            command,
            cwd,
            env,
            exec_server_env_config: None,
            network,
            expiration,
            capture_policy,
            sandbox,
            windows_sandbox_policy_cwd,
            windows_sandbox_level,
            windows_sandbox_private_desktop,
            permission_profile,
            file_system_sandbox_policy,
            network_sandbox_policy,
            windows_sandbox_filesystem_overrides: None,
            arg0,
        }
    }

    pub(crate) fn compatibility_sandbox_policy(&self) -> SandboxPolicy {
        compatibility_sandbox_policy_for_permission_profile(
            &self.permission_profile,
            &self.file_system_sandbox_policy,
            self.network_sandbox_policy,
            self.windows_sandbox_policy_cwd.as_path(),
        )
    }

    pub(crate) fn from_sandbox_exec_request(
        request: SandboxExecRequest,
        options: ExecOptions,
        windows_sandbox_policy_cwd: AbsolutePathBuf,
    ) -> Self {
        let SandboxExecRequest {
            command,
            cwd,
            mut env,
            network,
            sandbox,
            windows_sandbox_level,
            windows_sandbox_private_desktop,
            permission_profile,
            file_system_sandbox_policy,
            network_sandbox_policy,
            arg0,
        } = request;
        let ExecOptions {
            expiration,
            capture_policy,
        } = options;
        if !network_sandbox_policy.is_enabled() {
            env.insert(
                CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_string(),
                "1".to_string(),
            );
        }
        #[cfg(target_os = "macos")]
        if sandbox == SandboxType::MacosSeatbelt {
            env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
        }
        Self {
            command,
            cwd,
            env,
            exec_server_env_config: None,
            network,
            expiration,
            capture_policy,
            sandbox,
            windows_sandbox_policy_cwd,
            windows_sandbox_level,
            windows_sandbox_private_desktop,
            permission_profile,
            file_system_sandbox_policy,
            network_sandbox_policy,
            windows_sandbox_filesystem_overrides: None,
            arg0,
        }
    }
}

pub async fn execute_env(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
) -> codex_protocol::error::Result<ExecToolCallOutput> {
    execute_exec_request(exec_request, stdout_stream, /*after_spawn*/ None).await
}

pub async fn execute_exec_request_with_after_spawn(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> codex_protocol::error::Result<ExecToolCallOutput> {
    execute_exec_request(exec_request, stdout_stream, after_spawn).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::config_types::EnvironmentVariablePattern;
    use codex_protocol::config_types::ShellEnvironmentPolicyInherit;
    use pretty_assertions::assert_eq;

    #[test]
    fn exec_server_env_config_from_shell_policy_preserves_policy_fields() {
        let policy = ShellEnvironmentPolicy {
            inherit: ShellEnvironmentPolicyInherit::Core,
            ignore_default_excludes: false,
            exclude: vec![EnvironmentVariablePattern::new_case_insensitive("SECRET_*")],
            r#set: HashMap::from([("FROM_POLICY".to_string(), "set-value".to_string())]),
            include_only: vec![EnvironmentVariablePattern::new_case_insensitive("*PATH")],
            use_profile: true,
        };
        let local_policy_env = HashMap::from([
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("FROM_POLICY".to_string(), "set-value".to_string()),
        ]);

        let config = ExecServerEnvConfig::from_shell_policy(&policy, local_policy_env.clone());

        assert_eq!(config.policy.inherit, ShellEnvironmentPolicyInherit::Core);
        assert!(!config.policy.ignore_default_excludes);
        assert_eq!(
            config.policy.exclude,
            vec![EnvironmentVariablePattern::new_case_insensitive("SECRET_*")]
        );
        assert_eq!(config.policy.r#set, policy.r#set);
        assert_eq!(
            config.policy.include_only,
            vec![EnvironmentVariablePattern::new_case_insensitive("*PATH")]
        );
        assert_eq!(config.local_policy_env, local_policy_env);
    }

    #[test]
    fn exec_server_env_config_env_overlay_keeps_only_runtime_changes() {
        let config = ExecServerEnvConfig {
            policy: codex_exec_server::ExecEnvPolicy {
                inherit: ShellEnvironmentPolicyInherit::Core,
                ignore_default_excludes: false,
                exclude: Vec::new(),
                r#set: HashMap::new(),
                include_only: Vec::new(),
            },
            local_policy_env: HashMap::from([
                ("HOME".to_string(), "/client-home".to_string()),
                ("PATH".to_string(), "/client-path".to_string()),
                ("SHELL_SET".to_string(), "policy".to_string()),
            ]),
        };
        let request_env = HashMap::from([
            ("HOME".to_string(), "/client-home".to_string()),
            ("PATH".to_string(), "/sandbox-path".to_string()),
            ("SHELL_SET".to_string(), "policy".to_string()),
            ("CODEX_THREAD_ID".to_string(), "thread-1".to_string()),
        ]);

        assert_eq!(
            config.env_overlay(&request_env),
            HashMap::from([
                ("PATH".to_string(), "/sandbox-path".to_string()),
                ("CODEX_THREAD_ID".to_string(), "thread-1".to_string()),
            ])
        );
    }
}
