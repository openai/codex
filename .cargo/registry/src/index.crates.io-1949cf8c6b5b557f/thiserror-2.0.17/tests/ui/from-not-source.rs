use thiserror::Error;

#[derive(Error, Debug)]
pub struct Error {
    #[source]
    source: std::io::Error,
    #[from]
    other: anyhow::Error,
}

fn main() {}
