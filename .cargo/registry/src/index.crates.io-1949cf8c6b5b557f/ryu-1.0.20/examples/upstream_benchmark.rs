// cargo run --example upstream_benchmark --release

use rand::{Rng, SeedableRng};

const SAMPLES: usize = 10000;
const ITERATIONS: usize = 1000;

struct MeanAndVariance {
    n: i64,
    mean: f64,
    m2: f64,
}

impl MeanAndVariance {
    fn new() -> Self {
        MeanAndVariance {
            n: 0,
            mean: 0.0,
            m2: 0.0,
        }
    }

    fn update(&mut self, x: f64) {
        self.n += 1;
        let d = x - self.mean;
        self.mean += d / self.n as f64;
        let d2 = x - self.mean;
        self.m2 += d * d2;
    }

    fn variance(&self) -> f64 {
        self.m2 / (self.n - 1) as f64
    }

    fn stddev(&self) -> f64 {
        self.variance().sqrt()
    }
}

macro_rules! benchmark {
    ($name:ident, $ty:ident) => {
        fn $name() -> usize {
            let mut rng = rand_xorshift::XorShiftRng::from_seed([123u8; 16]);
            let mut mv = MeanAndVariance::new();
            let mut throwaway = 0;
            for _ in 0..SAMPLES {
                let f = loop {
                    let f = $ty::from_bits(rng.random());
                    if f.is_finite() {
                        break f;
                    }
                };

                let t1 = std::time::SystemTime::now();
                for _ in 0..ITERATIONS {
                    throwaway += ryu::Buffer::new().format_finite(f).len();
                }
                let duration = t1.elapsed().unwrap();
                let nanos = duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64;
                mv.update(nanos as f64 / ITERATIONS as f64);
            }
            println!(
                "{:12} {:8.3} {:8.3}",
                concat!(stringify!($name), ":"),
                mv.mean,
                mv.stddev(),
            );
            throwaway
        }
    };
}

benchmark!(pretty32, f32);
benchmark!(pretty64, f64);

fn main() {
    println!("{:>20}{:>9}", "Average", "Stddev");
    let mut throwaway = 0;
    throwaway += pretty32();
    throwaway += pretty64();
    if std::env::var_os("ryu-benchmark").is_some() {
        // Prevent the compiler from optimizing the code away.
        println!("{}", throwaway);
    }
}
