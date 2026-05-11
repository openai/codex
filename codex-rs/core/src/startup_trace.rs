pub(crate) fn enabled() -> bool {
    std::env::var_os("CODEX_STARTUP_TRACE").is_some()
}
