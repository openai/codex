#[cfg(feature = "heavyweight")]
use cases::classcase::to_class_case;

#[cfg(feature = "heavyweight")]
/// Demodulize a `&str`
///
/// ```
///     use inflector::string::demodulize::demodulize;
///     let mock_string: &str = "Bar";
///     let expected_string: String = "Bar".to_owned();
///     let asserted_string: String = demodulize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::demodulize::demodulize;
///     let mock_string: &str = "::Bar";
///     let expected_string: String = "Bar".to_owned();
///     let asserted_string: String = demodulize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::demodulize::demodulize;
///     let mock_string: &str = "Foo::Bar";
///     let expected_string: String = "Bar".to_owned();
///     let asserted_string: String = demodulize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::string::demodulize::demodulize;
///     let mock_string: &str = "Test::Foo::Bar";
///     let expected_string: String = "Bar".to_owned();
///     let asserted_string: String = demodulize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn demodulize(non_demodulize_string: &str) -> String {
    if non_demodulize_string.contains("::") {
        let split_string: Vec<&str> = non_demodulize_string.split("::").collect();
        to_class_case(split_string[split_string.len() - 1])
    } else {
        non_demodulize_string.to_owned()
    }
}
