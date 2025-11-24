use serde_json::Value;

macro_rules! bad {
    ($toml:expr, $msg:expr) => {
        match basic_toml::from_str::<Value>($toml) {
            Ok(s) => panic!("parsed to: {:#?}", s),
            Err(e) => assert_eq!(e.to_string(), $msg),
        }
    };
}

macro_rules! test( ($name:ident, $s:expr, $msg:expr) => (
    #[test]
    fn $name() { bad!($s, $msg); }
) );

test!(
    datetime_malformed_no_leads,
    include_str!("invalid/datetime-malformed-no-leads.toml"),
    "invalid number at line 1 column 12"
);
test!(
    datetime_malformed_no_secs,
    include_str!("invalid/datetime-malformed-no-secs.toml"),
    "invalid number at line 1 column 11"
);
test!(
    datetime_malformed_no_t,
    include_str!("invalid/datetime-malformed-no-t.toml"),
    "invalid number at line 1 column 8"
);
test!(
    datetime_malformed_with_milli,
    include_str!("invalid/datetime-malformed-with-milli.toml"),
    "invalid number at line 1 column 14"
);
test!(
    duplicate_key_table,
    include_str!("invalid/duplicate-key-table.toml"),
    "duplicate key: `type` for key `fruit` at line 4 column 8"
);
test!(
    duplicate_keys,
    include_str!("invalid/duplicate-keys.toml"),
    "duplicate key: `dupe` at line 2 column 1"
);
test!(
    duplicate_table,
    include_str!("invalid/duplicate-table.toml"),
    "redefinition of table `dependencies` for key `dependencies` at line 7 column 1"
);
test!(
    duplicate_tables,
    include_str!("invalid/duplicate-tables.toml"),
    "redefinition of table `a` for key `a` at line 2 column 1"
);
test!(
    empty_implicit_table,
    include_str!("invalid/empty-implicit-table.toml"),
    "expected a table key, found a period at line 1 column 10"
);
test!(
    empty_table,
    include_str!("invalid/empty-table.toml"),
    "expected a table key, found a right bracket at line 1 column 2"
);
test!(
    float_no_leading_zero,
    include_str!("invalid/float-no-leading-zero.toml"),
    "expected a value, found a period at line 1 column 10"
);
test!(
    float_no_suffix,
    include_str!("invalid/float-no-suffix.toml"),
    "invalid number at line 1 column 5"
);
test!(
    float_no_trailing_digits,
    include_str!("invalid/float-no-trailing-digits.toml"),
    "invalid number at line 1 column 12"
);
test!(
    key_after_array,
    include_str!("invalid/key-after-array.toml"),
    "expected newline, found an identifier at line 1 column 14"
);
test!(
    key_after_table,
    include_str!("invalid/key-after-table.toml"),
    "expected newline, found an identifier at line 1 column 11"
);
test!(
    key_empty,
    include_str!("invalid/key-empty.toml"),
    "expected a table key, found an equals at line 1 column 2"
);
test!(
    key_hash,
    include_str!("invalid/key-hash.toml"),
    "expected an equals, found a comment at line 1 column 2"
);
test!(
    key_newline,
    include_str!("invalid/key-newline.toml"),
    "expected an equals, found a newline at line 1 column 2"
);
test!(
    key_open_bracket,
    include_str!("invalid/key-open-bracket.toml"),
    "expected a right bracket, found an equals at line 1 column 6"
);
test!(
    key_single_open_bracket,
    include_str!("invalid/key-single-open-bracket.toml"),
    "expected a table key, found eof at line 1 column 2"
);
test!(
    key_space,
    include_str!("invalid/key-space.toml"),
    "expected an equals, found an identifier at line 1 column 3"
);
test!(
    key_start_bracket,
    include_str!("invalid/key-start-bracket.toml"),
    "expected a right bracket, found an equals at line 2 column 6"
);
test!(
    key_two_equals,
    include_str!("invalid/key-two-equals.toml"),
    "expected a value, found an equals at line 1 column 6"
);
test!(
    string_bad_byte_escape,
    include_str!("invalid/string-bad-byte-escape.toml"),
    "invalid escape character in string: `x` at line 1 column 13"
);
test!(
    string_bad_escape,
    include_str!("invalid/string-bad-escape.toml"),
    "invalid escape character in string: `a` at line 1 column 42"
);
test!(
    string_bad_line_ending_escape,
    include_str!("invalid/string-bad-line-ending-escape.toml"),
    "invalid escape character in string: ` ` at line 2 column 79"
);
test!(
    string_byte_escapes,
    include_str!("invalid/string-byte-escapes.toml"),
    "invalid escape character in string: `x` at line 1 column 12"
);
test!(
    string_no_close,
    include_str!("invalid/string-no-close.toml"),
    "newline in string found at line 1 column 42"
);
test!(
    table_array_implicit,
    include_str!("invalid/table-array-implicit.toml"),
    "table redefined as array for key `albums` at line 13 column 1"
);
test!(
    table_array_malformed_bracket,
    include_str!("invalid/table-array-malformed-bracket.toml"),
    "expected a right bracket, found a newline at line 1 column 10"
);
test!(
    table_array_malformed_empty,
    include_str!("invalid/table-array-malformed-empty.toml"),
    "expected a table key, found a right bracket at line 1 column 3"
);
test!(
    table_empty,
    include_str!("invalid/table-empty.toml"),
    "expected a table key, found a right bracket at line 1 column 2"
);
test!(
    table_nested_brackets_close,
    include_str!("invalid/table-nested-brackets-close.toml"),
    "expected newline, found an identifier at line 1 column 4"
);
test!(
    table_nested_brackets_open,
    include_str!("invalid/table-nested-brackets-open.toml"),
    "expected a right bracket, found a left bracket at line 1 column 3"
);
test!(
    table_whitespace,
    include_str!("invalid/table-whitespace.toml"),
    "expected a right bracket, found an identifier at line 1 column 10"
);
test!(
    table_with_pound,
    include_str!("invalid/table-with-pound.toml"),
    "expected a right bracket, found a comment at line 1 column 5"
);
test!(
    text_after_array_entries,
    include_str!("invalid/text-after-array-entries.toml"),
    "invalid TOML value, did you mean to use a quoted string? at line 2 column 46"
);
test!(
    text_after_integer,
    include_str!("invalid/text-after-integer.toml"),
    "expected newline, found an identifier at line 1 column 13"
);
test!(
    text_after_string,
    include_str!("invalid/text-after-string.toml"),
    "expected newline, found an identifier at line 1 column 41"
);
test!(
    text_after_table,
    include_str!("invalid/text-after-table.toml"),
    "expected newline, found an identifier at line 1 column 9"
);
test!(
    text_before_array_separator,
    include_str!("invalid/text-before-array-separator.toml"),
    "expected a right bracket, found an identifier at line 2 column 46"
);
test!(
    text_in_array,
    include_str!("invalid/text-in-array.toml"),
    "invalid TOML value, did you mean to use a quoted string? at line 3 column 3"
);
