use serde_json::Value;

macro_rules! bad {
    ($toml:expr, $msg:expr) => {
        match basic_toml::from_str::<Value>($toml) {
            Ok(s) => panic!("parsed to: {:#?}", s),
            Err(e) => assert_eq!(e.to_string(), $msg),
        }
    };
}

#[test]
fn bad() {
    bad!("a = 01", "invalid number at line 1 column 6");
    bad!("a = 1__1", "invalid number at line 1 column 5");
    bad!("a = 1_", "invalid number at line 1 column 5");
    bad!("''", "expected an equals, found eof at line 1 column 3");
    bad!("a = 9e99999", "invalid number at line 1 column 5");

    bad!(
        "a = \"\u{7f}\"",
        "invalid character in string: `\\u{7f}` at line 1 column 6"
    );
    bad!(
        "a = '\u{7f}'",
        "invalid character in string: `\\u{7f}` at line 1 column 6"
    );

    bad!("a = -0x1", "invalid number at line 1 column 5");
    bad!("a = 0x-1", "invalid number at line 1 column 7");

    // Dotted keys.
    bad!(
        "a.b.c = 1
         a.b = 2
        ",
        "duplicate key: `b` for key `a` at line 2 column 12"
    );
    bad!(
        "a = 1
         a.b = 2",
        "dotted key attempted to extend non-table type at line 1 column 5"
    );
    bad!(
        "a = {k1 = 1, k1.name = \"joe\"}",
        "dotted key attempted to extend non-table type at line 1 column 11"
    );
}
