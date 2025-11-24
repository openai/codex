use itertools::multizip;
use itertools::EitherOrBoth::{Both, Left, Right};
use itertools::Itertools;

#[test]
fn zip_longest_fused() {
    let a = [Some(1), None, Some(3), Some(4)];
    let b = [1, 2, 3];

    let unfused = a
        .iter()
        .batching(|it| *it.next().unwrap())
        .zip_longest(b.iter().cloned());
    itertools::assert_equal(unfused, vec![Both(1, 1), Right(2), Right(3)]);
}

#[test]
fn test_zip_longest_size_hint() {
    let c = (1..10).cycle();
    let v: &[_] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let v2 = &[10, 11, 12];

    assert_eq!(c.zip_longest(v.iter()).size_hint(), (usize::MAX, None));

    assert_eq!(v.iter().zip_longest(v2.iter()).size_hint(), (10, Some(10)));
}

#[test]
fn test_double_ended_zip_longest() {
    let xs = [1, 2, 3, 4, 5, 6];
    let ys = [1, 2, 3, 7];
    let a = xs.iter().copied();
    let b = ys.iter().copied();
    let mut it = a.zip_longest(b);
    assert_eq!(it.next(), Some(Both(1, 1)));
    assert_eq!(it.next(), Some(Both(2, 2)));
    assert_eq!(it.next_back(), Some(Left(6)));
    assert_eq!(it.next_back(), Some(Left(5)));
    assert_eq!(it.next_back(), Some(Both(4, 7)));
    assert_eq!(it.next(), Some(Both(3, 3)));
    assert_eq!(it.next(), None);
}

#[test]
fn test_double_ended_zip() {
    let xs = [1, 2, 3, 4, 5, 6];
    let ys = [1, 2, 3, 7];
    let a = xs.iter().copied();
    let b = ys.iter().copied();
    let mut it = multizip((a, b));
    assert_eq!(it.next_back(), Some((4, 7)));
    assert_eq!(it.next_back(), Some((3, 3)));
    assert_eq!(it.next_back(), Some((2, 2)));
    assert_eq!(it.next_back(), Some((1, 1)));
    assert_eq!(it.next_back(), None);
}
