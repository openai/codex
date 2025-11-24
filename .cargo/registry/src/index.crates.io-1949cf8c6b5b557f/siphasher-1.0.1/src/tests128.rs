// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::hash::{Hash, Hasher};

use super::sip128::{Hasher128, SipHasher, SipHasher13, SipHasher24};

// Hash just the bytes of the slice, without length prefix
struct Bytes<'a>(&'a [u8]);

impl<'a> Hash for Bytes<'a> {
    #[allow(unused_must_use)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let Bytes(v) = *self;
        state.write(v);
    }
}

fn hash_with<H: Hasher + Hasher128, T: Hash>(mut st: H, x: &T) -> [u8; 16] {
    x.hash(&mut st);
    st.finish128().as_bytes()
}

fn hash<T: Hash>(x: &T) -> [u8; 16] {
    hash_with(SipHasher::new(), x)
}

#[test]
fn test_siphash128_idempotent() {
    let val64 = 0xdead_beef_dead_beef_u64;
    assert_eq!(hash(&val64), hash(&val64));
    let val32 = 0xdeadbeef_u32;
    assert_eq!(hash(&val32), hash(&val32));
}

#[test]
#[allow(unused_must_use)]
fn test_siphash128_1_3() {
    let vecs: [[u8; 16]; 1] = [[
        231, 126, 188, 178, 39, 136, 165, 190, 253, 98, 219, 106, 221, 48, 48, 1,
    ]];

    let k0 = 0x_07_06_05_04_03_02_01_00;
    let k1 = 0x_0f_0e_0d_0c_0b_0a_09_08;
    let mut buf = Vec::new();
    let mut t = 0;
    let mut state_inc = SipHasher13::new_with_keys(k0, k1);

    while t < 1 {
        let vec = vecs[t];
        let out = hash_with(SipHasher13::new_with_keys(k0, k1), &Bytes(&buf));
        assert_eq!(vec, out[..]);

        let full = hash_with(SipHasher13::new_with_keys(k0, k1), &Bytes(&buf));
        let i = state_inc.finish128().as_bytes();

        assert_eq!(full, i);
        assert_eq!(full, vec);

        buf.push(t as u8);
        Hasher::write(&mut state_inc, &[t as u8]);

        t += 1;
    }
}

#[test]
#[allow(unused_must_use)]
fn test_siphash128_2_4() {
    let vecs: [[u8; 16]; 1] = [[
        163, 129, 127, 4, 186, 37, 168, 230, 109, 246, 114, 20, 199, 85, 2, 147,
    ]];

    let k0 = 0x_07_06_05_04_03_02_01_00;
    let k1 = 0x_0f_0e_0d_0c_0b_0a_09_08;
    let mut buf = Vec::new();
    let mut t = 0;
    let mut state_inc = SipHasher24::new_with_keys(k0, k1);

    while t < 1 {
        let vec = vecs[t];
        let out = hash_with(SipHasher24::new_with_keys(k0, k1), &Bytes(&buf));
        assert_eq!(vec, out[..]);

        let full = hash_with(SipHasher24::new_with_keys(k0, k1), &Bytes(&buf));
        let i = state_inc.finish128().as_bytes();

        assert_eq!(full, i);
        assert_eq!(full, vec);

        buf.push(t as u8);
        Hasher::write(&mut state_inc, &[t as u8]);

        t += 1;
    }
}

#[test]
fn test_siphash128_simple() {
    let array: &[u8] = &[1, 2, 3];
    let key: &[u8; 16] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let hasher = SipHasher13::new_with_key(key);
    let h = hasher.hash(array).as_bytes();
    _ = h;
}

#[test]
fn test_siphash128_incremental() {
    let array1: &[u8] = &[1, 2, 3];
    let array2: &[u8] = &[4, 5, 6];
    let key: &[u8; 16] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let mut hasher = SipHasher13::new_with_key(key);
    hasher.write(array1);
    hasher.write(array2);
    let h = hasher.finish128().as_bytes();
    _ = h;
}

#[test]
#[cfg(all(feature = "serde", feature = "serde_json"))]
fn test_siphash128_serde() {
    let val64 = 0xdead_beef_dead_beef_u64;
    let hash = hash(&val64);
    let serialized = serde_json::to_string(&hash).unwrap();
    let deserialized: [u8; 16] = serde_json::from_str(&serialized).unwrap();
    assert_eq!(hash, deserialized);
}
