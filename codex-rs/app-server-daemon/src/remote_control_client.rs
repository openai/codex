use std::path::Path;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCRequest;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlEnableParams;
use codex_app_server_protocol::RemoteControlEnableResponse;
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use codex_app_server_protocol::RequestId;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::time::Instant;
use tokio::time::sleep;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;

use crate::RemoteControlReadyStatus;
use crate::client;

const REMOTE_CONTROL_READY_TIMEOUT: Duration = Duration::from_secs(10);
const REMOTE_CONTROL_ENABLE_REQUEST_ID: RequestId = RequestId::Integer(2);

pub(crate) async fn enable_remote_control(socket_path: &Path) -> Result<RemoteControlReadyStatus> {
    let mut websocket = client::connect(socket_path).await?;
    enable_remote_control_with_timeout(&mut websocket, REMOTE_CONTROL_READY_TIMEOUT).await
}

pub(crate) async fn enable_remote_control_with_connect_retry(
    socket_path: &Path,
    connect_timeout: Duration,
    connect_retry_delay: Duration,
) -> Result<RemoteControlReadyStatus> {
    let mut websocket =
        connect_with_retry(socket_path, connect_timeout, connect_retry_delay).await?;
    enable_remote_control_with_timeout(&mut websocket, REMOTE_CONTROL_READY_TIMEOUT).await
}

async fn enable_remote_control_with_timeout<S>(
    websocket: &mut WebSocketStream<S>,
    ready_timeout: Duration,
) -> Result<RemoteControlReadyStatus>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    client::initialize(websocket, /*experimental_api*/ true).await?;
    let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
        method: "initialized".to_string(),
        params: None,
    });
    client::send_message(websocket, &initialized)
        .await
        .context("failed to send initialized notification")?;

    let enable = JSONRPCMessage::Request(JSONRPCRequest {
        id: REMOTE_CONTROL_ENABLE_REQUEST_ID,
        method: "remoteControl/enable".to_string(),
        params: Some(serde_json::to_value(RemoteControlEnableParams {
            ephemeral: true,
        })?),
        trace: None,
    });
    client::send_message(websocket, &enable)
        .await
        .context("failed to send remoteControl/enable request")?;

    let mut latest = read_enable_response(websocket).await?;
    if latest.status == RemoteControlConnectionStatus::Connecting {
        latest = wait_for_remote_control_status(websocket, latest, ready_timeout).await?;
    }
    websocket.close(None).await.ok();
    Ok(latest)
}

async fn connect_with_retry(
    socket_path: &Path,
    connect_timeout: Duration,
    connect_retry_delay: Duration,
) -> Result<WebSocketStream<codex_uds::UnixStream>> {
    let deadline = Instant::now() + connect_timeout;
    loop {
        match client::connect(socket_path).await {
            Ok(websocket) => return Ok(websocket),
            Err(_) if Instant::now() < deadline => {
                sleep(connect_retry_delay).await;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "app server did not become ready on {}",
                        socket_path.display()
                    )
                });
            }
        }
    }
}

async fn read_enable_response<S>(
    websocket: &mut WebSocketStream<S>,
) -> Result<RemoteControlReadyStatus>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    loop {
        let message = timeout(
            client::CONTROL_SOCKET_RESPONSE_TIMEOUT,
            client::read_message(websocket),
        )
        .await
        .context("timed out waiting for remoteControl/enable response")??;
        match message {
            JSONRPCMessage::Response(response)
                if response.id == REMOTE_CONTROL_ENABLE_REQUEST_ID =>
            {
                let response =
                    serde_json::from_value::<RemoteControlEnableResponse>(response.result)
                        .context("failed to parse remoteControl/enable response")?;
                return Ok(RemoteControlReadyStatus::from(response));
            }
            JSONRPCMessage::Error(err) if err.id == REMOTE_CONTROL_ENABLE_REQUEST_ID => {
                return Err(anyhow!(
                    "remoteControl/enable failed: {}",
                    err.error.message
                ));
            }
            JSONRPCMessage::Notification(notification)
                if remote_control_status_notification(&notification).is_some() =>
            {
                continue;
            }
            _ => {}
        }
    }
}

