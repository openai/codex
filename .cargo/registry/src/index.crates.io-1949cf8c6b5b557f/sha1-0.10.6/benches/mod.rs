#![feature(test)]
extern crate test;

use digest::bench_update;
use sha1::Sha1;
use test::Bencher;

bench_update!(
    Sha1::default();
    sha1_10 10;
    sha1_100 100;
    sha1_1000 1000;
    sha1_10000 10000;
);
