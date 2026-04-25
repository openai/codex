use super::*;

#[test]
fn encode_path_segment_leaves_unreserved_ascii_unchanged() {
    assert_eq!(
        encode_path_segment("account-123_ABC.~"),
        "account-123_ABC.~"
    );
}

#[test]
fn encode_path_segment_escapes_path_separators_and_spaces() {
    assert_eq!(
        encode_path_segment("account/123 with space"),
        "account%2F123%20with%20space"
    );
}
