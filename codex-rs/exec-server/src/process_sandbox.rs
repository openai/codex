use crate::ExecServerRuntimePaths;
use crate::protocol::ExecParams;
use crate::rpc::invalid_params;
use codex_app_server_protocol::JSONRPCErrorError;
#[cfg(target_os = "linux")]
use codex_protocol::models::PermissionProfile;
#[cfg(target_os = "linux")]
use codex_sandboxing::SandboxCommand;
#[cfg(target_os = "linux")]
use codex_sandboxing::SandboxManager;
#[cfg(target_os = "linux")]
use codex_sandboxing::SandboxTransformRequest;
#[cfg(target_os = "linux")]
use codex_sandboxing::SandboxType;
#[cfg(target_os = "linux")]
use codex_sandboxing::SandboxablePreference;

/// Converts a remote launch's sandbox policy into this Linux host's native wrapper.
#[cfg(target_os = "linux")]
pub(crate) fn prepare_exec_params(
    mut params: ExecParams,
    runtime_paths: &ExecServerRuntimePaths,
) -> Result<ExecParams, JSONRPCErrorError> {
    let Some(sandbox_context) = params.sandbox.take() else {
        return Ok(params);
    };
    let native_permissions: PermissionProfile = sandbox_context
        .permissions
        .try_into()
        .map_err(|err| invalid_params(format!("invalid sandbox permission path URI: {err}")))?;
    let (file_system_policy, network_policy) = native_permissions.to_runtime_permissions();
    let sandbox_manager = SandboxManager::new();
    let sandbox = sandbox_manager.select_initial(
        &file_system_policy,
        network_policy,
        SandboxablePreference::Auto,
        sandbox_context.windows_sandbox_level,
        params.enforce_managed_network,
    );
    if sandbox == SandboxType::None {
        return Ok(params);
    }

    let (program, args) = params
        .argv
        .split_first()
        .ok_or_else(|| invalid_params("argv must not be empty".to_string()))?;
    let command = SandboxCommand {
        program: program.into(),
        args: args.to_vec(),
        cwd: params.cwd.clone(),
        env: params.env.clone(),
        additional_permissions: None,
    };
    let sandbox_policy_cwd = sandbox_context.cwd.as_ref().unwrap_or(&params.cwd);
    let request = sandbox_manager
        .transform(SandboxTransformRequest {
            command,
            permissions: &native_permissions,
            sandbox,
            enforce_managed_network: params.enforce_managed_network,
            environment_id: None,
            network: None,
            sandbox_policy_cwd,
            codex_linux_sandbox_exe: runtime_paths.codex_linux_sandbox_exe.as_deref(),
            use_legacy_landlock: sandbox_context.use_legacy_landlock,
            windows_sandbox_level: sandbox_context.windows_sandbox_level,
            windows_sandbox_private_desktop: sandbox_context.windows_sandbox_private_desktop,
        })
        .map_err(|err| invalid_params(format!("failed to prepare process sandbox: {err}")))?;
    params.argv = request.command;
    params.env = request.env;
    params.arg0 = request.arg0;
    Ok(params)
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn prepare_exec_params(
    mut params: ExecParams,
    _runtime_paths: &ExecServerRuntimePaths,
) -> Result<ExecParams, JSONRPCErrorError> {
    if params.sandbox.take().is_none() {
        return Ok(params);
    }
    Err(invalid_params(
        "sandboxed remote process launches require a Linux exec-server".to_string(),
    ))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::collections::HashMap;

    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_path_uri::PathUri;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::FileSystemSandboxContext;
    use crate::ProcessId;

    fn exec_params(
        permissions: PermissionProfile,
        cwd_uri: PathUri,
        enforce_managed_network: bool,
    ) -> ExecParams {
        ExecParams {
            process_id: ProcessId::from("sandboxed"),
            argv: vec![
                "/bin/bash".to_string(),
                "-lc".to_string(),
                "pwd".to_string(),
            ],
            cwd: cwd_uri.clone(),
            env_policy: None,
            env: HashMap::new(),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            sandbox: Some(FileSystemSandboxContext::from_permission_profile_with_cwd(
                permissions,
                cwd_uri,
            )),
            enforce_managed_network,
        }
    }

    fn runtime_paths() -> std::io::Result<ExecServerRuntimePaths> {
        ExecServerRuntimePaths::new(
            "/executor/codex".into(),
            Some("/executor/codex-linux-sandbox".into()),
        )
    }

    #[test]
    fn remote_sandbox_uses_executor_linux_helper() {
        let cwd_uri = PathUri::from_path(std::env::current_dir().expect("cwd")).expect("cwd URI");
        let permissions = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::default(),
            NetworkSandboxPolicy::Restricted,
        );
        let runtime_paths = runtime_paths().expect("runtime paths");

        let params = prepare_exec_params(
            exec_params(permissions, cwd_uri, /*enforce_managed_network*/ false),
            &runtime_paths,
        )
        .expect("prepare sandbox");

        assert_eq!(
            params.argv.first(),
            Some(&"/executor/codex-linux-sandbox".to_string())
        );
        assert_eq!(
            params.arg0,
            Some("/executor/codex-linux-sandbox".to_string())
        );
        assert_eq!(params.sandbox, None);
    }

    #[test]
    fn managed_network_requires_executor_linux_helper() {
        let cwd_uri = PathUri::from_path(std::env::current_dir().expect("cwd")).expect("cwd URI");
        let permissions = PermissionProfile::from_runtime_permissions(
            &FileSystemSandboxPolicy::unrestricted(),
            NetworkSandboxPolicy::Enabled,
        );
        let runtime_paths = runtime_paths().expect("runtime paths");

        let params = prepare_exec_params(
            exec_params(permissions, cwd_uri, /*enforce_managed_network*/ true),
            &runtime_paths,
        )
        .expect("prepare sandbox");

        assert_eq!(
            params.argv.first(),
            Some(&"/executor/codex-linux-sandbox".to_string())
        );
    }
}
