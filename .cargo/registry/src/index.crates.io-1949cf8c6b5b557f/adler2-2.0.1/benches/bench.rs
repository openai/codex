extern crate adler2;
extern crate criterion;

use adler2::{adler32_slice, Adler32};
use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn simple(c: &mut Criterion) {
    {
        const SIZE: usize = 100;

        let mut group = c.benchmark_group("simple-100b");
        group.throughput(Throughput::Bytes(SIZE as u64));
        group.bench_function("zeroes-100", |bencher| {
            bencher.iter(|| {
                adler32_slice(&[0; SIZE]);
            });
        });
        group.bench_function("ones-100", |bencher| {
            bencher.iter(|| {
                adler32_slice(&[0xff; SIZE]);
            });
        });
    }

    {
        const SIZE: usize = 1024;

        let mut group = c.benchmark_group("simple-1k");
        group.throughput(Throughput::Bytes(SIZE as u64));

        group.bench_function("zeroes-1k", |bencher| {
            bencher.iter(|| {
                adler32_slice(&[0; SIZE]);
            });
        });

        group.bench_function("ones-1k", |bencher| {
            bencher.iter(|| {
                adler32_slice(&[0xff; SIZE]);
            });
        });
    }

    {
        const SIZE: usize = 1024 * 1024;

        let mut group = c.benchmark_group("simple-1m");
        group.throughput(Throughput::Bytes(SIZE as u64));
        group.bench_function("zeroes-1m", |bencher| {
            bencher.iter(|| {
                adler32_slice(&[0; SIZE]);
            });
        });

        group.bench_function("ones-1m", |bencher| {
            bencher.iter(|| {
                adler32_slice(&[0xff; SIZE]);
            });
        });
    }
}

fn chunked(c: &mut Criterion) {
    const SIZE: usize = 16 * 1024 * 1024;

    let data = vec![0xAB; SIZE];

    let mut group = c.benchmark_group("chunked-16m");
    group.throughput(Throughput::Bytes(SIZE as u64));
    group.bench_function("5552", |bencher| {
        bencher.iter(|| {
            let mut h = Adler32::new();
            for chunk in data.chunks(5552) {
                h.write_slice(chunk);
            }
            h.checksum()
        });
    });
    group.bench_function("8k", |bencher| {
        bencher.iter(|| {
            let mut h = Adler32::new();
            for chunk in data.chunks(8 * 1024) {
                h.write_slice(chunk);
            }
            h.checksum()
        });
    });
    group.bench_function("64k", |bencher| {
        bencher.iter(|| {
            let mut h = Adler32::new();
            for chunk in data.chunks(64 * 1024) {
                h.write_slice(chunk);
            }
            h.checksum()
        });
    });
    group.bench_function("1m", |bencher| {
        bencher.iter(|| {
            let mut h = Adler32::new();
            for chunk in data.chunks(1024 * 1024) {
                h.write_slice(chunk);
            }
            h.checksum()
        });
    });
}

criterion_group!(benches, simple, chunked);
criterion_main!(benches);
