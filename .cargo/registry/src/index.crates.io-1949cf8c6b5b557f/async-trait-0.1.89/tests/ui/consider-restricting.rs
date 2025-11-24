// https://github.com/rust-lang/rust/issues/93828

use async_trait::async_trait;

pub trait IntoUrl {}

#[async_trait]
pub trait ClientExt {
    async fn publish<T: IntoUrl>(&self, url: T);
}

struct Client;

#[async_trait]
impl ClientExt for Client {
    async fn publish<T: IntoUrl>(&self, _url: T) {}
}

struct Client2;

#[async_trait]
impl ClientExt for Client2 {
    async fn publish<T>(&self, _url: T) {}
}

fn main() {}
