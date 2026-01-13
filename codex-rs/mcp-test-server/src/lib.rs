mod echo_server;
mod resource_server;
mod stdio_server;
mod streamable_http_server;

pub use echo_server::run_echo_stdio_server;
pub use stdio_server::run_stdio_server;
pub use streamable_http_server::run_streamable_http_server;

fn stdio() -> (tokio::io::Stdin, tokio::io::Stdout) {
    (tokio::io::stdin(), tokio::io::stdout())
}
