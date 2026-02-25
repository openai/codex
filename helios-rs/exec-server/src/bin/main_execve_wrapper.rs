#[cfg(not(unix))]
fn main() {
    eprintln!("helios-execve-wrapper is only implemented for UNIX");
    std::process::exit(1);
}

#[cfg(unix)]
pub use helios_exec_server::main_execve_wrapper as main;
