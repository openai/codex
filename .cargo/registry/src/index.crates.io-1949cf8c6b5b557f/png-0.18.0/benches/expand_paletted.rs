//! Usage example:
//!
//! ```
//! $ alias bench="rustup run nightly cargo bench"
//! $ bench --bench=expand_paletted --features=benchmarks -- --save-baseline my_baseline
//! ... tweak something ...
//! $ bench --bench=expand_paletted --features=benchmarks -- --baseline my_baseline
//! ```

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use png::benchable_apis::{create_info_from_plte_trns_bitdepth, create_transform_fn, TransformFn};
use png::{Info, Transformations};
use rand::Rng;
use std::fmt::{self, Display};

#[derive(Clone, Copy)]
enum TrnsPresence {
    Present,
    Absent,
}

impl Display for TrnsPresence {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TrnsPresence::Present => write!(f, "trns=yes"),
            TrnsPresence::Absent => write!(f, "trns=no"),
        }
    }
}

fn expand_paletted_all(c: &mut Criterion) {
    let trns_options = [TrnsPresence::Absent, TrnsPresence::Present];
    let bit_depths = [4, 8];

    let input_size = {
        let typical_l1_cache_size = 32 * 1024;
        let mut factor = 1; // input
        factor += 4; // RGBA output
        factor += 1; // other data
        typical_l1_cache_size / factor
    };

    for trns in trns_options.iter().copied() {
        for bit_depth in bit_depths.iter().copied() {
            bench_expand_palette(c, trns, bit_depth, input_size);
        }
    }

    bench_create_fn(c, 256, 256); // Full PLTE and trNS
    bench_create_fn(c, 224, 32); // Partial PLTE and trNS
    bench_create_fn(c, 16, 1); // Guess: typical for small images?
}

criterion_group!(benches, expand_paletted_all);
criterion_main!(benches);

fn get_random_bytes<R: Rng>(rng: &mut R, n: usize) -> Vec<u8> {
    use rand::Fill;
    let mut result = vec![0u8; n];
    result.as_mut_slice().try_fill(rng).unwrap();
    result
}

struct Input {
    palette: Vec<u8>,
    trns: Option<Vec<u8>>,
    src: Vec<u8>,
    src_bit_depth: u8,
}

impl Input {
    fn new(trns: TrnsPresence, src_bit_depth: u8, input_size_in_bytes: usize) -> Self {
        let mut rng = rand::thread_rng();

        // We provide RGB entries for 192 out of 256 possible indices and Alpha/Transparency
        // entries for 32 out of 256 possible indices.  Rationale for these numbers:
        // * Oftentimes only a handful of colors at the edges of an icon need transparency
        // * In general, code needs to handle out-of-bounds indices, so it seems desirable
        //   to explicitly test this.
        let palette = get_random_bytes(&mut rng, 192.min(input_size_in_bytes) * 3);
        let trns = match trns {
            TrnsPresence::Absent => None,
            TrnsPresence::Present => Some(get_random_bytes(&mut rng, 32.min(input_size_in_bytes))),
        };
        let src = get_random_bytes(&mut rng, input_size_in_bytes);

        Self {
            palette,
            trns,
            src,
            src_bit_depth,
        }
    }

    fn output_size_in_bytes(&self) -> usize {
        let output_bytes_per_input_sample = match self.trns {
            None => 3,
            Some(_) => 4,
        };
        let samples_count_per_byte = (8 / self.src_bit_depth) as usize;
        let samples_count = self.src.len() * samples_count_per_byte;
        samples_count * output_bytes_per_input_sample
    }

    fn to_info(&self) -> Info {
        create_info_from_plte_trns_bitdepth(&self.palette, self.trns.as_deref(), self.src_bit_depth)
    }
}

#[inline(always)]
fn create_expand_palette_fn(info: &Info) -> TransformFn {
    create_transform_fn(info, Transformations::EXPAND).unwrap()
}

fn bench_create_fn(c: &mut Criterion, plte_size: usize, trns_size: usize) {
    let mut group = c.benchmark_group("expand_paletted(ctor)");
    group.sample_size(1000);

    let mut rng = rand::thread_rng();
    let plte = get_random_bytes(&mut rng, 3 * plte_size as usize);
    let trns = get_random_bytes(&mut rng, trns_size as usize);
    let info = create_info_from_plte_trns_bitdepth(&plte, Some(&trns), 8);
    group.bench_with_input(
        format!("plte={plte_size}/trns={trns_size:?}"),
        &info,
        |b, info| {
            b.iter(|| create_expand_palette_fn(info));
        },
    );
}

fn bench_expand_palette(
    c: &mut Criterion,
    trns: TrnsPresence,
    src_bit_depth: u8,
    input_size_in_bytes: usize,
) {
    let mut group = c.benchmark_group("expand_paletted(exec)");

    let input = Input::new(trns, src_bit_depth, input_size_in_bytes);
    let transform_fn = create_expand_palette_fn(&input.to_info());
    group.throughput(Throughput::Bytes(input.output_size_in_bytes() as u64));
    group.sample_size(500);
    group.bench_with_input(
        format!("{trns}/src_bits={src_bit_depth}/src_size={input_size_in_bytes}"),
        &input,
        |b, input| {
            let mut output = vec![0; input.output_size_in_bytes()];
            let info = input.to_info();
            b.iter(|| {
                transform_fn(input.src.as_slice(), output.as_mut_slice(), &info);
            });
        },
    );
}
