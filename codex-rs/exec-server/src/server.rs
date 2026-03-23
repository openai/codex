mod file_system_handler;
mod handler;
mod process_handler;
mod processor;
mod registry;
mod shell_exec_handler;
mod transport;

pub(crate) use handler::ExecServerHandler;
pub use transport::DEFAULT_LISTEN_URL;
pub use transport::ExecServerListenUrlParseError;

pub async fn run_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_main_with_listen_url(DEFAULT_LISTEN_URL).await
}

pub async fn run_main_with_listen_url(
    listen_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    transport::run_transport(listen_url).await
}
