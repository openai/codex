//! Licensed under the Apache License, Version 2.0
//! https://www.apache.org/licenses/LICENSE-2.0 or the MIT license
//! https://opensource.org/licenses/MIT, at your
//! option. This file may not be copied, modified, or distributed
//! except according to those terms.
#![no_std]
#![allow(deprecated)]

use crate::it::chain;
use crate::it::free::put_back;
use crate::it::interleave;
use crate::it::intersperse;
use crate::it::intersperse_with;
use crate::it::iproduct;
use crate::it::izip;
use crate::it::multizip;
use crate::it::Itertools;
use core::iter;
use itertools as it;

#[allow(dead_code)]
fn get_esi_then_esi<I: ExactSizeIterator + Clone>(it: I) {
    fn is_esi(_: impl ExactSizeIterator) {}
    is_esi(it.clone().get(1..4));
    is_esi(it.clone().get(1..=4));
    is_esi(it.clone().get(1..));
    is_esi(it.clone().get(..4));
    is_esi(it.clone().get(..=4));
    is_esi(it.get(..));
}

#[allow(dead_code)]
fn get_dei_esi_then_dei_esi<I: DoubleEndedIterator + ExactSizeIterator + Clone>(it: I) {
    fn is_dei_esi(_: impl DoubleEndedIterator + ExactSizeIterator) {}
    is_dei_esi(it.clone().get(1..4));
    is_dei_esi(it.clone().get(1..=4));
    is_dei_esi(it.clone().get(1..));
    is_dei_esi(it.clone().get(..4));
    is_dei_esi(it.clone().get(..=4));
    is_dei_esi(it.get(..));
}

#[test]
fn get_1_max() {
    let mut it = (0..5).get(1..=usize::MAX);
    assert_eq!(it.next(), Some(1));
    assert_eq!(it.next_back(), Some(4));
}

#[test]
#[should_panic]
fn get_full_range_inclusive() {
    let _it = (0..5).get(0..=usize::MAX);
}

#[test]
fn product0() {
    let mut prod = iproduct!();
    assert_eq!(prod.next(), Some(()));
    assert!(prod.next().is_none());
}

#[test]
fn iproduct1() {
    let s = "αβ";

    let mut prod = iproduct!(s.chars());
    assert_eq!(prod.next(), Some(('α',)));
    assert_eq!(prod.next(), Some(('β',)));
    assert!(prod.next().is_none());
}

#[test]
fn product2() {
    let s = "αβ";

    let mut prod = iproduct!(s.chars(), 0..2);
    assert!(prod.next() == Some(('α', 0)));
    assert!(prod.next() == Some(('α', 1)));
    assert!(prod.next() == Some(('β', 0)));
    assert!(prod.next() == Some(('β', 1)));
    assert!(prod.next().is_none());
}

#[test]
fn product_temporary() {
    for (_x, _y, _z) in iproduct!(
        [0, 1, 2].iter().cloned(),
        [0, 1, 2].iter().cloned(),
        [0, 1, 2].iter().cloned()
    ) {
        // ok
    }
}

#[test]
fn izip_macro() {
    let mut zip = izip!(2..3);
    assert!(zip.next() == Some(2));
    assert!(zip.next().is_none());

    let mut zip = izip!(0..3, 0..2, 0..2i8);
    for i in 0..2 {
        assert!((i as usize, i, i as i8) == zip.next().unwrap());
    }
    assert!(zip.next().is_none());

    let xs: [isize; 0] = [];
    let mut zip = izip!(0..3, 0..2, 0..2i8, &xs);
    assert!(zip.next().is_none());
}

#[test]
fn izip2() {
    let _zip1: iter::Zip<_, _> = izip!(1.., 2..);
    let _zip2: iter::Zip<_, _> = izip!(1.., 2..,);
}

#[test]
fn izip3() {
    let mut zip: iter::Map<iter::Zip<_, _>, _> = izip!(0..3, 0..2, 0..2i8);
    for i in 0..2 {
        assert!((i as usize, i, i as i8) == zip.next().unwrap());
    }
    assert!(zip.next().is_none());
}

