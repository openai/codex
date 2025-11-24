use fancy_regex::Regex;

pub fn regex(re: &str) -> Regex {
    let parse_result = Regex::new(re);
    assert!(
        parse_result.is_ok(),
        "Expected regex '{}' to be compiled successfully, got {:?}",
        re,
        parse_result.err()
    );
    parse_result.unwrap()
}
