extern crate criterion;
extern crate diff;

use criterion::Criterion;

criterion::criterion_group!(benches, bench_slice, bench_chars, bench_real_world);
criterion::criterion_main!(benches);

fn bench_slice(c: &mut Criterion) {
    c.bench_function("empty", |b| {
        let slice = [0u8; 0];
        b.iter(|| ::diff::slice(&slice, &slice));
    });

    c.bench_function("10 equal items", |b| {
        let slice = [0u8; 10];
        b.iter(|| ::diff::slice(&slice, &slice));
    });

    c.bench_function("10 non-equal items", |b| {
        let (left, right) = ([0u8; 10], [1u8; 10]);
        b.iter(|| ::diff::slice(&left, &right));
    });

    c.bench_function("100 equal items", |b| {
        let slice = [0u8; 100];
        b.iter(|| ::diff::slice(&slice, &slice));
    });

    c.bench_function("100 non-equal items", |b| {
        let (left, right) = ([0u8; 100], [1u8; 100]);
        b.iter(|| ::diff::slice(&left, &right));
    });

    c.bench_function("1000 equal items", |b| {
        let slice = [0u8; 1000];
        b.iter(|| ::diff::slice(&slice, &slice));
    });

    c.bench_function("1000 non-equal items", |b| {
        let (left, right) = ([0u8; 1000], [1u8; 1000]);
        b.iter(|| ::diff::slice(&left, &right));
    });
}

fn bench_chars(c: &mut Criterion) {
    c.bench_function("1024 byte string, last 256 different", |b| {
        let left = "?".repeat(768) + &"_".repeat(256);
        let right = "?".repeat(768) + &"!".repeat(256);
        assert_eq!(left.len(), right.len());
        b.iter(|| ::diff::chars(&left, &right));
    });
}

fn bench_real_world(c: &mut Criterion) {
    let gitignores = std::fs::read_to_string("tests/data/gitignores.txt")
        .unwrap()
        .split("!!!")
        .filter_map(|str| (!str.is_empty()).then(|| str.into()))
        .collect::<Vec<String>>();

    c.bench_function("diff::lines on gitignore files from rust-lang/rust", |b| {
        b.iter(|| {
            for (i, left) in gitignores.iter().enumerate() {
                // diff with previous 3, itself, and next 3
                for right in gitignores[i.saturating_sub(3)..(i + 3).min(gitignores.len())].iter() {
                    ::diff::lines(&left, &right);
                }
            }
        })
    });

    c.bench_function("diff::chars on gitignore files from rust-lang/rust", |b| {
        b.iter(|| {
            for (i, left) in gitignores.iter().enumerate() {
                // diff with previous 2, itself, and next 2
                for right in gitignores[i.saturating_sub(2)..(i + 2).min(gitignores.len())].iter() {
                    ::diff::chars(&left, &right);
                }
            }
        })
    });
}
