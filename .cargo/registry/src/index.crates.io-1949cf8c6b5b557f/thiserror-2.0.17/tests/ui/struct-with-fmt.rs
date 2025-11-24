use thiserror::Error;

#[derive(Error, Debug)]
#[error(fmt = core::fmt::Octal::fmt)]
pub struct Error(i32);

fn main() {}
