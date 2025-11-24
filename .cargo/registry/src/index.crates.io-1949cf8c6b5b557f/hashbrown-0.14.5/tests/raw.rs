#![cfg(feature = "raw")]

use hashbrown::raw::RawTable;
use std::mem;

#[test]
fn test_allocation_info() {
    assert_eq!(RawTable::<()>::new().allocation_info().1.size(), 0);
    assert_eq!(RawTable::<u32>::new().allocation_info().1.size(), 0);
    assert!(RawTable::<u32>::with_capacity(1).allocation_info().1.size() > mem::size_of::<u32>());
}
