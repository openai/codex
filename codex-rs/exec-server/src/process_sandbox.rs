use codex_app_server_protocol::JSONRPCErrorError;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::ExecServerRuntimePaths;
use crate::protocol::ExecLaunch;
use crate::protocol::ExecParams;
use crate::protocol::ExecSandboxMode;
use crate::rpc::invalid_request;

#[derive(Clone, Debug)]
pub(crate) struct ProcessSandboxTransformer {
    runtime_paths: ExecServerRuntimePaths,
}

pub(crate) struct MaterializedProcess {
    pub(crate) params: ExecParams,
    pub(crate) sandbox: Option<SandboxType>,
}

impl ProcessSandboxTransformer {
    pub(crate) fn new(runtime_paths: ExecServerRuntimePaths) -> Self {
        Self { runtime_paths }
    }

    pub(crate) fn materialize(
        &self,
        mut params: ExecParams,
    ) -> Result<MaterializedProcess, JSONRPCErrorError> {
        let ExecLaunch::SandboxIntent { intent } = params.launch.clone() else {
            return Ok(MaterializedProcess {
                params,
                sandbox: None,
            });
        };
        let (program, args) = params
            .argv
            .split_first()
            .ok_or_else(|| invalid_request("sandbox intent argv must not be empty".to_string()))?;
        let cwd = AbsolutePathBuf::from_absolute_path(params.cwd.as_path())
            .map_err(|err| invalid_request(format!("sandbox intent cwd is invalid: {err}")))?;
        let sandbox = executor_sandbox_type(intent.sandbox, intent.windows_sandbox_level)?;
        let command = SandboxCommand {
            program: program.clone().into(),
            args: args.to_vec(),
            cwd,
            env: crate::local_process::child_env(&params),
            additional_permissions: intent.additional_permissions,
        };
        let request = SandboxManager::new()
            .transform(SandboxTransformRequest {
                command,
                permissions: &intent.permissions,
                sandbox,
                enforce_managed_network: false,
                network: None,
                sandbox_policy_cwd: intent.sandbox_policy_cwd.as_path(),
                codex_linux_sandbox_exe: self.runtime_paths.codex_linux_sandbox_exe.as_deref(),
                use_legacy_landlock: intent.use_legacy_landlock,
                windows_sandbox_level: intent.windows_sandbox_level,
                windows_sandbox_private_desktop: intent.windows_sandbox_private_desktop,
            })
            .map_err(|err| invalid_request(format!("failed to prepare process sandbox: {err}")))?;
        params.argv = request.command;
        params.cwd = request.cwd.to_path_buf();
        params.env_policy = None;
        params.env = request.env;
        params.arg0 = request.arg0;
        params.launch = ExecLaunch::Materialized;
        Ok(MaterializedProcess {
            params,
            sandbox: Some(sandbox),
        })
    }
}

fn executor_sandbox_type(
    sandbox: ExecSandboxMode,
    windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel,
) -> Result<SandboxType, JSONRPCErrorError> {
    match sandbox {
        ExecSandboxMode::None => Ok(SandboxType::None),
        ExecSandboxMode::Platform => {
            let sandbox = codex_sandboxing::get_platform_sandbox(
                windows_sandbox_level
                    != codex_protocol::config_types::WindowsSandboxLevel::Disabled,
            )
            .ok_or_else(|| {
                invalid_request(
                    "process sandbox intent requires a platform sandbox on the executor"
                        .to_string(),
                )
            })?;
            if sandbox == SandboxType::WindowsRestrictedToken {
                return Err(invalid_request(
                    "process sandbox intent does not support WindowsRestrictedToken yet"
                        .to_string(),
                ));
            }
            Ok(sandbox)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::models::PermissionProfile;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    use super::ProcessSandboxTransformer;
    use crate::ExecServerRuntimePaths;
    use crate::ProcessId;
    use crate::protocol::ExecLaunch;
    use crate::protocol::ExecParams;
    use crate::protocol::ExecSandboxIntent;
    use crate::protocol::ExecSandboxMode;

    #[test]
    fn platform_sandbox_intent_uses_executor_platform() {
        let sandbox =
            super::executor_sandbox_type(ExecSandboxMode::Platform, WindowsSandboxLevel::Disabled);

        match codex_sandboxing::get_platform_sandbox(/*windows_sandbox_enabled*/ false) {
            Some(expected) => assert_eq!(sandbox.expect("platform sandbox"), expected),
            None => assert!(sandbox.is_err()),
        }
    }

    #[test]
    fn materializes_sandbox_intent_with_executor_runtime_paths() {
        let cwd = AbsolutePathBuf::from_absolute_path(
            std::env::current_dir().expect("current dir").as_path(),
        )
        .expect("absolute cwd");
        let params = ExecParams {
            process_id: ProcessId::from("sandbox-intent"),
            argv: vec!["true".to_string()],
            cwd: cwd.to_path_buf(),
            env_policy: None,
            env: HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]),
            tty: false,
            pipe_stdin: false,
            arg0: None,
            launch: ExecLaunch::SandboxIntent {
                intent: ExecSandboxIntent {
                    sandbox: ExecSandboxMode::None,
                    permissions: PermissionProfile::Disabled,
                    sandbox_policy_cwd: cwd,
                    use_legacy_landlock: false,
                    windows_sandbox_level: WindowsSandboxLevel::Disabled,
                    windows_sandbox_private_desktop: false,
                    additional_permissions: None,
                },
            },
        };
        let transformer = ProcessSandboxTransformer::new(
            ExecServerRuntimePaths::new(
                std::env::current_exe().expect("current exe"),
                /*codex_linux_sandbox_exe*/ None,
            )
            .expect("runtime paths"),
        );

        let materialized = transformer.materialize(params).expect("materialize");

        assert_eq!(materialized.params.launch, ExecLaunch::Materialized);
        assert_eq!(materialized.params.argv, vec!["true".to_string()]);
        assert_eq!(materialized.params.env_policy, None);
        assert_eq!(
            materialized.sandbox,
            Some(codex_sandboxing::SandboxType::None)
        );
    }
}