#[test]
fn multizip3() {
    let mut zip = multizip((0..3, 0..2, 0..2i8));
    for i in 0..2 {
        assert!((i as usize, i, i as i8) == zip.next().unwrap());
    }
    assert!(zip.next().is_none());

    let xs: [isize; 0] = [];
    let mut zip = multizip((0..3, 0..2, 0..2i8, xs.iter()));
    assert!(zip.next().is_none());

    for (_, _, _, _, _) in multizip((0..3, 0..2, xs.iter(), &xs, xs.to_vec())) {
        /* test compiles */
    }
}

#[test]
fn chain_macro() {
    let mut chain = chain!(2..3);
    assert!(chain.next() == Some(2));
    assert!(chain.next().is_none());

    let mut chain = chain!(0..2, 2..3, 3..5i8);
    for i in 0..5i8 {
        assert_eq!(Some(i), chain.next());
    }
    assert!(chain.next().is_none());

    let mut chain = chain!();
    assert_eq!(chain.next(), Option::<()>::None);
}

#[test]
fn chain2() {
    let _ = chain!(1.., 2..);
    let _ = chain!(1.., 2..,);
}

#[test]
fn write_to() {
    let xs = [7, 9, 8];
    let mut ys = [0; 5];
    let cnt = ys.iter_mut().set_from(xs.iter().copied());
    assert!(cnt == xs.len());
    assert!(ys == [7, 9, 8, 0, 0]);

    let cnt = ys.iter_mut().set_from(0..10);
    assert!(cnt == ys.len());
    assert!(ys == [0, 1, 2, 3, 4]);
}

#[test]
fn test_interleave() {
    let xs: [u8; 0] = [];
    let ys = [7u8, 9, 8, 10];
    let zs = [2u8, 77];
    let it = interleave(xs.iter(), ys.iter());
    it::assert_equal(it, ys.iter());

    let rs = [7u8, 2, 9, 77, 8, 10];
    let it = interleave(ys.iter(), zs.iter());
    it::assert_equal(it, rs.iter());
}

#[test]
fn test_intersperse() {
    let xs = [1u8, 2, 3];
    let ys = [1u8, 0, 2, 0, 3];
    let it = intersperse(&xs, &0);
    it::assert_equal(it, ys.iter());
}

#[test]
fn test_intersperse_with() {
    let xs = [1u8, 2, 3];
    let ys = [1u8, 10, 2, 10, 3];
    let i = 10;
    let it = intersperse_with(&xs, || &i);
    it::assert_equal(it, ys.iter());
}

#[test]
fn dropping() {
    let xs = [1, 2, 3];
    let mut it = xs.iter().dropping(2);
    assert_eq!(it.next(), Some(&3));
    assert!(it.next().is_none());
    let mut it = xs.iter().dropping(5);
    assert!(it.next().is_none());
}

#[test]
fn batching() {
    let xs = [0, 1, 2, 1, 3];
    let ys = [(0, 1), (2, 1)];

    // An iterator that gathers elements up in pairs
    let pit = xs
        .iter()
        .cloned()
        .batching(|it| it.next().and_then(|x| it.next().map(|y| (x, y))));
    it::assert_equal(pit, ys.iter().cloned());
}

#[test]
fn test_put_back() {
    let xs = [0, 1, 1, 1, 2, 1, 3, 3];
    let mut pb = put_back(xs.iter().cloned());
    pb.next();
    pb.put_back(1);
    pb.put_back(0);
    it::assert_equal(pb, xs.iter().cloned());
}

#[test]
fn merge() {
    it::assert_equal((0..10).step_by(2).merge((1..10).step_by(2)), 0..10);
}

#[test]
fn repeatn() {
    let s = "α";
    let mut it = it::repeat_n(s, 3);
    assert_eq!(it.len(), 3);
    assert_eq!(it.next(), Some(s));
    assert_eq!(it.next(), Some(s));
    assert_eq!(it.next(), Some(s));
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None);
}

