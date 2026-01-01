//! Entry point detection for SDK mode.
//!
//! Detects whether the CLI was launched in SDK mode by checking
//! the `CODEX_ENTRYPOINT` environment variable.

/// SDK entry point variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryPoint {
    /// Python SDK mode (CODEX_ENTRYPOINT=sdk-py)
    SdkPy,
    /// TypeScript SDK mode (CODEX_ENTRYPOINT=sdk-ts)
    SdkTs,
    /// Default interactive CLI mode
    Cli,
}

/// Detect the entry point from environment variable.
pub fn detect_entry_point() -> EntryPoint {
    match std::env::var("CODEX_ENTRYPOINT").as_deref() {
        Ok("sdk-py") => EntryPoint::SdkPy,
        Ok("sdk-ts") => EntryPoint::SdkTs,
        _ => EntryPoint::Cli,
    }
}

/// Check if we're running in SDK mode (Python or TypeScript).
pub fn is_sdk_mode() -> bool {
    matches!(detect_entry_point(), EntryPoint::SdkPy | EntryPoint::SdkTs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_entry_point() {
        // When CODEX_ENTRYPOINT is not set, should default to Cli
        std::env::remove_var("CODEX_ENTRYPOINT");
        assert_eq!(detect_entry_point(), EntryPoint::Cli);
        assert!(!is_sdk_mode());
    }
}
