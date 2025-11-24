#![allow(dead_code)]

use core::fmt::{self, Display};
use std::io;
use thiserror::Error;

macro_rules! unimplemented_display {
    ($ty:ty) => {
        impl Display for $ty {
            fn fmt(&self, _formatter: &mut fmt::Formatter) -> fmt::Result {
                unimplemented!()
            }
        }
    };
}

#[derive(Error, Debug)]
struct BracedError {
    msg: String,
    pos: usize,
}

#[derive(Error, Debug)]
struct TupleError(String, usize);

#[derive(Error, Debug)]
struct UnitError;

#[derive(Error, Debug)]
struct WithSource {
    #[source]
    cause: io::Error,
}

#[derive(Error, Debug)]
struct WithAnyhow {
    #[source]
    cause: anyhow::Error,
}

#[derive(Error, Debug)]
enum EnumError {
    Braced {
        #[source]
        cause: io::Error,
    },
    Tuple(#[source] io::Error),
    Unit,
}

unimplemented_display!(BracedError);
unimplemented_display!(TupleError);
unimplemented_display!(UnitError);
unimplemented_display!(WithSource);
unimplemented_display!(WithAnyhow);
unimplemented_display!(EnumError);
