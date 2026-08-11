use std::io::Write;
use std::time::Duration;

use anyhow::Context;
use clap::Args;
use codex_app_server::AppServerRuntimeOptions;
use codex_app_server::AppServerTransport;
use codex_app_server::AppServerWebsocketAuthSettings;
use codex_app_server_daemon::LifecycleCommand as AppServerLifecycleCommand;
use codex_app_server_daemon::LifecycleOutput as AppServerLifecycleOutput;
use codex_app_server_daemon::LifecycleStatus as AppServerLifecycleStatus;
use codex_app_server_daemon::RemoteControlReadyOutput as AppServerRemoteControlReadyOutput;
use codex_app_server_daemon::RemoteControlReadyStatus as AppServerRemoteControlReadyStatus;
use codex_app_server_daemon::RemoteControlStartOutput as AppServerRemoteControlStartOutput;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_arg0::Arg0DispatchPaths;
use codex_config::LoaderOverrides;
use codex_protocol::protocol::SessionSource;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use serde::Serialize;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const FOREGROUND_SOCKET_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const FOREGROUND_SOCKET_CONNECT_RETRY_DELAY: Duration = Duration::from_millis(50);
const FOREGROUND_APP_SERVER_ABORT_TIMEOUT: Duration = Duration::from_secs(1);

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
            Some(RemoteControlSubcommand::Pair) => "remote-control pair",
        }
    }
}

#[derive(Debug, Clone, Copy, clap::Subcommand)]
enum RemoteControlSubcommand {
    /// Start the app-server daemon with remote control enabled.
    Start,

    /// Stop the app-server daemon.
    Stop,

    /// Create and print a short-lived manual pairing code.
    Pair,
}

pub(crate) async fn run(
    command: RemoteControlCommand,
    arg0_paths: Arg0DispatchPaths,
    root_config_overrides: CliConfigOverrides,
) -> anyhow::Result<()> {
    match command.subcommand {
        None => {
            print_remote_control_progress(
                command.json,
                "Starting app-server with remote control enabled...",
            )?;
            run_foreground_remote_control(command.json, arg0_paths, root_config_overrides).await?;
        }
        Some(RemoteControlSubcommand::Start) => {
            print_remote_control_progress(
                command.json,
                "Starting app-server daemon with remote control enabled...",
            )?;
            let output = codex_app_server_daemon::ensure_remote_control_ready().await?;
            print_remote_control_start_output(&output, command.json)?;
        }
        Some(RemoteControlSubcommand::Stop) => {
            print_remote_control_progress(command.json, "Stopping remote control...")?;
            let output = codex_app_server_daemon::run(AppServerLifecycleCommand::Stop).await?;
            print_remote_control_stop_output(&output, command.json)?;
        }
        Some(RemoteControlSubcommand::Pair) => {
            let output = codex_app_server_daemon::start_remote_control_pairing().await?;
            print_remote_control_pairing_output(&output, command.json)?;
        }
    }
    Ok(())
}

fn print_remote_control_progress(json: bool, message: &str) -> anyhow::Result<()> {
    if json {
        return Ok(());
    }

    println!("{message}");
    std::io::stdout()
        .flush()
        .context("failed to flush remote-control progress message")?;
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
        remote_control_startup_mode: codex_app_server::RemoteControlStartupMode::EnabledEphemeral,
        install_shutdown_signal_handler: false,
        ..Default::default()
    };
    let (stop_rx, stop_signal_task) = foreground_stop_signal();
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

    let summary = match wait_for_foreground_remote_control_start(
        &mut app_server_task,
        wait_for_foreground_remote_control_ready(socket_path),
        stop_rx.clone(),
    )
    .await
    {
        ForegroundStartupResult::Ready(summary) => summary,
        ForegroundStartupResult::Stopped => {
            abort_foreground_app_server(app_server_task).await;
            stop_signal_task.abort();
            return Ok(());
        }
        ForegroundStartupResult::ReadyFailed(error) => {
            abort_foreground_app_server(app_server_task).await;
            stop_signal_task.abort();
            return Err(error);
        }
        ForegroundStartupResult::AppServerExited(error) => {
            stop_signal_task.abort();
            return Err(error);
        }
    };

    if *stop_rx.borrow() {
        abort_foreground_app_server(app_server_task).await;
        stop_signal_task.abort();
        return Ok(());
    }

    if let Err(error) = print_foreground_ready_output(&summary, json) {
        abort_foreground_app_server(app_server_task).await;
        stop_signal_task.abort();
        return Err(error);
    }

    let result = wait_for_foreground_app_server(app_server_task, stop_rx).await;
    stop_signal_task.abort();
    result
}

fn foreground_stop_signal() -> (watch::Receiver<bool>, JoinHandle<()>) {
    let (stop_tx, stop_rx) = watch::channel(false);
    let task = tokio::spawn(async move {
        if let Err(err) = tokio::signal::ctrl_c().await {
            eprintln!("failed to listen for Ctrl-C: {err}");
        }
        let _ = stop_tx.send(true);
    });
    (stop_rx, task)
}

