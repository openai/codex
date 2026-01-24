#[cfg(not(unix))]
fn main() {
    eprintln!("codex-execve-wrapper is only implemented for UNIX");
    std::process::exit(1);
// exec-server/src/bin/main_execve_wrapper.rs
}

#[cfg(unix)]
pub use codex_exec_server::main_execve_wrapper as main;
