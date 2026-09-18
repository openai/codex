use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::sandboxing::ExecOptions;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::execute_env;
use crate::tools::sandboxing::SandboxAttempt;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::SandboxErr;
use codex_protocol::models::AdditionalPermissionProfile;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[cfg(all(test, unix))]
#[path = "shell_snapshot_sandbox_tests.rs"]
mod tests;

#[derive(Clone)]
pub(crate) struct ShellSnapshotSandbox {
    sandbox: SandboxType,
    sandbox_requested: bool,
    permissions: PermissionProfile,
    enforce_managed_network: bool,
    sandbox_policy_cwd: PathUri,
    workspace_roots: Vec<PathUri>,
    sandbox_exe: Option<PathBuf>,
    use_legacy_landlock: bool,
    windows_sandbox_level: WindowsSandboxLevel,
    network: Option<NetworkProxy>,
    environment_id: String,
    additional_permissions: Option<AdditionalPermissionProfile>,
    network_denial_cancellation_token: Option<CancellationToken>,
}

pub(crate) fn snapshot_read_permissions(
    snapshot_path: &AbsolutePathBuf,
    permissions: &PermissionProfile,
    sandbox_cwd: &PathUri,
) -> Option<AdditionalPermissionProfile> {
    if let Ok(cwd) = sandbox_cwd.to_abs_path()
        && permissions
            .file_system_sandbox_policy()
            .can_read_local_path_with_cwd(snapshot_path.as_path(), cwd.as_path())
    {
        return None;
    }
    Some(AdditionalPermissionProfile {
        network: None,
        file_system: Some(FileSystemPermissions::from_read_write_roots(
            Some(vec![snapshot_path.clone()]),
            /*write*/ None,
        )),
    })
}

impl ShellSnapshotSandbox {
    pub(crate) fn new(
        attempt: &SandboxAttempt<'_>,
        network: Option<&NetworkProxy>,
        environment_id: &str,
        additional_permissions: Option<&AdditionalPermissionProfile>,
    ) -> Self {
        Self {
            sandbox: attempt.sandbox,
            sandbox_requested: attempt.sandbox_requested,
            permissions: attempt.permissions.clone(),
            enforce_managed_network: attempt.enforce_managed_network,
            sandbox_policy_cwd: attempt.sandbox_cwd.clone(),
            workspace_roots: attempt.workspace_roots.to_vec(),
            sandbox_exe: attempt.sandbox_exe.cloned(),
            use_legacy_landlock: attempt.use_legacy_landlock,
            windows_sandbox_level: attempt.windows_sandbox_level,
            network: attempt.network_proxy(network).cloned(),
            environment_id: environment_id.to_string(),
            additional_permissions: additional_permissions.cloned(),
            network_denial_cancellation_token: attempt.network_denial_cancellation_token.clone(),
        }
    }

    pub(crate) fn cache_key(&self) -> Result<String> {
        serde_json::to_string(&(
            self.sandbox.as_metric_tag(),
            self.sandbox_requested,
            &self.permissions,
            self.enforce_managed_network,
            &self.sandbox_policy_cwd,
            &self.workspace_roots,
            &self.sandbox_exe,
            self.use_legacy_landlock,
            self.windows_sandbox_level,
            self.network
                .as_ref()
                .map(|network| (network.http_addr(), network.socks_addr())),
            &self.environment_id,
            &self.additional_permissions,
        ))
        .context("failed to serialize shell snapshot sandbox policy")
    }

    pub(crate) async fn run(
        &self,
        args: Vec<String>,
        cwd: &AbsolutePathBuf,
        mut env: HashMap<String, String>,
        snapshot_timeout: Duration,
        shell_name: &str,
        snapshot_read_path: Option<&AbsolutePathBuf>,
    ) -> Result<String> {
        if self.sandbox_requested && self.sandbox == SandboxType::None {
            bail!("shell snapshot sandbox cannot be enforced on this host");
        }

        let managed_network = if let Some(network) = self.network.as_ref() {
            let prepared = network
                .prepare_for_optional_environment(env, Some(&self.environment_id))
                .context("failed to prepare managed network for shell snapshot")?;
            env = prepared.env;
            Some(prepared.sandbox_context)
        } else {
            None
        };
        let (program, args) = args
            .split_first()
            .ok_or_else(|| anyhow!("shell snapshot command is empty"))?;
        let snapshot_permissions = snapshot_read_path.and_then(|path| {
            snapshot_read_permissions(
                path,
                &self
                    .permissions
                    .clone()
                    .materialize_project_roots_with_path_uris(&self.workspace_roots),
                &self.sandbox_policy_cwd,
            )
        });
        let additional_permissions = merge_permission_profiles(
            self.additional_permissions.as_ref(),
            snapshot_permissions.as_ref(),
        );
        let manager = SandboxManager::new();
        let request = manager
            .transform(SandboxTransformRequest {
                command: SandboxCommand {
                    program: program.clone().into(),
                    args: args.to_vec(),
                    cwd: PathUri::from_abs_path(cwd),
                    env,
                    managed_network,
                    additional_permissions,
                },
                permissions: &self.permissions,
                sandbox: self.sandbox,
                enforce_managed_network: self.enforce_managed_network,
                environment_id: Some(&self.environment_id),
                network: self.network.as_ref(),
                sandbox_policy_cwd: &self.sandbox_policy_cwd,
                sandbox_exe: self.sandbox_exe.as_deref(),
                use_legacy_landlock: self.use_legacy_landlock,
                windows_sandbox_level: self.windows_sandbox_level,
            })
            .context("failed to prepare shell snapshot sandbox")?;
        let workspace_roots = self
            .workspace_roots
            .iter()
            .map(PathUri::to_abs_path)
            .collect::<std::io::Result<Vec<_>>>()
            .context("failed to resolve shell snapshot workspace roots")?;
        let cancellation = self
            .network_denial_cancellation_token
            .as_ref()
            .map_or_else(CancellationToken::new, CancellationToken::child_token);
        let _cancel_on_drop = cancellation.clone().drop_guard();
        let expiration = ExecExpiration::TimeoutOrCancellation {
            timeout: snapshot_timeout,
            cancellation,
        };
        let request = ExecRequest::from_sandbox_exec_request(
            request,
            ExecOptions {
                expiration,
                capture_policy: ExecCapturePolicy::SensitiveFullBuffer,
            },
            workspace_roots,
        )
        .context("failed to prepare shell snapshot execution")?;
        // Caller cancellation must leave execution alive long enough to clean up its process group.
        let output = tokio::spawn(execute_env(request, /*stdout_stream*/ None))
            .await
            .context("shell snapshot execution task failed")?
            .map_err(|err| match err.details() {
                // Startup output can contain credentials, even when execution fails.
                CodexErrorDetails::Sandbox(SandboxErr::Denied { output, .. }) => anyhow!(
                    "Snapshot command was denied by sandbox with status {}",
                    output.exit_code
                ),
                CodexErrorDetails::Sandbox(SandboxErr::Timeout { .. }) => {
                    anyhow!("Snapshot command timed out")
                }
                _ => anyhow::Error::new(err)
                    .context(format!("Failed to execute sandboxed {shell_name}")),
            })?;

        if output.exit_code != 0 {
            bail!("Snapshot command exited with status {}", output.exit_code);
        }

        Ok(output.stdout.text)
    }
}
