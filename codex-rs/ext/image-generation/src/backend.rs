use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageResponse;
use codex_api::ImagesClient;
use codex_api::ReqwestTransport;
use codex_login::default_client::build_reqwest_client;
use codex_model_provider::SharedModelProvider;
use http::HeaderMap;

/// Executes standalone image API requests for the image-generation extension.
///
/// Implementations receive fully resolved API request values and must preserve the endpoint
/// response shape. Tests may implement this with deterministic in-memory responses.
pub(crate) trait ImageGenerationBackend: Clone + Send + Sync + 'static {
    fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> impl std::future::Future<Output = Result<ImageResponse, String>> + Send;

    fn edit(
        &self,
        request: ImageEditRequest,
    ) -> impl std::future::Future<Output = Result<ImageResponse, String>> + Send;
}

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
}

impl ImageGenerationBackend for CodexImagesBackend {
    /// Sends a standalone image generation request through the configured Images client.
    async fn generate(&self, request: ImageGenerationRequest) -> Result<ImageResponse, String> {
        self.client()
            .await?
            .generate(&request, HeaderMap::new())
            .await
            .map_err(|err| err.to_string())
    }

    /// Sends a standalone image edit request through the configured Images client.
    async fn edit(&self, request: ImageEditRequest) -> Result<ImageResponse, String> {
        self.client()
            .await?
            .edit(&request, HeaderMap::new())
            .await
            .map_err(|err| err.to_string())
    }
}
