use super::*;
use serde_with::serde_conv;

#[test]
fn test_bool_as_string() {
    serde_conv!(BoolAsString, bool, |x: &bool| x.to_string(), |x: String| x
        .parse());

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct SWith(#[serde(with = "BoolAsString")] bool);

    is_equal(SWith(false), expect![[r#""false""#]]);
    is_equal(SWith(true), expect![[r#""true""#]]);
    check_error_deserialization::<SWith>(
        "123",
        expect![[r#"invalid type: integer `123`, expected a string at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct SAs(#[serde_as(as = "BoolAsString")] bool);

    is_equal(SAs(false), expect![[r#""false""#]]);
    is_equal(SAs(true), expect![[r#""true""#]]);
    check_error_deserialization::<SAs>(
        "123",
        expect![[r#"invalid type: integer `123`, expected a string at line 1 column 3"#]],
    );

    #[serde_as]
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct SAsVec(#[serde_as(as = "Vec<BoolAsString>")] Vec<bool>);

    is_equal(
        SAsVec(vec![false]),
        expect![[r#"
        [
          "false"
        ]"#]],
    );
    is_equal(
        SAsVec(vec![true]),
        expect![[r#"
        [
          "true"
        ]"#]],
    );
    check_error_deserialization::<SAsVec>(
        "123",
        expect![[r#"invalid type: integer `123`, expected a sequence at line 1 column 3"#]],
    );
}
