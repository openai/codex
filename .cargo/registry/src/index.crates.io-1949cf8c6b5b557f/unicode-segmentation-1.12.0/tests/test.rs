// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use quickcheck::quickcheck;
use unicode_segmentation::UnicodeSegmentation;

#[rustfmt::skip]
mod testdata;

#[test]
fn test_graphemes() {
    use crate::testdata::{TEST_DIFF, TEST_SAME};

    pub const EXTRA_DIFF: &[(&str, &[&str], &[&str])] = &[
        // Official test suite doesn't include two Prepend chars between two other chars.
        (
            "\u{20}\u{600}\u{600}\u{20}",
            &["\u{20}", "\u{600}\u{600}\u{20}"],
            &["\u{20}", "\u{600}", "\u{600}", "\u{20}"],
        ),
        // Test for Prepend followed by two Any chars
        (
            "\u{600}\u{20}\u{20}",
            &["\u{600}\u{20}", "\u{20}"],
            &["\u{600}", "\u{20}", "\u{20}"],
        ),
    ];

    pub const EXTRA_SAME: &[(&str, &[&str])] = &[
        // family emoji (more than two emoji joined by ZWJ)
        (
            "\u{1f468}\u{200d}\u{1f467}\u{200d}\u{1f466}",
            &["\u{1f468}\u{200d}\u{1f467}\u{200d}\u{1f466}"],
        ),
        // cartwheel emoji followed by two fitzpatrick skin tone modifiers
        // (test case from issue #19)
        (
            "\u{1F938}\u{1F3FE}\u{1F3FE}",
            &["\u{1F938}\u{1F3FE}\u{1F3FE}"],
        ),
    ];

    for &(s, g) in TEST_SAME.iter().chain(EXTRA_SAME) {
        // test forward iterator
        let our_extended: Vec<_> = UnicodeSegmentation::graphemes(s, true).collect();
        let our_legacy: Vec<_> = UnicodeSegmentation::graphemes(s, false).collect();
        assert_eq!(our_extended, g, "{s:?} extended");
        assert_eq!(our_legacy, g, "{s:?} legacy");

        // test reverse iterator
        assert!(UnicodeSegmentation::graphemes(s, true)
            .rev()
            .eq(g.iter().rev().cloned()));
        assert!(UnicodeSegmentation::graphemes(s, false)
            .rev()
            .eq(g.iter().rev().cloned()));
    }

    for &(s, gt, gf) in TEST_DIFF.iter().chain(EXTRA_DIFF) {
        // test forward iterator
        assert!(UnicodeSegmentation::graphemes(s, true).eq(gt.iter().cloned()));
        assert!(UnicodeSegmentation::graphemes(s, false).eq(gf.iter().cloned()));

        // test reverse iterator
        assert!(UnicodeSegmentation::graphemes(s, true)
            .rev()
            .eq(gt.iter().rev().cloned()));
        assert!(UnicodeSegmentation::graphemes(s, false)
            .rev()
            .eq(gf.iter().rev().cloned()));
    }

    // test the indices iterators
    let s = "a̐éö̲\r\n";
    let gr_inds = UnicodeSegmentation::grapheme_indices(s, true).collect::<Vec<(usize, &str)>>();
    let b: &[_] = &[(0, "a̐"), (3, "é"), (6, "ö̲"), (11, "\r\n")];
    assert_eq!(gr_inds, b);
    let gr_inds = UnicodeSegmentation::grapheme_indices(s, true)
        .rev()
        .collect::<Vec<(usize, &str)>>();
    let b: &[_] = &[(11, "\r\n"), (6, "ö̲"), (3, "é"), (0, "a̐")];
    assert_eq!(gr_inds, b);
    let mut gr_inds_iter = UnicodeSegmentation::grapheme_indices(s, true);
    {
        let gr_inds = gr_inds_iter.by_ref();
        let e1 = gr_inds.size_hint();
        assert_eq!(e1, (1, Some(13)));
        let c = gr_inds.count();
        assert_eq!(c, 4);
    }
    let e2 = gr_inds_iter.size_hint();
    assert_eq!(e2, (0, Some(0)));

    // make sure the reverse iterator does the right thing with "\n" at beginning of string
    let s = "\n\r\n\r";
    let gr = UnicodeSegmentation::graphemes(s, true)
        .rev()
        .collect::<Vec<&str>>();
    let b: &[_] = &["\r", "\r\n", "\n"];
    assert_eq!(gr, b);
}

