#[cfg(target_os = "windows")]
fn main() {
    eprintln!("codex-exec-server is disabled on Windows targets");
    std::process::exit(1);
}

#[cfg(not(target_os = "windows"))]
include!("posix.rs");