async fn wait_for_remote_control_status<S>(
    websocket: &mut WebSocketStream<S>,
    mut latest: RemoteControlReadyStatus,
    ready_timeout: Duration,
) -> Result<RemoteControlReadyStatus>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let deadline = tokio::time::Instant::now() + ready_timeout;
    while tokio::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = match timeout(remaining, client::read_message(websocket)).await {
            Ok(Ok(message)) => message,
            Ok(Err(err)) => return Err(err),
            Err(_) => {
                latest.timed_out = true;
                return Ok(latest);
            }
        };
        let JSONRPCMessage::Notification(notification) = message else {
            continue;
        };
        let Some(status) = remote_control_status_notification(&notification) else {
            continue;
        };
        latest = RemoteControlReadyStatus::from(status);
        if latest.status != RemoteControlConnectionStatus::Connecting {
            return Ok(latest);
        }
    }
    latest.timed_out = true;
    Ok(latest)
}

fn remote_control_status_notification(
    notification: &JSONRPCNotification,
) -> Option<RemoteControlStatusChangedNotification> {
    if notification.method != "remoteControl/status/changed" {
        return None;
    }
    let params = notification.params.clone()?;
    serde_json::from_value(params).ok()
}

impl From<RemoteControlEnableResponse> for RemoteControlReadyStatus {
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

impl From<RemoteControlStatusChangedNotification> for RemoteControlReadyStatus {
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

#[cfg(all(test, unix))]
mod tests {
    use anyhow::Result;
    use codex_app_server_protocol::JSONRPCResponse;
    use codex_uds::UnixListener;
    use pretty_assertions::assert_eq;
    use tempfile::TempDir;
    use tokio_tungstenite::accept_async;

    use super::*;

    const INITIALIZE_REQUEST_ID: RequestId = RequestId::Integer(1);
    const TEST_INSTALLATION_ID: &str = "11111111-1111-4111-8111-111111111111";
    const TEST_SERVER_NAME: &str = "owen-mbp";
    const TEST_CODEX_HOME: &str = "/tmp/codex-home";