#[test]
fn test_words() {
    use crate::testdata::TEST_WORD;

    // Unicode's official tests don't really test longer chains of flag emoji
    // TODO This could be improved with more tests like flag emoji with interspersed Extend chars and ZWJ
    const EXTRA_TESTS: &[(&str, &[&str])] = &[
        (
            "🇦🇫🇦🇽🇦🇱🇩🇿🇦🇸🇦🇩🇦🇴",
            &["🇦🇫", "🇦🇽", "🇦🇱", "🇩🇿", "🇦🇸", "🇦🇩", "🇦🇴"],
        ),
        ("🇦🇫🇦🇽🇦🇱🇩🇿🇦🇸🇦🇩🇦", &["🇦🇫", "🇦🇽", "🇦🇱", "🇩🇿", "🇦🇸", "🇦🇩", "🇦"]),
        (
            "🇦a🇫🇦🇽a🇦🇱🇩🇿🇦🇸🇦🇩🇦",
            &["🇦", "a", "🇫🇦", "🇽", "a", "🇦🇱", "🇩🇿", "🇦🇸", "🇦🇩", "🇦"],
        ),
        (
            "\u{1f468}\u{200d}\u{1f468}\u{200d}\u{1f466}",
            &["\u{1f468}\u{200d}\u{1f468}\u{200d}\u{1f466}"],
        ),
        ("😌👎🏼", &["😌", "👎🏼"]),
        // perhaps wrong, spaces should not be included?
        ("hello world", &["hello", " ", "world"]),
        ("🇨🇦🇨🇭🇿🇲🇿 hi", &["🇨🇦", "🇨🇭", "🇿🇲", "🇿", " ", "hi"]),
    ];
    for &(s, w) in TEST_WORD.iter().chain(EXTRA_TESTS.iter()) {
        macro_rules! assert_ {
            ($test:expr, $exp:expr, $name:expr) => {
                // collect into vector for better diagnostics in failure case
                let testing = $test.collect::<Vec<_>>();
                let expected = $exp.collect::<Vec<_>>();
                assert_eq!(
                    testing, expected,
                    "{} test for testcase ({:?}, {:?}) failed.",
                    $name, s, w
                )
            };
        }
        // test forward iterator
        assert_!(
            s.split_word_bounds(),
            w.iter().cloned(),
            "Forward word boundaries"
        );

        // test reverse iterator
        assert_!(
            s.split_word_bounds().rev(),
            w.iter().rev().cloned(),
            "Reverse word boundaries"
        );

        // generate offsets from word string lengths
        let mut indices = vec![0];
        for i in w.iter().cloned().map(|s| s.len()).scan(0, |t, n| {
            *t += n;
            Some(*t)
        }) {
            indices.push(i);
        }
        indices.pop();
        let indices = indices;

        // test forward indices iterator
        assert_!(
            s.split_word_bound_indices().map(|(l, _)| l),
            indices.iter().cloned(),
            "Forward word indices"
        );

        // test backward indices iterator
        assert_!(
            s.split_word_bound_indices().rev().map(|(l, _)| l),
            indices.iter().rev().cloned(),
            "Reverse word indices"
        );
    }
}

#[test]
fn test_sentences() {
    use crate::testdata::TEST_SENTENCE;

    for &(s, w) in TEST_SENTENCE.iter() {
        macro_rules! assert_ {
            ($test:expr, $exp:expr, $name:expr) => {
                // collect into vector for better diagnostics in failure case
                let testing = $test.collect::<Vec<_>>();
                let expected = $exp.collect::<Vec<_>>();
                assert_eq!(
                    testing, expected,
                    "{} test for testcase ({:?}, {:?}) failed.",
                    $name, s, w
                )
            };
        }

        assert_!(
            s.split_sentence_bounds(),
            w.iter().cloned(),
            "Forward sentence boundaries"
        );
    }
}

quickcheck! {
    fn quickcheck_forward_reverse_graphemes_extended(s: String) -> bool {
        let a = s.graphemes(true).collect::<Vec<_>>();
        let mut b = s.graphemes(true).rev().collect::<Vec<_>>();
        b.reverse();
        a == b
    }

    fn quickcheck_forward_reverse_graphemes_legacy(s: String) -> bool {
        let a = s.graphemes(false).collect::<Vec<_>>();
        let mut b = s.graphemes(false).rev().collect::<Vec<_>>();
        b.reverse();
        a == b
    }

    fn quickcheck_join_graphemes(s: String) -> bool {
        let a = s.graphemes(true).collect::<String>();
        let b = s.graphemes(false).collect::<String>();
        a == s && b == s
    }

    fn quickcheck_forward_reverse_words(s: String) -> bool {
        let a = s.split_word_bounds().collect::<Vec<_>>();
        let mut b = s.split_word_bounds().rev().collect::<Vec<_>>();
        b.reverse();
        a == b
    }

    fn quickcheck_join_words(s: String) -> bool {
        let a = s.split_word_bounds().collect::<String>();
        a == s
    }
}
