// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;
use crate::error::ZeroTrieBuildError;
use alloc::vec;
use alloc::vec::Vec;

/// To speed up the search algorithm, we limit the number of times the level-2 parameter (q)
/// can hit its max value (initially Q_FAST_MAX) before we try the next level-1 parameter (p).
/// In practice, this has a small impact on the resulting perfect hash, resulting in about
/// 1 in 10000 hash maps that fall back to the slow path.
const MAX_L2_SEARCH_MISSES: usize = 24;

/// Directly compute the perfect hash function.
///
/// Returns `(p, [q_0, q_1, ..., q_(N-1)])`, or an error if the PHF could not be computed.
#[allow(unused_labels)] // for readability
pub fn find(bytes: &[u8]) -> Result<(u8, Vec<u8>), ZeroTrieBuildError> {
    let n_usize = bytes.len();

    let mut p = 0u8;
    let mut qq = vec![0u8; n_usize];

    let mut bqs = vec![0u8; n_usize];
    let mut seen = vec![false; n_usize];
    let max_allowable_p = P_FAST_MAX;
    let mut max_allowable_q = Q_FAST_MAX;

    #[allow(non_snake_case)]
    let N = if n_usize > 0 && n_usize < 256 {
        n_usize as u8
    } else {
        debug_assert!(n_usize == 0 || n_usize == 256);
        return Ok((p, qq));
    };

    'p_loop: loop {
        let mut buckets: Vec<(usize, Vec<u8>)> = (0..n_usize).map(|i| (i, vec![])).collect();
        for byte in bytes {
            let l1 = f1(*byte, p, N) as usize;
            buckets[l1].1.push(*byte);
        }
        buckets.sort_by_key(|(_, v)| -(v.len() as isize));
        // println!("New P: p={p:?}, buckets={buckets:?}");
        let mut i = 0;
        let mut num_max_q = 0;
        bqs.fill(0);
        seen.fill(false);
        'q_loop: loop {
            if i == buckets.len() {
                for (local_j, real_j) in buckets.iter().map(|(j, _)| *j).enumerate() {
                    qq[real_j] = bqs[local_j];
                }
                // println!("Success: p={p:?}, num_max_q={num_max_q:?}, bqs={bqs:?}, qq={qq:?}");
                // if num_max_q > 0 {
                //     println!("num_max_q={num_max_q:?}");
                // }
                return Ok((p, qq));
            }
            let mut bucket = buckets[i].1.as_slice();
            'byte_loop: for (j, byte) in bucket.iter().enumerate() {
                let l2 = f2(*byte, bqs[i], N) as usize;
                if seen[l2] {
                    // println!("Skipping Q: p={p:?}, i={i:?}, byte={byte:}, q={i:?}, l2={:?}", f2(*byte, bqs[i], N));
                    for k_byte in &bucket[0..j] {
                        let l2 = f2(*k_byte, bqs[i], N) as usize;
                        assert!(seen[l2]);
                        seen[l2] = false;
                    }
                    'reset_loop: loop {
                        if bqs[i] < max_allowable_q {
                            bqs[i] += 1;
                            continue 'q_loop;
                        }
                        num_max_q += 1;
                        bqs[i] = 0;
                        if i == 0 || num_max_q > MAX_L2_SEARCH_MISSES {
                            if p == max_allowable_p && max_allowable_q != Q_REAL_MAX {
                                // println!("Could not solve fast function: trying again: {bytes:?}");
                                max_allowable_q = Q_REAL_MAX;
                                p = 0;
                                continue 'p_loop;
                            } else if p == max_allowable_p {
                                // If a fallback algorithm for `p` is added, relax this assertion
                                // and re-run the loop with a higher `max_allowable_p`.
                                debug_assert_eq!(max_allowable_p, P_REAL_MAX);
                                // println!("Could not solve PHF function");
                                return Err(ZeroTrieBuildError::CouldNotSolvePerfectHash);
                            } else {
                                p += 1;
                                continue 'p_loop;
                            }
                        }
                        i -= 1;
                        bucket = buckets[i].1.as_slice();
                        for byte in bucket {
                            let l2 = f2(*byte, bqs[i], N) as usize;
                            assert!(seen[l2]);
                            seen[l2] = false;
                        }
                    }
                } else {
                    // println!("Marking as seen: i={i:?}, byte={byte:}, l2={:?}", f2(*byte, bqs[i], N));
                    let l2 = f2(*byte, bqs[i], N) as usize;
                    seen[l2] = true;
                }
            }
            // println!("Found Q: i={i:?}, q={:?}", bqs[i]);
            i += 1;
        }
    }
}

