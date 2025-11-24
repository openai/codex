//! Compare `insert` and `insert_unique_unchecked` operations performance.

#![feature(test)]

extern crate test;

use hashbrown::HashMap;
use test::Bencher;

#[bench]
fn insert(b: &mut Bencher) {
    let keys: Vec<String> = (0..1000).map(|i| format!("xxxx{}yyyy", i)).collect();
    b.iter(|| {
        let mut m = HashMap::with_capacity(1000);
        for k in &keys {
            m.insert(k, k);
        }
        m
    });
}

#[bench]
fn insert_unique_unchecked(b: &mut Bencher) {
    let keys: Vec<String> = (0..1000).map(|i| format!("xxxx{}yyyy", i)).collect();
    b.iter(|| {
        let mut m = HashMap::with_capacity(1000);
        for k in &keys {
            unsafe {
                m.insert_unique_unchecked(k, k);
            }
        }
        m
    });
}
