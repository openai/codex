macro_rules! searcher {
    ($name:ident, $re:expr, $haystack:expr) => (
        searcher!($name, $re, $haystack, vec vec![]);
    );
    ($name:ident, $re:expr, $haystack:expr, $($steps:expr,)*) => (
        searcher!($name, $re, $haystack, vec vec![$($steps),*]);
    );
    ($name:ident, $re:expr, $haystack:expr, $($steps:expr),*) => (
        searcher!($name, $re, $haystack, vec vec![$($steps),*]);
    );
    ($name:ident, $re:expr, $haystack:expr, vec $expect_steps:expr) => (
        #[test]
        #[allow(unused_imports)]
        fn $name() {
            use std::str::pattern::{Pattern, Searcher};
            use std::str::pattern::SearchStep::{Match, Reject, Done};
            let re = regex::Regex::new($re).unwrap();
            let mut se = re.into_searcher($haystack);
            let mut got_steps = vec![];
            loop {
                match se.next() {
                    Done => break,
                    step => { got_steps.push(step); }
                }
            }
            assert_eq!(got_steps, $expect_steps);
        }
    );
}

searcher!(searcher_empty_regex_empty_haystack, r"", "", Match(0, 0));
searcher!(
    searcher_empty_regex,
    r"",
    "ab",
    Match(0, 0),
    Reject(0, 1),
    Match(1, 1),
    Reject(1, 2),
    Match(2, 2)
);
searcher!(searcher_empty_haystack, r"\d", "");
searcher!(searcher_one_match, r"\d", "5", Match(0, 1));
searcher!(searcher_no_match, r"\d", "a", Reject(0, 1));
searcher!(
    searcher_two_adjacent_matches,
    r"\d",
    "56",
    Match(0, 1),
    Match(1, 2)
);
searcher!(
    searcher_two_non_adjacent_matches,
    r"\d",
    "5a6",
    Match(0, 1),
    Reject(1, 2),
    Match(2, 3)
);
searcher!(searcher_reject_first, r"\d", "a6", Reject(0, 1), Match(1, 2));
searcher!(
    searcher_one_zero_length_matches,
    r"\d*",
    "a1b2",
    Match(0, 0),  // ^
    Reject(0, 1), // a
    Match(1, 2),  // a1
    Reject(2, 3), // a1b
    Match(3, 4),  // a1b2
);
searcher!(
    searcher_many_zero_length_matches,
    r"\d*",
    "a1bbb2",
    Match(0, 0),  // ^
    Reject(0, 1), // a
    Match(1, 2),  // a1
    Reject(2, 3), // a1b
    Match(3, 3),  // a1bb
    Reject(3, 4), // a1bb
    Match(4, 4),  // a1bbb
    Reject(4, 5), // a1bbb
    Match(5, 6),  // a1bbba
);
searcher!(
    searcher_unicode,
    r".+?",
    "Ⅰ1Ⅱ2",
    Match(0, 3),
    Match(3, 4),
    Match(4, 7),
    Match(7, 8)
);
