use async_trait::async_trait;

pub struct S {}

pub enum E {
    V {},
}

#[async_trait]
pub trait Trait {
    async fn method(self);
}

#[async_trait]
impl Trait for S {
    async fn method(self) {
        let _: () = self;
        let _: Self = Self;
    }
}

#[async_trait]
impl Trait for E {
    async fn method(self) {
        let _: () = self;
        let _: Self = Self::V;
    }
}

fn main() {}
