use cases::snakecase::to_snake_case;

/// Converts a `&str` to a `foreign_key`
///
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "foo_bar";
///     let expected_string: String = "foo_bar_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "Foo bar";
///     let expected_string: String = "foo_bar_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "Foo Bar";
///     let expected_string: String = "foo_bar_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "Foo::Bar";
///     let expected_string: String = "bar_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "Test::Foo::Bar";
///     let expected_string: String = "bar_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "FooBar";
///     let expected_string: String = "foo_bar_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "fooBar";
///     let expected_string: String = "foo_bar_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::to_foreign_key;
///     let mock_string: &str = "fooBar3";
///     let expected_string: String = "foo_bar_3_id".to_owned();
///     let asserted_string: String = to_foreign_key(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn to_foreign_key(non_foreign_key_string: &str) -> String {
    if non_foreign_key_string.contains("::") {
        let split_string: Vec<&str> = non_foreign_key_string.split("::").collect();
        safe_convert(split_string[split_string.len() - 1])
    } else {
        safe_convert(non_foreign_key_string)
    }
}
fn safe_convert(safe_string: &str) -> String {
    let snake_cased: String = to_snake_case(safe_string);
    if snake_cased.ends_with("_id") {
        snake_cased
    } else {
        format!("{}{}", snake_cased, "_id")
    }
}

/// Determines if a `&str` is a `foreign_key`
///
/// ```
///     use inflector::suffix::foreignkey::is_foreign_key;
///     let mock_string: &str = "Foo bar string that is really really long";
///     let asserted_bool: bool = is_foreign_key(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::is_foreign_key;
///     let mock_string: &str = "foo-bar-string-that-is-really-really-long";
///     let asserted_bool: bool = is_foreign_key(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::is_foreign_key;
///     let mock_string: &str = "FooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_foreign_key(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::is_foreign_key;
///     let mock_string: &str = "Foo Bar Is A Really Really Long String";
///     let asserted_bool: bool = is_foreign_key(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::is_foreign_key;
///     let mock_string: &str = "fooBarIsAReallyReallyLongString";
///     let asserted_bool: bool = is_foreign_key(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::is_foreign_key;
///     let mock_string: &str = "foo_bar_string_that_is_really_really_long";
///     let asserted_bool: bool = is_foreign_key(mock_string);
///     assert!(asserted_bool == false);
///
/// ```
/// ```
///     use inflector::suffix::foreignkey::is_foreign_key;
///     let mock_string: &str = "foo_bar_string_that_is_really_really_long_id";
///     let asserted_bool: bool = is_foreign_key(mock_string);
///     assert!(asserted_bool == true);
///
/// ```
pub fn is_foreign_key(test_string: &str) -> bool {
    to_foreign_key(test_string.clone()) == test_string
}