#[test]
fn count_clones() {
    // Check that RepeatN only clones N - 1 times.

    use core::cell::Cell;
    #[derive(PartialEq, Debug)]
    struct Foo {
        n: Cell<usize>,
    }

    impl Clone for Foo {
        fn clone(&self) -> Self {
            let n = self.n.get();
            self.n.set(n + 1);
            Self {
                n: Cell::new(n + 1),
            }
        }
    }

    for n in 0..10 {
        let f = Foo { n: Cell::new(0) };
        let it = it::repeat_n(f, n);
        // drain it
        let last = it.last();
        if n == 0 {
            assert_eq!(last, None);
        } else {
            assert_eq!(
                last,
                Some(Foo {
                    n: Cell::new(n - 1)
                })
            );
        }
    }
}

#[test]
fn part() {
    let mut data = [7, 1, 1, 9, 1, 1, 3];
    let i = it::partition(&mut data, |elt| *elt >= 3);
    assert_eq!(i, 3);
    assert_eq!(data, [7, 3, 9, 1, 1, 1, 1]);

    let i = it::partition(&mut data, |elt| *elt == 1);
    assert_eq!(i, 4);
    assert_eq!(data, [1, 1, 1, 1, 9, 3, 7]);

    let mut data = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    let i = it::partition(&mut data, |elt| *elt % 3 == 0);
    assert_eq!(i, 3);
    assert_eq!(data, [9, 6, 3, 4, 5, 2, 7, 8, 1]);
}

#[test]
fn tree_reduce() {
    for i in 0..100 {
        assert_eq!((0..i).tree_reduce(|x, y| x + y), (0..i).fold1(|x, y| x + y));
    }
}

#[test]
fn exactly_one() {
    assert_eq!((0..10).filter(|&x| x == 2).exactly_one().unwrap(), 2);
    assert!((0..10)
        .filter(|&x| x > 1 && x < 4)
        .exactly_one()
        .unwrap_err()
        .eq(2..4));
    assert!((0..10)
        .filter(|&x| x > 1 && x < 5)
        .exactly_one()
        .unwrap_err()
        .eq(2..5));
    assert!((0..10)
        .filter(|&_| false)
        .exactly_one()
        .unwrap_err()
        .eq(0..0));
}

#[test]
fn at_most_one() {
    assert_eq!((0..10).filter(|&x| x == 2).at_most_one().unwrap(), Some(2));
    assert!((0..10)
        .filter(|&x| x > 1 && x < 4)
        .at_most_one()
        .unwrap_err()
        .eq(2..4));
    assert!((0..10)
        .filter(|&x| x > 1 && x < 5)
        .at_most_one()
        .unwrap_err()
        .eq(2..5));
    assert_eq!((0..10).filter(|&_| false).at_most_one().unwrap(), None);
}

#[test]
fn sum1() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    assert_eq!(v[..0].iter().cloned().sum1::<i32>(), None);
    assert_eq!(v[1..2].iter().cloned().sum1::<i32>(), Some(1));
    assert_eq!(v[1..3].iter().cloned().sum1::<i32>(), Some(3));
    assert_eq!(v.iter().cloned().sum1::<i32>(), Some(55));
}

#[test]
fn product1() {
    let v: &[i32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    assert_eq!(v[..0].iter().cloned().product1::<i32>(), None);
    assert_eq!(v[..1].iter().cloned().product1::<i32>(), Some(0));
    assert_eq!(v[1..3].iter().cloned().product1::<i32>(), Some(2));
    assert_eq!(v[1..5].iter().cloned().product1::<i32>(), Some(24));
}

#[test]
fn next_array() {
    let v = [1, 2, 3, 4, 5];
    let mut iter = v.iter();
    assert_eq!(iter.next_array(), Some([]));
    assert_eq!(iter.next_array().map(|[&x, &y]| [x, y]), Some([1, 2]));
    assert_eq!(iter.next_array().map(|[&x, &y]| [x, y]), Some([3, 4]));
    assert_eq!(iter.next_array::<2>(), None);
}

#[test]
fn collect_array() {
    let v = [1, 2];
    let iter = v.iter().cloned();
    assert_eq!(iter.collect_array(), Some([1, 2]));

    let v = [1];
    let iter = v.iter().cloned();
    assert_eq!(iter.collect_array::<2>(), None);

    let v = [1, 2, 3];
    let iter = v.iter().cloned();
    assert_eq!(iter.collect_array::<2>(), None);
}
