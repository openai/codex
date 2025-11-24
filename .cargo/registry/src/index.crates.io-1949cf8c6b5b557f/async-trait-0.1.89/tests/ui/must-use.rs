#![deny(unused_must_use)]

use async_trait::async_trait;

#[async_trait]
trait Interface {
    async fn f(&self);
}

struct Thing;

#[async_trait]
impl Interface for Thing {
    async fn f(&self) {}
}

pub async fn f() {
    Thing.f();
}

fn main() {}
