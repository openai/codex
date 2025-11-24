use thiserror::Error;

#[derive(Debug)]
struct NoDisplay;

#[derive(Error, Debug)]
#[error("thread: {thread}")]
pub struct Error {
    thread: NoDisplay,
}

#[derive(Error, Debug)]
#[error("thread: {thread:o}")]
pub struct ErrorOctal {
    thread: NoDisplay,
}

fn main() {}
