pub use ansi_to_tui::IntoText;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    const BASIC: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/ascii/archlinux.ascii"
    ));
    #[cfg(feature = "zero-copy")]
    c.bench_function("Parsing bench zero copy", |b| {
        b.iter(|| {
            let s = black_box(&BASIC);
            black_box(s.to_text()).unwrap();
        })
    });
    c.bench_function("Parsing bench", |b| {
        b.iter(|| {
            let s = black_box(&BASIC);
            black_box(s.into_text()).unwrap();
        })
    });
    const CODE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/ascii/code.ascii"));
    #[cfg(feature = "zero-copy")]
    c.bench_function("Parsing bench zero copy code", |b| {
        b.iter(|| {
            let s = black_box(&CODE);
            black_box(s.to_text()).unwrap();
        })
    });
    c.bench_function("Parsing bench code", |b| {
        b.iter(|| {
            let s = black_box(&CODE);
            black_box(s.into_text()).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
