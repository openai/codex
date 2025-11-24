use thiserror::Error;

#[derive(Debug)]
struct NoDisplay;

#[derive(Error, Debug)]
#[error("thread: {thread}")]
pub struct Error {
    thread: NoDisplay,
}

fn main() {}
