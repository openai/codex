extern crate difflib;

use difflib::differ::Differ;
use difflib::sequencematcher::{Match, Opcode, SequenceMatcher};

#[test]
fn test_longest_match() {
    let matcher = SequenceMatcher::new(" abcd", "abcd abcd");
    let m = matcher.find_longest_match(0, 5, 0, 9);
    assert_eq!(m.first_start, 0);
    assert_eq!(m.second_start, 4);
    assert_eq!(m.size, 5);
}

#[test]
fn test_all_matches() {
    let mut matcher = SequenceMatcher::new("abxcd", "abcd");
    let result = matcher.get_matching_blocks();
    let mut expected_result = Vec::new();
    expected_result.push(Match {
        first_start: 0,
        second_start: 0,
        size: 2,
    });
    expected_result.push(Match {
        first_start: 3,
        second_start: 2,
        size: 2,
    });
    expected_result.push(Match {
        first_start: 5,
        second_start: 4,
        size: 0,
    });
    assert_eq!(result, expected_result);
}

#[test]
fn test_get_opcodes() {
    let mut matcher = SequenceMatcher::new("qabxcd", "abycdf");
    let result = matcher.get_opcodes();
    let mut expected_result = Vec::new();
    expected_result.push(Opcode {
        tag: "delete".to_string(),
        first_start: 0,
        first_end: 1,
        second_start: 0,
        second_end: 0,
    });
    expected_result.push(Opcode {
        tag: "equal".to_string(),
        first_start: 1,
        first_end: 3,
        second_start: 0,
        second_end: 2,
    });
    expected_result.push(Opcode {
        tag: "replace".to_string(),
        first_start: 3,
        first_end: 4,
        second_start: 2,
        second_end: 3,
    });
    expected_result.push(Opcode {
        tag: "equal".to_string(),
        first_start: 4,
        first_end: 6,
        second_start: 3,
        second_end: 5,
    });
    expected_result.push(Opcode {
        tag: "insert".to_string(),
        first_start: 6,
        first_end: 6,
        second_start: 5,
        second_end: 6,
    });
    assert_eq!(result, expected_result);
}

#[test]
fn test_ratio() {
    let mut matcher = SequenceMatcher::new("abcd", "bcde");
    assert_eq!(matcher.ratio(), 0.75);
}

#[test]
fn test_get_close_matches() {
    let words = vec!["ape", "apple", "peach", "puppy"];
    let result = difflib::get_close_matches("appel", words, 3, 0.6);
    assert_eq!(result, vec!["apple", "ape"]);
}

#[test]
fn test_differ_compare() {
    let first_text = vec!["one\n", "two\n", "three\n"];
    let second_text = vec!["ore\n", "tree\n", "emu\n"];
    let differ = Differ::new();
    let result = differ.compare(&first_text, &second_text).join("");
    assert_eq!(
        result,
        "- one\n?  ^\n+ ore\n?  ^\n- two\n- three\n?  -\n+ tree\n+ emu\n"
    );
}

fn is_junk_char(ch: &char) -> bool {
    if *ch == ' ' || *ch == '\t' {
        return true;
    }
    false
}

#[test]
fn test_differ_compare_with_func() {
    let first_text = vec!["one\n", "two\n", "three\n"];
    let second_text = vec!["ore\n", "tree\n", "emu\n"];
    let mut differ = Differ::new();
    differ.char_junk = Some(is_junk_char);
    let result = differ.compare(&first_text, &second_text).join("");
    assert_eq!(
        result,
        "- one\n?  ^\n+ ore\n?  ^\n- two\n- three\n?  -\n+ tree\n+ emu\n"
    );
}

#[test]
fn test_differ_restore() {
    let first_text = vec!["one\n", "  two\n", "three\n"];
    let second_text = vec!["ore\n", "tree\n", "emu\n"];
    let differ = Differ::new();
    let diff = differ.compare(&first_text, &second_text);
    assert_eq!(first_text, Differ::restore(&diff, 1));
    assert_eq!(second_text, Differ::restore(&diff, 2));
}

#[test]
fn test_unified_diff() {
    let first_text = "one two three four".split(" ").collect::<Vec<&str>>();
    let second_text = "zero one tree four".split(" ").collect::<Vec<&str>>();
    let result = difflib::unified_diff(
        &first_text,
        &second_text,
        "Original",
        "Current",
        "2005-01-26 23:30:50",
        "2010-04-02 10:20:52",
        3,
    ).join("");
    assert_eq!(
        result,
        "--- Original\t2005-01-26 23:30:50\n+++ Current\t2010-04-02 10:20:52\n@@ -1,4 \
         +1,4 @@\n+zero one-two-three+tree four"
    );
}

#[test]
fn test_context_diff() {
    let first_text = "one two three four".split(" ").collect::<Vec<&str>>();
    let second_text = "zero one tree four".split(" ").collect::<Vec<&str>>();
    let result = difflib::context_diff(
        &first_text,
        &second_text,
        "Original",
        "Current",
        "2005-01-26 23:30:50",
        "2010-04-02 10:20:52",
        3,
    ).join("");
    assert_eq!(
        result,
        "*** Original\t2005-01-26 23:30:50\n--- Current\t2010-04-02 \
         10:20:52\n***************\n*** 1,4 ****\n  one! two! three  four--- 1,4 ----\n+ \
         zero  one! tree  four"
    );
}

#[test]
fn test_integer_slice() {
    let s1 = vec![1, 2, 3, 4, 5];
    let s2 = vec![5, 4, 3, 2, 1];
    let result = SequenceMatcher::new(&s1, &s2).get_matching_blocks();
    let mut expected_result = Vec::new();
    expected_result.push(Match {
        first_start: 0,
        second_start: 4,
        size: 1,
    });
    expected_result.push(Match {
        first_start: 5,
        second_start: 5,
        size: 0,
    });
    assert_eq!(result, expected_result);
}
