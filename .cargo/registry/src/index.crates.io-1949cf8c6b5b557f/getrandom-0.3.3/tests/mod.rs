use core::mem::MaybeUninit;
use getrandom::{fill, fill_uninit};

#[cfg(all(feature = "wasm_js", target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[test]
fn test_zero() {
    // Test that APIs are happy with zero-length requests
    fill(&mut [0u8; 0]).unwrap();
    let res = fill_uninit(&mut []).unwrap();
    assert!(res.is_empty());
}

trait DiffBits: Sized {
    fn diff_bits(ab: (&Self, &Self)) -> usize;
}

impl DiffBits for u8 {
    fn diff_bits((a, b): (&Self, &Self)) -> usize {
        (a ^ b).count_ones() as usize
    }
}

impl DiffBits for u32 {
    fn diff_bits((a, b): (&Self, &Self)) -> usize {
        (a ^ b).count_ones() as usize
    }
}

impl DiffBits for u64 {
    fn diff_bits((a, b): (&Self, &Self)) -> usize {
        (a ^ b).count_ones() as usize
    }
}

// Return the number of bits in which s1 and s2 differ
fn num_diff_bits<T: DiffBits>(s1: &[T], s2: &[T]) -> usize {
    assert_eq!(s1.len(), s2.len());
    s1.iter().zip(s2.iter()).map(T::diff_bits).sum()
}

// TODO: use `[const { MaybeUninit::uninit() }; N]` after MSRV is bumped to 1.79+
// or `MaybeUninit::uninit_array`
fn uninit_vec(n: usize) -> Vec<MaybeUninit<u8>> {
    vec![MaybeUninit::uninit(); n]
}

// Tests the quality of calling getrandom on two large buffers
#[test]
fn test_diff() {
    const N: usize = 1000;
    let mut v1 = [0u8; N];
    let mut v2 = [0u8; N];
    fill(&mut v1).unwrap();
    fill(&mut v2).unwrap();

    let mut t1 = uninit_vec(N);
    let mut t2 = uninit_vec(N);
    let r1 = fill_uninit(&mut t1).unwrap();
    let r2 = fill_uninit(&mut t2).unwrap();
    assert_eq!(r1.len(), N);
    assert_eq!(r2.len(), N);

    // Between 3.5 and 4.5 bits per byte should differ. Probability of failure:
    // ~ 2^(-94) = 2 * CDF[BinomialDistribution[8000, 0.5], 3500]
    let d1 = num_diff_bits(&v1, &v2);
    assert!(d1 > 3500);
    assert!(d1 < 4500);
    let d2 = num_diff_bits(r1, r2);
    assert!(d2 > 3500);
    assert!(d2 < 4500);
}

#[test]
fn test_diff_u32() {
    const N: usize = 1000 / 4;
    let mut v1 = [0u32; N];
    let mut v2 = [0u32; N];
    for v in v1.iter_mut() {
        *v = getrandom::u32().unwrap();
    }
    for v in v2.iter_mut() {
        *v = getrandom::u32().unwrap();
    }

    // Between 3.5 and 4.5 bits per byte should differ. Probability of failure:
    // ~ 2^(-94) = 2 * CDF[BinomialDistribution[8000, 0.5], 3500]
    let d1 = num_diff_bits(&v1, &v2);
    assert!(d1 > 3500);
    assert!(d1 < 4500);
}

#[test]
fn test_diff_u64() {
    const N: usize = 1000 / 8;
    let mut v1 = [0u64; N];
    let mut v2 = [0u64; N];
    for v in v1.iter_mut() {
        *v = getrandom::u64().unwrap();
    }
    for v in v2.iter_mut() {
        *v = getrandom::u64().unwrap();
    }

    // Between 3.5 and 4.5 bits per byte should differ. Probability of failure:
    // ~ 2^(-94) = 2 * CDF[BinomialDistribution[8000, 0.5], 3500]
    let d1 = num_diff_bits(&v1, &v2);
    assert!(d1 > 3500);
    assert!(d1 < 4500);
}

#[test]
fn test_small() {
    const N: usize = 64;
    // For each buffer size, get at least 256 bytes and check that between
    // 3 and 5 bits per byte differ. Probability of failure:
    // ~ 2^(-91) = 64 * 2 * CDF[BinomialDistribution[8*256, 0.5], 3*256]
    for size in 1..=N {
        let mut num_bytes = 0;
        let mut diff_bits = 0;
        while num_bytes < 256 {
            let mut buf1 = [0u8; N];
            let mut buf2 = [0u8; N];

            let s1 = &mut buf1[..size];
            let s2 = &mut buf2[..size];

            fill(s1).unwrap();
            fill(s2).unwrap();

            num_bytes += size;
            diff_bits += num_diff_bits(s1, s2);
        }
        assert!(diff_bits > 3 * num_bytes);
        assert!(diff_bits < 5 * num_bytes);
    }
}

