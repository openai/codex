use itertools::Itertools;
use itertools::{put_back, put_back_n};

#[test]
fn peeking_take_while_peekable() {
    let mut r = (0..10).peekable();
    r.peeking_take_while(|x| *x <= 3).count();
    assert_eq!(r.next(), Some(4));
}

#[test]
fn peeking_take_while_put_back() {
    let mut r = put_back(0..10);
    r.peeking_take_while(|x| *x <= 3).count();
    assert_eq!(r.next(), Some(4));
    r.peeking_take_while(|_| true).count();
    assert_eq!(r.next(), None);
}

#[test]
fn peeking_take_while_put_back_n() {
    let mut r = put_back_n(6..10);
    for elt in (0..6).rev() {
        r.put_back(elt);
    }
    r.peeking_take_while(|x| *x <= 3).count();
    assert_eq!(r.next(), Some(4));
    r.peeking_take_while(|_| true).count();
    assert_eq!(r.next(), None);
}

#[test]
fn peeking_take_while_slice_iter() {
    let v = [1, 2, 3, 4, 5, 6];
    let mut r = v.iter();
    r.peeking_take_while(|x| **x <= 3).count();
    assert_eq!(r.next(), Some(&4));
    r.peeking_take_while(|_| true).count();
    assert_eq!(r.next(), None);
}

#[test]
fn peeking_take_while_slice_iter_rev() {
    let v = [1, 2, 3, 4, 5, 6];
    let mut r = v.iter().rev();
    r.peeking_take_while(|x| **x >= 3).count();
    assert_eq!(r.next(), Some(&2));
    r.peeking_take_while(|_| true).count();
    assert_eq!(r.next(), None);
}

#[test]
fn peeking_take_while_nested() {
    let mut xs = (0..10).peekable();
    let ys: Vec<_> = xs
        .peeking_take_while(|x| *x < 6)
        .peeking_take_while(|x| *x != 3)
        .collect();
    assert_eq!(ys, vec![0, 1, 2]);
    assert_eq!(xs.next(), Some(3));

    let mut xs = (4..10).peekable();
    let ys: Vec<_> = xs
        .peeking_take_while(|x| *x != 3)
        .peeking_take_while(|x| *x < 6)
        .collect();
    assert_eq!(ys, vec![4, 5]);
    assert_eq!(xs.next(), Some(6));
}
