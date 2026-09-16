use super::*;
#[cfg(target_os = "windows")]
use anyhow::Context as _;
use codex_utils_path_uri::PathUri;

#[derive(Clone)]
pub(crate) struct WindowsSandboxRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    config: Arc<Config>,
    config_manager: ConfigManager,
    #[cfg(target_os = "windows")]
    registration_refresh: Arc<tokio::sync::OnceCell<()>>,
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
            #[cfg(target_os = "windows")]
            registration_refresh: Arc::default(),
        }
    }

    pub(crate) async fn windows_sandbox_readiness(
        &self,
        request_id: &ConnectionRequestId,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        #[cfg(target_os = "windows")]
        if codex_windows_sandbox::registered_core_requested()
            && matches!(
                WindowsSandboxLevel::from_config(&self.config),
                WindowsSandboxLevel::Elevated
            )
            && !codex_login::is_workload_identity_selected()
        {
            let processor = self.clone();
            let request_id = request_id.clone();
            // Deployment must not hold up unrelated RPCs in the serial dispatcher.
            tokio::spawn(async move {
                // Coalesce readiness requests. A failed refresh leaves manual setup available.
                processor.registration_refresh.get_or_init(|| async {
                    let config = Arc::clone(&processor.config);
                    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                        if !codex_windows_sandbox::registered_core_needs_refresh(&config.codex_home)? {
                            return Ok(());
                        }
                        let env_map = std::env::vars().collect();
                        let policy = config.permissions.effective_permission_profile();
                        let (settings, listeners) = match config.permissions.network.as_ref() {
                            Some(network) => network.windows_sandbox_proxy_listeners()?,
                            None => (
                                codex_windows_sandbox::WindowsSandboxProvisioningSettings::from_environment(&policy, &env_map),
                                codex_windows_sandbox::WindowsSandboxProxyListeners::from_environment(&policy, &env_map),
                            ),
                        };
                        codex_windows_sandbox::refresh_registered_core_via_service(
                            &config.codex_home, settings, listeners,
                        )?;
                        Ok(())
                    }).await.map_err(anyhow::Error::from).and_then(std::convert::identity);
                    if let Err(error) = result {
                        warn!("Registered Core startup refresh requires manual setup: {error:#}");
                    }
                }).await;
                processor
                    .outgoing
                    .send_response(
                        request_id,
                        determine_windows_sandbox_readiness(&processor.config),
                    )
                    .await;
            });
            return Ok(None);
        }
        #[cfg(not(target_os = "windows"))]
        let _ = request_id;
        Ok(Some(
            determine_windows_sandbox_readiness(&self.config).into(),
        ))
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
        let (config, command_cwd) = load_setup_config(
            &self.config_manager,
            self.config.cwd.as_path(),
            params.cwd.map(PathBuf::from),
        )
        .await
        .map_err(|err| config_load_error(&err))?;
        let setup_mode = resolve_allowed_windows_sandbox_setup_mode(
            config.config_layer_stack.requirements(),
            params.mode,
        )?;

        // Provisioning installs native Windows filesystem permissions. Validate
        // this boundary before acknowledging that setup has started.
        let workspace_roots = config
            .effective_workspace_roots()
            .iter()
            .map(PathUri::to_abs_path)
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|err| {
                invalid_request(format!(
                    "workspace roots are not native to this host: {err}"
                ))
            })?;

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
                mode: setup_mode,
                permission_profile: config.permissions.effective_permission_profile(),
                workspace_roots,
                command_cwd,
                env_map: std::env::vars().collect(),
                codex_home: config.codex_home.to_path_buf(),
            };
            let setup_result = async {
                // Workload identity is process-local, so use the existing helper path with
                // the caller's resolved configuration instead of loading auth in the service.
                #[cfg(target_os = "windows")]
                if setup_mode == CoreWindowsSandboxSetupMode::Elevated
                    && (codex_windows_sandbox::registered_core_requested()
                        || config.features.enabled(Feature::WindowsSandboxService))
                    && !codex_login::is_workload_identity_selected()
                {
                    let provisioning = match config
                        .permissions
                        .network
                        .as_ref()
                        .map_or_else(
                            || {
                                Ok((
                                    codex_windows_sandbox::WindowsSandboxProvisioningSettings::from_environment(
                                        &setup_request.permission_profile,
                                        &setup_request.env_map,
                                    ),
                                    codex_windows_sandbox::WindowsSandboxProxyListeners::from_environment(
                                        &setup_request.permission_profile,
                                        &setup_request.env_map,
                                    ),
                                ))
                            },
                            codex_core::config::NetworkProxySpec::windows_sandbox_proxy_listeners,
                        )
                    {
                        Ok(provisioning) => Some(provisioning),
                        Err(error) => {
                            if codex_windows_sandbox::registered_core_requested() {
                                return Err(error).context("registered Core requires service-compatible proxy settings");
                            }
                            warn!(
                                "Windows sandbox service does not support the configured proxy listeners; falling back to elevated setup: {error}"
                            );
                            None
                        }
                    };
                    if let Some((settings, listeners)) = provisioning {
                        let service_setup_request = setup_request.clone();
                        let service_setup_start = Instant::now();
                        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                            let Ok(permissions) = codex_windows_sandbox::ResolvedWindowsSandboxPermissions::try_from_permission_profile_for_workspace_roots(
                                &service_setup_request.permission_profile,
                                &service_setup_request.workspace_roots,
                            ) else {
                                anyhow::ensure!(!codex_windows_sandbox::registered_core_requested(),
                                    "registered Core requires a service-compatible sandbox policy");
                                // The existing setup path can still succeed for completed
                                // provisioning without resolving the current profile.
                                return Ok(());
                            };
                            permissions.validate_elevated_filesystem_policy(
                                &service_setup_request.command_cwd,
                            )?;
                            // The shared setup path below handles helper fallback and
                            // refreshes workspace ACLs after provisioning.
                            codex_windows_sandbox::provision_windows_sandbox_via_service(
                                &service_setup_request.codex_home,
                                settings,
                                listeners,
                            )?;
                            Ok(())
                        })
                        .await
                        .map_err(|error| {
                            anyhow::anyhow!(
                                "Windows sandbox service provisioning task failed: {error}"
                            )
                        })
                        .and_then(std::convert::identity)
                        .inspect_err(|error| {
                            codex_core::windows_sandbox::emit_windows_sandbox_setup_failure_metrics(
                                setup_mode,
                                service_setup_start.elapsed(),
                                error,
                            );
                        })?;
                    }
                }
                codex_core::windows_sandbox::run_windows_sandbox_setup(setup_request).await
            }
            .await;
            let notification = WindowsSandboxSetupCompletedNotification {
                mode: match setup_mode {
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

async fn load_setup_config(
    manager: &ConfigManager,
    fallback_cwd: &std::path::Path,
    requested_cwd: Option<PathBuf>,
) -> std::io::Result<(Config, PathBuf)> {
    // Setup without a project must not grant writes to the app's install directory.
    let workspace_roots = requested_cwd.is_none().then(Vec::new);
    let cwd = requested_cwd.unwrap_or_else(|| fallback_cwd.to_path_buf());
    let config = manager
        .load_for_cwd(
            /*request_overrides*/ None,
            ConfigOverrides {
                cwd: Some(cwd.clone()),
                workspace_roots,
                ..Default::default()
            },
            Some(cwd.clone()),
        )
        .await?;
    Ok((config, cwd))
}

#[cfg(test)]
#[path = "windows_sandbox_setup_config_tests.rs"]
mod setup_config_tests;

/// Resolves the requested API mode after checking that managed requirements allow it.
fn resolve_allowed_windows_sandbox_setup_mode(
    requirements: &codex_config::ConfigRequirements,
    requested_mode: WindowsSandboxSetupMode,
) -> Result<CoreWindowsSandboxSetupMode, JSONRPCErrorError> {
    let (setup_mode, config_mode) = match requested_mode {
        WindowsSandboxSetupMode::Elevated => (
            CoreWindowsSandboxSetupMode::Elevated,
            codex_config::types::WindowsSandboxModeToml::Elevated,
        ),
        WindowsSandboxSetupMode::Unelevated => (
            CoreWindowsSandboxSetupMode::Unelevated,
            codex_config::types::WindowsSandboxModeToml::Unelevated,
        ),
    };
    requirements
        .windows_sandbox_mode
        .can_set(&Some(config_mode))
        .map_err(|err| invalid_request(format!("invalid Windows sandbox setup mode: {err}")))?;
    Ok(setup_mode)
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
    use codex_config::ConfigRequirements;
    use codex_config::Constrained;
    use codex_config::ConstrainedWithSource;
    use codex_config::types::WindowsSandboxModeToml;

    #[test]
    fn resolve_allowed_windows_sandbox_setup_mode_rejects_disallowed_mode() {
        let requirements = ConfigRequirements {
            windows_sandbox_mode: ConstrainedWithSource::new(
                Constrained::allow_only(Some(WindowsSandboxModeToml::Elevated)),
                /*source*/ None,
            ),
            ..Default::default()
        };

        let err = resolve_allowed_windows_sandbox_setup_mode(
            &requirements,
            WindowsSandboxSetupMode::Unelevated,
        )
        .expect_err("unelevated setup should be rejected");

        assert_eq!(err.code, INVALID_REQUEST_ERROR_CODE);
        assert!(
            err.message.contains("invalid Windows sandbox setup mode"),
            "{err:?}"
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
