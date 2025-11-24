#![allow(clippy::extra_unused_type_parameters)]

mod drop;

use self::drop::{DetectDrop, Flag};
use anyhow::Error;
use std::mem;

#[test]
fn test_error_size() {
    assert_eq!(mem::size_of::<Error>(), mem::size_of::<usize>());
}

#[test]
fn test_null_pointer_optimization() {
    assert_eq!(mem::size_of::<Result<(), Error>>(), mem::size_of::<usize>());
}

#[test]
fn test_autotraits() {
    fn assert<E: Unpin + Send + Sync + 'static>() {}
    assert::<Error>();
}

#[test]
fn test_drop() {
    let has_dropped = Flag::new();
    drop(Error::new(DetectDrop::new(&has_dropped)));
    assert!(has_dropped.get());
}
