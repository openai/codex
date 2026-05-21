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

    pub(crate) async fn windows_sandbox_readiness(
        &self,
    ) -> Result<WindowsSandboxReadinessResponse, JSONRPCErrorError> {
        Ok(determine_windows_sandbox_readiness(&self.config))
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
        // Validate requirements before acknowledging setup so callers do not get a
        // `started` response for a Windows sandbox mode that cannot be persisted.
        let command_cwd = params
            .cwd
            .map(PathBuf::from)
            .unwrap_or_else(|| self.config.cwd.to_path_buf());
        let config = self
            .config_manager
            .load_for_cwd(
                /*request_overrides*/ None,
                ConfigOverrides {
                    cwd: Some(command_cwd.clone()),
                    ..Default::default()
                },
                Some(command_cwd.clone()),
            )
            .await
            .map_err(|err| config_load_error(&err))?;
        let (mode, requested_mode) = match params.mode {
            WindowsSandboxSetupMode::Elevated => (
                CoreWindowsSandboxSetupMode::Elevated,
                codex_config::types::WindowsSandboxModeToml::Elevated,
            ),
            WindowsSandboxSetupMode::Unelevated => (
                CoreWindowsSandboxSetupMode::Unelevated,
                codex_config::types::WindowsSandboxModeToml::Unelevated,
            ),
        };
        config
            .config_layer_stack
            .requirements()
            .windows_sandbox_mode
            .can_set(&Some(requested_mode))
            .map_err(|err| invalid_request(format!("invalid Windows sandbox setup mode: {err}")))?;

        self.outgoing
            .send_response(
                request_id.clone(),
                WindowsSandboxSetupStartResponse { started: true },
            )
            .await;

        let outgoing = Arc::clone(&self.outgoing);
        let connection_id = request_id.connection_id;

        tokio::spawn(async move {
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
            let setup_result =
                codex_core::windows_sandbox::run_windows_sandbox_setup(setup_request).await;
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

fn determine_windows_sandbox_readiness(config: &Config) -> WindowsSandboxReadinessResponse {
    if !cfg!(windows) {
        return WindowsSandboxReadinessResponse {
            status: WindowsSandboxReadiness::NotConfigured,
        };
    }

    determine_windows_sandbox_readiness_from_state(
        WindowsSandboxLevel::from_config(config),
        sandbox_setup_is_complete(config.codex_home.as_path()),
    )
}

fn determine_windows_sandbox_readiness_from_state(
    windows_sandbox_level: WindowsSandboxLevel,
    sandbox_setup_is_complete: bool,
) -> WindowsSandboxReadinessResponse {
    let status = match windows_sandbox_level {
        WindowsSandboxLevel::Disabled => WindowsSandboxReadiness::NotConfigured,
        WindowsSandboxLevel::RestrictedToken => WindowsSandboxReadiness::Ready,
        WindowsSandboxLevel::Elevated => {
            if sandbox_setup_is_complete {
                WindowsSandboxReadiness::Ready
            } else {
                WindowsSandboxReadiness::UpdateRequired
            }
        }
    };

    WindowsSandboxReadinessResponse { status }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_code::INVALID_REQUEST_ERROR_CODE;
    use codex_config::CloudRequirementsLoader;
    use codex_config::ConfigRequirementsToml;
    use codex_config::LoaderOverrides;
    use codex_config::types::WindowsSandboxModeToml;

    #[tokio::test]
    async fn windows_sandbox_setup_start_rejects_disallowed_mode() {
        let codex_home = tempfile::tempdir().expect("tempdir");
        let config = codex_core::config::ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .fallback_cwd(Some(codex_home.path().to_path_buf()))
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await
            .expect("config");
        let config_manager = ConfigManager::new_for_tests(
            codex_home.path().to_path_buf(),
            Vec::new(),
            LoaderOverrides::without_managed_config_for_tests(),
            CloudRequirementsLoader::new(async {
                Ok(Some(ConfigRequirementsToml {
                    windows: Some(codex_config::WindowsRequirementsToml {
                        allowed_sandbox_implementations: Some(vec![
                            WindowsSandboxModeToml::Elevated,
                        ]),
                    }),
                    ..Default::default()
                }))
            }),
        );
        let (outgoing_tx, mut outgoing_rx) = tokio::sync::mpsc::channel(1);
        let processor = WindowsSandboxRequestProcessor::new(
            Arc::new(OutgoingMessageSender::new(
                outgoing_tx,
                codex_analytics::AnalyticsEventsClient::disabled(),
            )),
            Arc::new(config),
            config_manager,
        );

        let err = processor
            .windows_sandbox_setup_start_inner(
                &ConnectionRequestId {
                    connection_id: ConnectionId(1),
                    request_id: RequestId::Integer(1),
                },
                WindowsSandboxSetupStartParams {
                    mode: WindowsSandboxSetupMode::Unelevated,
                    cwd: None,
                },
            )
            .await
            .expect_err("unelevated setup should be rejected");

        assert_eq!(err.code, INVALID_REQUEST_ERROR_CODE);
        assert!(
            err.message.contains("invalid Windows sandbox setup mode"),
            "{err:?}"
        );
        assert!(
            outgoing_rx.try_recv().is_err(),
            "disallowed setup should not send a started response"
        );
    }

    #[test]
    fn determine_windows_sandbox_readiness_reports_not_configured_when_disabled() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::Disabled,
            /*sandbox_setup_is_complete*/ false,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::NotConfigured);
    }

    #[test]
    fn determine_windows_sandbox_readiness_reports_ready_for_unelevated_mode() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::RestrictedToken,
            /*sandbox_setup_is_complete*/ false,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::Ready);
    }

    #[test]
    fn determine_windows_sandbox_readiness_reports_ready_for_complete_elevated_mode() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::Elevated,
            /*sandbox_setup_is_complete*/ true,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::Ready);
    }

    #[test]
    fn determine_windows_sandbox_readiness_reports_update_required_when_elevated_setup_is_stale() {
        let response = determine_windows_sandbox_readiness_from_state(
            WindowsSandboxLevel::Elevated,
            /*sandbox_setup_is_complete*/ false,
        );

        assert_eq!(response.status, WindowsSandboxReadiness::UpdateRequired);
    }
}
