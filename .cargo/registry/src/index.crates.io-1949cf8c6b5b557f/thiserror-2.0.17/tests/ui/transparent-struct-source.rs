use thiserror::Error;

#[derive(Error, Debug)]
#[error(transparent)]
pub struct Error(#[source] anyhow::Error);

fn main() {}
