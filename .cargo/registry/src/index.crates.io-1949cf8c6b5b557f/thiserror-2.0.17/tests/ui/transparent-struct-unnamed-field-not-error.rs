use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]
pub struct Error(String);

fn main() {}
