#![deny(warnings)]
use cases::case::*;
#[cfg(feature = "heavyweight")]
use string::singularize::to_singular;
#[cfg(feature = "heavyweight")]
/// Converts a `&str` to `ClassCase` `String`
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "FooBar";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "FooBars";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "Foo Bar";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "foo-bar";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "fooBar";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "FOO_BAR";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "foo_bar";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "foo_bars";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::to_class_case;
///     let mock_string: &str = "Foo bar";
///     let expected_string: String = "FooBar".to_string();
///     let asserted_string: String = to_class_case(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn to_class_case(non_class_case_string: &str) -> String {
    let options = CamelOptions {
        new_word: true,
        last_char: ' ',
        first_word: false,
        injectable_char: ' ',
        has_seperator: false,
        inverted: false,
    };
    let class_plural = to_case_camel_like(non_class_case_string, options);
    let split: (&str, &str) =
        class_plural.split_at(class_plural.rfind(char::is_uppercase).unwrap_or(0));
    format!("{}{}", split.0, to_singular(split.1))
}

#[cfg(feature = "heavyweight")]
/// Determines if a `&str` is `ClassCase` `bool`
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "Foo";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "foo";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "FooBarIsAReallyReallyLongStrings";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "FooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "foo-bar-string-that-is-really-really-long";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "foo_bar_is_a_really_really_long_strings";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "fooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "FOO_BAR_STRING_THAT_IS_REALLY_REALLY_LONG";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "foo_bar_string_that_is_really_really_long";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "Foo bar string that is really really long";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
///
/// ```
///     use inflector::cases::classcase::is_class_case;
///     let mock_string: &str = "Foo Bar Is A Really Really Long String";
///     let asserted_bool: bool = is_class_case(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
pub fn is_class_case(test_string: &str) -> bool {
    to_class_case(&test_string.clone()) == test_string
}

#[cfg(all(feature = "unstable", test))]
#[cfg(feature = "heavyweight")]
mod benchmarks {
    extern crate test;
    use self::test::Bencher;

    #[bench]
    fn bench_class_case(b: &mut Bencher) {
        b.iter(|| super::to_class_case("Foo bar"));
    }

    #[bench]
    fn bench_is_class(b: &mut Bencher) {
        b.iter(|| super::is_class_case("Foo bar"));
    }

    #[bench]
    fn bench_class_from_snake(b: &mut Bencher) {
        b.iter(|| super::to_class_case("foo_bar"));
    }
}

#[cfg(test)]
#[cfg(feature = "heavyweight")]
mod tests {
    use ::to_class_case;
    use ::is_class_case;

    #[test]
    fn from_camel_case() {
        let convertable_string: String = "fooBar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_screaming_class_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_table_case() {
        let convertable_string: String = "foo_bars".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn from_case_with_loads_of_space() {
        let convertable_string: String = "foo           bar".to_owned();
        let expected: String = "FooBar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn a_name_with_a_dot() {
        let convertable_string: String = "Robert C. Martin".to_owned();
        let expected: String = "RobertCMartin".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn random_text_with_bad_chars() {
        let convertable_string: String = "Random text with *(bad) chars".to_owned();
        let expected: String = "RandomTextWithBadChar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn trailing_bad_chars() {
        let convertable_string: String = "trailing bad_chars*(()())".to_owned();
        let expected: String = "TrailingBadChar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn leading_bad_chars() {
        let convertable_string: String = "-!#$%leading bad chars".to_owned();
        let expected: String = "LeadingBadChar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn wrapped_in_bad_chars() {
        let convertable_string: String = "-!#$%wrapped in bad chars&*^*&(&*^&(<><?>><?><>))".to_owned();
        let expected: String = "WrappedInBadChar".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn has_a_sign() {
        let convertable_string: String = "has a + sign".to_owned();
        let expected: String = "HasASign".to_owned();
        assert_eq!(to_class_case(&convertable_string), expected)
    }

    #[test]
    fn is_correct_from_class_case() {
        let convertable_string: String = "fooBar".to_owned();
        assert_eq!(is_class_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_pascal_case() {
        let convertable_string: String = "FooBar".to_owned();
        assert_eq!(is_class_case(&convertable_string), true)
    }

    #[test]
    fn is_correct_from_kebab_case() {
        let convertable_string: String = "foo-bar".to_owned();
        assert_eq!(is_class_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_sentence_case() {
        let convertable_string: String = "Foo bar".to_owned();
        assert_eq!(is_class_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_title_case() {
        let convertable_string: String = "Foo Bar".to_owned();
        assert_eq!(is_class_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_train_case() {
        let convertable_string: String = "Foo-Bar".to_owned();
        assert_eq!(is_class_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_screaming_snake_case() {
        let convertable_string: String = "FOO_BAR".to_owned();
        assert_eq!(is_class_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_snake_case() {
        let convertable_string: String = "foo_bar".to_owned();
        assert_eq!(is_class_case(&convertable_string), false)
    }

    #[test]
    fn is_correct_from_table_case() {
        let convertable_string: String = "FooBar".to_owned();
        assert_eq!(is_class_case(&convertable_string), true)
    }
}

