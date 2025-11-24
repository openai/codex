#![allow(clippy::extra_unused_type_parameters)]

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("...")]
pub struct ErrorStruct {
    #[from]
    source: io::Error,
}

#[derive(Error, Debug)]
#[error("...")]
pub struct ErrorStructOptional {
    #[from]
    source: Option<io::Error>,
}

#[derive(Error, Debug)]
#[error("...")]
pub struct ErrorTuple(#[from] io::Error);

#[derive(Error, Debug)]
#[error("...")]
pub struct ErrorTupleOptional(#[from] Option<io::Error>);

#[derive(Error, Debug)]
#[error("...")]
pub enum ErrorEnum {
    Test {
        #[from]
        source: io::Error,
    },
}

#[derive(Error, Debug)]
#[error("...")]
pub enum ErrorEnumOptional {
    Test {
        #[from]
        source: Option<io::Error>,
    },
}

#[derive(Error, Debug)]
#[error("...")]
pub enum Many {
    Any(#[from] anyhow::Error),
    Io(#[from] io::Error),
}

fn assert_impl<T: From<io::Error>>() {}

#[test]
fn test_from() {
    assert_impl::<ErrorStruct>();
    assert_impl::<ErrorStructOptional>();
    assert_impl::<ErrorTuple>();
    assert_impl::<ErrorTupleOptional>();
    assert_impl::<ErrorEnum>();
    assert_impl::<ErrorEnumOptional>();
    assert_impl::<Many>();
}
