#[cfg(target_os = "windows")]
mod windows_appcontainer;

#[cfg(target_os = "windows")]
pub use windows_appcontainer::run_main;

#[cfg(not(target_os = "windows"))]
pub fn run_main() -> ! {
    panic!("codex-windows-sandbox is only supported on Windows");
}
