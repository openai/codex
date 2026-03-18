mod handler;
mod jsonrpc;
mod processor;
mod transport;

pub(crate) use handler::ExecServerHandler;
pub use transport::ExecServerTransport;
pub use transport::ExecServerTransportParseError;

pub async fn run_main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    run_main_with_transport(ExecServerTransport::Stdio).await
}

pub async fn run_main_with_transport(
    transport: ExecServerTransport,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    transport::run_transport(transport).await
}
