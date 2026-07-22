use codex_api::ImageEditRequest;
use codex_api::ImageGenerationRequest;
use codex_api::ImageResponse;
use codex_api::ImagesClient;
use codex_api::ReqwestTransport;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::create_client;
use codex_model_provider::SharedModelProvider;
use http::HeaderMap;

#[derive(Clone)]
pub(crate) struct CodexImagesBackend {
    provider: SharedModelProvider,
    originator: Option<String>,
}

impl CodexImagesBackend {
    /// Creates a backend that sends image requests through the active model provider.
    pub(crate) fn new(provider: SharedModelProvider, originator: Option<String>) -> Self {
        Self {
            provider,
            originator,
        }
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
            ReqwestTransport::from_http_client(create_client()),
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
            .generate(&request, image_request_headers(self.originator.as_deref()))
            .await
            .map_err(|err| err.to_string())
    }

    /// Sends a standalone image edit request through the configured Images client.
    pub(crate) async fn edit(&self, request: ImageEditRequest) -> Result<ImageResponse, String> {
        self.client()
            .await?
            .edit(&request, image_request_headers(self.originator.as_deref()))
            .await
            .map_err(|err| err.to_string())
    }
}

fn image_request_headers(originator: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(originator) = originator {
        add_originator_header(&mut headers, originator);
    }
    headers
}
