use std::{borrow::Cow, sync::OnceLock};

use regex::Regex;

use crate::units::MetricUnit;

pub fn normalize_unit(unit: &str) -> Cow<str> {
    static METRIC_UNIT_RE: OnceLock<Regex> = OnceLock::new();
    let normalized_unit = METRIC_UNIT_RE
        .get_or_init(|| Regex::new(r"[^a-zA-Z0-9_]").expect("Regex should compile"))
        .replace_all(super::truncate(unit, 15), "");
    if normalized_unit.is_empty() {
        MetricUnit::None.to_string().into()
    } else {
        normalized_unit
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_from() {
        let expected = "aA1_";

        let actual = super::normalize_unit("aA1_-./+ö{😀\n\t\r\\| ,").to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_from_empty() {
        let expected = "none";

        let actual = super::normalize_unit("").to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_from_empty_after_normalization() {
        let expected = "none";

        let actual = super::normalize_unit("+").to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_length_restriction() {
        let expected = "a".repeat(15);

        let actual = super::normalize_unit("a".repeat(20).as_ref()).to_string();

        assert_eq!(expected, actual);
    }
}
