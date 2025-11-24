use std::{borrow::Cow, sync::OnceLock};

use regex::Regex;

pub fn normalize_name(name: &str) -> Cow<str> {
    static METRIC_NAME_RE: OnceLock<Regex> = OnceLock::new();
    METRIC_NAME_RE
        .get_or_init(|| Regex::new(r"[^a-zA-Z0-9_\-.]").expect("Regex should compile"))
        .replace_all(super::truncate(name, 150), "_")
}

#[cfg(test)]
mod test {

    #[test]
    fn test_from() {
        let expected = "aA1_-.____________";

        let actual = super::normalize_name("aA1_-./+ö{😀\n\t\r\\| ,");

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_length_restriction() {
        let expected = "a".repeat(150);

        let too_long_name = "a".repeat(155);
        let actual = super::normalize_name(&too_long_name);

        assert_eq!(expected, actual);
    }
}
