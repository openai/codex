//! Development-related functionality

pub use blobby;

mod fixed;
mod mac;
mod rng;
mod variable;
mod xof;

pub use fixed::*;
pub use mac::*;
pub use variable::*;
pub use xof::*;

/// Define hash function test
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! new_test {
    ($name:ident, $test_name:expr, $hasher:ty, $test_func:ident $(,)?) => {
        #[test]
        fn $name() {
            use digest::dev::blobby::Blob2Iterator;
            let data = include_bytes!(concat!("data/", $test_name, ".blb"));

            for (i, row) in Blob2Iterator::new(data).unwrap().enumerate() {
                let [input, output] = row.unwrap();
                if let Some(desc) = $test_func::<$hasher>(input, output) {
                    panic!(
                        "\n\
                         Failed test №{}: {}\n\
                         input:\t{:?}\n\
                         output:\t{:?}\n",
                        i, desc, input, output,
                    );
                }
            }
        }
    };
}

/// Define [`Update`][crate::Update] impl benchmark
#[macro_export]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
macro_rules! bench_update {
    (
        $init:expr;
        $($name:ident $bs:expr;)*
    ) => {
        $(
            #[bench]
            fn $name(b: &mut Bencher) {
                let mut d = $init;
                let data = [0; $bs];

                b.iter(|| {
                    digest::Update::update(&mut d, &data[..]);
                });

                b.bytes = $bs;
            }
        )*
    };
}

/// Feed ~1 MiB of pseudorandom data to an updatable state.
pub fn feed_rand_16mib<D: crate::Update>(d: &mut D) {
    let buf = &mut [0u8; 1024];
    let mut rng = rng::RNG;
    let n = 16 * (1 << 20) / buf.len();
    for _ in 0..n {
        rng.fill(buf);
        d.update(buf);
        // additional byte, so size of fed data
        // will not be multiple of block size
        d.update(&[42]);
    }
}
