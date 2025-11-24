#![feature(test, maybe_uninit_uninit_array_transpose)]
extern crate test;

use std::{
    mem::{size_of, MaybeUninit},
    slice,
};

// Call getrandom on a zero-initialized stack buffer
#[inline(always)]
fn bench_fill<const N: usize>() {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).unwrap();
    test::black_box(&buf[..]);
}

// Call fill_uninit on an uninitialized stack buffer
#[inline(always)]
fn bench_fill_uninit<const N: usize>() {
    let mut uninit = [MaybeUninit::uninit(); N];
    let buf: &[u8] = getrandom::fill_uninit(&mut uninit).unwrap();
    test::black_box(buf);
}

#[bench]
pub fn bench_u32(b: &mut test::Bencher) {
    #[inline(never)]
    fn inner() -> u32 {
        getrandom::u32().unwrap()
    }
    b.bytes = 4;
    b.iter(inner);
}
#[bench]
pub fn bench_u32_via_fill(b: &mut test::Bencher) {
    #[inline(never)]
    fn inner() -> u32 {
        let mut res = MaybeUninit::<u32>::uninit();
        let dst: &mut [MaybeUninit<u8>] =
            unsafe { slice::from_raw_parts_mut(res.as_mut_ptr().cast(), size_of::<u32>()) };
        getrandom::fill_uninit(dst).unwrap();
        unsafe { res.assume_init() }
    }
    b.bytes = 4;
    b.iter(inner);
}

#[bench]
pub fn bench_u64(b: &mut test::Bencher) {
    #[inline(never)]
    fn inner() -> u64 {
        getrandom::u64().unwrap()
    }
    b.bytes = 8;
    b.iter(inner);
}

#[bench]
pub fn bench_u64_via_fill(b: &mut test::Bencher) {
    #[inline(never)]
    fn inner() -> u64 {
        let mut res = MaybeUninit::<u64>::uninit();
        let dst: &mut [MaybeUninit<u8>] =
            unsafe { slice::from_raw_parts_mut(res.as_mut_ptr().cast(), size_of::<u64>()) };
        getrandom::fill_uninit(dst).unwrap();
        unsafe { res.assume_init() }
    }
    b.bytes = 8;
    b.iter(inner);
}

// We benchmark using #[inline(never)] "inner" functions for two reasons:
//  - Avoiding inlining reduces a source of variance when running benchmarks.
//  - It is _much_ easier to get the assembly or IR for the inner loop.
//
// For example, using cargo-show-asm (https://github.com/pacak/cargo-show-asm),
// we can get the assembly for a particular benchmark's inner loop by running:
//   cargo asm --bench buffer --release buffer::p384::bench_getrandom::inner
macro_rules! bench {
    ( $name:ident, $size:expr ) => {
        pub mod $name {
            #[bench]
            pub fn bench_fill(b: &mut test::Bencher) {
                #[inline(never)]
                fn inner() {
                    super::bench_fill::<{ $size }>()
                }

                b.bytes = $size as u64;
                b.iter(inner);
            }
            #[bench]
            pub fn bench_fill_uninit(b: &mut test::Bencher) {
                #[inline(never)]
                fn inner() {
                    super::bench_fill_uninit::<{ $size }>()
                }

                b.bytes = $size as u64;
                b.iter(inner);
            }
        }
    };
}

// 16 bytes (128 bits) is the size of an 128-bit AES key/nonce.
bench!(aes128, 128 / 8);

// 32 bytes (256 bits) is the seed sized used for rand::thread_rng
// and the `random` value in a ClientHello/ServerHello for TLS.
// This is also the size of a 256-bit AES/HMAC/P-256/Curve25519 key
// and/or nonce.
bench!(p256, 256 / 8);

// A P-384/HMAC-384 key and/or nonce.
bench!(p384, 384 / 8);

// Initializing larger buffers is not the primary use case of this library, as
// this should normally be done by a userspace CSPRNG. However, we have a test
// here to see the effects of a lower (amortized) syscall overhead.
bench!(page, 4096);
