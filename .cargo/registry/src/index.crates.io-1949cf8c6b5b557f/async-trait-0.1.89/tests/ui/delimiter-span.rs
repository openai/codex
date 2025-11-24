#![allow(unused_macro_rules)]

use async_trait::async_trait;

macro_rules! picky {
    ($(t:tt)*) => {};
}

#[async_trait]
trait Trait {
    async fn method();
}

struct Struct;

#[async_trait]
impl Trait for Struct {
    async fn method() {
        picky!({ 123, self });
        picky!({ 123 });
    }
}

fn main() {}
