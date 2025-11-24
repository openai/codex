mod common;
use std::env;

use common::tempdir_with_dotenv;
use dotenvy::dotenv;

#[test]
fn test_issue_12() {
    let _f = tempdir_with_dotenv(
        r#"
# Start of .env file
# Comment line with single ' quote
# Comment line with double " quote
 # Comment line with double " quote and starts with a space
TESTKEY1=test_val # 1 '" comment
TESTKEY2=test_val_with_#_hash # 2 '" comment
TESTKEY3="test_val quoted with # hash" # 3 '" comment
TESTKEY4="Line 1
# Line 2
Line 3" # 4 Multiline "' comment
TESTKEY5="Line 4
# Line 5
Line 6
" # 5 Multiline "' comment
# End of .env file
"#,
    )
    .expect("should write test env");

    dotenv().expect("should succeed");
    assert_eq!(
        env::var("TESTKEY1").expect("testkey1 env key not set"),
        "test_val"
    );
    assert_eq!(
        env::var("TESTKEY2").expect("testkey2 env key not set"),
        "test_val_with_#_hash"
    );
    assert_eq!(
        env::var("TESTKEY3").expect("testkey3 env key not set"),
        "test_val quoted with # hash"
    );
    assert_eq!(
        env::var("TESTKEY4").expect("testkey4 env key not set"),
        r#"Line 1
# Line 2
Line 3"#
    );
    assert_eq!(
        env::var("TESTKEY5").expect("testkey5 env key not set"),
        r#"Line 4
# Line 5
Line 6
"#
    );
}
