#![allow(
    clippy::match_like_matches_macro,
    clippy::needless_pass_by_value,
    clippy::uninlined_format_args
)]

use serde_json::{json, Value};

fn to_json(toml: Value) -> Value {
    fn doit(s: &str, json: Value) -> Value {
        json!({ "type": s, "value": json })
    }

    match toml {
        Value::Null => unreachable!(),
        Value::String(s) => doit("string", Value::String(s)),
        Value::Number(n) => {
            let repr = n.to_string();
            if repr.contains('.') {
                let float: f64 = repr.parse().unwrap();
                let mut repr = format!("{:.15}", float);
                repr.truncate(repr.trim_end_matches('0').len());
                if repr.ends_with('.') {
                    repr.push('0');
                }
                doit("float", Value::String(repr))
            } else {
                doit("integer", Value::String(repr))
            }
        }
        Value::Bool(b) => doit("bool", Value::String(format!("{}", b))),
        Value::Array(arr) => {
            let is_table = match arr.first() {
                Some(&Value::Object(_)) => true,
                _ => false,
            };
            let json = Value::Array(arr.into_iter().map(to_json).collect());
            if is_table {
                json
            } else {
                doit("array", json)
            }
        }
        Value::Object(table) => {
            let mut map = serde_json::Map::new();
            for (k, v) in table {
                map.insert(k, to_json(v));
            }
            Value::Object(map)
        }
    }
}

fn run(toml_raw: &str, json_raw: &str) {
    println!("parsing:\n{}", toml_raw);
    let toml: Value = basic_toml::from_str(toml_raw).unwrap();
    let json: Value = serde_json::from_str(json_raw).unwrap();

    // Assert toml == json
    let toml_json = to_json(toml.clone());
    assert!(
        json == toml_json,
        "expected\n{}\ngot\n{}\n",
        serde_json::to_string_pretty(&json).unwrap(),
        serde_json::to_string_pretty(&toml_json).unwrap()
    );

    // Assert round trip
    println!("round trip parse: {}", toml);
    let toml2: Value = basic_toml::from_str(&basic_toml::to_string(&toml).unwrap()).unwrap();
    assert_eq!(toml, toml2);
}

macro_rules! test( ($name:ident, $toml:expr, $json:expr) => (
    #[test]
    fn $name() { run($toml, $json); }
) );

