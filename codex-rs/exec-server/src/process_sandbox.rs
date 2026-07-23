use std::collections::HashMap;
use std::sync::Arc;

use codex_exec_server_protocol::JSONRPCErrorError;
use codex_network_proxy::CUSTOM_CA_ENV_KEYS;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_network_proxy::NetworkPolicyDecider;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyHandle;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::RemoteNetworkProxyLaunchConfig;
use codex_network_proxy::is_managed_mitm_ca_trust_bundle_path;
#[cfg(target_os = "windows")]
use codex_network_proxy::strip_managed_proxy_env;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxDirectSpawnTransformRequest;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
use codex_sandboxing::WindowsSandboxFilesystemOverrides;
use codex_sandboxing::WindowsSandboxProxySettingsMode;
use codex_sandboxing::WindowsSandboxSpawnRequest;
use codex_sandboxing::resolve_windows_elevated_filesystem_overrides;
use codex_sandboxing::resolve_windows_restricted_token_filesystem_overrides;
use codex_sandboxing::windows_sandbox_uses_elevated_backend;
use codex_sandboxing::with_managed_mitm_ca_readable_root;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

#[cfg(unix)]
use crate::CODEX_ARG0_EXEC_HELPER_ARG1;
use crate::ExecServerRuntimePaths;
use crate::protocol::ExecParams;
use crate::rpc::internal_error;
use crate::rpc::invalid_params;

pub(crate) struct PreparedExecRequest {
    pub(crate) command: Vec<String>,
    pub(crate) cwd: AbsolutePathBuf,
    pub(crate) env: HashMap<String, String>,
    pub(crate) arg0: Option<String>,
    pub(crate) sandbox: SandboxType,
    pub(crate) network_proxy_handle: Option<NetworkProxyHandle>,
    windows_sandbox: Option<PreparedWindowsSandboxRequest>,
}

struct PreparedWindowsSandboxRequest {
    permission_profile: PermissionProfile,
    workspace_roots: Vec<AbsolutePathBuf>,
    windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel,
    proxy_enforced: bool,
    network_proxy_restricting_sid: Option<String>,
    proxy_settings_mode: WindowsSandboxProxySettingsMode,
    filesystem_overrides: Option<WindowsSandboxFilesystemOverrides>,
    use_private_desktop: bool,
}

impl PreparedExecRequest {
    pub(crate) fn windows_sandbox_spawn_request(&self) -> Option<WindowsSandboxSpawnRequest<'_>> {
        self.windows_sandbox
            .as_ref()
            .map(|request| WindowsSandboxSpawnRequest {
                permission_profile: &request.permission_profile,
                workspace_roots: &request.workspace_roots,
                windows_sandbox_level: request.windows_sandbox_level,
                proxy_enforced: request.proxy_enforced,
                network_proxy_restricting_sid: request.network_proxy_restricting_sid.as_deref(),
                proxy_settings_mode: request.proxy_settings_mode,
                filesystem_overrides: request.filesystem_overrides.as_ref(),
                use_private_desktop: request.use_private_desktop,
            })
    }
}

