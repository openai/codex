use std::time::Duration;

use anyhow::Context;
use clap::Args;
use codex_app_server::AppServerRuntimeOptions;
use codex_app_server::AppServerTransport;
use codex_app_server::AppServerWebsocketAuthSettings;
use codex_app_server_client::AppServerEvent;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_daemon::LifecycleCommand as AppServerLifecycleCommand;
use codex_app_server_daemon::LifecycleOutput as AppServerLifecycleOutput;
use codex_app_server_daemon::LifecycleStatus as AppServerLifecycleStatus;
use codex_app_server_daemon::RemoteControlReadyOutput as AppServerRemoteControlReadyOutput;
use codex_app_server_daemon::RemoteControlReadyStatus as AppServerRemoteControlReadyStatus;
use codex_app_server_daemon::RemoteControlStartOutput as AppServerRemoteControlStartOutput;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_arg0::Arg0DispatchPaths;
use codex_config::LoaderOverrides;
use codex_protocol::protocol::SessionSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use serde::Serialize;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;

const FOREGROUND_SOCKET_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FOREGROUND_SOCKET_CONNECT_RETRY_DELAY: Duration = Duration::from_millis(50);
const REMOTE_CONTROL_READY_TIMEOUT: Duration = Duration::from_secs(10);
const REMOTE_CONTROL_CLIENT_NAME: &str = "codex-remote-control";

#[derive(Debug, Args)]
pub(crate) struct RemoteControlCommand {
    /// Emit machine-readable JSON.
    #[arg(long = "json", global = true)]
    json: bool,

    #[command(subcommand)]
    subcommand: Option<RemoteControlSubcommand>,
}

impl RemoteControlCommand {
    pub(crate) fn subcommand_name(&self) -> &'static str {
        match self.subcommand {
            None => "remote-control",
            Some(RemoteControlSubcommand::Start) => "remote-control start",
            Some(RemoteControlSubcommand::Stop) => "remote-control stop",
        }
    }
}

#[derive(Debug, Clone, Copy, clap::Subcommand)]
enum RemoteControlSubcommand {
    /// Start the app-server daemon with remote control enabled.
    Start,

    /// Stop the app-server daemon.
    Stop,
}

pub(crate) async fn run(
    command: RemoteControlCommand,
    arg0_paths: Arg0DispatchPaths,
    root_config_overrides: CliConfigOverrides,
) -> anyhow::Result<()> {
    match command.subcommand {
        None => {
            run_foreground_remote_control(command.json, arg0_paths, root_config_overrides).await?;
        }
        Some(RemoteControlSubcommand::Start) => {
            let output = codex_app_server_daemon::ensure_remote_control_ready().await?;
            print_remote_control_start_output(&output, command.json)?;
        }
        Some(RemoteControlSubcommand::Stop) => {
            let output = codex_app_server_daemon::run(AppServerLifecycleCommand::Stop).await?;
            print_remote_control_stop_output(&output, command.json)?;
        }
    }
    Ok(())
}

async fn run_foreground_remote_control(
    json: bool,
    arg0_paths: Arg0DispatchPaths,
    root_config_overrides: CliConfigOverrides,
) -> anyhow::Result<()> {
    let socket_dir = tempfile::Builder::new()
        .prefix("codex-rc-")
        .tempdir_in("/tmp")
        .or_else(|_| tempfile::tempdir())
        .context("failed to create private app-server socket directory")?;
    let socket_path = socket_dir.path().join("rc.sock");
    let socket_path = AbsolutePathBuf::from_absolute_path(&socket_path)
        .context("private app-server socket path was not absolute")?;
    let transport = AppServerTransport::UnixSocket {
        socket_path: socket_path.clone(),
    };
    let runtime_options = AppServerRuntimeOptions {
        remote_control_enabled: true,
        ..Default::default()
    };
    let mut app_server_task = tokio::spawn(codex_app_server::run_main_with_transport_options(
        arg0_paths,
        root_config_overrides,
        LoaderOverrides::default(),
        /*strict_config*/ false,
        /*default_analytics_enabled*/ false,
        transport,
        SessionSource::VSCode,
        AppServerWebsocketAuthSettings::default(),
        runtime_options,
    ));

    let summary = tokio::select! {
        ready_result = wait_for_foreground_remote_control_ready(socket_path) => {
            match ready_result {
                Ok(summary) => summary,
                Err(error) => {
                    abort_foreground_app_server(app_server_task).await;
                    return Err(error);
                }
            }
        }
        app_server_result = &mut app_server_task => {
            return Err(foreground_app_server_exited_before_ready(app_server_result));
        }
    };

    if let Err(error) = print_foreground_ready_output(&summary, json) {
        abort_foreground_app_server(app_server_task).await;
        return Err(error);
    }

    app_server_task
        .await
        .context("foreground app-server task failed to join")?
        .context("foreground app-server exited with an error")?;
    Ok(())
}

