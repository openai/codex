use async_trait::async_trait;
use std::sync::Mutex;

async fn f() {}

#[async_trait]
trait Test {
    async fn test(&self) {
        let mutex = Mutex::new(());
        let _guard = mutex.lock().unwrap();
        f().await;
    }

    async fn test_ret(&self) -> bool {
        let mutex = Mutex::new(());
        let _guard = mutex.lock().unwrap();
        f().await;
        true
    }
}

fn main() {}