enum ForegroundStartupResult {
    Ready(AppServerRemoteControlReadyStatus),
    Stopped,
    ReadyFailed(anyhow::Error),
    AppServerExited(anyhow::Error),
}

async fn wait_for_foreground_remote_control_start(
    app_server_task: &mut JoinHandle<std::io::Result<()>>,
    ready: impl std::future::Future<Output = anyhow::Result<AppServerRemoteControlReadyStatus>>,
    mut stop_rx: watch::Receiver<bool>,
) -> ForegroundStartupResult {
    tokio::pin!(ready);

    tokio::select! {
        ready_result = &mut ready => match ready_result {
            Ok(summary) => ForegroundStartupResult::Ready(summary),
            Err(error) => ForegroundStartupResult::ReadyFailed(error),
        },
        app_server_result = app_server_task => {
            ForegroundStartupResult::AppServerExited(
                foreground_app_server_exited_before_ready(app_server_result)
            )
        }
        _ = wait_for_stop_signal(&mut stop_rx) => ForegroundStartupResult::Stopped,
    }
}

async fn wait_for_foreground_app_server(
    mut app_server_task: JoinHandle<std::io::Result<()>>,
    mut stop_rx: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    tokio::select! {
        app_server_result = &mut app_server_task => {
            app_server_result
                .context("foreground app-server task failed to join")?
                .context("foreground app-server exited with an error")?;
        }
        _ = wait_for_stop_signal(&mut stop_rx) => {
            abort_foreground_app_server(app_server_task).await;
        }
    }

    Ok(())
}

async fn wait_for_stop_signal(stop_rx: &mut watch::Receiver<bool>) {
    if *stop_rx.borrow() {
        return;
    }
    let _ = stop_rx.wait_for(|stopped| *stopped).await;
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
    let _ = timeout(FOREGROUND_APP_SERVER_ABORT_TIMEOUT, app_server_task).await;
}

async fn wait_for_foreground_remote_control_ready(
    socket_path: AbsolutePathBuf,
) -> anyhow::Result<AppServerRemoteControlReadyStatus> {
    codex_app_server_daemon::enable_remote_control_on_socket(
        socket_path.as_path(),
        FOREGROUND_SOCKET_CONNECT_TIMEOUT,
        FOREGROUND_SOCKET_CONNECT_RETRY_DELAY,
    )
    .await
}

fn print_remote_control_start_output(
    output: &AppServerRemoteControlReadyOutput,
    json: bool,
) -> anyhow::Result<()> {
    ensure_remote_control_startable(&output.remote_control)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&RemoteControlStartJsonOutput::daemon(output))?
        );
        return Ok(());
    }

    for line in remote_control_start_human_lines(
        &output.remote_control,
        RemoteControlHumanOutputMode::Daemon,
    )? {
        println!("{line}");
    }
    for line in daemon_app_server_human_lines(&output.daemon) {
        println!("{line}");
    }
    Ok(())
}

