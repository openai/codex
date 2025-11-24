use ahash::{AHasher, RandomState};
use std::hash::{BuildHasher, Hash, Hasher};

#[macro_use]
extern crate no_panic;

#[inline(never)]
#[no_panic]
fn hash_test_final(num: i32, string: &str) -> (u64, u64) {
    use core::hash::Hasher;
    let mut hasher1 = RandomState::with_seeds(1, 2, 3, 4).build_hasher();
    let mut hasher2 = RandomState::with_seeds(3, 4, 5, 6).build_hasher();
    hasher1.write_i32(num);
    hasher2.write(string.as_bytes());
    (hasher1.finish(), hasher2.finish())
}

#[inline(never)]
fn hash_test_final_wrapper(num: i32, string: &str) {
    hash_test_final(num, string);
}

struct SimpleBuildHasher {
    hasher: AHasher,
}

impl SimpleBuildHasher {
    fn hash_one<T: Hash>(&self, x: T) -> u64
    where
        Self: Sized,
    {
        let mut hasher = self.build_hasher();
        x.hash(&mut hasher);
        hasher.finish()
    }
}

impl BuildHasher for SimpleBuildHasher {
    type Hasher = AHasher;

    fn build_hasher(&self) -> Self::Hasher {
        self.hasher.clone()
    }
}

#[inline(never)]
#[no_panic]
fn hash_test_specialize(num: i32, string: &str) -> (u64, u64) {
    let hasher1 = RandomState::with_seeds(1, 2, 3, 4).build_hasher();
    let hasher2 = RandomState::with_seeds(1, 2, 3, 4).build_hasher();
    (
        SimpleBuildHasher { hasher: hasher1 }.hash_one(num),
        SimpleBuildHasher { hasher: hasher2 }.hash_one(string.as_bytes()),
    )
}

#[inline(never)]
fn hash_test_random_wrapper(num: i32, string: &str) {
    hash_test_specialize(num, string);
}

#[inline(never)]
#[no_panic]
fn hash_test_random(num: i32, string: &str) -> (u64, u64) {
    let build_hasher1 = RandomState::with_seeds(1, 2, 3, 4);
    let build_hasher2 = RandomState::with_seeds(1, 2, 3, 4);
    (build_hasher1.hash_one(&num), build_hasher2.hash_one(string.as_bytes()))
}

#[inline(never)]
fn hash_test_specialize_wrapper(num: i32, string: &str) {
    hash_test_specialize(num, string);
}

#[test]
fn test_no_panic() {
    hash_test_final_wrapper(2, "Foo");
    hash_test_specialize_wrapper(2, "Bar");
    hash_test_random(2, "Baz");
    hash_test_random_wrapper(2, "Bat");
}
