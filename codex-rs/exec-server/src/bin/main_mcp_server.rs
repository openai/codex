#[cfg(not(unix))]
fn main() {
    eprintln!("codex-exec-mcp-server is only implemented for UNIX");
    std::process::exit(1);
// exec-server/src/bin/main_mcp_server.rs
}

#[cfg(unix)]
pub use codex_exec_server::main_mcp_server as main;
