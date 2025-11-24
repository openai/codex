use super::*;
use serde_with::{formats::SpaceSeparator, PickFirst};

#[test]
fn test_pick_first_two() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "PickFirst<(_, DisplayFromStr)>")] u32);

    is_equal(S(123), expect![[r#"123"#]]);
    check_deserialization(S(123), r#""123""#);
    check_error_deserialization::<S>(
        r#""Abc""#,
        expect![[r#"
            PickFirst could not deserialize any variant:
              First: invalid type: string "Abc", expected u32
              Second: invalid digit found in string"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2(#[serde_as(as = "PickFirst<(DisplayFromStr, _)>")] u32);

    is_equal(S2(123), expect![[r#""123""#]]);
    check_deserialization(S2(123), r#"123"#);

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S3(
        #[serde_as(as = "PickFirst<(_, StringWithSeparator::<SpaceSeparator, String>,)>")]
        Vec<String>,
    );
    is_equal(
        S3(vec!["A".to_string(), "B".to_string(), "C".to_string()]),
        expect![[r#"
            [
              "A",
              "B",
              "C"
            ]"#]],
    );
    check_deserialization(
        S3(vec!["A".to_string(), "B".to_string(), "C".to_string()]),
        r#""A B C""#,
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S4(
        #[serde_as(as = "PickFirst<(StringWithSeparator::<CommaSeparator, String>, _,)>")]
        Vec<String>,
    );
    is_equal(
        S4(vec!["A".to_string(), "B".to_string(), "C".to_string()]),
        expect![[r#""A,B,C""#]],
    );
    check_deserialization(
        S4(vec!["A".to_string(), "B".to_string(), "C".to_string()]),
        r#"["A", "B", "C"]"#,
    );
}

#[test]
fn test_pick_first_three() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(
        #[serde_as(
            as = "PickFirst<(_, Vec<DisplayFromStr>, StringWithSeparator::<CommaSeparator, u32>)>"
        )]
        Vec<u32>,
    );
    is_equal(
        S(vec![1, 2, 3]),
        expect![[r#"
        [
          1,
          2,
          3
        ]"#]],
    );
    check_deserialization(
        S(vec![1, 2, 3]),
        r#"
        [
          "1",
          "2",
          "3"
        ]"#,
    );
    check_deserialization(S(vec![1, 2, 3]), r#""1,2,3""#);
    check_error_deserialization::<S>(
        r#""Abc""#,
        expect![[r#"
            PickFirst could not deserialize any variant:
              First: invalid type: string "Abc", expected a sequence
              Second: invalid type: string "Abc", expected a sequence
              Third: invalid digit found in string"#]],
    );

    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S2(
        #[serde_as(
            as = "PickFirst<(StringWithSeparator::<CommaSeparator, u32>, _, Vec<DisplayFromStr>)>"
        )]
        Vec<u32>,
    );
    is_equal(S2(vec![1, 2, 3]), expect![[r#""1,2,3""#]]);
    check_deserialization(
        S2(vec![1, 2, 3]),
        r#"
        [
          "1",
          "2",
          "3"
        ]"#,
    );
    check_deserialization(
        S2(vec![1, 2, 3]),
        r#"
        [
          1,
          2,
          3
        ]"#,
    );
}

#[test]
fn test_pick_first_four() {
    #[serde_as]
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct S(#[serde_as(as = "PickFirst<(_, _, _, _)>")] u32);

    is_equal(S(123), expect![[r#"123"#]]);
    check_error_deserialization::<S>(
        r#""Abc""#,
        expect![[r#"
            PickFirst could not deserialize any variant:
              First: invalid type: string "Abc", expected u32
              Second: invalid type: string "Abc", expected u32
              Third: invalid type: string "Abc", expected u32
              Fourth: invalid type: string "Abc", expected u32"#]],
    );
}
