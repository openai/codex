extern crate criterion;
extern crate weezl;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::fs;
use weezl::{decode::Decoder, BitOrder, LzwStatus};

pub fn criterion_benchmark(c: &mut Criterion, file: &str) {
    let data = fs::read(file).expect("Benchmark input not found");
    let mut group = c.benchmark_group("msb-8");
    let id = BenchmarkId::new(file, data.len());
    let mut outbuf = vec![0; 1 << 26]; // 64MB, what wuff uses..
    let mut decode_once = |data: &[u8]| {
        let mut decoder = Decoder::new(BitOrder::Msb, 8);
        let mut written = 0;
        let outbuf = outbuf.as_mut_slice();
        let mut data = data;
        loop {
            let result = decoder.decode_bytes(data, &mut outbuf[..]);
            let done = result.status.expect("Error");
            data = &data[result.consumed_in..];
            written += result.consumed_out;
            black_box(&outbuf[..result.consumed_out]);
            if let LzwStatus::Done = done {
                break;
            }
            if let LzwStatus::NoProgress = done {
                panic!("Need to make progress");
            }
        }
        written
    };
    group.throughput(Throughput::Bytes(decode_once(&data) as u64));
    group.bench_with_input(id, &data, |b, data| {
        b.iter(|| {
            decode_once(data);
        })
    });
}

pub fn bench_toml(c: &mut Criterion) {
    criterion_benchmark(c, "benches/Cargo-8-msb.lzw");
}

pub fn bench_binary(c: &mut Criterion) {
    criterion_benchmark(c, "benches/binary-8-msb.lzw");
}

pub fn bench_lib(c: &mut Criterion) {
    criterion_benchmark(c, "benches/lib-8-msb.lzw");
}

criterion_group!(benches, bench_toml, bench_binary, bench_lib);
criterion_main!(benches);
