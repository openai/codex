//! Blueprint CLI commands tests

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_parse_bool_flag() {
        assert_eq!(parse_bool_flag("on").unwrap(), true);
        assert_eq!(parse_bool_flag("off").unwrap(), false);
        assert_eq!(parse_bool_flag("true").unwrap(), true);
        assert_eq!(parse_bool_flag("false").unwrap(), false);
        assert_eq!(parse_bool_flag("yes").unwrap(), true);
        assert_eq!(parse_bool_flag("no").unwrap(), false);
        assert_eq!(parse_bool_flag("1").unwrap(), true);
        assert_eq!(parse_bool_flag("0").unwrap(), false);

        // Invalid values should error
        assert!(parse_bool_flag("maybe").is_err());
    }

    #[test]
    fn test_parse_execution_mode() {
        assert_eq!(
            parse_execution_mode("single").unwrap(),
            codex_core::blueprint::ExecutionMode::Single
        );
        assert_eq!(
            parse_execution_mode("orchestrated").unwrap(),
            codex_core::blueprint::ExecutionMode::Orchestrated
        );
        assert_eq!(
            parse_execution_mode("competition").unwrap(),
            codex_core::blueprint::ExecutionMode::Competition
        );

        // Case insensitive
        assert_eq!(
            parse_execution_mode("SINGLE").unwrap(),
            codex_core::blueprint::ExecutionMode::Single
        );

        // Invalid values should error
        assert!(parse_execution_mode("invalid").is_err());
    }
}
