#![deny(warnings)]
use cases::case::*;
/// Determines if a `&str` is `kebab-case`
///
/// ```
///     use inflector::cases::kebabcase::is_kebab_case;
///     let mock_string: &str = "foo-bar-string-that-is-really-really-long";
///     let asserted_bool: bool = is_kebab_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::is_kebab_case;
///     let mock_string: &str = "FooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_kebab_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::is_kebab_case;
///     let mock_string: &str = "fooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_kebab_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::is_kebab_case;
///     let mock_string: &str = "FOO_BAR_STRING_THAT_IS_REALLY_REALLY_LONG";
///     let asserted_bool: bool = is_kebab_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::is_kebab_case;
///     let mock_string: &str = "foo_bar_string_that_is_really_really_long";
///     let asserted_bool: bool = is_kebab_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::is_kebab_case;
///     let mock_string: &str = "Foo bar string that is really really long";
///     let asserted_bool: bool = is_kebab_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::is_kebab_case;
///     let mock_string: &str = "Foo Bar Is A Really Really Long String";
///     let asserted_bool: bool = is_kebab_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
pub fn is_kebab_case(test_string: &str) -> bool {
    test_string == to_kebab_case(test_string.clone())
}

/// Converts a `&str` to `kebab-case` `String`
///
/// ```
///     use inflector::cases::kebabcase::to_kebab_case;
///     let mock_string: &str = "foo-bar";
///     let expected_string: String = "foo-bar".to_string();
///     let asserted_string: String = to_kebab_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::to_kebab_case;
///     let mock_string: &str = "FOO_BAR";
///     let expected_string: String = "foo-bar".to_string();
///     let asserted_string: String = to_kebab_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::to_kebab_case;
///     let mock_string: &str = "foo_bar";
///     let expected_string: String = "foo-bar".to_string();
///     let asserted_string: String = to_kebab_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::to_kebab_case;
///     let mock_string: &str = "Foo Bar";
///     let expected_string: String = "foo-bar".to_string();
///     let asserted_string: String = to_kebab_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::to_kebab_case;
///     let mock_string: &str = "Foo bar";
///     let expected_string: String = "foo-bar".to_string();
///     let asserted_string: String = to_kebab_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::to_kebab_case;
///     let mock_string: &str = "FooBar";
///     let expected_string: String = "foo-bar".to_string();
///     let asserted_string: String = to_kebab_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::kebabcase::to_kebab_case;
///     let mock_string: &str = "fooBar";
///     let expected_string: String = "foo-bar".to_string();
///     let asserted_string: String = to_kebab_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn to_kebab_case(non_kebab_case_string: &str) -> String {
    to_case_snake_like(non_kebab_case_string, "-", "lower")
}

#[cfg(all(feature = "unstable", test))]
mod benchmarks {
    extern crate test;
    use self::test::Bencher;

    #[bench]
    fn bench_kebab(b: &mut Bencher) {
        b.iter(|| super::to_kebab_case("Foo bar"));
    }

    #[bench]
    fn bench_is_kebab(b: &mut Bencher) {
        b.iter(|| super::is_kebab_case("Foo bar"));
    }

    #[bench]
    fn bench_kebab_from_snake(b: &mut Bencher) {
        b.iter(|| super::to_kebab_case("test_test_test"));
    }
}

#[cfg(test)]
mod tests {
    use ::to_kebab_case;
    use ::is_kebab_case;

    #[test]
    fn from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn from_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        let expected: String = "foo-bar".to_owned();
        assert_eq!(to_kebab_case(&convertable_string), expected)
    }

    #[test]
    fn is_correct_from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), true)
    }

    #[test]
    fn is_correct_from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        assert_eq!(is_kebab_case(&convertable_string), false)
    }
}

