//! Checks lossless comparison independently of the MCP envelope.

use pretty_assertions::assert_eq;

fn equivalent(left: &str, right: &str) -> bool {
    let a = serde_json::from_str(left).unwrap();
    let b = serde_json::from_str(right).unwrap();
    let matched = super::equivalent(a, b);
    assert_eq!(matched, super::equivalent(b, a));
    matched
}

#[test]
fn reordered_fields_can_skip_over_nested_or_flat_values_after_a_common_prefix() {
    assert!(equivalent(
        r#"{"id":1,"a":[{"s":"\\\"],{","t":[[]]},{}],"b":{"\u006b":"x"},"c":null}"#,
        r#"{ "id":1.0,"c":null,"b":{"k":"\u0078"},"\u0061":[{"t":[ [] ],"s":"\\\"],{"},{}] }"#,
    ));
}

#[test]
fn deeply_nested_objects_can_reorder_around_a_large_value_at_every_level() {
    let leaf = format!("[{}0]", "1,".repeat(32_768));
    let left = format!(
        "{}{leaf}{}",
        r#"{"first":true,"v":"#.repeat(60),
        "}".repeat(60)
    );
    let right = format!(
        "{}{leaf}{}",
        r#"{"v": "#.repeat(60),
        r#","first":true}"#.repeat(60)
    );
    assert!(equivalent(&left, &right));
}

#[test]
fn requires_identical_subtrees_at_the_recursion_limit() {
    let deep = format!("{}0{}", "[".repeat(1_000), "]".repeat(1_000));
    assert!(equivalent(&format!("[{deep}]"), &format!("[ {deep}]")));
    assert!(!equivalent(&deep, &deep.replace('0', "0.0")));
}

#[test]
fn different_values_and_repeated_keys_do_not_disappear() {
    for (left, right) in [
        ("9007199254740993", "9007199254740992"),
        ("[1.00000000000000001]", "[1]"),
        ("1.25000000000000001", "1.25"),
        ("1e-500", "0"),
        ("-0", "0"),
        ("-0.0", "0"),
        ("[-0e3]", "[0]"),
        ("10e9223372036854775807", "1"),
        ("-1.0", "1"),
        (r#""1.0""#, "1"),
        ("[2,1]", "[1,2]"),
        ("[1]", "[1,2]"),
        (r#""a b""#, r#""ab""#),
        (r#"{"a":2,"a":1}"#, r#"{"a":1}"#),
        (r#"{"a":1,"b":0}"#, r#"{"b":0,"\u0061":2,"a":1}"#),
        (r#"{"a":1,"b":0}"#, r#"{"b":0,"c":1}"#),
        (r#"{"a":[],"b":{}}"#, r#"{"b":[],"a":{}}"#),
    ] {
        assert!(!equivalent(left, right), "{left} vs {right}");
    }
}

#[test]
fn large_arrays_can_contain_reordered_objects_and_equivalent_decimals() {
    let rows = r#"{"a":1,"b":2},"#.repeat(20_000);
    let other_rows = r#"{"b":2,"a":1},"#.repeat(20_000);
    assert!(equivalent(
        &format!("[{rows}1]"),
        &format!("[{other_rows}1.0]")
    ));
}

#[test]
fn wide_objects_can_match_with_reordered_fields() {
    let mut fields: Vec<_> = (0..1_000).map(|i| format!(r#""k{i}":{i}"#)).collect();
    let structured = format!("{{{}}}", fields.join(","));
    fields.reverse();
    let reordered = format!("{{{}}}", fields.join(", "));
    assert!(equivalent(&structured, &reordered));
}
