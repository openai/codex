use super::getrandom_impl;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(feature = "test-in-browser")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn test_zero() {
    // Test that APIs are happy with zero-length requests
    getrandom_impl(&mut [0u8; 0]).unwrap();
}

// Return the number of bits in which s1 and s2 differ
#[cfg(not(feature = "custom"))]
fn num_diff_bits(s1: &[u8], s2: &[u8]) -> usize {
    assert_eq!(s1.len(), s2.len());
    s1.iter()
        .zip(s2.iter())
        .map(|(a, b)| (a ^ b).count_ones() as usize)
        .sum()
}

// Tests the quality of calling getrandom on two large buffers
#[test]
#[cfg(not(feature = "custom"))]
fn test_diff() {
    let mut v1 = [0u8; 1000];
    getrandom_impl(&mut v1).unwrap();

    let mut v2 = [0u8; 1000];
    getrandom_impl(&mut v2).unwrap();

    // Between 3.5 and 4.5 bits per byte should differ. Probability of failure:
    // ~ 2^(-94) = 2 * CDF[BinomialDistribution[8000, 0.5], 3500]
    let d = num_diff_bits(&v1, &v2);
    assert!(d > 3500);
    assert!(d < 4500);
}

// Tests the quality of calling getrandom repeatedly on small buffers
#[test]
#[cfg(not(feature = "custom"))]
fn test_small() {
    // For each buffer size, get at least 256 bytes and check that between
    // 3 and 5 bits per byte differ. Probability of failure:
    // ~ 2^(-91) = 64 * 2 * CDF[BinomialDistribution[8*256, 0.5], 3*256]
    for size in 1..=64 {
        let mut num_bytes = 0;
        let mut diff_bits = 0;
        while num_bytes < 256 {
            let mut s1 = vec![0u8; size];
            getrandom_impl(&mut s1).unwrap();
            let mut s2 = vec![0u8; size];
            getrandom_impl(&mut s2).unwrap();

            num_bytes += size;
            diff_bits += num_diff_bits(&s1, &s2);
        }
        assert!(diff_bits > 3 * num_bytes);
        assert!(diff_bits < 5 * num_bytes);
    }
}

#[test]
fn test_huge() {
    let mut huge = [0u8; 100_000];
    getrandom_impl(&mut huge).unwrap();
}

// On WASM, the thread API always fails/panics
#[cfg(not(target_arch = "wasm32"))]
#[test]
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
                getrandom_impl(&mut v).unwrap();
                thread::yield_now();
            }
        });
    }

    // start all the tasks
    for tx in txs.iter() {
        tx.send(()).unwrap();
    }
}
