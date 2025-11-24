use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]
pub struct Error(#[error(transparent)] std::io::Error);

fn main() {}
