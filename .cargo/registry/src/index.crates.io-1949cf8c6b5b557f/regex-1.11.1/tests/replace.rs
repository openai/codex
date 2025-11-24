macro_rules! replace(
    ($name:ident, $which:ident, $re:expr,
     $search:expr, $replace:expr, $result:expr) => (
        #[test]
        fn $name() {
            let re = regex::Regex::new($re).unwrap();
            assert_eq!(re.$which($search, $replace), $result);
        }
    );
);

replace!(first, replace, r"[0-9]", "age: 26", "Z", "age: Z6");
replace!(plus, replace, r"[0-9]+", "age: 26", "Z", "age: Z");
replace!(all, replace_all, r"[0-9]", "age: 26", "Z", "age: ZZ");
replace!(groups, replace, r"([^ ]+)[ ]+([^ ]+)", "w1 w2", "$2 $1", "w2 w1");
replace!(
    double_dollar,
    replace,
    r"([^ ]+)[ ]+([^ ]+)",
    "w1 w2",
    "$2 $$1",
    "w2 $1"
);
// replace!(adjacent_index, replace,
// r"([^aeiouy])ies$", "skies", "$1y", "sky");
replace!(
    named,
    replace_all,
    r"(?P<first>[^ ]+)[ ]+(?P<last>[^ ]+)(?P<space>[ ]*)",
    "w1 w2 w3 w4",
    "$last $first$space",
    "w2 w1 w4 w3"
);
replace!(
    trim,
    replace_all,
    "^[ \t]+|[ \t]+$",
    " \t  trim me\t   \t",
    "",
    "trim me"
);
replace!(number_hyphen, replace, r"(.)(.)", "ab", "$1-$2", "a-b");
// replace!(number_underscore, replace, r"(.)(.)", "ab", "$1_$2", "a_b");
replace!(
    simple_expand,
    replace_all,
    r"([a-z]) ([a-z])",
    "a b",
    "$2 $1",
    "b a"
);
replace!(
    literal_dollar1,
    replace_all,
    r"([a-z]+) ([a-z]+)",
    "a b",
    "$$1",
    "$1"
);
replace!(
    literal_dollar2,
    replace_all,
    r"([a-z]+) ([a-z]+)",
    "a b",
    "$2 $$c $1",
    "b $c a"
);
replace!(
    no_expand1,
    replace,
    r"([^ ]+)[ ]+([^ ]+)",
    "w1 w2",
    regex::NoExpand("$2 $1"),
    "$2 $1"
);
replace!(
    no_expand2,
    replace,
    r"([^ ]+)[ ]+([^ ]+)",
    "w1 w2",
    regex::NoExpand("$$1"),
    "$$1"
);
replace!(
    closure_returning_reference,
    replace,
    r"([0-9]+)",
    "age: 26",
    |captures: &regex::Captures<'_>| { captures[1][0..1].to_owned() },
    "age: 2"
);
replace!(
    closure_returning_value,
    replace,
    r"[0-9]+",
    "age: 26",
    |_captures: &regex::Captures<'_>| "Z".to_owned(),
    "age: Z"
);

// See https://github.com/rust-lang/regex/issues/314
replace!(
    match_at_start_replace_with_empty,
    replace_all,
    r"foo",
    "foobar",
    "",
    "bar"
);

// See https://github.com/rust-lang/regex/issues/393
replace!(single_empty_match, replace, r"^", "bar", "foo", "foobar");

// See https://github.com/rust-lang/regex/issues/399
replace!(
    capture_longest_possible_name,
    replace_all,
    r"(.)",
    "b",
    "${1}a $1a",
    "ba "
);

replace!(
    impl_string,
    replace,
    r"[0-9]",
    "age: 26",
    "Z".to_string(),
    "age: Z6"
);
replace!(
    impl_string_ref,
    replace,
    r"[0-9]",
    "age: 26",
    &"Z".to_string(),
    "age: Z6"
);
replace!(
    impl_cow_str_borrowed,
    replace,
    r"[0-9]",
    "age: 26",
    std::borrow::Cow::<'_, str>::Borrowed("Z"),
    "age: Z6"
);
replace!(
    impl_cow_str_borrowed_ref,
    replace,
    r"[0-9]",
    "age: 26",
    &std::borrow::Cow::<'_, str>::Borrowed("Z"),
    "age: Z6"
);
replace!(
    impl_cow_str_owned,
    replace,
    r"[0-9]",
    "age: 26",
    std::borrow::Cow::<'_, str>::Owned("Z".to_string()),
    "age: Z6"
);
replace!(
    impl_cow_str_owned_ref,
    replace,
    r"[0-9]",
    "age: 26",
    &std::borrow::Cow::<'_, str>::Owned("Z".to_string()),
    "age: Z6"
);

#[test]
fn replacen_no_captures() {
    let re = regex::Regex::new(r"[0-9]").unwrap();
    assert_eq!(re.replacen("age: 1234", 2, "Z"), "age: ZZ34");
}

#[test]
fn replacen_with_captures() {
    let re = regex::Regex::new(r"([0-9])").unwrap();
    assert_eq!(re.replacen("age: 1234", 2, "${1}Z"), "age: 1Z2Z34");
}
