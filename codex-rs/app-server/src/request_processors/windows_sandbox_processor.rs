use super::*;

#[derive(Clone)]
pub(crate) struct WindowsSandboxRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    config: Arc<Config>,
    config_manager: ConfigManager,
}

impl WindowsSandboxRequestProcessor {
    pub(crate) fn new(
        outgoing: Arc<OutgoingMessageSender>,
        config: Arc<Config>,
        config_manager: ConfigManager,
    ) -> Self {
        Self {
            outgoing,
            config,
            config_manager,
        }
    }

    pub(crate) async fn windows_sandbox_setup_start(
        &self,
        request_id: &ConnectionRequestId,
        params: WindowsSandboxSetupStartParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        self.windows_sandbox_setup_start_inner(request_id, params)
            .await
            .map(|()| None)
    }

    async fn windows_sandbox_setup_start_inner(
        &self,
        request_id: &ConnectionRequestId,
        params: WindowsSandboxSetupStartParams,
    ) -> Result<(), JSONRPCErrorError> {
        self.outgoing
            .send_response(
                request_id.clone(),
                WindowsSandboxSetupStartResponse { started: true },
            )
            .await;

        let mode = match params.mode {
            WindowsSandboxSetupMode::Elevated => CoreWindowsSandboxSetupMode::Elevated,
            WindowsSandboxSetupMode::Unelevated => CoreWindowsSandboxSetupMode::Unelevated,
        };
        let config = Arc::clone(&self.config);
        let config_manager = self.config_manager.clone();
        let command_cwd = params
            .cwd
            .map(PathBuf::from)
            .unwrap_or_else(|| config.cwd.to_path_buf());
        let outgoing = Arc::clone(&self.outgoing);
        let connection_id = request_id.connection_id;

        tokio::spawn(async move {
            let derived_config = config_manager
                .load_for_cwd(
                    /*request_overrides*/ None,
                    ConfigOverrides {
                        cwd: Some(command_cwd.clone()),
                        ..Default::default()
                    },
                    Some(command_cwd.clone()),
                )
                .await;
            let setup_result = match derived_config {
                Ok(config) => {
                    let setup_request = WindowsSandboxSetupRequest {
                        mode,
                        policy: config
                            .permissions
                            .legacy_sandbox_policy(config.cwd.as_path()),
                        policy_cwd: config.cwd.to_path_buf(),
                        command_cwd,
                        env_map: std::env::vars().collect(),
                        codex_home: config.codex_home.to_path_buf(),
                        active_profile: config.active_profile.clone(),
                    };
                    codex_core::windows_sandbox::run_windows_sandbox_setup(setup_request).await
                }
                Err(err) => Err(err.into()),
            };
            let notification = WindowsSandboxSetupCompletedNotification {
                mode: match mode {
                    CoreWindowsSandboxSetupMode::Elevated => WindowsSandboxSetupMode::Elevated,
                    CoreWindowsSandboxSetupMode::Unelevated => WindowsSandboxSetupMode::Unelevated,
                },
                success: setup_result.is_ok(),
                error: setup_result.err().map(|err| err.to_string()),
            };
            outgoing
                .send_server_notification_to_connections(
                    &[connection_id],
                    ServerNotification::WindowsSandboxSetupCompleted(notification),
                )
                .await;
        });
        Ok(())
    }
}