pub(crate) async fn prepare_exec_request(
    params: &ExecParams,
    env: HashMap<String, String>,
    runtime_paths: Option<&ExecServerRuntimePaths>,
    network_policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> Result<PreparedExecRequest, JSONRPCErrorError> {
    #[cfg(target_os = "windows")]
    let mut env = env;
    #[cfg(target_os = "windows")]
    let network_proxy = if params.sandbox.is_none() {
        // Shared Windows ingress selects a route from the sandbox token's SID. Native launches
        // have no route SID, so leave them direct.
        if params.network_proxy.is_some() {
            strip_managed_proxy_env(&mut env);
        }
        None
    } else {
        params.network_proxy.as_ref()
    };
    #[cfg(not(target_os = "windows"))]
    let network_proxy = params.network_proxy.as_ref();

    let (env, managed_network, network_proxy_handle, network_proxy_restricting_sid) =
        prepare_managed_network(
            params.managed_network.as_ref(),
            network_proxy,
            env,
            network_policy_decider,
        )
        .await?;
    let Some(sandbox_context) = params.sandbox.as_ref() else {
        return Ok(PreparedExecRequest {
            command: params.argv.clone(),
            cwd: native_path(&params.cwd, "cwd")?,
            env,
            arg0: params.arg0.clone(),
            sandbox: SandboxType::None,
            network_proxy_handle,
            windows_sandbox: None,
        });
    };
    let windows_sandbox_proxy_settings_mode = sandbox_context
        .windows_sandbox_proxy_settings_mode
        .unwrap_or_default();
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
    let workspace_roots = native_workspace_roots.as_slice();
    let permissions = permissions.materialize_project_roots_with_workspace_roots(workspace_roots);
    let managed_mitm_ca_trust_bundle_path = managed_network.as_ref().and_then(|_| {
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
    #[cfg(unix)]
    let sandbox_helper_paths = params
        .arg0
        .iter()
        .map(|_| runtime_paths.codex_self_exe.clone())
        .collect::<Vec<_>>();
    // Bubblewrap launches the configured helper, which may re-enter this executable to apply
    // seccomp, so the outer filesystem sandbox must expose both paths.
    #[cfg(target_os = "linux")]
    let sandbox_helper_paths = {
        let mut sandbox_helper_paths = sandbox_helper_paths;
        if !sandbox_helper_paths.contains(&runtime_paths.codex_self_exe) {
            sandbox_helper_paths.push(runtime_paths.codex_self_exe.clone());
        }
        sandbox_helper_paths.extend(runtime_paths.codex_linux_sandbox_exe.iter().cloned());
        sandbox_helper_paths
    };
    #[cfg(unix)]
    let file_system_policy = file_system_policy
        .with_additional_readable_roots(native_sandbox_policy_cwd.as_path(), &sandbox_helper_paths);
    #[cfg(unix)]
    let permissions = PermissionProfile::from_runtime_permissions_with_enforcement(
        permissions.enforcement(),
        &file_system_policy,
        network_policy,
    );
    let sandbox_manager = SandboxManager::new();
    let sandbox = sandbox_manager.select_initial(
        &file_system_policy,
        network_policy,
        SandboxablePreference::Require,
        sandbox_context.windows_sandbox_level,
        params.enforce_managed_network,
    );
    if sandbox == SandboxType::None {
        return Err(invalid_params(
            "sandbox intent cannot be enforced on this executor".to_string(),
        ));
    }
    let (program, args) = params
        .argv
        .split_first()
        .ok_or_else(|| invalid_params("argv must not be empty".to_string()))?;
    #[cfg(unix)]
    let (program, args) = params.arg0.as_ref().map_or_else(
        || (program.into(), args.to_vec()),
        |arg0| {
            let mut helper_args = Vec::with_capacity(params.argv.len() + 2);
            helper_args.push(CODEX_ARG0_EXEC_HELPER_ARG1.to_string());
            helper_args.push(arg0.clone());
            helper_args.extend(params.argv.iter().cloned());
            (
                runtime_paths
                    .codex_self_exe
                    .as_path()
                    .as_os_str()
                    .to_owned(),
                helper_args,
            )
        },
    );
    #[cfg(not(unix))]
    let (program, args) = (program.into(), args.to_vec());
    let transform_request = SandboxDirectSpawnTransformRequest {
        workspace_roots,
        windows_sandbox_proxy_settings_mode,
        transform: SandboxTransformRequest {
            command: SandboxCommand {
                program,
                args,
                cwd: params.cwd.clone(),
                env,
                managed_network,
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
    };
    let mut request = if sandbox == SandboxType::WindowsRestrictedToken {
        // The shared launcher invokes the native Windows session spawner directly.
        sandbox_manager.transform(transform_request.transform)
    } else {
        sandbox_manager.transform_for_direct_spawn(transform_request)
    }
    .map_err(|err| invalid_params(format!("failed to prepare process sandbox: {err}")))?;
    let windows_sandbox = if sandbox == SandboxType::WindowsRestrictedToken {
        request.arg0 = params.arg0.clone();
        let proxy_enforced = params.enforce_managed_network;
        let use_elevated = windows_sandbox_uses_elevated_backend(
            sandbox_context.windows_sandbox_level,
            proxy_enforced,
        );
        let filesystem_overrides = if use_elevated {
            resolve_windows_elevated_filesystem_overrides(
                sandbox,
                &permissions,
                &native_sandbox_policy_cwd,
                use_elevated,
            )
        } else {
            resolve_windows_restricted_token_filesystem_overrides(
                sandbox,
                &permissions,
                &native_sandbox_policy_cwd,
                sandbox_context.windows_sandbox_level,
            )
        }
        .map_err(|err| invalid_params(format!("failed to prepare process sandbox: {err}")))?;
        Some(PreparedWindowsSandboxRequest {
            permission_profile: permissions,
            workspace_roots: native_workspace_roots,
            windows_sandbox_level: sandbox_context.windows_sandbox_level,
            proxy_enforced,
            network_proxy_restricting_sid,
            proxy_settings_mode: windows_sandbox_proxy_settings_mode,
            filesystem_overrides,
            use_private_desktop: sandbox_context.windows_sandbox_private_desktop,
        })
    } else {
        None
    };
    Ok(PreparedExecRequest {
        command: request.command,
        cwd: native_path(&request.cwd, "cwd")?,
        env: request.env,
        arg0: request.arg0,
        sandbox: request.sandbox,
        network_proxy_handle,
        windows_sandbox,
    })
}

async fn prepare_managed_network(
    managed_network: Option<&ManagedNetworkSandboxContext>,
    network_proxy: Option<&RemoteNetworkProxyLaunchConfig>,
    env: HashMap<String, String>,
    network_policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
) -> Result<
    (
        HashMap<String, String>,
        Option<ManagedNetworkSandboxContext>,
        Option<NetworkProxyHandle>,
        Option<String>,
    ),
    JSONRPCErrorError,
> {
    let Some(network_proxy) = network_proxy.cloned() else {
        return Ok((env, managed_network.cloned(), None, None));
    };
    let state = NetworkProxyState::from_remote_launch_config(network_proxy)
        .map_err(|err| invalid_params(format!("invalid network proxy config: {err}")))?;
    let mut builder = NetworkProxy::builder().state(Arc::new(state));
    if let Some(network_policy_decider) = network_policy_decider {
        builder = builder.policy_decider_arc(network_policy_decider);
    }
    let proxy = builder
        .build()
        .await
        .map_err(|err| internal_error(format!("failed to build executor network proxy: {err}")))?;
    let handle = proxy
        .run()
        .await
        .map_err(|err| internal_error(format!("failed to start executor network proxy: {err}")))?;
    #[cfg(target_os = "windows")]
    let network_proxy_restricting_sid = Some(
        proxy
            .network_proxy_restricting_sid(/*environment_id*/ None)
            .ok_or_else(|| {
                internal_error(
                    "managed Windows proxy route is missing its restricting SID".to_string(),
                )
            })?,
    );
    #[cfg(not(target_os = "windows"))]
    let network_proxy_restricting_sid = None;
    let prepared = proxy
        .prepare_for_optional_environment(env, /*environment_id*/ None)
        .map_err(|err| {
            internal_error(format!("failed to prepare executor network proxy: {err}"))
        })?;
    Ok((
        prepared.env,
        Some(prepared.sandbox_context),
        Some(handle),
        network_proxy_restricting_sid,
    ))
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
