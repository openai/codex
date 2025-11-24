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
fn times() {
    fn multi_bad(s: &str, msg: &str) {
        bad!(s, msg);
        bad!(&s.replace('T', " "), msg);
        bad!(&s.replace('T', "t"), msg);
        bad!(&s.replace('Z', "z"), msg);
    }

    multi_bad(
        "foo = 1997-09-09T09:09:09Z",
        "invalid number at line 1 column 7",
    );
    multi_bad(
        "foo = 1997-09-09T09:09:09+09:09",
        "invalid number at line 1 column 7",
    );
    multi_bad(
        "foo = 1997-09-09T09:09:09-09:09",
        "invalid number at line 1 column 7",
    );
    multi_bad(
        "foo = 1997-09-09T09:09:09",
        "invalid number at line 1 column 7",
    );
    multi_bad("foo = 1997-09-09", "invalid number at line 1 column 7");
    bad!("foo = 1997-09-09 ", "invalid number at line 1 column 7");
    bad!(
        "foo = 1997-09-09 # comment",
        "invalid number at line 1 column 7"
    );
    multi_bad("foo = 09:09:09", "invalid number at line 1 column 8");
    multi_bad(
        "foo = 1997-09-09T09:09:09.09Z",
        "invalid number at line 1 column 7",
    );
    multi_bad(
        "foo = 1997-09-09T09:09:09.09+09:09",
        "invalid number at line 1 column 7",
    );
    multi_bad(
        "foo = 1997-09-09T09:09:09.09-09:09",
        "invalid number at line 1 column 7",
    );
    multi_bad(
        "foo = 1997-09-09T09:09:09.09",
        "invalid number at line 1 column 7",
    );
    multi_bad("foo = 09:09:09.09", "invalid number at line 1 column 8");
}

#[test]
fn bad_times() {
    bad!("foo = 199-09-09", "invalid number at line 1 column 7");
    bad!("foo = 199709-09", "invalid number at line 1 column 7");
    bad!("foo = 1997-9-09", "invalid number at line 1 column 7");
    bad!("foo = 1997-09-9", "invalid number at line 1 column 7");
    bad!(
        "foo = 1997-09-0909:09:09",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = T",
        "invalid TOML value, did you mean to use a quoted string? at line 1 column 7"
    );
    bad!(
        "foo = T.",
        "invalid TOML value, did you mean to use a quoted string? at line 1 column 7"
    );
    bad!(
        "foo = TZ",
        "invalid TOML value, did you mean to use a quoted string? at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09+",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09+09",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09+09:9",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09+0909",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09-",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09-09",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09-09:9",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T09:09:09.09-0909",
        "invalid number at line 1 column 7"
    );

    bad!(
        "foo = 1997-00-09T09:09:09.09Z",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-00T09:09:09.09Z",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T30:09:09.09Z",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T12:69:09.09Z",
        "invalid number at line 1 column 7"
    );
    bad!(
        "foo = 1997-09-09T12:09:69.09Z",
        "invalid number at line 1 column 7"
    );
}
