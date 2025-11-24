use async_trait::async_trait;

#[async_trait]
trait Trait {
    async fn f(&self);
}

struct Thing;

#[async_trait]
impl Trait for Thing {
    async fn f(&self);
}

fn main() {}