impl PerfectByteHashMap<Vec<u8>> {
    /// Computes a new [`PerfectByteHashMap`].
    ///
    /// (this is a doc-hidden API)
    pub fn try_new(keys: &[u8]) -> Result<Self, ZeroTrieBuildError> {
        let n_usize = keys.len();
        let n = n_usize as u8;
        let (p, mut qq) = find(keys)?;
        let mut keys_permuted = vec![0; n_usize];
        for key in keys {
            let l1 = f1(*key, p, n) as usize;
            let q = qq[l1];
            let l2 = f2(*key, q, n) as usize;
            keys_permuted[l2] = *key;
        }
        let mut result = Vec::with_capacity(n_usize * 2 + 1);
        result.push(p);
        result.append(&mut qq);
        result.append(&mut keys_permuted);
        Ok(Self(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;
    use std::print;
    use std::println;

    fn print_byte_to_stdout(byte: u8) {
        let c = char::from(byte);
        if c.is_ascii_alphanumeric() {
            print!("'{c}'");
        } else {
            print!("0x{byte:X}");
        }
    }

    fn random_alphanums(seed: u64, len: usize) -> Vec<u8> {
        use rand::seq::SliceRandom;
        use rand::SeedableRng;
        let mut bytes: Vec<u8> =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".into();
        let mut rng = rand_pcg::Lcg64Xsh32::seed_from_u64(seed);
        bytes.partial_shuffle(&mut rng, len).0.into()
    }

    #[test]
    fn test_random_distributions() {
        let mut p_distr = vec![0; 256];
        let mut q_distr = vec![0; 256];
        for len in 0..50 {
            for seed in 0..50 {
                let bytes = random_alphanums(seed, len);
                let (p, qq) = find(bytes.as_slice()).unwrap();
                p_distr[p as usize] += 1;
                for q in qq {
                    q_distr[q as usize] += 1;
                }
            }
        }
        println!("p_distr: {p_distr:?}");
        println!("q_distr: {q_distr:?}");

        let fast_p = p_distr[0..=P_FAST_MAX as usize].iter().sum::<usize>();
        let slow_p = p_distr[(P_FAST_MAX + 1) as usize..].iter().sum::<usize>();
        let fast_q = q_distr[0..=Q_FAST_MAX as usize].iter().sum::<usize>();
        let slow_q = q_distr[(Q_FAST_MAX + 1) as usize..].iter().sum::<usize>();

        assert_eq!(2500, fast_p);
        assert_eq!(0, slow_p);
        assert_eq!(61243, fast_q);
        assert_eq!(7, slow_q);

        let bytes = random_alphanums(0, 16);

        #[allow(non_snake_case)]
        let N = u8::try_from(bytes.len()).unwrap();

        let (p, qq) = find(bytes.as_slice()).unwrap();

        println!("Results:");
        for byte in bytes.iter() {
            print_byte_to_stdout(*byte);
            let l1 = f1(*byte, p, N) as usize;
            let q = qq[l1];
            let l2 = f2(*byte, q, N) as usize;
            println!(" => l1 {l1} => q {q} => l2 {l2}");
        }
    }
}
