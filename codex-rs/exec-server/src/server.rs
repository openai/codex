mod file_system_handler;
mod handler;
mod process_handler;
mod processor;
mod registry;
mod session_registry;
mod transport;

pub(crate) use handler::ExecServerHandler;
pub use transport::DEFAULT_LISTEN_URL;
pub use transport::ExecServerListenUrlParseError;

use crate::ExecServerRuntimePaths;

pub async fn run_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_main_with_listen_url(DEFAULT_LISTEN_URL).await
}

pub async fn run_main_with_listen_url(
    listen_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_main_with_listen_url_and_paths(
        listen_url,
        ExecServerRuntimePaths::from_current_environment()?,
    )
    .await
}

pub async fn run_main_with_listen_url_and_paths(
    listen_url: &str,
    runtime_paths: ExecServerRuntimePaths,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    transport::run_transport(listen_url, runtime_paths).await
}
