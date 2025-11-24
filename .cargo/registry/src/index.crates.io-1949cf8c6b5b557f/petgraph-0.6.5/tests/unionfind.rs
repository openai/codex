extern crate petgraph;
extern crate rand;

use petgraph::unionfind::UnionFind;
use rand::{thread_rng, ChaChaRng, Rng, SeedableRng};
use std::collections::HashSet;

#[test]
fn uf_test() {
    let n = 8;
    let mut u = UnionFind::new(n);
    for i in 0..n {
        assert_eq!(u.find(i), i);
        assert_eq!(u.find_mut(i), i);
        assert!(!u.union(i, i));
    }

    u.union(0, 1);
    assert_eq!(u.find(0), u.find(1));
    u.union(1, 3);
    u.union(1, 4);
    u.union(4, 7);
    assert_eq!(u.find(0), u.find(3));
    assert_eq!(u.find(1), u.find(3));
    assert!(u.find(0) != u.find(2));
    assert_eq!(u.find(7), u.find(0));
    u.union(5, 6);
    assert_eq!(u.find(6), u.find(5));
    assert!(u.find(6) != u.find(7));

    // check that there are now 3 disjoint sets
    let set = (0..n).map(|i| u.find(i)).collect::<HashSet<_>>();
    assert_eq!(set.len(), 3);
}

#[test]
fn uf_test_with_equiv() {
    let n = 8;
    let mut u = UnionFind::new(n);
    for i in 0..n {
        assert_eq!(u.find(i), i);
        assert_eq!(u.find_mut(i), i);
        assert!(u.equiv(i, i));
    }

    u.union(0, 1);
    assert!(u.equiv(0, 1));
    u.union(1, 3);
    u.union(1, 4);
    u.union(4, 7);
    assert!(u.equiv(0, 7));
    assert!(u.equiv(1, 3));
    assert!(!u.equiv(0, 2));
    assert!(u.equiv(7, 0));
    u.union(5, 6);
    assert!(u.equiv(6, 5));
    assert!(!u.equiv(6, 7));

    // check that there are now 3 disjoint sets
    let set = (0..n).map(|i| u.find(i)).collect::<HashSet<_>>();
    assert_eq!(set.len(), 3);
}

#[test]
fn uf_rand() {
    let n = 1 << 14;
    let mut rng = ChaChaRng::from_rng(thread_rng()).unwrap();
    let mut u = UnionFind::new(n);
    for _ in 0..100 {
        let a = rng.gen_range(0, n);
        let b = rng.gen_range(0, n);
        let ar = u.find(a);
        let br = u.find(b);
        assert_eq!(ar != br, u.union(a, b));
    }
}

#[test]
fn uf_u8() {
    let n = 256;
    let mut rng = ChaChaRng::from_rng(thread_rng()).unwrap();
    let mut u = UnionFind::<u8>::new(n);
    for _ in 0..(n * 8) {
        let a = rng.gen();
        let b = rng.gen();
        let ar = u.find(a);
        let br = u.find(b);
        assert_eq!(ar != br, u.union(a, b));
    }
}

#[test]
fn labeling() {
    let mut u = UnionFind::<u32>::new(48);
    for i in 0..24 {
        u.union(i + 1, i);
    }
    for i in 25..47 {
        u.union(i, i + 1);
    }
    u.union(23, 25);
    u.union(24, 23);
    let v = u.into_labeling();
    assert!(v.iter().all(|x| *x == v[0]));
}
