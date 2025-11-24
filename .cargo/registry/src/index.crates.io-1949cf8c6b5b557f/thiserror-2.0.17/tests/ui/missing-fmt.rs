use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("...")]
    A(usize),
    B(usize),
}

fn main() {}
