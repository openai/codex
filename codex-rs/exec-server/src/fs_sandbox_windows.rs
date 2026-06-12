use std::collections::HashMap;
use std::io::Write;

use codex_app_server_protocol::JSONRPCErrorError;
use codex_sandboxing::SandboxExecRequest;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::rpc::internal_error;
use crate::rpc::invalid_request;

const WINDOWS_SANDBOX_WRAPPER_SETUP_ENV_ALLOWLIST: &[&str] = &["USERNAME", "USERPROFILE"];

pub(super) fn materialize_sandboxed_helper(
    request: &mut SandboxExecRequest,
    source: &AbsolutePathBuf,
) -> Result<(), JSONRPCErrorError> {
    let codex_home = codex_utils_home_dir::find_codex_home().map_err(|err| {
        internal_error(format!(
            "windows fs sandbox helper failed to resolve CODEX_HOME: {err}"
        ))
    })?;
    let helper =
        codex_windows_sandbox::resolve_exe_for_launch(source.as_path(), codex_home.as_path());
    let helper = AbsolutePathBuf::from_absolute_path(helper.as_path()).map_err(|err| {
        internal_error(format!(
            "windows fs sandbox helper path is not absolute: {err}"
        ))
    })?;
    let Some(program) = request.command.first_mut() else {
        return Err(invalid_request("fs sandbox command was empty".to_string()));
    };
    *program = helper.as_path().to_string_lossy().into_owned();
    Ok(())
}

pub(super) fn wrap_sandbox_exec_request(
    request: &mut SandboxExecRequest,
    helper: &AbsolutePathBuf,
) -> Result<WindowsSandboxWrapperRequestFile, JSONRPCErrorError> {
    let codex_home = codex_utils_home_dir::find_codex_home().map_err(|err| {
        internal_error(format!(
            "windows fs sandbox helper failed to resolve CODEX_HOME: {err}"
        ))
    })?;
    let sandboxed_env = request.env.clone();
    let wrapper_request =
        codex_windows_sandbox::create_windows_sandbox_wrapper_request_for_permission_profile(
            std::mem::take(&mut request.command),
            request.cwd.clone(),
            sandboxed_env,
            request.permission_profile.clone(),
            request.windows_sandbox_level,
            request.windows_sandbox_private_desktop,
            codex_home.to_path_buf(),
        );
    let request_file =
        WindowsSandboxWrapperRequestFile::create(codex_home.as_path(), &wrapper_request)?;
    let mut args = codex_windows_sandbox::create_windows_sandbox_command_args_for_request_file(
        request_file.path.as_path(),
    );
    request.command = Vec::with_capacity(1 + args.len());
    request
        .command
        .push(helper.as_path().to_string_lossy().into_owned());
    request.command.append(&mut args);
    request.sandbox = codex_sandboxing::SandboxType::None;
    request.arg0 = None;
    add_wrapper_setup_env(&mut request.env);
    Ok(request_file)
}

fn add_wrapper_setup_env(env: &mut HashMap<String, String>) {
    add_wrapper_setup_env_from_vars(env, std::env::vars_os());
}

fn add_wrapper_setup_env_from_vars(
    env: &mut HashMap<String, String>,
    vars: impl IntoIterator<Item = (std::ffi::OsString, std::ffi::OsString)>,
) {
    for (key, value) in vars {
        let key = key.to_string_lossy().into_owned();
        if !WINDOWS_SANDBOX_WRAPPER_SETUP_ENV_ALLOWLIST
            .iter()
            .any(|allowed| key.eq_ignore_ascii_case(allowed))
        {
            continue;
        }
        if env
            .keys()
            .any(|existing| existing.eq_ignore_ascii_case(&key))
        {
            continue;
        }
        env.insert(key, value.to_string_lossy().into_owned());
    }
}

pub(super) struct WindowsSandboxWrapperRequestFile {
    path: std::path::PathBuf,
}

impl WindowsSandboxWrapperRequestFile {
    fn create(
        codex_home: &std::path::Path,
        request: &codex_windows_sandbox::WindowsSandboxWrapperRequest,
    ) -> Result<Self, JSONRPCErrorError> {
        let request_dir = wrapper_request_dir(codex_home);
        std::fs::create_dir_all(&request_dir).map_err(|err| {
            internal_error(format!(
                "failed to create windows fs sandbox wrapper request dir {}: {err}",
                request_dir.display()
            ))
        })?;
        codex_windows_sandbox::ensure_current_user_cleanup_access(&request_dir).map_err(|err| {
            internal_error(format!(
                "failed to grant cleanup access to windows fs sandbox wrapper request dir {}: {err}",
                request_dir.display()
            ))
        })?;
        let path = request_dir.join(format!(
            "fs-helper-wrapper-request-{}.json",
            uuid::Uuid::new_v4()
        ));
        let request_json = serde_json::to_vec(request).map_err(|err| {
            internal_error(format!(
                "failed to encode or decode fs sandbox helper message: {err}"
            ))
        })?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|err| {
                internal_error(format!(
                    "failed to create windows fs sandbox wrapper request file {}: {err}",
                    path.display()
                ))
            })?;
        file.write_all(&request_json).map_err(|err| {
            internal_error(format!(
                "failed to write windows fs sandbox wrapper request file {}: {err}",
                path.display()
            ))
        })?;
        Ok(Self { path })
    }
}

fn wrapper_request_dir(codex_home: &std::path::Path) -> std::path::PathBuf {
    codex_windows_sandbox::sandbox_secrets_dir(codex_home).join("wrapper-requests")
}

impl Drop for WindowsSandboxWrapperRequestFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[path = "fs_sandbox_windows_tests.rs"]
mod tests;
