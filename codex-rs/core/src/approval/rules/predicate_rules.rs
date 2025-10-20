// Custom predicate functions for complex command matching
//
// This module contains predicate logic that can't be expressed as simple
// static patterns. These are pure functions that take command arguments
// and return whether they match.

use regex_lite::Regex;
use std::sync::LazyLock;

static SED_READ_ONLY_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| match Regex::new(r"^\d+(,\d+)?p$") {
        Ok(regex) => regex,
        Err(err) => panic!("failed to compile SED_READ_ONLY_PATTERN: {err}"),
    });

/// Check if sed command is in read-only mode (sed -n N[,M]p file)
pub fn is_sed_permitted(cmd: &[String]) -> bool {
    // Must be exactly: sed -n <pattern> <file>
    matches!(cmd, [_, flag, pattern, file]
        if flag == "-n" && !file.is_empty() && SED_READ_ONLY_PATTERN.is_match(pattern))
}
