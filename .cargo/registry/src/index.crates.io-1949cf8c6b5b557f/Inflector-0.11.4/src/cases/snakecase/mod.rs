#![deny(warnings)]
use cases::case::*;
/// Converts a `&str` to `snake_case` `String`
///
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "foo_bar";
///     let expected_string: String = "foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "HTTP Foo bar";
///     let expected_string: String = "http_foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "HTTPFooBar";
///     let expected_string: String = "http_foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "Foo bar";
///     let expected_string: String = "foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "Foo Bar";
///     let expected_string: String = "foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "FooBar";
///     let expected_string: String = "foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "FOO_BAR";
///     let expected_string: String = "foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "fooBar";
///     let expected_string: String = "foo_bar".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::cases::snakecase::to_snake_case;
///     let mock_string: &str = "fooBar3";
///     let expected_string: String = "foo_bar_3".to_string();
///     let asserted_string: String = to_snake_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn to_snake_case(non_snake_case_string: &str) -> String {
    to_case_snake_like(non_snake_case_string, "_", "lower")
}

/// Determines of a `&str` is `snake_case`
///
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "Foo bar string that is really really long";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "foo-bar-string-that-is-really-really-long";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "FooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "Foo Bar Is A Really Really Long String";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "FOO_BAR_IS_A_REALLY_REALLY_LONG_STRING";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "fooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "foo_bar_string_that_is_really_really_long";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "foo_bar1_string_that_is_really_really_long";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::cases::snakecase::is_snake_case;
///     let mock_string: &str = "foo_bar_1_string_that_is_really_really_long";
///     let asserted_bool: bool = is_snake_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
pub fn is_snake_case(test_string: &str) -> bool {
    test_string == to_snake_case(test_string.clone())
}

#[cfg(all(feature = "unstable", test))]
mod benchmarks {
    extern crate test;
    use self::test::Bencher;

    #[bench]
    fn bench_snake_from_title(b: &mut Bencher) {
        b.iter(|| super::to_snake_case("Foo bar"));
    }

    #[bench]
    fn bench_snake_from_camel(b: &mut Bencher) {
        b.iter(|| super::to_snake_case("fooBar"));
    }

    #[bench]
    fn bench_snake_from_snake(b: &mut Bencher) {
        b.iter(|| super::to_snake_case("foo_bar_bar_bar"));
    }

    #[bench]
    fn bench_is_snake(b: &mut Bencher) {
        b.iter(|| super::is_snake_case("Foo bar"));
    }

}

#[cfg(test)]
mod tests {
    use ::to_snake_case;
    use ::is_snake_case;

    #[test]
    fn from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn from_case_with_loads_of_space() {
        let convertable_string: String = "foo           bar".to_owned();
        let expected: String = "foo_bar".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn a_name_with_a_dot() {
        let convertable_string: String = "Robert C. Martin".to_owned();
        let expected: String = "robert_c_martin".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn random_text_with_bad_chars() {
        let convertable_string: String = "Random text with *(bad) chars".to_owned();
        let expected: String = "random_text_with_bad_chars".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn trailing_bad_chars() {
        let convertable_string: String = "trailing bad_chars*(()())".to_owned();
        let expected: String = "trailing_bad_chars".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn leading_bad_chars() {
        let convertable_string: String = "-!#$%leading bad chars".to_owned();
        let expected: String = "leading_bad_chars".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn wrapped_in_bad_chars() {
        let convertable_string: String = "-!#$%wrapped in bad chars&*^*&(&*^&(<><?>><?><>))".to_owned();
        let expected: String = "wrapped_in_bad_chars".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn has_a_sign() {
        let convertable_string: String = "has a + sign".to_owned();
        let expected: String = "has_a_sign".to_owned();
        assert_eq!(to_snake_case(&convertable_string), expected)
    }

    #[test]
    fn is_correct_from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        assert_eq!(is_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        assert_eq!(is_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        assert_eq!(is_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        assert_eq!(is_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        assert_eq!(is_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        assert_eq!(is_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        assert_eq!(is_snake_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        assert_eq!(is_snake_case(&convertable_string), true)
    }
}
