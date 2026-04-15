// v2 websocket integration tests. Keep the websocket helper module in the same
// shard as modules that import it via `super::connection_handling_websocket`.
#[cfg(unix)]
#[path = "suite/v2/command_exec.rs"]
mod command_exec;
#[path = "suite/v2/connection_handling_websocket.rs"]
mod connection_handling_websocket;
#[cfg(unix)]
#[path = "suite/v2/connection_handling_websocket_unix.rs"]
mod connection_handling_websocket_unix;
#[path = "suite/v2/thread_name_websocket.rs"]
mod thread_name_websocket;
