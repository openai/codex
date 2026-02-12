//! Linux sandbox helper entry point.
//!
//! On Linux, `codex-linux-sandbox` applies:
//! - in-process restrictions (`no_new_privs` + seccomp), and
//! - bubblewrap for filesystem isolation.
#[cfg(target_os = "linux")]
mod bwrap;
#[cfg(target_os = "linux")]
mod landlock;
#[cfg(target_os = "linux")]
mod linux_run_main;
#[cfg(target_os = "linux")]
mod vendored_bwrap;

#[cfg(target_os = "linux")]
pub fn run_main() -> ! {
    linux_run_main::run_main();
}

#[cfg(not(target_os = "linux"))]
pub fn run_main() -> ! {
    panic!("codex-linux-sandbox is only supported on Linux");
}
