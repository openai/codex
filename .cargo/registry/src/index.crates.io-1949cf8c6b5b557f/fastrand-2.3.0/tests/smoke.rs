#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn bool() {
    for x in &[false, true] {
        while fastrand::bool() != *x {}
    }
}

#[test]
fn u8() {
    for x in 0..10 {
        while fastrand::u8(..10) != x {}
    }

    for x in 200..=u8::MAX {
        while fastrand::u8(200..) != x {}
    }
}

#[test]
fn i8() {
    for x in -128..-120 {
        while fastrand::i8(..-120) != x {}
    }

    for x in 120..=127 {
        while fastrand::i8(120..) != x {}
    }
}

#[test]
fn u32() {
    for n in 1u32..10_000 {
        let n = n.wrapping_mul(n);
        let n = n.wrapping_mul(n);
        if n != 0 {
            for _ in 0..1000 {
                assert!(fastrand::u32(..n) < n);
            }
        }
    }
}

#[test]
fn u64() {
    for n in 1u64..10_000 {
        let n = n.wrapping_mul(n);
        let n = n.wrapping_mul(n);
        let n = n.wrapping_mul(n);
        if n != 0 {
            for _ in 0..1000 {
                assert!(fastrand::u64(..n) < n);
            }
        }
    }
}

#[test]
fn u128() {
    for n in 1u128..10_000 {
        let n = n.wrapping_mul(n);
        let n = n.wrapping_mul(n);
        let n = n.wrapping_mul(n);
        let n = n.wrapping_mul(n);
        if n != 0 {
            for _ in 0..1000 {
                assert!(fastrand::u128(..n) < n);
            }
        }
    }
}

#[test]
fn fill() {
    let mut r = fastrand::Rng::new();
    let mut a = [0u8; 64];
    let mut b = [0u8; 64];

    r.fill(&mut a);
    r.fill(&mut b);

    assert_ne!(a, b);
}

#[test]
fn rng() {
    let mut r = fastrand::Rng::new();

    assert_ne!(r.u64(..), r.u64(..));

    r.seed(7);
    let a = r.u64(..);
    r.seed(7);
    let b = r.u64(..);
    assert_eq!(a, b);
}

#[test]
fn rng_init() {
    let mut a = fastrand::Rng::new();
    let mut b = fastrand::Rng::new();
    assert_ne!(a.u64(..), b.u64(..));

    a.seed(7);
    b.seed(7);
    assert_eq!(a.u64(..), b.u64(..));
}

#[test]
fn with_seed() {
    let mut a = fastrand::Rng::with_seed(7);
    let mut b = fastrand::Rng::new();
    b.seed(7);
    assert_eq!(a.u64(..), b.u64(..));
}

#[test]
fn choose_multiple() {
    let mut a = fastrand::Rng::new();
    let mut elements = (0..20).collect::<Vec<_>>();

    while !elements.is_empty() {
        let chosen = a.choose_multiple(0..20, 5);
        for &x in &chosen {
            elements.retain(|&y| y != x);
        }
    }
}

#[test]
fn choice() {
    let items = [1, 4, 9, 5, 2, 3, 6, 7, 8, 0];
    let mut r = fastrand::Rng::new();

    for item in &items {
        while r.choice(&items).unwrap() != item {}
    }
}
