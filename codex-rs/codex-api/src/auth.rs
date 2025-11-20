#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    async fn bearer_token(&self) -> Option<String>;
    async fn account_id(&self) -> Option<String> {
        None
    }
}
