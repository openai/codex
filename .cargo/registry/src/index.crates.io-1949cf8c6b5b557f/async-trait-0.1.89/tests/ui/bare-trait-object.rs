#![deny(bare_trait_objects)]

use async_trait::async_trait;

#[async_trait]
trait Trait {
    async fn f(&self);
}

#[async_trait]
impl Trait for Send + Sync {
    async fn f(&self) {}
}

fn main() {}
