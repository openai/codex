#![cfg(feature = "spin_no_std")]
#![no_std]

#[macro_use]
extern crate lazy_static;

lazy_static! {
    /// Documentation!
    pub static ref NUMBER: u32 = times_two(3);
}

fn times_two(n: u32) -> u32 {
    n * 2
}

#[test]
fn test_basic() {
    assert_eq!(*NUMBER, 6);
}