fn foreground_app_server_exited_before_ready(
    result: Result<std::io::Result<()>, tokio::task::JoinError>,
) -> anyhow::Error {
    match result {
        Ok(Ok(())) => {
            anyhow::anyhow!("foreground app-server exited before remote control became ready")
        }
        Ok(Err(error)) => anyhow::Error::new(error)
            .context("foreground app-server exited before remote control became ready"),
        Err(error) => anyhow::Error::new(error)
            .context("foreground app-server task failed before remote control became ready"),
    }
}

async fn abort_foreground_app_server(app_server_task: JoinHandle<std::io::Result<()>>) {
    app_server_task.abort();
    let _ = app_server_task.await;
}

async fn wait_for_foreground_remote_control_ready(
    socket_path: AbsolutePathBuf,
) -> anyhow::Result<RemoteControlReadySummary> {
    let mut client = connect_foreground_client(socket_path).await?;
    enable_remote_control_and_wait(&mut client).await
}

async fn connect_foreground_client(
    socket_path: AbsolutePathBuf,
) -> anyhow::Result<RemoteAppServerClient> {
    let socket_path_display = socket_path.as_path().display().to_string();
    let connect_args = RemoteAppServerConnectArgs {
        endpoint: RemoteAppServerEndpoint::UnixSocket { socket_path },
        client_name: REMOTE_CONTROL_CLIENT_NAME.to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    };
    let deadline = Instant::now() + FOREGROUND_SOCKET_CONNECT_TIMEOUT;
    loop {
        match RemoteAppServerClient::connect(connect_args.clone()).await {
            Ok(client) => return Ok(client),
            Err(_) if Instant::now() < deadline => {
                sleep(FOREGROUND_SOCKET_CONNECT_RETRY_DELAY).await;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("app server did not become ready on {}", socket_path_display)
                });
            }
        }
    }
}

async fn enable_remote_control_and_wait(
    client: &mut RemoteAppServerClient,
) -> anyhow::Result<RemoteControlReadySummary> {
    let enable_response: RemoteControlEnableResponse = client
        .request_typed(ClientRequest::RemoteControlEnable {
            request_id: RequestId::String("remote-control-enable".to_string()),
            params: None,
        })
        .await
        .context("failed to enable remote control")?;
    wait_for_remote_control_ready(
        client,
        RemoteControlReadySummary::from(enable_response),
        REMOTE_CONTROL_READY_TIMEOUT,
    )
    .await
}

async fn wait_for_remote_control_ready(
    client: &mut RemoteAppServerClient,
    mut summary: RemoteControlReadySummary,
    ready_timeout: Duration,
) -> anyhow::Result<RemoteControlReadySummary> {
    if summary.status != RemoteControlConnectionStatus::Connecting {
        return Ok(summary);
    }

    let deadline = Instant::now() + ready_timeout;
    loop {
        let now = Instant::now();
        if now >= deadline {
            summary.timed_out = true;
            return Ok(summary);
        }

        match timeout(deadline.duration_since(now), client.next_event()).await {
            Ok(Some(event)) => {
                if apply_remote_control_event(&mut summary, event)
                    && summary.status != RemoteControlConnectionStatus::Connecting
                {
                    return Ok(summary);
                }
            }
            Ok(None) => {
                anyhow::bail!("app-server disconnected before remote control became ready");
            }
            Err(_) => {
                summary.timed_out = true;
                return Ok(summary);
            }
        }
    }
}

