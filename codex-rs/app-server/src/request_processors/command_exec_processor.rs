use super::*;

#[derive(Clone)]
pub(crate) struct CommandExecRequestProcessor {
    arg0_paths: Arg0DispatchPaths,
    config: Arc<Config>,
    outgoing: Arc<OutgoingMessageSender>,
    command_exec_manager: CommandExecManager,
}

impl CommandExecRequestProcessor {
    pub(crate) fn new(
        arg0_paths: Arg0DispatchPaths,
        config: Arc<Config>,
        outgoing: Arc<OutgoingMessageSender>,
    ) -> Self {
        Self {
            arg0_paths,
            config,
            outgoing,
            command_exec_manager: CommandExecManager::default(),
        }
    }

    pub(crate) async fn one_off_command_exec(
        &self,
        request_id: &ConnectionRequestId,
        params: CommandExecParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.exec_one_off_command(request_id, params)
            .await
            .map(|()| None)
    }

    pub(crate) async fn command_exec_write(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecWriteParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.command_exec_manager
            .write(request_id, params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn command_exec_resize(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecResizeParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.command_exec_manager
            .resize(request_id, params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn command_exec_terminate(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecTerminateParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.command_exec_manager
            .terminate(request_id, params)
            .await
            .map(|response| Some(response.into()))
    }

    pub(crate) async fn connection_closed(&self, connection_id: ConnectionId) {
        self.command_exec_manager
            .connection_closed(connection_id)
            .await;
    }

    async fn exec_one_off_command(
        &self,
        request_id: &ConnectionRequestId,
        params: CommandExecParams,
    ) -> Result<(), JSONRPCErrorError> {
        self.exec_one_off_command_inner(request_id.clone(), params)
            .await
    }

    async fn exec_one_off_command_inner(
        &self,
        request_id: ConnectionRequestId,
        params: CommandExecParams,
    ) -> Result<(), JSONRPCErrorError> {
        tracing::debug!("ExecOneOffCommand params: {params:?}");

        let request = request_id.clone();

        if params.command.is_empty() {
            return Err(invalid_request("command must not be empty"));
        }

        let CommandExecParams {
            command,
            process_id,
            tty,
            stream_stdin,
            stream_stdout_stderr,
            output_bytes_cap,
            disable_output_cap,
            disable_timeout,
            timeout_ms,
            cwd,
            env: env_overrides,
            size,
            sandbox_policy,
            permission_profile,
        } = params;
        if sandbox_policy.is_some() && permission_profile.is_some() {
            return Err(invalid_request(
                "`permissionProfile` cannot be combined with `sandboxPolicy`",
            ));
        }

        if size.is_some() && !tty {
            return Err(invalid_params("command/exec size requires tty: true"));
        }

        if disable_output_cap && output_bytes_cap.is_some() {
            return Err(invalid_params(
                "command/exec cannot set both outputBytesCap and disableOutputCap",
            ));
        }

        if disable_timeout && timeout_ms.is_some() {
            return Err(invalid_params(
                "command/exec cannot set both timeoutMs and disableTimeout",
            ));
        }

        let cwd = cwd.map_or_else(|| self.config.cwd.clone(), |cwd| self.config.cwd.join(cwd));
        let mut env = create_env(
            &self.config.permissions.shell_environment_policy,
            /*thread_id*/ None,
        );
        if let Some(env_overrides) = env_overrides {
            for (key, value) in env_overrides {
                match value {
                    Some(value) => {
                        env.insert(key, value);
                    }
                    None => {
                        env.remove(&key);
                    }
                }
            }
        }
        let timeout_ms = match timeout_ms {
            Some(timeout_ms) => match u64::try_from(timeout_ms) {
                Ok(timeout_ms) => Some(timeout_ms),
                Err(_) => {
                    return Err(invalid_params(format!(
                        "command/exec timeoutMs must be non-negative, got {timeout_ms}"
                    )));
                }
            },
            None => None,
        };
        let managed_network_requirements_enabled =
            self.config.managed_network_requirements_enabled();
        let started_network_proxy = match self.config.permissions.network.as_ref() {
            Some(spec) => match spec
                .start_proxy(
                    self.config.permissions.permission_profile(),
                    /*policy_decider*/ None,
                    /*blocked_request_observer*/ None,
                    managed_network_requirements_enabled,
                    NetworkProxyAuditMetadata::default(),
                )
                .await
            {
                Ok(started) => Some(started),
                Err(err) => {
                    return Err(internal_error(format!(
                        "failed to start managed network proxy: {err}"
                    )));
                }
            },
            None => None,
        };
        let windows_sandbox_level = WindowsSandboxLevel::from_config(&self.config);
        let output_bytes_cap = if disable_output_cap {
            None
        } else {
            Some(output_bytes_cap.unwrap_or(DEFAULT_OUTPUT_BYTES_CAP))
        };
        let expiration = if disable_timeout {
            ExecExpiration::Cancellation(CancellationToken::new())
        } else {
            match timeout_ms {
                Some(timeout_ms) => timeout_ms.into(),
                None => ExecExpiration::DefaultTimeout,
            }
        };
        let capture_policy = if disable_output_cap {
            ExecCapturePolicy::FullBuffer
        } else {
            ExecCapturePolicy::ShellTool
        };
        let sandbox_cwd = if permission_profile.is_some() {
            cwd.clone()
        } else {
            self.config.cwd.clone()
        };
        let exec_params = ExecParams {
            command,
            cwd: cwd.clone(),
            expiration,
            capture_policy,
            env,
            network: started_network_proxy
                .as_ref()
                .map(codex_core::config::StartedNetworkProxy::proxy),
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level,
            windows_sandbox_private_desktop: self
                .config
                .permissions
                .windows_sandbox_private_desktop,
            justification: None,
            arg0: None,
        };

        let effective_permission_profile = if let Some(permission_profile) = permission_profile {
            let permission_profile =
                codex_protocol::models::PermissionProfile::from(permission_profile);
            let (mut file_system_sandbox_policy, network_sandbox_policy) =
                permission_profile.to_runtime_permissions();
            let configured_file_system_sandbox_policy =
                self.config.permissions.file_system_sandbox_policy();
            Self::preserve_configured_deny_read_restrictions(
                &mut file_system_sandbox_policy,
                &configured_file_system_sandbox_policy,
            );
            let effective_permission_profile =
                codex_protocol::models::PermissionProfile::from_runtime_permissions_with_enforcement(
                    permission_profile.enforcement(),
                    &file_system_sandbox_policy,
                    network_sandbox_policy,
                );
            self.config
                .permissions
                .can_set_permission_profile(&effective_permission_profile)
                .map_err(|err| invalid_request(format!("invalid permission profile: {err}")))?;
            effective_permission_profile
        } else if let Some(policy) = sandbox_policy.map(|policy| policy.to_core()) {
            self.config
                .permissions
                .can_set_legacy_sandbox_policy(&policy, &sandbox_cwd)
                .map_err(|err| invalid_request(format!("invalid sandbox policy: {err}")))?;
            let file_system_sandbox_policy =
                codex_protocol::permissions::FileSystemSandboxPolicy::from_legacy_sandbox_policy_for_cwd(&policy, &sandbox_cwd);
            let network_sandbox_policy =
                codex_protocol::permissions::NetworkSandboxPolicy::from(&policy);
            let permission_profile =
                codex_protocol::models::PermissionProfile::from_runtime_permissions_with_enforcement(
                    codex_protocol::models::SandboxEnforcement::from_legacy_sandbox_policy(&policy),
                    &file_system_sandbox_policy,
                    network_sandbox_policy,
                );
            self.config
                .permissions
                .can_set_permission_profile(&permission_profile)
                .map_err(|err| invalid_request(format!("invalid sandbox policy: {err}")))?;
            permission_profile
        } else {
            self.config.permissions.effective_permission_profile()
        };

        let codex_linux_sandbox_exe = self.arg0_paths.codex_linux_sandbox_exe.clone();
        let outgoing = self.outgoing.clone();
        let request_for_task = request.clone();
        let started_network_proxy_for_task = started_network_proxy;
        let use_legacy_landlock = self.config.features.use_legacy_landlock();
        let size = match size.map(crate::command_exec::terminal_size_from_protocol) {
            Some(Ok(size)) => Some(size),
            Some(Err(error)) => return Err(error),
            None => None,
        };

        let exec_request = codex_core::exec::build_exec_request(
            exec_params,
            &effective_permission_profile,
            &sandbox_cwd,
            &codex_linux_sandbox_exe,
            use_legacy_landlock,
        )
        .map_err(|err| internal_error(format!("exec failed: {err}")))?;
        self.command_exec_manager
            .start(StartCommandExecParams {
                outgoing,
                request_id: request_for_task,
                process_id,
                exec_request,
                started_network_proxy: started_network_proxy_for_task,
                tty,
                stream_stdin,
                stream_stdout_stderr,
                output_bytes_cap,
                size,
            })
            .await
    }

    fn preserve_configured_deny_read_restrictions(
        file_system_sandbox_policy: &mut FileSystemSandboxPolicy,
        configured_file_system_sandbox_policy: &FileSystemSandboxPolicy,
    ) {
        file_system_sandbox_policy
            .preserve_deny_read_restrictions_from(configured_file_system_sandbox_policy);
    }
}

#[cfg(test)]
#[path = "command_exec_processor_tests.rs"]
mod command_exec_processor_tests;