// Tests the quality of calling getrandom repeatedly on small buffers
#[test]
fn test_small_uninit() {
    const N: usize = 64;
    // For each buffer size, get at least 256 bytes and check that between
    // 3 and 5 bits per byte differ. Probability of failure:
    // ~ 2^(-91) = 64 * 2 * CDF[BinomialDistribution[8*256, 0.5], 3*256]
    for size in 1..=N {
        let mut num_bytes = 0;
        let mut diff_bits = 0;
        while num_bytes < 256 {
            let mut buf1 = uninit_vec(N);
            let mut buf2 = uninit_vec(N);

            let s1 = &mut buf1[..size];
            let s2 = &mut buf2[..size];

            let r1 = fill_uninit(s1).unwrap();
            let r2 = fill_uninit(s2).unwrap();
            assert_eq!(r1.len(), size);
            assert_eq!(r2.len(), size);

            num_bytes += size;
            diff_bits += num_diff_bits(r1, r2);
        }
        assert!(diff_bits > 3 * num_bytes);
        assert!(diff_bits < 5 * num_bytes);
    }
}

#[test]
fn test_huge() {
    let mut huge = [0u8; 100_000];
    fill(&mut huge).unwrap();
}

#[test]
fn test_huge_uninit() {
    const N: usize = 100_000;
    let mut huge = uninit_vec(N);
    let res = fill_uninit(&mut huge).unwrap();
    assert_eq!(res.len(), N);
}

#[test]
#[cfg_attr(
    target_arch = "wasm32",
    ignore = "The thread API always fails/panics on WASM"
)]
fn test_multithreading() {
    extern crate std;
    use std::{sync::mpsc::channel, thread, vec};

    let mut txs = vec![];
    for _ in 0..20 {
        let (tx, rx) = channel();
        txs.push(tx);

        thread::spawn(move || {
            // wait until all the tasks are ready to go.
            rx.recv().unwrap();
            let mut v = [0u8; 1000];

            for _ in 0..100 {
                fill(&mut v).unwrap();
                thread::yield_now();
            }
        });
    }

    // start all the tasks
    for tx in txs.iter() {
        tx.send(()).unwrap();
    }
}

#[cfg(getrandom_backend = "custom")]
mod custom {
    use getrandom::Error;

    struct Xoshiro128PlusPlus {
        s: [u32; 4],
    }

    impl Xoshiro128PlusPlus {
        fn new(mut seed: u64) -> Self {
            const PHI: u64 = 0x9e3779b97f4a7c15;
            let mut s = [0u32; 4];
            for val in s.iter_mut() {
                seed = seed.wrapping_add(PHI);
                let mut z = seed;
                z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
                z = z ^ (z >> 31);
                *val = z as u32;
            }
            Self { s }
        }

        fn next_u32(&mut self) -> u32 {
            let res = self.s[0]
                .wrapping_add(self.s[3])
                .rotate_left(7)
                .wrapping_add(self.s[0]);

            let t = self.s[1] << 9;

            self.s[2] ^= self.s[0];
            self.s[3] ^= self.s[1];
            self.s[1] ^= self.s[2];
            self.s[0] ^= self.s[3];

            self.s[2] ^= t;

            self.s[3] = self.s[3].rotate_left(11);

            res
        }
    }

    // This implementation uses current timestamp as a PRNG seed.
    //
    // WARNING: this custom implementation is for testing purposes ONLY!
    #[no_mangle]
    unsafe extern "Rust" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), Error> {
        use std::time::{SystemTime, UNIX_EPOCH};

        assert_ne!(len, 0);

        if len == 142 {
            return Err(Error::new_custom(142));
        }

        let dest_u32 = dest.cast::<u32>();
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let mut rng = Xoshiro128PlusPlus::new(ts.as_nanos() as u64);
        for i in 0..len / 4 {
            let val = rng.next_u32();
            core::ptr::write_unaligned(dest_u32.add(i), val);
        }
        if len % 4 != 0 {
            let start = 4 * (len / 4);
            for i in start..len {
                let val = rng.next_u32();
                core::ptr::write_unaligned(dest.add(i), val as u8);
            }
        }
        Ok(())
    }

    // Test that enabling the custom feature indeed uses the custom implementation
    #[test]
    fn test_custom() {
        let mut buf = [0u8; 142];
        let res = getrandom::fill(&mut buf);
        assert!(res.is_err());
    }
}
