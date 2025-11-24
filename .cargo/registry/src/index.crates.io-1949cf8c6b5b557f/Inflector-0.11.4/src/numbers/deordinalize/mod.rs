/// Deorginalizes a `&str`
///
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "0.1";
///     let expected_string: String = "0.1".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "-1st";
///     let expected_string: String = "-1".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "0th";
///     let expected_string: String = "0".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "1st";
///     let expected_string: String = "1".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "2nd";
///     let expected_string: String = "2".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "3rd";
///     let expected_string: String = "3".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "9th";
///     let expected_string: String = "9".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "12th";
///     let expected_string: String = "12".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "12000th";
///     let expected_string: String = "12000".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "12001th";
///     let expected_string: String = "12001".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "12002nd";
///     let expected_string: String = "12002".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "12003rd";
///     let expected_string: String = "12003".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
/// ```
///     use inflector::numbers::deordinalize::deordinalize;
///     let mock_string: &str = "12004th";
///     let expected_string: String = "12004".to_owned();
///     let asserted_string: String = deordinalize(mock_string);
///     assert!(asserted_string == expected_string);
///
/// ```
pub fn deordinalize(non_ordinalized_string: &str) -> String {
    if non_ordinalized_string.contains('.') {
        non_ordinalized_string.to_owned()
    } else {
        non_ordinalized_string.trim_end_matches("st")
            .trim_end_matches("nd")
            .trim_end_matches("rd")
            .trim_end_matches("th")
            .to_owned()
    }
}
