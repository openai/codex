#[cfg(target_os = "windows")]
fn main() {
    eprintln!("codex-exec-server is not implemented on Windows targets");
    std::process::exit(1);
}

#[cfg(not(target_os = "windows"))]
mod posix;

#[cfg(not(target_os = "windows"))]
pub use posix::main;
