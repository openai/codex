use core::fmt::Debug;
use thiserror::Error;

#[derive(Error, Debug)]
#[error("error")]
struct Error<'a>(#[from] Inner<'a>);

#[derive(Error, Debug)]
#[error("{0}")]
struct Inner<'a>(&'a str);

#[derive(Error, Debug)]
enum Enum<'a> {
    #[error("error")]
    Foo(#[from] Generic<&'a str>),
}

#[derive(Error, Debug)]
#[error("{0:?}")]
struct Generic<T: Debug>(T);

fn main() -> Result<(), Error<'static>> {
    Err(Error(Inner("some text")))
}
