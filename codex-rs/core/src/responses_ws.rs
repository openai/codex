use crate::api_bridge::CoreAuthProvider;
use codex_api::Prompt as ApiPrompt;
use codex_api::Provider;
use codex_api::ResponseStream;
use codex_api::ResponsesOptions;
use codex_api::ResponsesWsSession;
use codex_api::error::ApiError;
use tokio::sync::Mutex;

pub struct ResponsesWsManager {
    session: Mutex<Option<ResponsesWsSession<CoreAuthProvider>>>,
    base_url: Mutex<Option<String>>,
}

impl std::fmt::Debug for ResponsesWsManager {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ResponsesWsManager").finish()
    }
}

impl ResponsesWsManager {
    pub(crate) fn new() -> Self {
        Self {
            session: Mutex::new(None),
            base_url: Mutex::new(None),
        }
    }

    pub(crate) async fn reset(&self) {
        {
            let mut guard = self.session.lock().await;
            *guard = None;
        }
        let mut base_url = self.base_url.lock().await;
        *base_url = None;
    }

    pub(crate) async fn stream_prompt(
        &self,
        provider: Provider,
        auth: CoreAuthProvider,
        model: &str,
        prompt: &ApiPrompt,
        options: ResponsesOptions,
    ) -> Result<ResponseStream, ApiError> {
        let should_reset = self
            .base_url
            .lock()
            .await
            .as_ref()
            .map(|url| url != &provider.base_url)
            .unwrap_or(false);
        if should_reset {
            self.reset().await;
        }

        let existing = { self.session.lock().await.clone() };
        let session = if let Some(session) = existing {
            session
        } else {
            let session = ResponsesWsSession::new(provider.clone(), auth);
            {
                let mut guard = self.session.lock().await;
                if guard.is_none() {
                    *guard = Some(session.clone());
                    let mut base_url = self.base_url.lock().await;
                    *base_url = Some(provider.base_url.clone());
                }
            }
            session
        };

        let stream = session.stream_prompt(model, prompt, options).await;
        if stream.is_err() {
            self.reset().await;
        }
        stream
    }
}