fn print_foreground_ready_output(
    summary: &AppServerRemoteControlReadyStatus,
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
    fn foreground(summary: &'a AppServerRemoteControlReadyStatus) -> Self {
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
    output: &AppServerRemoteControlReadyStatus,
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

fn ensure_remote_control_startable(
    output: &AppServerRemoteControlReadyStatus,
) -> anyhow::Result<()> {
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
    summary: &AppServerRemoteControlReadyStatus,
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

fn daemon_app_server_human_lines(output: &AppServerRemoteControlStartOutput) -> Vec<String> {
    let (managed_codex_path, managed_codex_version) = daemon_app_server_identity(output);
    vec![
        "Daemon used app-server:".to_string(),
        format!("  path: {}", managed_codex_path.display()),
        format!("  version: {}", managed_codex_version.unwrap_or("unknown")),
    ]
}

fn daemon_app_server_identity(
    output: &AppServerRemoteControlStartOutput,
) -> (&std::path::Path, Option<&str>) {
    match output {
        AppServerRemoteControlStartOutput::Bootstrap(output) => (
            &output.managed_codex_path,
            output.managed_codex_version.as_deref(),
        ),
        AppServerRemoteControlStartOutput::Start(output) => (
            &output.managed_codex_path,
            output.managed_codex_version.as_deref(),
        ),
    }
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

fn print_remote_control_pairing_output(
    output: &RemoteControlPairingStartResponse,
    json: bool,
) -> anyhow::Result<()> {
    println!("{}", format_remote_control_pairing_output(output, json)?);
    Ok(())
}

fn format_remote_control_pairing_output(
    output: &RemoteControlPairingStartResponse,
    json: bool,
) -> anyhow::Result<String> {
    if json {
        return Ok(serde_json::to_string(output)?);
    }

    let manual_pairing_code = output
        .manual_pairing_code
        .as_deref()
        .context("remote-control pairing response did not include a manual pairing code")?;
    Ok(format!("Pairing code: {manual_pairing_code}"))
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

    fn remote_control_status(
        status: RemoteControlConnectionStatus,
    ) -> AppServerRemoteControlReadyStatus {
        AppServerRemoteControlReadyStatus {
            status,
            server_name: "owen-mbp".to_string(),
            environment_id: Some("env_test".to_string()),
            timed_out: status == RemoteControlConnectionStatus::Connecting,
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
                managed_codex_path: PathBuf::from("/opt/codex/bin/codex"),
                managed_codex_version: Some("1.0.0".to_string()),
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

    fn pairing_response(manual_pairing_code: Option<&str>) -> RemoteControlPairingStartResponse {
        RemoteControlPairingStartResponse {
            pairing_code: "pairing-code".to_string(),
            manual_pairing_code: manual_pairing_code.map(str::to_string),
            environment_id: "env_test".to_string(),
            expires_at: 1_700_000_000,
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
    fn daemon_app_server_human_lines_include_path_and_version() {
        assert_eq!(
            daemon_app_server_human_lines(
                &daemon_ready_output(RemoteControlConnectionStatus::Connected).daemon
            ),
            vec![
                "Daemon used app-server:".to_string(),
                "  path: /opt/codex/bin/codex".to_string(),
                "  version: 1.0.0".to_string(),
            ]
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
                    "managedCodexPath": "/opt/codex/bin/codex",
                    "managedCodexVersion": "1.0.0",
                    "socketPath": "/tmp/app-server-control.sock",
                    "cliVersion": "1.0.0",
                    "appServerVersion": "2.0.0",
                },
            })
        );
    }

    #[test]
    fn remote_control_daemon_json_rejects_unstartable_status() {
        assert_eq!(
            print_remote_control_start_output(
                &daemon_ready_output(RemoteControlConnectionStatus::Errored),
                /*json*/ true
            )
            .expect_err("errored daemon status should fail")
            .to_string(),
            "Remote control is enabled on owen-mbp but the connection is errored."
        );
    }

    #[test]
    fn remote_control_pairing_human_output_labels_the_manual_code() {
        assert_eq!(
            format_remote_control_pairing_output(
                &pairing_response(Some("ABCD-EFGH")),
                /*json*/ false,
            )
            .expect("manual pairing output"),
            "Pairing code: ABCD-EFGH"
        );
    }

    #[test]
    fn remote_control_pairing_json_output_preserves_pairing_artifacts() {
        let output = format_remote_control_pairing_output(
            &pairing_response(Some("ABCD-EFGH")),
            /*json*/ true,
        )
        .expect("pairing JSON output");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&output).expect("valid JSON"),
            json!({
                "pairingCode": "pairing-code",
                "manualPairingCode": "ABCD-EFGH",
                "environmentId": "env_test",
                "expiresAt": 1_700_000_000,
            })
        );
    }

    #[test]
    fn remote_control_pairing_human_output_requires_manual_code() {
        assert_eq!(
            format_remote_control_pairing_output(
                &pairing_response(/*manual_pairing_code*/ None),
                /*json*/ false,
            )
            .expect_err("missing manual pairing code should fail")
            .to_string(),
            "remote-control pairing response did not include a manual pairing code"
        );
    }

    #[tokio::test]
    async fn foreground_wait_aborts_app_server_on_stop_signal() {
        let app_server_task = tokio::spawn(std::future::pending::<std::io::Result<()>>());
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        stop_tx.send(true).expect("send stop signal");

        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            wait_for_foreground_app_server(app_server_task, stop_rx),
        )
        .await
        .expect("foreground wait should return after stop signal")
        .expect("stop signal should shut down cleanly");
    }

    #[tokio::test]
    async fn foreground_start_wait_stops_before_ready() {
        let mut app_server_task = tokio::spawn(std::future::pending::<std::io::Result<()>>());
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        stop_tx.send(true).expect("send stop signal");

        let startup = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            wait_for_foreground_remote_control_start(
                &mut app_server_task,
                std::future::pending::<anyhow::Result<AppServerRemoteControlReadyStatus>>(),
                stop_rx,
            ),
        )
        .await
        .expect("foreground startup wait should return after stop signal");

        assert!(matches!(startup, ForegroundStartupResult::Stopped));
        app_server_task.abort();
        let _ = app_server_task.await;
    }

    #[tokio::test]
    async fn foreground_start_wait_reports_app_server_exit_before_ready() {
        let mut app_server_task =
            tokio::spawn(async { Err(std::io::Error::other("startup failed before socket bind")) });
        let (_stop_tx, stop_rx) = tokio::sync::watch::channel(false);

        let startup = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            wait_for_foreground_remote_control_start(
                &mut app_server_task,
                std::future::pending::<anyhow::Result<AppServerRemoteControlReadyStatus>>(),
                stop_rx,
            ),
        )
        .await
        .expect("foreground startup wait should return after app-server exits");

        let ForegroundStartupResult::AppServerExited(error) = startup else {
            panic!("expected app-server exit before ready");
        };

        assert_eq!(
            error.to_string(),
            "foreground app-server exited before remote control became ready"
        );
    }
}
