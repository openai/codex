use super::*;
use serde_with::{DefaultOnError, DefaultOnNull};

#[test]
fn test_default_on_error() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "DefaultOnError<DisplayFromStr>")] u32);

    // Normal
    is_equal(S(123), expect![[r#""123""#]]);
    is_equal(S(0), expect![[r#""0""#]]);
    // Error cases
    check_deserialization(S(0), r#""""#);
    check_deserialization(S(0), r#""12+3""#);
    check_deserialization(S(0), r#""abc""#);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2(#[serde_as(as = "DefaultOnError<Vec<DisplayFromStr>>")] Vec<u32>);

    // Normal
    is_equal(
        S2(vec![1, 2, 3]),
        expect![[r#"
            [
              "1",
              "2",
              "3"
            ]"#]],
    );
    is_equal(S2(vec![]), expect![[r#"[]"#]]);
    // Error cases
    check_deserialization(S2(vec![]), r#"2"#);
    check_deserialization(S2(vec![]), r#""not_a_list""#);
    check_deserialization(S2(vec![]), r#"{}"#);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Struct2 {
        #[serde_as(as = "DefaultOnError<Vec<DisplayFromStr>>")]
        value: Vec<u32>,
    }
    check_deserialization(Struct2 { value: vec![] }, r#"{"value":}"#);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S3(#[serde_as(as = "Vec<DefaultOnError<DisplayFromStr>>")] Vec<u32>);

    // Normal
    is_equal(
        S3(vec![1, 2, 3]),
        expect![[r#"
            [
              "1",
              "2",
              "3"
            ]"#]],
    );
    is_equal(S3(vec![]), expect![[r#"[]"#]]);
    // Error cases
    check_deserialization(S3(vec![0, 3, 0]), r#"[2,"3",4]"#);
    check_deserialization(S3(vec![0, 0]), r#"["AA",5]"#);
}

#[test]
fn test_default_on_null() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "DefaultOnNull<DisplayFromStr>")] u32);

    // Normal
    is_equal(S(123), expect![[r#""123""#]]);
    is_equal(S(0), expect![[r#""0""#]]);
    // Null case
    check_deserialization(S(0), r#"null"#);
    // Error cases
    check_error_deserialization::<S>(
        r#""12+3""#,
        expect![[r#"invalid digit found in string at line 1 column 6"#]],
    );
    check_error_deserialization::<S>(
        r#""abc""#,
        expect![[r#"invalid digit found in string at line 1 column 5"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2(#[serde_as(as = "Vec<DefaultOnNull>")] Vec<u32>);

    // Normal
    is_equal(
        S2(vec![1, 2, 0, 3]),
        expect![[r#"
            [
              1,
              2,
              0,
              3
            ]"#]],
    );
    is_equal(S2(vec![]), expect![[r#"[]"#]]);
    // Null cases
    check_deserialization(S2(vec![1, 0, 2]), r#"[1, null, 2]"#);
    check_error_deserialization::<S2>(
        r#"["not_a_number"]"#,
        expect![[r#"invalid type: string "not_a_number", expected u32 at line 1 column 15"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S3(#[serde_as(as = "Vec<DefaultOnNull<DisplayFromStr>>")] Vec<u32>);

    // Normal
    is_equal(
        S3(vec![1, 2, 3]),
        expect![[r#"
            [
              "1",
              "2",
              "3"
            ]"#]],
    );
    // Null case
    check_deserialization(S3(vec![0, 3, 0]), r#"[null,"3",null]"#);
    check_error_deserialization::<S3>(
        r#"[null,3,null]"#,
        expect![[r#"invalid type: integer `3`, expected a string at line 1 column 7"#]],
    );
}
