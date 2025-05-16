#![allow(clippy::expect_used)]

use regex::Regex;

// This is defined in its own file so we can limit the scope of
// `allow(clippy::expect_used)` because we cannot scope it to the `lazy_static!`
// macro.
lazy_static::lazy_static! {
    /// Regular expression that matches Codex-style source file citations such as:
    ///
    /// ```text
    /// 【F:src/main.rs†L10-L20】
    /// ```
    ///
    /// Capture groups:
    /// 1. file path (anything except the dagger `†` symbol)
    /// 2. start line number (digits)
    /// 3. optional end line (digits or `?`)
    pub(crate) static ref CITATION_REGEX: Regex = Regex::new(
        r"【F:([^†]+)†L(\d+)(?:-L(\d+|\?))?】"
    ).expect("failed to compile citation regex");
}
