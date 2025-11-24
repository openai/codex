extern crate criterion;
extern crate unicode_xid;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use unicode_xid::UnicodeXID;

fn bench_unicode_xid(c: &mut Criterion) {
    let unicode_chars = chars(1..0x3000);
    let ascii_chars = chars(1..0x80);

    let mut group = c.benchmark_group("UnicodeXID");
    group.throughput(Throughput::Bytes(unicode_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_xid_start", "unicode"),
        &unicode_chars,
        |b, chars| b.iter(|| chars.iter().copied().map(UnicodeXID::is_xid_start).last()),
    );
    group.throughput(Throughput::Bytes(ascii_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_xid_start", "ascii"),
        &ascii_chars,
        |b, chars| b.iter(|| chars.iter().copied().map(UnicodeXID::is_xid_start).last()),
    );
    group.throughput(Throughput::Bytes(unicode_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_xid_continue", "unicode"),
        &unicode_chars,
        |b, chars| {
            b.iter(|| {
                chars
                    .iter()
                    .copied()
                    .map(UnicodeXID::is_xid_continue)
                    .last()
            })
        },
    );
    group.throughput(Throughput::Bytes(ascii_chars.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("is_xid_continue", "ascii"),
        &ascii_chars,
        |b, chars| {
            b.iter(|| {
                chars
                    .iter()
                    .copied()
                    .map(UnicodeXID::is_xid_continue)
                    .last()
            })
        },
    );
    group.finish();
}

fn chars(range: std::ops::Range<u32>) -> Vec<char> {
    range.filter_map(|i| std::char::from_u32(i)).collect()
}

criterion_group!(benches, bench_unicode_xid);
criterion_main!(benches);
