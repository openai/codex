// To run: `cargo criterion`
//
// This benchmarks each of the different libraries at several ratios of ASCII to
// non-ASCII content. There is one additional benchmark labeled "baseline" which
// just iterates over characters in a string, converting UTF-8 to 32-bit chars.
//
// Criterion will show a time in milliseconds. The non-baseline bench functions
// each make one million function calls (2 calls per character, 500K characters
// in the strings created by gen_string). The "time per call" listed in our
// readme is computed by subtracting this baseline from the other bench
// functions' time, then dividing by one million (ms -> ns).

#![allow(
    clippy::incompatible_msrv, // https://github.com/rust-lang/rust-clippy/issues/12257
    clippy::needless_pass_by_value,
)]

#[path = "../tests/fst/mod.rs"]
mod fst;
#[path = "../tests/roaring/mod.rs"]
mod roaring;
#[path = "../tests/trie/mod.rs"]
mod trie;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::distr::{Bernoulli, Distribution, Uniform};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use std::time::Duration;

fn gen_string(p_nonascii: u32) -> String {
    let mut rng = SmallRng::from_seed([b'!'; 32]);
    let pick_nonascii = Bernoulli::from_ratio(p_nonascii, 100).unwrap();
    let ascii = Uniform::new_inclusive('\0', '\x7f').unwrap();
    let nonascii = Uniform::new_inclusive(0x80 as char, char::MAX).unwrap();

    let mut string = String::new();
    for _ in 0..500_000 {
        let distribution = if pick_nonascii.sample(&mut rng) {
            nonascii
        } else {
            ascii
        };
        string.push(distribution.sample(&mut rng));
    }

    string
}

fn bench(c: &mut Criterion, group_name: &str, string: String) {
    let mut group = c.benchmark_group(group_name);
    group.measurement_time(Duration::from_secs(10));
    group.bench_function("baseline", |b| {
        b.iter(|| {
            for ch in string.chars() {
                black_box(ch);
            }
        });
    });
    group.bench_function("unicode-ident", |b| {
        b.iter(|| {
            for ch in string.chars() {
                black_box(unicode_ident::is_xid_start(ch));
                black_box(unicode_ident::is_xid_continue(ch));
            }
        });
    });
    group.bench_function("unicode-xid", |b| {
        b.iter(|| {
            for ch in string.chars() {
                black_box(unicode_xid::UnicodeXID::is_xid_start(ch));
                black_box(unicode_xid::UnicodeXID::is_xid_continue(ch));
            }
        });
    });
    group.bench_function("ucd-trie", |b| {
        b.iter(|| {
            for ch in string.chars() {
                black_box(trie::XID_START.contains_char(ch));
                black_box(trie::XID_CONTINUE.contains_char(ch));
            }
        });
    });
    group.bench_function("fst", |b| {
        let xid_start_fst = fst::xid_start_fst();
        let xid_continue_fst = fst::xid_continue_fst();
        b.iter(|| {
            for ch in string.chars() {
                let ch_bytes = (ch as u32).to_be_bytes();
                black_box(xid_start_fst.contains(ch_bytes));
                black_box(xid_continue_fst.contains(ch_bytes));
            }
        });
    });
    group.bench_function("roaring", |b| {
        let xid_start_bitmap = roaring::xid_start_bitmap();
        let xid_continue_bitmap = roaring::xid_continue_bitmap();
        b.iter(|| {
            for ch in string.chars() {
                black_box(xid_start_bitmap.contains(ch as u32));
                black_box(xid_continue_bitmap.contains(ch as u32));
            }
        });
    });
    group.finish();
}

fn bench0(c: &mut Criterion) {
    bench(c, "0%-nonascii", gen_string(0));
}

fn bench1(c: &mut Criterion) {
    bench(c, "1%-nonascii", gen_string(1));
}

fn bench10(c: &mut Criterion) {
    bench(c, "10%-nonascii", gen_string(10));
}

fn bench100(c: &mut Criterion) {
    bench(c, "100%-nonascii", gen_string(100));
}

criterion_group!(benches, bench0, bench1, bench10, bench100);
criterion_main!(benches);
