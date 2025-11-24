use crate::{Digest, FixedOutput, FixedOutputReset, HashMarker, Update};
use core::fmt::Debug;

/// Fixed-output resettable digest test via the `Digest` trait
pub fn fixed_reset_test<D>(input: &[u8], output: &[u8]) -> Option<&'static str>
where
    D: FixedOutputReset + Debug + Clone + Default + Update + HashMarker,
{
    let mut hasher = D::new();
    // Test that it works when accepting the message all at once
    hasher.update(input);
    let mut hasher2 = hasher.clone();
    if hasher.finalize()[..] != output[..] {
        return Some("whole message");
    }

    // Test if reset works correctly
    hasher2.reset();
    hasher2.update(input);
    if hasher2.finalize_reset()[..] != output[..] {
        return Some("whole message after reset");
    }

    // Test that it works when accepting the message in chunks
    for n in 1..core::cmp::min(17, input.len()) {
        let mut hasher = D::new();
        for chunk in input.chunks(n) {
            hasher.update(chunk);
            hasher2.update(chunk);
        }
        if hasher.finalize()[..] != output[..] {
            return Some("message in chunks");
        }
        if hasher2.finalize_reset()[..] != output[..] {
            return Some("message in chunks");
        }
    }

    None
}

/// Variable-output resettable digest test
pub fn fixed_test<D>(input: &[u8], output: &[u8]) -> Option<&'static str>
where
    D: FixedOutput + Default + Debug + Clone,
{
    let mut hasher = D::default();
    // Test that it works when accepting the message all at once
    hasher.update(input);
    if hasher.finalize_fixed()[..] != output[..] {
        return Some("whole message");
    }

    // Test that it works when accepting the message in chunks
    for n in 1..core::cmp::min(17, input.len()) {
        let mut hasher = D::default();
        for chunk in input.chunks(n) {
            hasher.update(chunk);
        }
        if hasher.finalize_fixed()[..] != output[..] {
            return Some("message in chunks");
        }
    }
    None
}
