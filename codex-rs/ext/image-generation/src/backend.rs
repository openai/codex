use codex_api::ApiError;
use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageResponse;
use codex_api::ImagesClient;
use codex_api::ReqwestTransport;
use codex_api::TransportError;
use codex_login::default_client::build_reqwest_client;
use codex_model_provider::SharedModelProvider;
use http::HeaderMap;
use serde::Deserialize;

const MAX_ERROR_MESSAGE_CHARS: usize = 4000;

#[derive(Clone)]
pub(crate) struct CodexImagesBackend {
    provider: SharedModelProvider,
}

impl CodexImagesBackend {
    /// Creates a backend that sends image requests through the active model provider.
    pub(crate) fn new(provider: SharedModelProvider) -> Self {
        Self { provider }
    }

    /// Resolves the provider and auth required for the current image API request.
    async fn client(&self) -> Result<ImagesClient<ReqwestTransport>, String> {
        let provider = self
            .provider
            .api_provider()
            .await
            .map_err(|err| err.to_string())?;
        let auth = self
            .provider
            .api_auth()
            .await
            .map_err(|err| err.to_string())?;
        Ok(ImagesClient::new(
            ReqwestTransport::new(build_reqwest_client()),
            provider,
            auth,
        ))
    }

    /// Sends a standalone image generation request through the configured Images client.
    pub(crate) async fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageResponse, String> {
        self.client()
            .await?
            .generate(&request, HeaderMap::new())
            .await
            .map_err(image_api_error_message)
    }

    /// Sends a standalone image edit request through the configured Images client.
    pub(crate) async fn edit(&self, request: ImageEditRequest) -> Result<ImageResponse, String> {
        self.client()
            .await?
            .edit(&request, HeaderMap::new())
            .await
            .map_err(image_api_error_message)
    }
}

#[derive(Deserialize)]
struct ErrorPayload {
    error: Option<ErrorMessage>,
    detail: Option<ErrorDetail>,
}

#[derive(Deserialize)]
struct ErrorMessage {
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ErrorDetail {
    Message(String),
    Object { message: Option<String> },
}

fn image_api_error_message(err: ApiError) -> String {
    let message = if let ApiError::Transport(TransportError::Http {
        body: Some(body), ..
    }) = &err
    {
        if let Some(message) = parse_error_message(body) {
            message
        } else if !body.trim().is_empty() {
            body.trim().to_string()
        } else {
            err.to_string()
        }
    } else {
        err.to_string()
    };

    truncate_error_message(&message)
}

fn parse_error_message(body: &str) -> Option<String> {
    let payload = serde_json::from_str::<ErrorPayload>(body).ok()?;
    payload
        .error
        .and_then(|error| error.message)
        .or(match payload.detail {
            Some(ErrorDetail::Message(message)) => Some(message),
            Some(ErrorDetail::Object { message }) => message,
            None => None,
        })
        .filter(|message| !message.trim().is_empty())
}

fn truncate_error_message(message: &str) -> String {
    if message.chars().count() <= MAX_ERROR_MESSAGE_CHARS {
        return message.to_string();
    }

    message
        .chars()
        .take(MAX_ERROR_MESSAGE_CHARS - 1)
        .chain(std::iter::once('…'))
        .collect()
}

#[cfg(test)]
#[path = "backend_tests.rs"]
mod tests;
