use askama::filters::{Html, escape};
use criterion::{Criterion, black_box, criterion_group, criterion_main};

criterion_main!(benches);
criterion_group!(benches, functions);

fn functions(c: &mut Criterion) {
    c.bench_function("Escaping", escaping);
}

fn escaping(b: &mut criterion::Bencher<'_>) {
    b.iter(|| {
        for &s in black_box(STRINGS) {
            let _ = black_box(format!("{}", escape(s, Html).unwrap()));
        }
    });
}

const STRINGS: &[&str] = include!("strings.inc");
