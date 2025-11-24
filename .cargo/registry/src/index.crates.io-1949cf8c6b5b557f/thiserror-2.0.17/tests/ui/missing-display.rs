use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    First,
    Second,
}

fn main() {}
