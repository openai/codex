use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]
#[error(transparent)]
pub struct Error(anyhow::Error);

fn main() {}
