#![deny(warnings)]

use async_trait::async_trait;

#[async_trait]
pub trait Trait {
    async fn f() {
        unimplemented!()
    }
}

#[async_trait]
pub trait TraitFoo {
    async fn f() {
        let _y = unimplemented!();
        let _z = _y;
    }
}

fn main() {}
