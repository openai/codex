use thiserror::Error;

#[derive(Error, Debug)]
#[error]
pub struct MyError;

fn main() {
    // No error on the following line. Thiserror emits an Error impl despite the
    // bad attribute.
    _ = &MyError as &dyn std::error::Error;
}
