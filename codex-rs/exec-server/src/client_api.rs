use std::time::Duration;

use crate::protocol::ExecExitedNotification;
use crate::protocol::ExecOutputDeltaNotification;

/// Connection options for any exec-server client transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecServerClientConnectOptions {
    pub client_name: String,
    pub initialize_timeout: Duration,
}

/// WebSocket connection arguments for a remote exec-server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteExecServerConnectArgs {
    pub websocket_url: String,
    pub client_name: String,
    pub connect_timeout: Duration,
    pub initialize_timeout: Duration,
}

/// Connection-level server events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecServerEvent {
    OutputDelta(ExecOutputDeltaNotification),
    Exited(ExecExitedNotification),
}
