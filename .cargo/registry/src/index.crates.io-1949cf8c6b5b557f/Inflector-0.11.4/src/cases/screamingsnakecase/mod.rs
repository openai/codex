#![deny(warnings)]
use cases::case::*;
/// Converts a `&str` to `SCREAMING_SNAKE_CASE` `String`
///
/// ```
///     use inflector::cases::screamingsnakecase::to_screaming_snake_case;
///     let mock_string: &str = "foo_bar";
///     let expected_string: String = "FOO_BAR".to_string();
///     let asserted_string: String = to_screaming_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::to_screaming_snake_case;
///     let mock_string: &str = "HTTP Foo bar";
///     let expected_string: String = "HTTP_FOO_BAR".to_string();
///     let asserted_string: String = to_screaming_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::to_screaming_snake_case;
///     let mock_string: &str = "Foo bar";
///     let expected_string: String = "FOO_BAR".to_string();
///     let asserted_string: String = to_screaming_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::to_screaming_snake_case;
///     let mock_string: &str = "Foo Bar";
///     let expected_string: String = "FOO_BAR".to_string();
///     let asserted_string: String = to_screaming_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::to_screaming_snake_case;
///     let mock_string: &str = "FooBar";
///     let expected_string: String = "FOO_BAR".to_string();
///     let asserted_string: String = to_screaming_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::to_screaming_snake_case;
///     let mock_string: &str = "fooBar";
///     let expected_string: String = "FOO_BAR".to_string();
///     let asserted_string: String = to_screaming_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::to_screaming_snake_case;
///     let mock_string: &str = "fooBar3";
///     let expected_string: String = "FOO_BAR_3".to_string();
///     let asserted_string: String = to_screaming_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn to_screaming_snake_case(non_snake_case_string: &str) -> String {
    to_case_snake_like(non_snake_case_string, "_", "upper")
}

/// Determines of a `&str` is `SCREAMING_SNAKE_CASE`
///
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "Foo bar string that is really really long";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "foo-bar-string-that-is-really-really-long";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "FooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "Foo Bar Is A Really Really Long String";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "fooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "FOO_BAR_STRING_THAT_IS_REALLY_REALLY_LONG";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "FOO_BAR1_STRING_THAT_IS_REALLY_REALLY_LONG";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
/// ```
///     use inflector::cases::screamingsnakecase::is_screaming_snake_case;
///     let mock_string: &str = "FOO_BAR_1_STRING_THAT_IS_REALLY_REALLY_LONG";
///     let asserted_bool: bool = is_screaming_snake_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
pub fn is_screaming_snake_case(test_string: &str) -> bool {
    test_string == to_screaming_snake_case(test_string.clone())
}


#[cfg(all(feature = "unstable", test))]
mod benchmarks {
    extern crate test;
    use self::test::Bencher;

    #[bench]
    fn bench_screaming_snake(b: &mut Bencher) {
        b.iter(|| super::to_screaming_snake_case("Foo bar"));
    }

    #[bench]
    fn bench_is_screaming_snake(b: &mut Bencher) {
        b.iter(|| super::is_screaming_snake_case("Foo bar"));
    }

}

#[cfg(test)]
mod tests {
    use ::to_screaming_snake_case;
    use ::is_screaming_snake_case;

    #[test]
    fn from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_screaming_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        let expected: String = "FOO_BAR".to_owned();
        assert_eq!(to_screaming_snake_case(&convertable_string), expected)
    }

    #[test]
    fn is_correct_from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_screaming_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), true)
    }

    #[test]
    fn is_correct_from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        assert_eq!(is_screaming_snake_case(&convertable_string), false)
    }
}
