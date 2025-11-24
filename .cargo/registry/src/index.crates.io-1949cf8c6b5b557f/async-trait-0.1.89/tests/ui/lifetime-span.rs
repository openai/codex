use async_trait::async_trait;

struct A;
struct B;

#[async_trait]
pub trait Trait<'r> {
    async fn method(&'r self);
}

#[async_trait]
impl Trait for A {
    async fn method(&self) {}
}

#[async_trait]
impl<'r> Trait<'r> for B {
    async fn method(&self) {}
}

#[async_trait]
pub trait Trait2 {
    async fn method<'r>(&'r self);
}

#[async_trait]
impl Trait2 for A {
    async fn method(&self) {}
}

#[async_trait]
impl<'r> Trait2<'r> for B {
    async fn method(&'r self) {}
}

fn main() {}
