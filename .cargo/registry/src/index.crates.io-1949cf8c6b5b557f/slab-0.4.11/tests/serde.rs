#![cfg(feature = "serde")]
#![warn(rust_2018_idioms)]

use serde::{Deserialize, Serialize};
use serde_test::{assert_tokens, Token};
use slab::Slab;

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct SlabPartialEq<T>(Slab<T>);

impl<T: PartialEq> PartialEq for SlabPartialEq<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self
                .0
                .iter()
                .zip(other.0.iter())
                .all(|(this, other)| this.0 == other.0 && this.1 == other.1)
    }
}

#[test]
fn test_serde_empty() {
    let slab = Slab::<usize>::new();
    assert_tokens(
        &SlabPartialEq(slab),
        &[Token::Map { len: Some(0) }, Token::MapEnd],
    );
}

#[test]
fn test_serde() {
    let vec = vec![(1, 2), (3, 4), (5, 6)];
    let slab: Slab<_> = vec.iter().cloned().collect();
    assert_tokens(
        &SlabPartialEq(slab),
        &[
            Token::Map { len: Some(3) },
            Token::U64(1),
            Token::I32(2),
            Token::U64(3),
            Token::I32(4),
            Token::U64(5),
            Token::I32(6),
            Token::MapEnd,
        ],
    );
}
