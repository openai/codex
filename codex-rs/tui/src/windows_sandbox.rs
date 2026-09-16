//! Windows sandbox configuration, managed requirements, and executor selection for the TUI.

use crate::legacy_core::config::Config;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::ConfigReadResponse;
use codex_app_server_protocol::ConfigRequirementsReadResponse;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::WindowsSandboxImplementation;
use codex_app_server_protocol::WindowsSandboxSetupMode;
use codex_config::types::WindowsSandboxModeToml;
use codex_features::Feature;
use codex_protocol::config_types::WindowsSandboxLevel;
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct WindowsSandboxConfig {
    pub(crate) mode: Option<WindowsSandboxSetupMode>,
    // None means policy has not been loaded; a loaded null list allows both modes.
    pub(crate) requirements: Option<Option<Vec<WindowsSandboxSetupMode>>>,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
impl WindowsSandboxConfig {
    pub(crate) fn from_responses(
        config: &ConfigReadResponse,
        requirements: ConfigRequirementsReadResponse,
    ) -> Self {
        let mut state = Self {
            mode: config
                .config
                .additional
                .get("windows")
                .and_then(|windows| windows.get("sandbox"))
                .and_then(|mode| serde_json::from_value(mode.clone()).ok())
                .or_else(|| {
                    let features = config.config.additional.get("features")?;
                    [
                        (
                            "elevated_windows_sandbox",
                            WindowsSandboxSetupMode::Elevated,
                        ),
                        (
                            "experimental_windows_sandbox",
                            WindowsSandboxSetupMode::Unelevated,
                        ),
                        (
                            "enable_experimental_windows_sandbox",
                            WindowsSandboxSetupMode::Unelevated,
                        ),
                    ]
                    .into_iter()
                    .find_map(|(key, mode)| {
                        (features.get(key).and_then(serde_json::Value::as_bool) == Some(true))
                            .then_some(mode)
                    })
                }),
            requirements: Some(
                requirements
                    .requirements
                    .and_then(|requirements| requirements.allowed_windows_sandbox_implementations)
                    .map(|allowed| {
                        allowed
                            .into_iter()
                            .filter_map(|implementation| match implementation {
                                WindowsSandboxImplementation::Elevated => {
                                    Some(WindowsSandboxSetupMode::Elevated)
                                }
                                WindowsSandboxImplementation::Unelevated => {
                                    Some(WindowsSandboxSetupMode::Unelevated)
                                }
                                WindowsSandboxImplementation::Mxc => None,
                            })
                            .collect()
                    }),
            ),
        };
        if let Some(Some(allowed)) = &state.requirements
            && !state.mode.is_some_and(|mode| allowed.contains(&mode))
        {
            // Managed requirements prefer elevated when the configured value is disallowed.
            state.mode = [
                WindowsSandboxSetupMode::Elevated,
                WindowsSandboxSetupMode::Unelevated,
            ]
            .into_iter()
            .find(|mode| allowed.contains(mode));
        }
        state
    }

    pub(crate) fn level(&self) -> WindowsSandboxLevel {
        match self.mode {
            Some(WindowsSandboxSetupMode::Elevated) => WindowsSandboxLevel::Elevated,
            Some(WindowsSandboxSetupMode::Unelevated) => WindowsSandboxLevel::RestrictedToken,
            None => WindowsSandboxLevel::Disabled,
        }
    }

    pub(crate) fn allows(&self, mode: WindowsSandboxSetupMode) -> bool {
        match &self.requirements {
            Some(Some(allowed)) => allowed.contains(&mode),
            Some(None) => true,
            None => false,
        }
    }

    pub(crate) fn requires_elevated(&self) -> bool {
        self.mode == Some(WindowsSandboxSetupMode::Elevated)
            && matches!(self.requirements, Some(Some(_)))
    }

    pub(crate) async fn read(
        handle: AppServerRequestHandle,
        cwd: String,
    ) -> color_eyre::Result<Self> {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let config = crate::config_update::read_effective_config(handle.clone(), cwd).await?;
            let requirements = handle
                .request_typed(ClientRequest::ConfigRequirementsRead {
                    request_id: RequestId::String(format!(
                        "tui-windows-sandbox-requirements-{}",
                        Uuid::new_v4()
                    )),
                    params: None,
                })
                .await?;
            Ok(Self::from_responses(&config, requirements))
        })
        .await?
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn level_from_config(config: &Config) -> WindowsSandboxLevel {
    match config.permissions.windows_sandbox_mode {
        Some(WindowsSandboxModeToml::Elevated) => WindowsSandboxLevel::Elevated,
        Some(WindowsSandboxModeToml::Unelevated) => WindowsSandboxLevel::RestrictedToken,
        None if config.features.enabled(Feature::WindowsSandboxElevated) => {
            WindowsSandboxLevel::Elevated
        }
        None if config.features.enabled(Feature::WindowsSandbox) => {
            WindowsSandboxLevel::RestrictedToken
        }
        None => WindowsSandboxLevel::Disabled,
    }
}

/// A server-local connection can still select remote executors. Missing selections are unknown.
pub(crate) fn host_from_environments(
    environments: Option<&[codex_app_server_protocol::ThreadEnvironment]>,
) -> crate::app::WindowsSandboxHost {
    use crate::app::WindowsSandboxHost;
    let Some(environments) = environments.filter(|environments| !environments.is_empty()) else {
        return WindowsSandboxHost::Unknown;
    };
    let local = environments
        .iter()
        .any(|environment| environment.environment_id == codex_exec_server::LOCAL_ENVIRONMENT_ID);
    let remote = environments
        .iter()
        .any(|environment| environment.environment_id != codex_exec_server::LOCAL_ENVIRONMENT_ID);
    match (local, remote) {
        (true, false) => WindowsSandboxHost::Local,
        (true, true) => WindowsSandboxHost::Mixed,
        (false, true) => WindowsSandboxHost::Remote,
        (false, false) => WindowsSandboxHost::Unknown,
    }
}

#[cfg(test)]
#[path = "windows_sandbox_tests.rs"]
mod tests;
