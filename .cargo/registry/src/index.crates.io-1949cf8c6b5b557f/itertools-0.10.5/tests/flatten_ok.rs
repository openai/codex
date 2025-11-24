use itertools::{assert_equal, Itertools};
use std::{ops::Range, vec::IntoIter};

fn mix_data() -> IntoIter<Result<Range<i32>, bool>> {
    vec![Ok(0..2), Err(false), Ok(2..4), Err(true), Ok(4..6)].into_iter()
}

fn ok_data() -> IntoIter<Result<Range<i32>, bool>> {
    vec![Ok(0..2), Ok(2..4), Ok(4..6)].into_iter()
}

#[test]
fn flatten_ok_mixed_expected_forward() {
    assert_equal(
        mix_data().flatten_ok(),
        vec![
            Ok(0),
            Ok(1),
            Err(false),
            Ok(2),
            Ok(3),
            Err(true),
            Ok(4),
            Ok(5),
        ],
    );
}

#[test]
fn flatten_ok_mixed_expected_reverse() {
    assert_equal(
        mix_data().flatten_ok().rev(),
        vec![
            Ok(5),
            Ok(4),
            Err(true),
            Ok(3),
            Ok(2),
            Err(false),
            Ok(1),
            Ok(0),
        ],
    );
}

#[test]
fn flatten_ok_collect_mixed_forward() {
    assert_eq!(
        mix_data().flatten_ok().collect::<Result<Vec<_>, _>>(),
        Err(false)
    );
}

#[test]
fn flatten_ok_collect_mixed_reverse() {
    assert_eq!(
        mix_data().flatten_ok().rev().collect::<Result<Vec<_>, _>>(),
        Err(true)
    );
}

#[test]
fn flatten_ok_collect_ok_forward() {
    assert_eq!(
        ok_data().flatten_ok().collect::<Result<Vec<_>, _>>(),
        Ok((0..6).collect())
    );
}

#[test]
fn flatten_ok_collect_ok_reverse() {
    assert_eq!(
        ok_data().flatten_ok().rev().collect::<Result<Vec<_>, _>>(),
        Ok((0..6).rev().collect())
    );
}
