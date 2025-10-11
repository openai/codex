#[cfg(target_os = "windows")]
mod firewall;
#[cfg(target_os = "windows")]
mod low_integrity;
#[cfg(target_os = "windows")]
mod process;
#[cfg(target_os = "windows")]
mod sandbox;
#[cfg(target_os = "windows")]
mod temp_user;

#[cfg(target_os = "windows")]
pub use sandbox::run_main;

#[cfg(not(target_os = "windows"))]
pub fn run_main() -> ! {
    panic!("codex-windows-sandbox is only supported on Windows");
}