    #[tokio::test]
    async fn enable_remote_control_uses_connected_enable_response_without_later_notification()
    -> Result<()> {
        let status = run_enable_remote_control_scenario(EnableScenario {
            initial_notification: Some(remote_control_status(
                RemoteControlConnectionStatus::Connected,
                Some("env_test"),
            )),
            enable_response: remote_control_status(
                RemoteControlConnectionStatus::Connected,
                Some("env_test"),
            ),
            after_enable_notification: None,
            ready_timeout: Duration::from_millis(20),
        })
        .await?;

        assert_eq!(
            status,
            RemoteControlReadyStatus {
                status: RemoteControlConnectionStatus::Connected,
                server_name: TEST_SERVER_NAME.to_string(),
                environment_id: Some("env_test".to_string()),
                timed_out: false,
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn enable_remote_control_waits_for_connected_notification() -> Result<()> {
        let status = run_enable_remote_control_scenario(EnableScenario {
            initial_notification: None,
            enable_response: remote_control_status(
                RemoteControlConnectionStatus::Connecting,
                /*environment_id*/ None,
            ),
            after_enable_notification: Some(remote_control_status(
                RemoteControlConnectionStatus::Connected,
                Some("env_test"),
            )),
            ready_timeout: Duration::from_secs(1),
        })
        .await?;

        assert_eq!(
            status,
            RemoteControlReadyStatus {
                status: RemoteControlConnectionStatus::Connected,
                server_name: TEST_SERVER_NAME.to_string(),
                environment_id: Some("env_test".to_string()),
                timed_out: false,
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn enable_remote_control_reports_connecting_after_timeout() -> Result<()> {
        let status = run_enable_remote_control_scenario(EnableScenario {
            initial_notification: None,
            enable_response: remote_control_status(
                RemoteControlConnectionStatus::Connecting,
                /*environment_id*/ None,
            ),
            after_enable_notification: None,
            ready_timeout: Duration::from_millis(20),
        })
        .await?;

        assert_eq!(
            status,
            RemoteControlReadyStatus {
                status: RemoteControlConnectionStatus::Connecting,
                server_name: TEST_SERVER_NAME.to_string(),
                environment_id: None,
                timed_out: true,
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn enable_remote_control_returns_errored_enable_response() -> Result<()> {
        let status = run_enable_remote_control_scenario(EnableScenario {
            initial_notification: None,
            enable_response: remote_control_status(
                RemoteControlConnectionStatus::Errored,
                /*environment_id*/ None,
            ),
            after_enable_notification: None,
            ready_timeout: Duration::from_millis(20),
        })
        .await?;

        assert_eq!(
            status,
            RemoteControlReadyStatus {
                status: RemoteControlConnectionStatus::Errored,
                server_name: TEST_SERVER_NAME.to_string(),
                environment_id: None,
                timed_out: false,
            }
        );
        Ok(())
    }

    struct EnableScenario {
        initial_notification: Option<RemoteControlStatusChangedNotification>,
        enable_response: RemoteControlStatusChangedNotification,
        after_enable_notification: Option<RemoteControlStatusChangedNotification>,
        ready_timeout: Duration,
    }

    async fn run_enable_remote_control_scenario(
        scenario: EnableScenario,
    ) -> Result<RemoteControlReadyStatus> {
        let dir = TempDir::new()?;
        let socket_path = dir.path().join("app-server.sock");
        let listener = UnixListener::bind(&socket_path).await?;
        let ready_timeout = scenario.ready_timeout;
        let server_task = tokio::spawn(serve_enable_remote_control_scenario(listener, scenario));

        let mut websocket = client::connect(&socket_path).await?;
        let status = enable_remote_control_with_timeout(&mut websocket, ready_timeout).await?;
        server_task.await??;
        Ok(status)
    }

    async fn serve_enable_remote_control_scenario(
        mut listener: UnixListener,
        scenario: EnableScenario,
    ) -> Result<()> {
        let stream = listener.accept().await?;
        let mut websocket = accept_async(stream).await?;

        let initialize = client::read_message(&mut websocket).await?;
        let JSONRPCMessage::Request(initialize) = initialize else {
            panic!("expected initialize request");
        };
        assert_eq!(initialize.id, INITIALIZE_REQUEST_ID);
        assert_eq!(initialize.method, "initialize");
        let Some(initialize_params) = initialize.params else {
            panic!("expected initialize params");
        };
        assert_eq!(
            initialize_params["capabilities"]["experimentalApi"],
            serde_json::Value::Bool(true)
        );
        client::send_message(
            &mut websocket,
            &JSONRPCMessage::Response(JSONRPCResponse {
                id: INITIALIZE_REQUEST_ID,
                result: serde_json::json!({
                    "userAgent": "codex_app_server/1.2.3",
                    "codexHome": TEST_CODEX_HOME,
                    "platformFamily": "unix",
                    "platformOs": "macos",
                }),
            }),
        )
        .await?;

        let initialized = client::read_message(&mut websocket).await?;
        let JSONRPCMessage::Notification(initialized) = initialized else {
            panic!("expected initialized notification");
        };
        assert_eq!(initialized.method, "initialized");

        if let Some(status) = scenario.initial_notification {
            send_remote_control_status(&mut websocket, status).await?;
        }

        let enable = client::read_message(&mut websocket).await?;
        let JSONRPCMessage::Request(enable) = enable else {
            panic!("expected remoteControl/enable request");
        };
        assert_eq!(enable.id, REMOTE_CONTROL_ENABLE_REQUEST_ID);
        assert_eq!(enable.method, "remoteControl/enable");
        assert_eq!(
            enable.params,
            Some(serde_json::json!({ "ephemeral": true }))
        );
        client::send_message(
            &mut websocket,
            &JSONRPCMessage::Response(JSONRPCResponse {
                id: REMOTE_CONTROL_ENABLE_REQUEST_ID,
                result: serde_json::to_value(RemoteControlEnableResponse::from(
                    scenario.enable_response,
                ))?,
            }),
        )
        .await?;

        if let Some(status) = scenario.after_enable_notification {
            send_remote_control_status(&mut websocket, status).await?;
        } else {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }

    async fn send_remote_control_status<S>(
        websocket: &mut WebSocketStream<S>,
        status: RemoteControlStatusChangedNotification,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        client::send_message(
            websocket,
            &JSONRPCMessage::Notification(JSONRPCNotification {
                method: "remoteControl/status/changed".to_string(),
                params: Some(serde_json::to_value(status)?),
            }),
        )
        .await
    }

    fn remote_control_status(
        status: RemoteControlConnectionStatus,
        environment_id: Option<&str>,
    ) -> RemoteControlStatusChangedNotification {
        RemoteControlStatusChangedNotification {
            status,
            server_name: TEST_SERVER_NAME.to_string(),
            installation_id: TEST_INSTALLATION_ID.to_string(),
            environment_id: environment_id.map(str::to_string),
        }
    }
}
