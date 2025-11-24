pub trait Trait {
    async fn method(&self);
}

pub struct Struct;

impl Trait for Struct {
    async fn method(&self) {}
}

fn main() {
    let _: &dyn Trait;
}
