// Aggregates all former standalone integration tests as modules.
mod login_server_e2e;
#[cfg(target_os = "macos")]
mod native_browser;
