use std::collections::HashMap;

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_network_proxy::CUSTOM_CA_ENV_KEYS;
use codex_network_proxy::is_managed_mitm_ca_trust_bundle_path;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxDirectSpawnTransformRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
use codex_sandboxing::with_managed_mitm_ca_readable_root;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

use crate::ExecServerRuntimePaths;
use crate::protocol::ExecParams;
use crate::rpc::invalid_params;

pub(crate) struct PreparedExecRequest {
    pub(crate) command: Vec<String>,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) env: HashMap<String, String>,
    pub(crate) arg0: Option<String>,
    pub(crate) sandbox: SandboxType,
}

pub(crate) fn prepare_exec_request(
    params: &ExecParams,
    env: HashMap<String, String>,
    runtime_paths: Option<&ExecServerRuntimePaths>,
) -> Result<PreparedExecRequest, JSONRPCErrorError> {
    let Some(sandbox_context) = params.sandbox.as_ref() else {
        return Ok(PreparedExecRequest {
            command: params.argv.clone(),
            cwd: native_path(&params.cwd, "cwd")?,
            env,
            arg0: params.arg0.clone(),
            sandbox: SandboxType::None,
        });
    };
    let runtime_paths = runtime_paths
        .ok_or_else(|| invalid_params("sandbox runtime paths are not configured".to_string()))?;
    // TODO(jif): Transport permissions before orchestrator-local paths are materialized,
    // then resolve executor-local helper and workspace paths here.
    let permissions: PermissionProfile = sandbox_context
        .permissions
        .clone()
        .try_into()
        .map_err(|err| invalid_params(format!("invalid sandbox permission path URI: {err}")))?;
    let sandbox_policy_cwd = sandbox_context.cwd.as_ref().unwrap_or(&params.cwd);
    let native_sandbox_policy_cwd = native_path(sandbox_policy_cwd, "sandbox cwd")?;
    let native_workspace_roots = sandbox_context
        .workspace_roots
        .iter()
        .map(|root| native_path(root, "sandbox workspace root"))
        .collect::<Result<Vec<_>, _>>()?;
    let workspace_roots = if native_workspace_roots.is_empty() {
        std::slice::from_ref(&native_sandbox_policy_cwd)
    } else {
        native_workspace_roots.as_slice()
    };
    let permissions = permissions.materialize_project_roots_with_workspace_roots(workspace_roots);
    let managed_mitm_ca_trust_bundle_path = params.managed_network.as_ref().and_then(|_| {
        CUSTOM_CA_ENV_KEYS.iter().find_map(|key| {
            let path = env.get(*key)?;
            if !is_managed_mitm_ca_trust_bundle_path(path) {
                return None;
            }
            AbsolutePathBuf::from_absolute_path(path).ok()
        })
    });
    let permissions = with_managed_mitm_ca_readable_root(
        permissions,
        managed_mitm_ca_trust_bundle_path.as_ref(),
        native_sandbox_policy_cwd.as_path(),
    );
    let (file_system_policy, network_policy) = permissions.to_runtime_permissions();
    let sandbox_manager = SandboxManager::new();
    let sandbox = sandbox_manager.select_initial(
        &file_system_policy,
        network_policy,
        SandboxablePreference::Require,
        sandbox_context.windows_sandbox_level,
        params.enforce_managed_network,
    );
    match sandbox {
        SandboxType::None => {
            return Err(invalid_params(
                "sandbox intent cannot be enforced on this executor".to_string(),
            ));
        }
        SandboxType::WindowsRestrictedToken => {
            // TODO(jif): Launch generic remote commands through the Windows sandbox session API
            // while preserving argv and TTY behavior and passing the child environment out of band.
            return Err(invalid_params(
                "sandboxed remote process launch is not supported on Windows".to_string(),
            ));
        }
        SandboxType::MacosSeatbelt | SandboxType::LinuxSeccomp => {}
    }
    let (program, args) = params
        .argv
        .split_first()
        .ok_or_else(|| invalid_params("argv must not be empty".to_string()))?;
    let request = sandbox_manager
        .transform_for_direct_spawn(SandboxDirectSpawnTransformRequest {
            workspace_roots,
            windows_sandbox_proxy_settings_mode:
                codex_sandboxing::WindowsSandboxProxySettingsMode::Reconcile,
            transform: SandboxTransformRequest {
                // TODO(jif): Preserve params.arg0 for the inner command across the sandbox
                // wrapper, or reject sandboxed requests with a custom arg0.
                command: SandboxCommand {
                    program: program.into(),
                    args: args.to_vec(),
                    cwd: params.cwd.clone(),
                    env,
                    managed_network: params.managed_network.clone(),
                    additional_permissions: None,
                },
                permissions: &permissions,
                sandbox,
                enforce_managed_network: params.enforce_managed_network,
                environment_id: None,
                network: None,
                sandbox_policy_cwd,
                codex_linux_sandbox_exe: runtime_paths.codex_linux_sandbox_exe.as_deref(),
                use_legacy_landlock: sandbox_context.use_legacy_landlock,
                windows_sandbox_level: sandbox_context.windows_sandbox_level,
                windows_sandbox_private_desktop: sandbox_context.windows_sandbox_private_desktop,
            },
        })
        .map_err(|err| invalid_params(format!("failed to prepare process sandbox: {err}")))?;
    Ok(PreparedExecRequest {
        command: request.command,
        cwd: native_path(&request.cwd, "cwd")?,
        env: request.env,
        arg0: request.arg0,
        sandbox: request.sandbox,
    })
}

fn native_path(path: &PathUri, label: &str) -> Result<AbsolutePathBuf, JSONRPCErrorError> {
    path.to_abs_path().map_err(|err| {
        invalid_params(format!(
            "{label} URI `{path}` is not valid on this exec-server host: {err}"
        ))
    })
}

#[cfg(test)]
#[path = "process_sandbox_tests.rs"]
mod tests;
