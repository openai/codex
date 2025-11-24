#![allow(dead_code)]
#![allow(clippy::module_name_repetitions, clippy::shadow_unrelated)]

use rayon::ThreadPoolBuilder;
use std::env;

pub mod eq;
pub mod parse;

/// Read the `ABORT_AFTER_FAILURE` environment variable, and parse it.
pub fn abort_after() -> usize {
    match env::var("ABORT_AFTER_FAILURE") {
        Ok(s) => s.parse().expect("failed to parse ABORT_AFTER_FAILURE"),
        Err(_) => usize::max_value(),
    }
}

/// Configure Rayon threadpool.
pub fn rayon_init() {
    let stack_size = match env::var("RUST_MIN_STACK") {
        Ok(s) => s.parse().expect("failed to parse RUST_MIN_STACK"),
        Err(_) => 20 * 1024 * 1024,
    };
    ThreadPoolBuilder::new()
        .stack_size(stack_size)
        .build_global()
        .unwrap();
}
