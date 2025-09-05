/// Returns the current Codex CLI version as embedded at compile time.
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_matches_env() {
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
    }
}
