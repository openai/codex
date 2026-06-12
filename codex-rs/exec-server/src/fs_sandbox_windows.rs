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
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::path::PathBuf;

    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_sandboxing::SandboxExecRequest;
    use codex_sandboxing::SandboxType;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use super::WindowsSandboxWrapperRequestFile;
    use super::add_wrapper_setup_env_from_vars;
    use super::materialize_sandboxed_helper;
    use super::wrapper_request_dir;

    struct EnvVarGuard {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn wrapper_setup_env_preserves_only_setup_identity() {
        let mut env = HashMap::from([("PATH".to_string(), r"C:\Windows\System32".to_string())]);

        add_wrapper_setup_env_from_vars(
            &mut env,
            [
                ("USERNAME", "alice"),
                ("USERPROFILE", r"C:\Users\alice"),
                ("OPENAI_API_KEY", "secret"),
            ]
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
        );

        assert_eq!(
            env,
            HashMap::from([
                ("PATH".to_string(), r"C:\Windows\System32".to_string()),
                ("USERNAME".to_string(), "alice".to_string()),
                ("USERPROFILE".to_string(), r"C:\Users\alice".to_string()),
            ])
        );
    }

    #[test]
    fn wrapper_request_dir_uses_sandbox_secrets() {
        let codex_home = std::env::temp_dir().join("codex-home");
        let sandbox_dir = codex_windows_sandbox::sandbox_dir(&codex_home);
        let secrets_dir = codex_windows_sandbox::sandbox_secrets_dir(&codex_home);
        let request_dir = wrapper_request_dir(&codex_home);

        assert!(!request_dir.starts_with(sandbox_dir));
        assert!(request_dir.starts_with(secrets_dir));
    }

    #[test]
    fn wrapper_request_file_is_removed_on_drop() {
        let codex_home = tempfile::TempDir::new().expect("codex home");
        let command_cwd =
            AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("absolute command cwd");
        let request =
            codex_windows_sandbox::create_windows_sandbox_wrapper_request_for_permission_profile(
                vec!["helper.exe".to_string()],
                command_cwd,
                HashMap::new(),
                PermissionProfile::External {
                    network: NetworkSandboxPolicy::Restricted,
                },
                WindowsSandboxLevel::RestrictedToken,
                /*windows_sandbox_private_desktop*/ false,
                codex_home.path().to_path_buf(),
            );

        let request_file = WindowsSandboxWrapperRequestFile::create(codex_home.path(), &request)
            .expect("create wrapper request file");
        let path = request_file.path.clone();
        assert!(path.exists());

        drop(request_file);

        assert!(!path.exists());
    }

    #[test]
    fn materialized_helper_rewrites_inner_command_path() {
        let codex_home = tempfile::TempDir::new().expect("codex home");
        let _guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
        let helper_dir = tempfile::TempDir::new().expect("helper dir");
        let configured_helper = helper_dir.path().join("configured-codex-helper.exe");
        std::fs::write(&configured_helper, b"helper").expect("write configured helper");
        let configured_helper =
            AbsolutePathBuf::from_absolute_path(&configured_helper).expect("absolute helper");
        let cwd = AbsolutePathBuf::from_absolute_path(helper_dir.path()).expect("absolute cwd");
        let file_system_sandbox_policy = FileSystemSandboxPolicy::read_only();
        let mut request = SandboxExecRequest {
            command: vec![
                configured_helper.as_path().display().to_string(),
                "--codex-run-as-fs-helper".to_string(),
            ],
            cwd,
            env: HashMap::new(),
            network: None,
            sandbox: SandboxType::WindowsRestrictedToken,
            windows_sandbox_level: WindowsSandboxLevel::RestrictedToken,
            windows_sandbox_private_desktop: false,
            permission_profile: PermissionProfile::read_only(),
            file_system_sandbox_policy,
            network_sandbox_policy: NetworkSandboxPolicy::Restricted,
            arg0: None,
        };

        materialize_sandboxed_helper(&mut request, &configured_helper)
            .expect("materialize sandboxed helper");

        let materialized_helper = PathBuf::from(&request.command[0]);
        assert_eq!(
            materialized_helper.file_name(),
            configured_helper.as_path().file_name()
        );
        assert_eq!(
            materialized_helper
                .parent()
                .and_then(std::path::Path::file_name),
            Some(std::ffi::OsStr::new(".sandbox-bin"))
        );
        assert!(materialized_helper.exists());
    }

    #[test]
    fn wrapper_request_preserves_elevated_level() {
        let codex_home = tempfile::TempDir::new().expect("codex home");
        let _guard = EnvVarGuard::set("CODEX_HOME", codex_home.path().as_os_str());
        let command_cwd =
            AbsolutePathBuf::from_absolute_path(codex_home.path()).expect("absolute command cwd");
        let file_system_sandbox_policy = FileSystemSandboxPolicy::read_only();
        let mut request = SandboxExecRequest {
            command: vec![
                "helper.exe".to_string(),
                "--codex-run-as-fs-helper".to_string(),
            ],
            cwd: command_cwd,
            env: HashMap::new(),
            network: None,
            sandbox: SandboxType::WindowsRestrictedToken,
            windows_sandbox_level: WindowsSandboxLevel::Elevated,
            windows_sandbox_private_desktop: true,
            permission_profile: PermissionProfile::read_only(),
            file_system_sandbox_policy,
            network_sandbox_policy: NetworkSandboxPolicy::Restricted,
            arg0: None,
        };
        let wrapper = AbsolutePathBuf::from_absolute_path(codex_home.path().join("codex.exe"))
            .expect("absolute wrapper");

        let request_file = super::wrap_sandbox_exec_request(&mut request, &wrapper)
            .expect("wrap sandbox exec request");
        let wrapper_request: codex_windows_sandbox::WindowsSandboxWrapperRequest =
            serde_json::from_slice(
                &std::fs::read(&request_file.path).expect("read wrapper request"),
            )
            .expect("decode wrapper request");

        assert_eq!(
            wrapper_request.windows_sandbox_level,
            WindowsSandboxLevel::Elevated
        );
        assert_eq!(wrapper_request.windows_sandbox_private_desktop, true);
    }
}
