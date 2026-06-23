use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::InstallationMethod;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_app_server_protocol::UpdateCheckResponse;
use codex_app_server_protocol::UpdateInstallResponse;
use codex_install_context::InstallContext;
use codex_install_context::InstallMethod;
use codex_install_context::StandalonePlatform;
use codex_update::UpdateAction;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Clone, Copy, Default)]
pub(crate) struct UpdateRequestProcessor;

impl UpdateRequestProcessor {
    pub(crate) async fn check(&self) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let context = InstallContext::current();
        let installation_method = installation_method(&context.method);
        let can_auto_update = UpdateAction::from_install_context(context).is_some();
        let latest_version = codex_update::latest_version()
            .await
            .map_err(|err| internal_error(format!("failed to check for Codex updates: {err}")))?;

        Ok(Some(
            UpdateCheckResponse {
                current_version: env!("CARGO_PKG_VERSION").to_string(),
                latest_version,
                installation_method,
                can_auto_update,
            }
            .into(),
        ))
    }

    pub(crate) async fn install(&self) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let Some(action) = codex_update::get_update_action() else {
            return Err(invalid_request(
                "automatic updates are unavailable for this Codex installation",
            ));
        };

        #[cfg(windows)]
        let mut command = if action == UpdateAction::StandaloneWindows {
            let (program, args) = action.command_args();
            let mut command = Command::new(program);
            command.args(args);
            command
        } else {
            let mut command = Command::new("cmd");
            command.args(["/C", &action.command_str()]);
            command
        };
        #[cfg(not(windows))]
        let mut command = {
            let (program, args) = action.command_args();
            let mut command = Command::new(program);
            command.args(args);
            command
        };

        let status = command
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to run Codex update via `{}`: {err}",
                    action.command_str()
                ))
            })?;
        if !status.success() {
            return Err(internal_error(format!(
                "Codex update via `{}` failed with status {status}",
                action.command_str()
            )));
        }

        let version_output = Command::new("codex")
            .arg("--version")
            .output()
            .await
            .map_err(|err| {
                internal_error(format!(
                    "Codex update succeeded, but the installed version could not be read: {err}"
                ))
            })?;
        if !version_output.status.success() {
            return Err(internal_error(format!(
                "Codex update succeeded, but `codex --version` failed with status {}",
                version_output.status
            )));
        }
        let version_stdout = String::from_utf8(version_output.stdout).map_err(|err| {
            internal_error(format!(
                "Codex update succeeded, but `codex --version` returned invalid UTF-8: {err}"
            ))
        })?;
        let installed_version = version_stdout
            .trim()
            .strip_prefix("codex-cli ")
            .filter(|version| !version.is_empty())
            .ok_or_else(|| {
                internal_error(format!(
                    "Codex update succeeded, but `codex --version` returned an unexpected value: {version_stdout:?}"
                ))
            })?
            .to_string();

        Ok(Some(
            UpdateInstallResponse {
                installed_version,
                success: true,
            }
            .into(),
        ))
    }
}

fn installation_method(method: &InstallMethod) -> InstallationMethod {
    match method {
        InstallMethod::Npm => InstallationMethod::Npm,
        InstallMethod::Bun => InstallationMethod::Bun,
        InstallMethod::Brew => InstallationMethod::Brew,
        InstallMethod::Standalone {
            platform: StandalonePlatform::Unix,
            ..
        } => InstallationMethod::StandaloneUnix,
        InstallMethod::Standalone {
            platform: StandalonePlatform::Windows,
            ..
        } => InstallationMethod::StandaloneWindows,
        InstallMethod::Other => InstallationMethod::Other,
    }
}
