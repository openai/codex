use bencher::Bencher;
use crc32fast::Hasher;
use rand::Rng;

fn bench(b: &mut Bencher, size: usize, hasher_init: Hasher) {
    let mut bytes = vec![0u8; size];
    rand::thread_rng().fill(&mut bytes[..]);

    b.iter(|| {
        let mut hasher = hasher_init.clone();
        hasher.update(&bytes);
        bencher::black_box(hasher.finalize())
    });

    b.bytes = size as u64;
}

fn bench_kilobyte_baseline(b: &mut Bencher) {
    bench(b, 1024, Hasher::internal_new_baseline(0, 0))
}

fn bench_kilobyte_specialized(b: &mut Bencher) {
    bench(b, 1024, Hasher::internal_new_specialized(0, 0).unwrap())
}

fn bench_megabyte_baseline(b: &mut Bencher) {
    bench(b, 1024 * 1024, Hasher::internal_new_baseline(0, 0))
}

fn bench_megabyte_specialized(b: &mut Bencher) {
    bench(
        b,
        1024 * 1024,
        Hasher::internal_new_specialized(0, 0).unwrap(),
    )
}

fn bench_combine_inner(b: &mut Bencher, i1: u32, l1: u64, i2: u32, l2: u64) {
    let h1 = Hasher::new_with_initial_len(i1, l1);
    let h2 = Hasher::new_with_initial_len(i2, l2);

    b.iter(|| {
        let mut h = h1.clone();
        h.combine(&h2);
        bencher::black_box(h);
    })
}

fn bench_combine_16(b: &mut Bencher) {
    let (i1, l1, i2, l2): (u32, u64, u32, u16) = rand::thread_rng().gen();
    bench_combine_inner(b, i1, l1, i2, u64::from(l2))
}

fn bench_combine_32(b: &mut Bencher) {
    let (i1, l1, i2, l2): (u32, u64, u32, u32) = rand::thread_rng().gen();
    bench_combine_inner(b, i1, l1, i2, u64::from(l2))
}

fn bench_combine_64(b: &mut Bencher) {
    let (i1, l1, i2, l2): (u32, u64, u32, u64) = rand::thread_rng().gen();
    bench_combine_inner(b, i1, l1, i2, l2)
}

bencher::benchmark_group!(
    bench_baseline,
    bench_kilobyte_baseline,
    bench_megabyte_baseline
);
bencher::benchmark_group!(
    bench_specialized,
    bench_kilobyte_specialized,
    bench_megabyte_specialized
);
bencher::benchmark_group!(
    bench_combine,
    bench_combine_16,
    bench_combine_32,
    bench_combine_64
);
bencher::benchmark_main!(bench_baseline, bench_specialized, bench_combine);
