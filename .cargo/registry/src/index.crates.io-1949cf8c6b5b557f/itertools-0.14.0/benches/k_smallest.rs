use criterion::{black_box, criterion_group, criterion_main, Bencher, BenchmarkId, Criterion};
use itertools::Itertools;
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};

fn strict(b: &mut Bencher, (k, vals): &(usize, &Vec<usize>)) {
    b.iter(|| black_box(vals.iter()).k_smallest(*k))
}

fn relaxed(b: &mut Bencher, (k, vals): &(usize, &Vec<usize>)) {
    b.iter(|| black_box(vals.iter()).k_smallest_relaxed(*k))
}

fn ascending(n: usize) -> Vec<usize> {
    (0..n).collect()
}

fn random(n: usize) -> Vec<usize> {
    let mut vals = (0..n).collect_vec();
    vals.shuffle(&mut StdRng::seed_from_u64(42));
    vals
}

fn descending(n: usize) -> Vec<usize> {
    (0..n).rev().collect()
}

fn k_smallest(c: &mut Criterion, order: &str, vals: fn(usize) -> Vec<usize>) {
    let mut g = c.benchmark_group(format!("k-smallest/{order}"));

    for log_n in 20..23 {
        let n = 1 << log_n;

        let vals = vals(n);

        for log_k in 7..10 {
            let k = 1 << log_k;

            let params = format!("{log_n}/{log_k}");
            let input = (k, &vals);
            g.bench_with_input(BenchmarkId::new("strict", &params), &input, strict);
            g.bench_with_input(BenchmarkId::new("relaxed", &params), &input, relaxed);
        }
    }

    g.finish()
}

fn k_smallest_asc(c: &mut Criterion) {
    k_smallest(c, "asc", ascending);
}

fn k_smallest_rand(c: &mut Criterion) {
    k_smallest(c, "rand", random);
}

fn k_smallest_desc(c: &mut Criterion) {
    k_smallest(c, "desc", descending);
}

criterion_group!(benches, k_smallest_asc, k_smallest_rand, k_smallest_desc);
criterion_main!(benches);
