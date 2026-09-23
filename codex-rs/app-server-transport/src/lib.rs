pub mod daemon_recovery;
#[cfg(any(windows, test))]
mod daemon_shutdown;
#[cfg(windows)]
pub use daemon_shutdown::DAEMON_SHUTDOWN_FILE_ENV;
#[cfg(windows)]
pub use daemon_shutdown::daemon_shutdown_signal;
/// Only managed app-server launches accept the local socket shutdown request.
pub const DAEMON_SHUTDOWN_SOCKET_ENV: &str = "CODEX_DAEMON_SHUTDOWN_SOCKET";
mod connection_auth;
mod outgoing_message;
mod transport;

pub use connection_auth::ConnectionAuth;
pub use outgoing_message::ConnectionId;
pub use outgoing_message::OutgoingError;
pub use outgoing_message::OutgoingMessage;
pub use outgoing_message::OutgoingResponse;
pub use outgoing_message::QueuedOutgoingMessage;
pub use transport::AppServerStartupLock;
pub use transport::AppServerTransport;
pub use transport::AppServerTransportParseError;
pub use transport::CHANNEL_CAPACITY;
pub use transport::ConnectionOrigin;
pub use transport::DaemonShutdownAccess;
pub use transport::REMOTE_CONTROL_DISABLED_ENV_VAR;
pub use transport::RemoteControlDisabledByRequirements;
pub use transport::RemoteControlEnableError;
pub use transport::RemoteControlHandle;
pub use transport::RemoteControlPolicy;
pub use transport::RemoteControlStartConfig;
pub use transport::RemoteControlStartupMode;
pub use transport::RemoteControlUnavailable;
pub use transport::TransportEvent;
pub use transport::acquire_app_server_startup_lock;
pub use transport::app_server_control_socket_path;
pub use transport::app_server_startup_lock_path;
pub use transport::daemon_recovery_file_path;
pub use transport::start_control_socket_acceptor;
pub use transport::start_remote_control;
pub use transport::start_stdio_connection;
pub use transport::start_websocket_acceptor;
pub use transport::take_remote_control_disabled_env;
