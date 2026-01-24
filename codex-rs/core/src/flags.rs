use env_flags::env_flags;

env_flags! {
// core/src/flags.rs
    /// Fixture path for offline tests (see client.rs).
    pub CODEX_RS_SSE_FIXTURE: Option<&str> = None;
}