fn apply_remote_control_event(
    summary: &mut RemoteControlReadySummary,
    event: AppServerEvent,
) -> bool {
    match event {
        AppServerEvent::ServerNotification(ServerNotification::RemoteControlStatusChanged(
            notification,
        )) => {
            *summary = RemoteControlReadySummary::from(notification);
            true
        }
        AppServerEvent::Lagged { skipped: _ }
        | AppServerEvent::ServerNotification(_)
        | AppServerEvent::ServerRequest(_)
        | AppServerEvent::Disconnected { message: _ } => false,
    }
}

fn print_remote_control_start_output(
    output: &AppServerRemoteControlReadyOutput,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&RemoteControlStartJsonOutput::daemon(output))?
        );
        return Ok(());
    }

    let summary = RemoteControlReadySummary::from(&output.remote_control);
    for line in remote_control_start_human_lines(&summary, RemoteControlHumanOutputMode::Daemon)? {
        println!("{line}");
    }
    Ok(())
}

fn print_foreground_ready_output(
    summary: &RemoteControlReadySummary,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        ensure_remote_control_startable(summary)?;
        println!(
            "{}",
            serde_json::to_string(&RemoteControlStartJsonOutput::foreground(summary))?
        );
        return Ok(());
    }

    for line in remote_control_start_human_lines(summary, RemoteControlHumanOutputMode::Foreground)?
    {
        println!("{line}");
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RemoteControlReadySummary {
    status: RemoteControlConnectionStatus,
    server_name: String,
    environment_id: Option<String>,
    timed_out: bool,
}

impl From<RemoteControlEnableResponse> for RemoteControlReadySummary {
    fn from(response: RemoteControlEnableResponse) -> Self {
        let RemoteControlEnableResponse {
            status,
            server_name,
            installation_id: _,
            environment_id,
        } = response;
        Self {
            status,
            server_name,
            environment_id,
            timed_out: false,
        }
    }
}

impl From<RemoteControlStatusChangedNotification> for RemoteControlReadySummary {
    fn from(notification: RemoteControlStatusChangedNotification) -> Self {
        let RemoteControlStatusChangedNotification {
            status,
            server_name,
            installation_id: _,
            environment_id,
        } = notification;
        Self {
            status,
            server_name,
            environment_id,
            timed_out: false,
        }
    }
}

impl From<&AppServerRemoteControlReadyStatus> for RemoteControlReadySummary {
    fn from(status: &AppServerRemoteControlReadyStatus) -> Self {
        Self {
            status: status.status,
            server_name: status.server_name.clone(),
            environment_id: status.environment_id.clone(),
            timed_out: status.timed_out,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RemoteControlStartJsonOutput<'a> {
    mode: RemoteControlModeJson,
    status: RemoteControlConnectionStatus,
    server_name: &'a str,
    environment_id: Option<&'a str>,
    timed_out: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    daemon: Option<&'a AppServerRemoteControlStartOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum RemoteControlModeJson {
    Foreground,
    Daemon,
}

impl<'a> RemoteControlStartJsonOutput<'a> {
    fn foreground(summary: &'a RemoteControlReadySummary) -> Self {
        Self {
            mode: RemoteControlModeJson::Foreground,
            status: summary.status,
            server_name: &summary.server_name,
            environment_id: summary.environment_id.as_deref(),
            timed_out: summary.timed_out,
            daemon: None,
        }
    }

    fn daemon(output: &'a AppServerRemoteControlReadyOutput) -> Self {
        let remote_control = &output.remote_control;
        Self {
            mode: RemoteControlModeJson::Daemon,
            status: remote_control.status,
            server_name: &remote_control.server_name,
            environment_id: remote_control.environment_id.as_deref(),
            timed_out: remote_control.timed_out,
            daemon: Some(&output.daemon),
        }
    }
}

fn remote_control_start_human_message(
    output: &RemoteControlReadySummary,
) -> anyhow::Result<String> {
    ensure_remote_control_startable(output)?;
    match output.status {
        RemoteControlConnectionStatus::Connected => Ok(format!(
            "This machine is available for remote control as {}.",
            output.server_name
        )),
        RemoteControlConnectionStatus::Connecting => Ok(format!(
            "Remote control is enabled on {} and still connecting.",
            output.server_name
        )),
        RemoteControlConnectionStatus::Errored | RemoteControlConnectionStatus::Disabled => {
            unreachable!("errored and disabled statuses are rejected before formatting")
        }
    }
}

fn ensure_remote_control_startable(output: &RemoteControlReadySummary) -> anyhow::Result<()> {
    match output.status {
        RemoteControlConnectionStatus::Connected | RemoteControlConnectionStatus::Connecting => {
            Ok(())
        }
        RemoteControlConnectionStatus::Errored => {
            anyhow::bail!(
                "Remote control is enabled on {} but the connection is errored.",
                output.server_name
            );
        }
        RemoteControlConnectionStatus::Disabled => {
            anyhow::bail!("Remote control is disabled on {}.", output.server_name);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RemoteControlHumanOutputMode {
    Foreground,
    Daemon,
}

fn remote_control_start_human_lines(
    summary: &RemoteControlReadySummary,
    mode: RemoteControlHumanOutputMode,
) -> anyhow::Result<Vec<String>> {
    let mut lines = vec![remote_control_start_human_message(summary)?];
    match mode {
        RemoteControlHumanOutputMode::Foreground => {
            lines.push("Press Ctrl-C to stop.".to_string());
        }
        RemoteControlHumanOutputMode::Daemon => {}
    }
    Ok(lines)
}

fn print_remote_control_stop_output(
    output: &AppServerLifecycleOutput,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string(output)?);
        return Ok(());
    }

    println!("{}", remote_control_stop_human_message(output));
    Ok(())
}

fn remote_control_stop_human_message(output: &AppServerLifecycleOutput) -> String {
    match output.status {
        AppServerLifecycleStatus::Stopped => "Remote control stopped.".to_string(),
        AppServerLifecycleStatus::NotRunning => "Remote control is not running.".to_string(),
        AppServerLifecycleStatus::Started
        | AppServerLifecycleStatus::Restarted
        | AppServerLifecycleStatus::AlreadyRunning
        | AppServerLifecycleStatus::Running => {
            format!(
                "Remote control stop completed with status {:?}.",
                output.status
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::path::PathBuf;

    use super::*;

    fn remote_control_status(status: RemoteControlConnectionStatus) -> RemoteControlReadySummary {
        RemoteControlReadySummary {
            status,
            server_name: "owen-mbp".to_string(),
            environment_id: Some("env_test".to_string()),
            timed_out: status == RemoteControlConnectionStatus::Connecting,
        }
    }

    fn enable_response(status: RemoteControlConnectionStatus) -> RemoteControlEnableResponse {
        RemoteControlEnableResponse {
            status,
            server_name: "owen-mbp".to_string(),
            installation_id: "install_test".to_string(),
            environment_id: Some("env_test".to_string()),
        }
    }

    fn status_notification(
        status: RemoteControlConnectionStatus,
    ) -> RemoteControlStatusChangedNotification {
        RemoteControlStatusChangedNotification {
            status,
            server_name: "owen-mbp".to_string(),
            installation_id: "install_test".to_string(),
            environment_id: Some("env_test".to_string()),
        }
    }

    fn daemon_ready_output(
        status: RemoteControlConnectionStatus,
    ) -> AppServerRemoteControlReadyOutput {
        AppServerRemoteControlReadyOutput {
            daemon: AppServerRemoteControlStartOutput::Start(AppServerLifecycleOutput {
                status: AppServerLifecycleStatus::Started,
                backend: None,
                pid: Some(42),
                socket_path: PathBuf::from("/tmp/app-server-control.sock"),
                cli_version: Some("1.0.0".to_string()),
                app_server_version: Some("2.0.0".to_string()),
            }),
            remote_control: AppServerRemoteControlReadyStatus {
                status,
                server_name: "owen-mbp".to_string(),
                environment_id: Some("env_test".to_string()),
                timed_out: status == RemoteControlConnectionStatus::Connecting,
            },
        }
    }

    #[test]
    fn remote_control_human_start_messages_use_server_name() {
        assert_eq!(
            remote_control_start_human_message(&remote_control_status(
                RemoteControlConnectionStatus::Connected
            ))
            .expect("connected message"),
            "This machine is available for remote control as owen-mbp."
        );
        assert_eq!(
            remote_control_start_human_message(&remote_control_status(
                RemoteControlConnectionStatus::Connecting
            ))
            .expect("connecting message"),
            "Remote control is enabled on owen-mbp and still connecting."
        );
        assert_eq!(
            remote_control_start_human_message(&remote_control_status(
                RemoteControlConnectionStatus::Errored
            ))
            .expect_err("errored status should fail")
            .to_string(),
            "Remote control is enabled on owen-mbp but the connection is errored."
        );
        assert_eq!(
            remote_control_start_human_message(&remote_control_status(
                RemoteControlConnectionStatus::Disabled
            ))
            .expect_err("disabled status should fail")
            .to_string(),
            "Remote control is disabled on owen-mbp."
        );
    }

    #[test]
    fn remote_control_human_lines_include_foreground_stop_hint_only() {
        let summary = remote_control_status(RemoteControlConnectionStatus::Connected);

        assert_eq!(
            remote_control_start_human_lines(&summary, RemoteControlHumanOutputMode::Foreground)
                .expect("foreground lines"),
            vec![
                "This machine is available for remote control as owen-mbp.".to_string(),
                "Press Ctrl-C to stop.".to_string(),
            ]
        );
        assert_eq!(
            remote_control_start_human_lines(&summary, RemoteControlHumanOutputMode::Daemon)
                .expect("daemon lines"),
            vec!["This machine is available for remote control as owen-mbp.".to_string()]
        );
    }

    #[test]
    fn remote_control_json_output_marks_foreground_or_daemon() {
        let foreground_summary = remote_control_status(RemoteControlConnectionStatus::Connected);
        assert_eq!(
            serde_json::to_value(RemoteControlStartJsonOutput::foreground(
                &foreground_summary
            ))
            .expect("foreground JSON"),
            json!({
                "mode": "foreground",
                "status": "connected",
                "serverName": "owen-mbp",
                "environmentId": "env_test",
                "timedOut": false,
            })
        );

        let daemon_output = daemon_ready_output(RemoteControlConnectionStatus::Connected);
        assert_eq!(
            serde_json::to_value(RemoteControlStartJsonOutput::daemon(&daemon_output))
                .expect("daemon JSON"),
            json!({
                "mode": "daemon",
                "status": "connected",
                "serverName": "owen-mbp",
                "environmentId": "env_test",
                "timedOut": false,
                "daemon": {
                    "status": "started",
                    "pid": 42,
                    "socketPath": "/tmp/app-server-control.sock",
                    "cliVersion": "1.0.0",
                    "appServerVersion": "2.0.0",
                },
            })
        );
    }

    #[test]
    fn remote_control_summary_uses_enable_response_as_authoritative() {
        assert_eq!(
            RemoteControlReadySummary::from(enable_response(
                RemoteControlConnectionStatus::Connected
            )),
            remote_control_status(RemoteControlConnectionStatus::Connected)
        );
    }

    #[test]
    fn remote_control_status_notification_updates_connecting_summary() {
        let mut summary = remote_control_status(RemoteControlConnectionStatus::Connecting);

        let changed = apply_remote_control_event(
            &mut summary,
            AppServerEvent::ServerNotification(ServerNotification::RemoteControlStatusChanged(
                status_notification(RemoteControlConnectionStatus::Connected),
            )),
        );

        assert!(changed);
        assert_eq!(
            summary,
            remote_control_status(RemoteControlConnectionStatus::Connected)
        );
    }
}
