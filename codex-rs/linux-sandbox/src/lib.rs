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
mod launcher;
#[cfg(target_os = "linux")]
mod linux_run_main;
#[cfg(target_os = "linux")]
mod proxy_routing;
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

#[cfg(target_os = "linux")]
pub fn dispatch_if_requested() {
    let argv0 = std::env::args_os().next().unwrap_or_default();
    let exe_name = std::path::Path::new(&argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if exe_name == codex_sandboxing::landlock::CODEX_LINUX_SANDBOX_ARG0 {
        run_main();
    }
}

#[cfg(not(target_os = "linux"))]
pub fn dispatch_if_requested() {}
