use std::env;

/// Cargo, at least sometimes, sets SSL_CERT_FILE and SSL_CERT_DIR internally,
/// it uses OpenSSL. So, always unset both at the beginning of a test even if
/// the test doesn't use either.
///
/// # Safety
///
/// This is only safe if used together with `#[serial]` because calling
/// `[env::remove_var()]` is unsafe if another thread is running.
///
/// Note that `env::remove_var()` is scheduled to become unsafe in Rust
/// Edition 2024.
pub(crate) unsafe fn clear_env() {
    env::remove_var("SSL_CERT_FILE");
    env::remove_var("SSL_CERT_DIR");
}
