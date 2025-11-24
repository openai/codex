#[macro_use]
extern crate criterion;

use base64::{
    display,
    engine::{general_purpose::STANDARD, Engine},
    write,
};
use criterion::{black_box, Bencher, BenchmarkId, Criterion, Throughput};
use rand::{Rng, SeedableRng};
use std::io::{self, Read, Write};

fn do_decode_bench(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size * 3 / 4);
    fill(&mut v);
    let encoded = STANDARD.encode(&v);

    b.iter(|| {
        let orig = STANDARD.decode(&encoded);
        black_box(&orig);
    });
}

fn do_decode_bench_reuse_buf(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size * 3 / 4);
    fill(&mut v);
    let encoded = STANDARD.encode(&v);

    let mut buf = Vec::new();
    b.iter(|| {
        STANDARD.decode_vec(&encoded, &mut buf).unwrap();
        black_box(&buf);
        buf.clear();
    });
}

fn do_decode_bench_slice(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size * 3 / 4);
    fill(&mut v);
    let encoded = STANDARD.encode(&v);

    let mut buf = vec![0; size];
    b.iter(|| {
        STANDARD.decode_slice(&encoded, &mut buf).unwrap();
        black_box(&buf);
    });
}

fn do_decode_bench_stream(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size * 3 / 4);
    fill(&mut v);
    let encoded = STANDARD.encode(&v);

    let mut buf = vec![0; size];
    buf.truncate(0);

    b.iter(|| {
        let mut cursor = io::Cursor::new(&encoded[..]);
        let mut decoder = base64::read::DecoderReader::new(&mut cursor, &STANDARD);
        decoder.read_to_end(&mut buf).unwrap();
        buf.clear();
        black_box(&buf);
    });
}

fn do_encode_bench(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size);
    fill(&mut v);
    b.iter(|| {
        let e = STANDARD.encode(&v);
        black_box(&e);
    });
}

fn do_encode_bench_display(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size);
    fill(&mut v);
    b.iter(|| {
        let e = format!("{}", display::Base64Display::new(&v, &STANDARD));
        black_box(&e);
    });
}

fn do_encode_bench_reuse_buf(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size);
    fill(&mut v);
    let mut buf = String::new();
    b.iter(|| {
        STANDARD.encode_string(&v, &mut buf);
        buf.clear();
    });
}

fn do_encode_bench_slice(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size);
    fill(&mut v);
    // conservative estimate of encoded size
    let mut buf = vec![0; v.len() * 2];
    b.iter(|| STANDARD.encode_slice(&v, &mut buf).unwrap());
}

fn do_encode_bench_stream(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size);
    fill(&mut v);
    let mut buf = Vec::with_capacity(size * 2);

    b.iter(|| {
        buf.clear();
        let mut stream_enc = write::EncoderWriter::new(&mut buf, &STANDARD);
        stream_enc.write_all(&v).unwrap();
        stream_enc.flush().unwrap();
    });
}

fn do_encode_bench_string_stream(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size);
    fill(&mut v);

    b.iter(|| {
        let mut stream_enc = write::EncoderStringWriter::new(&STANDARD);
        stream_enc.write_all(&v).unwrap();
        stream_enc.flush().unwrap();
        let _ = stream_enc.into_inner();
    });
}

fn do_encode_bench_string_reuse_buf_stream(b: &mut Bencher, &size: &usize) {
    let mut v: Vec<u8> = Vec::with_capacity(size);
    fill(&mut v);

    let mut buf = String::new();
    b.iter(|| {
        buf.clear();
        let mut stream_enc = write::EncoderStringWriter::from_consumer(&mut buf, &STANDARD);
        stream_enc.write_all(&v).unwrap();
        stream_enc.flush().unwrap();
        let _ = stream_enc.into_inner();
    });
}

fn fill(v: &mut Vec<u8>) {
    let cap = v.capacity();
    // weak randomness is plenty; we just want to not be completely friendly to the branch predictor
    let mut r = rand::rngs::SmallRng::from_entropy();
    while v.len() < cap {
        v.push(r.gen::<u8>());
    }
}

const BYTE_SIZES: [usize; 5] = [3, 50, 100, 500, 3 * 1024];

// Benchmarks over these byte sizes take longer so we will run fewer samples to
// keep the benchmark runtime reasonable.
const LARGE_BYTE_SIZES: [usize; 3] = [3 * 1024 * 1024, 10 * 1024 * 1024, 30 * 1024 * 1024];

fn encode_benchmarks(c: &mut Criterion, label: &str, byte_sizes: &[usize]) {
    let mut group = c.benchmark_group(label);
    group
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(3));

    for size in byte_sizes {
        group
            .throughput(Throughput::Bytes(*size as u64))
            .bench_with_input(BenchmarkId::new("encode", size), size, do_encode_bench)
            .bench_with_input(
                BenchmarkId::new("encode_display", size),
                size,
                do_encode_bench_display,
            )
            .bench_with_input(
                BenchmarkId::new("encode_reuse_buf", size),
                size,
                do_encode_bench_reuse_buf,
            )
            .bench_with_input(
                BenchmarkId::new("encode_slice", size),
                size,
                do_encode_bench_slice,
            )
            .bench_with_input(
                BenchmarkId::new("encode_reuse_buf_stream", size),
                size,
                do_encode_bench_stream,
            )
            .bench_with_input(
                BenchmarkId::new("encode_string_stream", size),
                size,
                do_encode_bench_string_stream,
            )
            .bench_with_input(
                BenchmarkId::new("encode_string_reuse_buf_stream", size),
                size,
                do_encode_bench_string_reuse_buf_stream,
            );
    }

    group.finish();
}

fn decode_benchmarks(c: &mut Criterion, label: &str, byte_sizes: &[usize]) {
    let mut group = c.benchmark_group(label);

    for size in byte_sizes {
        group
            .warm_up_time(std::time::Duration::from_millis(500))
            .measurement_time(std::time::Duration::from_secs(3))
            .throughput(Throughput::Bytes(*size as u64))
            .bench_with_input(BenchmarkId::new("decode", size), size, do_decode_bench)
            .bench_with_input(
                BenchmarkId::new("decode_reuse_buf", size),
                size,
                do_decode_bench_reuse_buf,
            )
            .bench_with_input(
                BenchmarkId::new("decode_slice", size),
                size,
                do_decode_bench_slice,
            )
            .bench_with_input(
                BenchmarkId::new("decode_stream", size),
                size,
                do_decode_bench_stream,
            );
    }

    group.finish();
}

fn bench(c: &mut Criterion) {
    encode_benchmarks(c, "encode_small_input", &BYTE_SIZES[..]);
    encode_benchmarks(c, "encode_large_input", &LARGE_BYTE_SIZES[..]);
    decode_benchmarks(c, "decode_small_input", &BYTE_SIZES[..]);
    decode_benchmarks(c, "decode_large_input", &LARGE_BYTE_SIZES[..]);
}

criterion_group!(benches, bench);
criterion_main!(benches);
