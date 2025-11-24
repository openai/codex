use std::convert::TryFrom;
use std::ops::RangeBounds;

fn test_char_coverage<R>(n: usize, range: R)
where
    R: Iterator<Item = char> + RangeBounds<char> + Clone,
{
    use std::collections::HashSet;

    let all: HashSet<char> = range.clone().collect();
    let mut covered = HashSet::new();
    for _ in 0..n {
        let c = fastrand::char(range.clone());
        assert!(all.contains(&c));
        covered.insert(c);
    }
    assert_eq!(covered, all);
}

#[test]
fn test_char() {
    // ASCII control chars.
    let nul = 0u8 as char;
    let soh = 1u8 as char;
    let stx = 2u8 as char;
    // Some undefined Hangul Jamo codepoints just before
    // the surrogate area.
    let last_jamo = char::try_from(0xd7ffu32).unwrap();
    let penultimate_jamo = char::try_from(last_jamo as u32 - 1).unwrap();
    // Private-use codepoints just after the surrogate area.
    let first_private = char::try_from(0xe000u32).unwrap();
    let second_private = char::try_from(first_private as u32 + 1).unwrap();
    // Private-use codepoints at the end of Unicode space.
    let last_private = std::char::MAX;
    let penultimate_private = char::try_from(last_private as u32 - 1).unwrap();

    test_char_coverage(100, nul..stx);
    test_char_coverage(100, nul..=soh);

    test_char_coverage(400, penultimate_jamo..second_private);
    test_char_coverage(400, penultimate_jamo..=second_private);

    test_char_coverage(100, penultimate_private..=last_private);
}
