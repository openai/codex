#[cfg(target_os = "windows")]
mod windows_restricted_token;

#[cfg(target_os = "windows")]
pub use windows_restricted_token::run_main;

#[cfg(not(target_os = "windows"))]
pub fn run_main() -> ! {
    panic!("codex-windows-sandbox is only supported on Windows");
}
