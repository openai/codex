#[cfg(feature = "heavyweight")]
use cases::classcase::to_class_case;

#[cfg(feature = "heavyweight")]
/// Deconstantizes a `&str`
///
/// ```
///     use inflector::string::deconstantize::deconstantize;
///     let mock_string: &str = "Bar";
///     let expected_string: String = "".to_owned();
///     let asserted_string: String = deconstantize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::deconstantize::deconstantize;
///     let mock_string: &str = "::Bar";
///     let expected_string: String = "".to_owned();
///     let asserted_string: String = deconstantize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::deconstantize::deconstantize;
///     let mock_string: &str = "Foo::Bar";
///     let expected_string: String = "Foo".to_owned();
///     let asserted_string: String = deconstantize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::deconstantize::deconstantize;
///     let mock_string: &str = "Test::Foo::Bar";
///     let expected_string: String = "Foo".to_owned();
///     let asserted_string: String = deconstantize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn deconstantize(non_deconstantized_string: &str) -> String {
    if non_deconstantized_string.contains("::") {
        let split_string: Vec<&str> = non_deconstantized_string.split("::").collect();
        if split_string.len() > 1 {
            to_class_case(split_string[split_string.len() - 2])
        } else {
            "".to_owned()
        }
    } else {
        "".to_owned()
    }
}
