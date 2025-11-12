//! Exit code handling for Codex.
//!
//! Provides systematic exit reason tracking with POSIX-compliant exit codes.

/// The reason Codex is exiting with a non-zero status.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Server or runtime error
    Error = 1,

    /// User interrupted with SIGINT (Ctrl+C)
    ///
    /// Following POSIX convention: 128 + SIGINT (signal 2) = 130
    Interrupted = 130,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_discriminants() {
        assert_eq!(ExitReason::Error as i32, 1);
        assert_eq!(ExitReason::Interrupted as i32, 130); // 128 + 2
    }
}
