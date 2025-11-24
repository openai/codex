#![deny(warnings)]
#[cfg(feature = "heavyweight")]
use string::pluralize::to_plural;
#[cfg(feature = "heavyweight")]
use cases::case::*;
#[cfg(feature = "heavyweight")]
/// Converts a `&str` to `table-case` `String`
///
/// ```
/// use inflector::cases::tablecase::to_table_case;
/// let mock_string: &str = "foo-bar";
/// let expected_string: String = "foo_bars".to_string();
/// let asserted_string: String = to_table_case(mock_string);
/// assert!(asserted_string == expected_string);
/// ```
///
/// ```
/// use inflector::cases::tablecase::to_table_case;
/// let mock_string: &str = "FOO_BAR";
/// let expected_string: String = "foo_bars".to_string();
/// let asserted_string: String = to_table_case(mock_string);
/// assert!(asserted_string == expected_string);
/// ```
///
/// ```
/// use inflector::cases::tablecase::to_table_case;
/// let mock_string: &str = "foo_bar";
/// let expected_string: String = "foo_bars".to_string();
/// let asserted_string: String = to_table_case(mock_string);
/// assert!(asserted_string == expected_string);
/// ```
///
/// ```
/// use inflector::cases::tablecase::to_table_case;
/// let mock_string: &str = "Foo Bar";
/// let expected_string: String = "foo_bars".to_string();
/// let asserted_string: String = to_table_case(mock_string);
/// assert!(asserted_string == expected_string);
/// ```
///
/// ```
/// use inflector::cases::tablecase::to_table_case;
/// let mock_string: &str = "Foo bar";
/// let expected_string: String = "foo_bars".to_string();
/// let asserted_string: String = to_table_case(mock_string);
/// assert!(asserted_string == expected_string);
/// ```
///
/// ```
/// use inflector::cases::tablecase::to_table_case;
/// let mock_string: &str = "FooBar";
/// let expected_string: String = "foo_bars".to_string();
/// let asserted_string: String = to_table_case(mock_string);
/// assert!(asserted_string == expected_string);
/// ```
///
/// ```
/// use inflector::cases::tablecase::to_table_case;
/// let mock_string: &str = "fooBar";
/// let expected_string: String = "foo_bars".to_string();
/// let asserted_string: String = to_table_case(mock_string);
/// assert!(asserted_string == expected_string);
/// ```
pub fn to_table_case(non_table_case_string: &str) -> String {
    let snaked: String = to_case_snake_like(non_table_case_string, "_", "lower");
    let split: (&str, &str) = snaked.split_at(snaked.rfind('_').unwrap_or(0));
    format!("{}{}", split.0, to_plural(split.1))
}

#[cfg(feature = "heavyweight")]
/// Determines if a `&str` is `table-case`
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "foo_bar_strings";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == true);
/// ```
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "foo-bar-string-that-is-really-really-long";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == false);
/// ```
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "FooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == false);
/// ```
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "fooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == false);
/// ```
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "FOO_BAR_STRING_THAT_IS_REALLY_REALLY_LONG";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == false);
/// ```
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "foo_bar_string_that_is_really_really_long";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == false);
/// ```
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "Foo bar string that is really really long";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == false);
/// ```
///
/// ```
///     use inflector::cases::tablecase::is_table_case;
///     let mock_string: &str = "Foo Bar Is A Really Really Long String";
///     let asserted_bool: bool = is_table_case(mock_string);
///     assert!(asserted_bool == false);
/// ```
pub fn is_table_case(test_string: &str) -> bool {
     to_table_case(&test_string.clone()) == test_string
}

#[cfg(all(feature = "unstable", test))]
#[cfg(feature = "heavyweight")]
mod benchmarks {
    extern crate test;
    use self::test::Bencher;

    #[bench]
    fn bench_table_case(b: &mut Bencher) {
        b.iter(|| super::to_table_case("Foo bar"));
    }

    #[bench]
    fn bench_is_table_case(b: &mut Bencher) {
        b.iter(|| super::is_table_case("Foo bar"));
    }
}

#[cfg(test)]
#[cfg(feature = "heavyweight")]
mod tests {
    use ::to_table_case;
    use ::is_table_case;

    #[test]
    fn from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn from_table_case() {
        let convertable_string: String = "foo_bars".to_owned();
        let expected: String = "foo_bars".to_owned();
        assert_eq!(to_table_case(&convertable_string), expected)
    }

    #[test]
    fn is_correct_from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        assert_eq!(is_table_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_table_case() {
        let convertable_string: String = "foo_bars".to_owned();
        assert_eq!(is_table_case(&convertable_string), true)
    }
}
