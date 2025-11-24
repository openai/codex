//! Usage example:
//!
//! ```
//! $ alias bench="rustup run nightly cargo bench"
//! $ bench --bench=expand_adam7 --features=benchmarks -- --save-baseline my_baseline
//! ... tweak something, say the expansion of 8-bit ...
//! $ bench --bench=expand_adam7 --features=benchmarks -- bpp=8 --baseline my_baseline
//! ```
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use png::benchable_apis::adam7;

fn expand_all(c: &mut Criterion) {
    let expand_bpp = [1, 2, 4, 8, 16, 24, 32];
    let expand_size = [1 << 4, 1 << 8, 1 << 12];

    for &bpp in expand_bpp.iter() {
        for &sz in expand_size.iter() {
            bench_group_expand_full(c, sz, bpp);
        }
    }
}

criterion_group!(benches, expand_all);
criterion_main!(benches);

fn bench_group_expand_full(c: &mut Criterion, sz: u32, bpp: u8) {
    let mut group = c.benchmark_group("expand-adam7");

    group.throughput(Throughput::Bytes(
        u64::from(sz) * (u64::from(sz) * u64::from(bpp)).div_ceil(8),
    ));

    let row_bytes = (sz as usize * usize::from(bpp)).div_ceil(8);
    let buffer = vec![0u8; (sz as usize) * row_bytes];
    let buffer = core::cell::RefCell::new(buffer);
    let rowdata = core::hint::black_box(vec![0u8; row_bytes]);

    group.bench_with_input(format!("size={sz:?}/bpp={bpp}"), &buffer, |b, img| {
        b.iter(|| adam7(&mut *img.borrow_mut(), &rowdata, sz, sz, bpp));
    });
}
