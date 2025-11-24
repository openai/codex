#![cfg(not(miri))] // FIXME: takes too long

use hashbrown::HashSet;
use rand::{distr::Alphanumeric, rngs::SmallRng, Rng, SeedableRng};
use std::iter;

#[test]
fn test_hashset_insert_remove() {
    let mut m: HashSet<Vec<char>> = HashSet::new();
    let seed = u64::from_le_bytes(*b"testseed");

    let rng = &mut SmallRng::seed_from_u64(seed);
    let tx: Vec<Vec<char>> = iter::repeat_with(|| {
        rng.sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    })
    .take(4096)
    .collect();

    // more readable with explicit `true` / `false`
    #[allow(clippy::bool_assert_comparison)]
    for _ in 0..32 {
        for x in &tx {
            assert_eq!(m.contains(x), false);
            assert_eq!(m.insert(x.clone()), true);
        }
        for (i, x) in tx.iter().enumerate() {
            println!("removing {i} {x:?}");
            assert_eq!(m.remove(x), true);
        }
    }
}