test!(
    array_empty,
    include_str!("valid/array-empty.toml"),
    include_str!("valid/array-empty.json")
);
test!(
    array_nospaces,
    include_str!("valid/array-nospaces.toml"),
    include_str!("valid/array-nospaces.json")
);
test!(
    arrays_hetergeneous,
    include_str!("valid/arrays-hetergeneous.toml"),
    include_str!("valid/arrays-hetergeneous.json")
);
#[cfg(any())]
test!(
    arrays,
    include_str!("valid/arrays.toml"),
    include_str!("valid/arrays.json")
);
test!(
    arrays_nested,
    include_str!("valid/arrays-nested.toml"),
    include_str!("valid/arrays-nested.json")
);
test!(
    array_mixed_types_ints_and_floats,
    include_str!("valid/array-mixed-types-ints-and-floats.toml"),
    include_str!("valid/array-mixed-types-ints-and-floats.json")
);
test!(
    array_mixed_types_arrays_and_ints,
    include_str!("valid/array-mixed-types-arrays-and-ints.toml"),
    include_str!("valid/array-mixed-types-arrays-and-ints.json")
);
test!(
    array_mixed_types_strings_and_ints,
    include_str!("valid/array-mixed-types-strings-and-ints.toml"),
    include_str!("valid/array-mixed-types-strings-and-ints.json")
);
test!(
    empty,
    include_str!("valid/empty.toml"),
    include_str!("valid/empty.json")
);
test!(
    bool,
    include_str!("valid/bool.toml"),
    include_str!("valid/bool.json")
);
test!(
    comments_everywhere,
    include_str!("valid/comments-everywhere.toml"),
    include_str!("valid/comments-everywhere.json")
);
#[cfg(any())]
test!(
    datetime,
    include_str!("valid/datetime.toml"),
    include_str!("valid/datetime.json")
);
#[cfg(any())]
test!(
    example,
    include_str!("valid/example.toml"),
    include_str!("valid/example.json")
);
test!(
    float,
    include_str!("valid/float.toml"),
    include_str!("valid/float.json")
);
#[cfg(any())]
test!(
    implicit_and_explicit_after,
    include_str!("valid/implicit-and-explicit-after.toml"),
    include_str!("valid/implicit-and-explicit-after.json")
);
#[cfg(any())]
test!(
    implicit_and_explicit_before,
    include_str!("valid/implicit-and-explicit-before.toml"),
    include_str!("valid/implicit-and-explicit-before.json")
);
test!(
    implicit_groups,
    include_str!("valid/implicit-groups.toml"),
    include_str!("valid/implicit-groups.json")
);
test!(
    integer,
    include_str!("valid/integer.toml"),
    include_str!("valid/integer.json")
);
test!(
    key_equals_nospace,
    include_str!("valid/key-equals-nospace.toml"),
    include_str!("valid/key-equals-nospace.json")
);
test!(
    key_space,
    include_str!("valid/key-space.toml"),
    include_str!("valid/key-space.json")
);
test!(
    key_special_chars,
    include_str!("valid/key-special-chars.toml"),
    include_str!("valid/key-special-chars.json")
);
test!(
    key_with_pound,
    include_str!("valid/key-with-pound.toml"),
    include_str!("valid/key-with-pound.json")
);
test!(
    key_empty,
    include_str!("valid/key-empty.toml"),
    include_str!("valid/key-empty.json")
);
test!(
    long_float,
    include_str!("valid/long-float.toml"),
    include_str!("valid/long-float.json")
);
test!(
    long_integer,
    include_str!("valid/long-integer.toml"),
    include_str!("valid/long-integer.json")
);
test!(
    multiline_string,
    include_str!("valid/multiline-string.toml"),
    include_str!("valid/multiline-string.json")
);
test!(
    raw_multiline_string,
    include_str!("valid/raw-multiline-string.toml"),
    include_str!("valid/raw-multiline-string.json")
);
test!(
    raw_string,
    include_str!("valid/raw-string.toml"),
    include_str!("valid/raw-string.json")
);
test!(
    string_empty,
    include_str!("valid/string-empty.toml"),
    include_str!("valid/string-empty.json")
);
test!(
    string_escapes,
    include_str!("valid/string-escapes.toml"),
    include_str!("valid/string-escapes.json")
);
test!(
    string_simple,
    include_str!("valid/string-simple.toml"),
    include_str!("valid/string-simple.json")
);
test!(
    string_with_pound,
    include_str!("valid/string-with-pound.toml"),
    include_str!("valid/string-with-pound.json")
);
test!(
    table_array_implicit,
    include_str!("valid/table-array-implicit.toml"),
    include_str!("valid/table-array-implicit.json")
);
test!(
    table_array_many,
    include_str!("valid/table-array-many.toml"),
    include_str!("valid/table-array-many.json")
);
test!(
    table_array_nest,
    include_str!("valid/table-array-nest.toml"),
    include_str!("valid/table-array-nest.json")
);
test!(
    table_array_one,
    include_str!("valid/table-array-one.toml"),
    include_str!("valid/table-array-one.json")
);
test!(
    table_empty,
    include_str!("valid/table-empty.toml"),
    include_str!("valid/table-empty.json")
);
test!(
    table_sub_empty,
    include_str!("valid/table-sub-empty.toml"),
    include_str!("valid/table-sub-empty.json")
);
test!(
    table_multi_empty,
    include_str!("valid/table-multi-empty.toml"),
    include_str!("valid/table-multi-empty.json")
);
test!(
    table_whitespace,
    include_str!("valid/table-whitespace.toml"),
    include_str!("valid/table-whitespace.json")
);
test!(
    table_with_pound,
    include_str!("valid/table-with-pound.toml"),
    include_str!("valid/table-with-pound.json")
);
test!(
    unicode_escape,
    include_str!("valid/unicode-escape.toml"),
    include_str!("valid/unicode-escape.json")
);
test!(
    unicode_literal,
    include_str!("valid/unicode-literal.toml"),
    include_str!("valid/unicode-literal.json")
);
#[cfg(any())]
test!(
    hard_example,
    include_str!("valid/hard_example.toml"),
    include_str!("valid/hard_example.json")
);
#[cfg(any())]
test!(
    example2,
    include_str!("valid/example2.toml"),
    include_str!("valid/example2.json")
);
#[cfg(any())]
test!(
    example3,
    include_str!("valid/example-v0.3.0.toml"),
    include_str!("valid/example-v0.3.0.json")
);
#[cfg(any())]
test!(
    example4,
    include_str!("valid/example-v0.4.0.toml"),
    include_str!("valid/example-v0.4.0.json")
);
#[cfg(any())]
test!(
    example_bom,
    include_str!("valid/example-bom.toml"),
    include_str!("valid/example.json")
);

#[cfg(any())]
test!(
    datetime_truncate,
    include_str!("valid/datetime-truncate.toml"),
    include_str!("valid/datetime-truncate.json")
);
test!(
    key_quote_newline,
    include_str!("valid/key-quote-newline.toml"),
    include_str!("valid/key-quote-newline.json")
);
test!(
    table_array_nest_no_keys,
    include_str!("valid/table-array-nest-no-keys.toml"),
    include_str!("valid/table-array-nest-no-keys.json")
);
test!(
    dotted_keys,
    include_str!("valid/dotted-keys.toml"),
    include_str!("valid/dotted-keys.json")
);

test!(
    quote_surrounded_value,
    include_str!("valid/quote-surrounded-value.toml"),
    include_str!("valid/quote-surrounded-value.json")
);

test!(
    float_exponent,
    include_str!("valid/float-exponent.toml"),
    include_str!("valid/float-exponent.json")
);

test!(
    string_delim_end,
    include_str!("valid/string-delim-end.toml"),
    include_str!("valid/string-delim-end.json")
);
