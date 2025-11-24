use assert_json_diff::{
    assert_json_eq, assert_json_include, assert_json_matches, assert_json_matches_no_panic,
    CompareMode, Config, NumericMode,
};
use serde::Serialize;
use serde_json::json;

#[test]
fn can_pass() {
    assert_json_include!(
        actual: json!({ "a": { "b": true }, "c": [true, null, 1] }),
        expected: json!({ "a": { "b": true }, "c": [true, null, 1] })
    );

    assert_json_include!(
        actual: json!({ "a": { "b": true } }),
        expected: json!({ "a": {} })
    );

    assert_json_include!(
        actual: json!({ "a": { "b": true } }),
        expected: json!({ "a": {} }),
    );

    assert_json_include!(
        expected: json!({ "a": {} }),
        actual: json!({ "a": { "b": true } }),
    );
}

#[test]
#[should_panic]
fn can_fail() {
    assert_json_include!(
        actual: json!({ "a": { "b": true }, "c": [true, null, 1] }),
        expected: json!({ "a": { "b": false }, "c": [false, null, {}] })
    );
}

#[test]
#[should_panic]
fn different_numeric_types_include_should_fail() {
    assert_json_include!(
        actual: json!({ "a": { "b": true }, "c": 1 }),
        expected: json!({ "a": { "b": true }, "c": 1.0 })
    );
}

#[test]
#[should_panic]
fn different_numeric_types_eq_should_fail() {
    assert_json_eq!(
        json!({ "a": { "b": true }, "c": 1 }),
        json!({ "a": { "b": true }, "c": 1.0 })
    );
}

#[test]
fn different_numeric_types_assume_float() {
    let actual = json!({ "a": { "b": true }, "c": [true, null, 1] });
    let expected = json!({ "a": { "b": true }, "c": [true, null, 1.0] });
    let config = Config::new(CompareMode::Inclusive).numeric_mode(NumericMode::AssumeFloat);
    assert_json_matches!(actual, expected, config.clone());

    assert_json_matches!(actual, expected, config.compare_mode(CompareMode::Strict))
}

#[test]
fn can_pass_with_exact_match() {
    assert_json_eq!(json!({ "a": { "b": true } }), json!({ "a": { "b": true } }));
    assert_json_eq!(json!({ "a": { "b": true } }), json!({ "a": { "b": true } }),);
}

#[test]
#[should_panic]
fn can_fail_with_exact_match() {
    assert_json_eq!(json!({ "a": { "b": true } }), json!({ "a": {} }));
}

#[test]
fn inclusive_match_without_panicking() {
    assert!(assert_json_matches_no_panic(
        &json!({ "a": 1, "b": 2 }),
        &json!({ "b": 2}),
        Config::new(CompareMode::Inclusive,).numeric_mode(NumericMode::Strict),
    )
    .is_ok());

    assert!(assert_json_matches_no_panic(
        &json!({ "a": 1, "b": 2 }),
        &json!("foo"),
        Config::new(CompareMode::Inclusive,).numeric_mode(NumericMode::Strict),
    )
    .is_err());
}

#[test]
fn exact_match_without_panicking() {
    assert!(assert_json_matches_no_panic(
        &json!([1, 2, 3]),
        &json!([1, 2, 3]),
        Config::new(CompareMode::Strict).numeric_mode(NumericMode::Strict)
    )
    .is_ok());

    assert!(assert_json_matches_no_panic(
        &json!([1, 2, 3]),
        &json!("foo"),
        Config::new(CompareMode::Strict).numeric_mode(NumericMode::Strict)
    )
    .is_err());
}

#[derive(Serialize)]
struct User {
    id: i32,
    username: String,
}

#[test]
fn include_with_serializable() {
    let user = User {
        id: 1,
        username: "bob".to_string(),
    };

    assert_json_include!(
        actual: json!({
            "id": 1,
            "username": "bob",
            "email": "bob@example.com"
        }),
        expected: user,
    );
}

#[test]
fn include_with_serializable_ref() {
    let user = User {
        id: 1,
        username: "bob".to_string(),
    };

    assert_json_include!(
        actual: &json!({
             "id": 1,
             "username": "bob",
             "email": "bob@example.com"
         }),
        expected: &user,
    );
}

#[test]
fn eq_with_serializable() {
    let user = User {
        id: 1,
        username: "bob".to_string(),
    };

    assert_json_eq!(
        json!({
            "id": 1,
            "username": "bob"
        }),
        user,
    );
}

#[test]
fn eq_with_serializable_ref() {
    let user = User {
        id: 1,
        username: "bob".to_string(),
    };

    assert_json_eq!(
        &json!({
            "id": 1,
            "username": "bob"
        }),
        &user,
    );
}
