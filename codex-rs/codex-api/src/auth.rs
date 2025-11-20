use codex_client::Request;

#[async_trait::async_trait]
pub trait AuthProvider: Send + Sync {
    async fn bearer_token(&self) -> Option<String>;
    async fn account_id(&self) -> Option<String> {
        None
    }
}

pub(crate) fn add_auth_headers<A: AuthProvider>(auth: &A, req: &mut Request) -> Request {
    let mut headers = req.headers.clone();
    if let Some(token) = futures::executor::block_on(auth.bearer_token())
        && let Ok(header) = format!("Bearer {token}").parse()
    {
        let _ = headers.insert(http::header::AUTHORIZATION, header);
    }
    if let Some(account_id) = futures::executor::block_on(auth.account_id())
        && let Ok(header) = account_id.parse()
    {
        let _ = headers.insert("ChatGPT-Account-ID", header);
    }
    req.headers = headers;
    req.clone()
}
