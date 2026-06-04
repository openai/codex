use crate::auth::SharedAuthProvider;
use crate::endpoint::session::EndpointSession;
use crate::error::ApiError;
use crate::images::ImageEditRequest;
use crate::images::ImageGenerationRequest;
use crate::images::ImageResponse;
use crate::provider::Provider;
use codex_client::HttpTransport;
use http::HeaderMap;
use http::Method;
use serde::Serialize;
use serde_json::to_value;
use std::sync::Arc;

pub struct ImagesClient<T: HttpTransport> {
    session: EndpointSession<T>,
}

impl<T: HttpTransport> ImagesClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            session: EndpointSession::new(transport, provider, auth),
        }
    }

    pub async fn generate(
        &self,
        request: &ImageGenerationRequest,
        extra_headers: HeaderMap,
    ) -> Result<ImageResponse, ApiError> {
        self.post_image_request(
            "images/generations",
            request,
            extra_headers,
            "image generation",
        )
        .await
    }

    pub async fn edit(
        &self,
        request: &ImageEditRequest,
        extra_headers: HeaderMap,
    ) -> Result<ImageResponse, ApiError> {
        self.post_image_request("images/edits", request, extra_headers, "image edit")
            .await
    }

    async fn post_image_request<R: Serialize>(
        &self,
        path: &str,
        request: &R,
        extra_headers: HeaderMap,
        operation: &str,
    ) -> Result<ImageResponse, ApiError> {
        let body = to_value(request)
            .map_err(|e| ApiError::Stream(format!("failed to encode {operation} request: {e}")))?;
        let resp = self
            .session
            .execute(Method::POST, path, extra_headers, Some(body))
            .await?;
        serde_json::from_slice(&resp.body)
            .map_err(|e| ApiError::Stream(format!("failed to decode {operation} response: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthProvider;
    use crate::images::ImageBackground;
    use crate::images::ImageData;
    use crate::images::ImageQuality;
    use crate::images::ImageUrl;
    use crate::provider::RetryConfig;
    use async_trait::async_trait;
    use codex_client::Request;
    use codex_client::RequestBody;
    use codex_client::Response;
    use codex_client::StreamResponse;
    use codex_client::TransportError;
    use http::StatusCode;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Clone, Default)]
    struct DummyAuth;

    impl AuthProvider for DummyAuth {
        fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
    }

    #[derive(Clone)]
    struct CapturingTransport {
        last_request: Arc<Mutex<Option<Request>>>,
        response_body: Arc<Vec<u8>>,
    }

    impl CapturingTransport {
        fn new(response_body: Vec<u8>) -> Self {
            Self {
                last_request: Arc::new(Mutex::new(None)),
                response_body: Arc::new(response_body),
            }
        }
    }

    #[async_trait]
    impl HttpTransport for CapturingTransport {
        async fn execute(&self, req: Request) -> Result<Response, TransportError> {
            *self.last_request.lock().expect("lock request store") = Some(req);
            Ok(Response {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: self.response_body.as_ref().clone().into(),
            })
        }

        async fn stream(&self, _req: Request) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Build("stream should not run".to_string()))
        }
    }

    fn provider() -> Provider {
        Provider {
            name: "test".to_string(),
            base_url: "https://example.com/api/codex".to_string(),
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: Duration::from_millis(1),
                retry_429: false,
                retry_5xx: true,
                retry_transport: true,
            },
            stream_idle_timeout: Duration::from_secs(1),
        }
    }

    fn response_body() -> Vec<u8> {
        serde_json::to_vec(&json!({
            "created": 1778832973u64,
            "background": "opaque",
            "data": [{"b64_json": "REDACT"}],
            "output_format": "png",
            "quality": "medium",
            "size": "1024x1536",
            "usage": {
                "input_tokens": 1474,
                "input_tokens_details": {
                    "image_tokens": 1457,
                    "text_tokens": 17,
                },
                "output_tokens": 1372,
                "output_tokens_details": {
                    "image_tokens": 1372,
                    "text_tokens": 0,
                },
                "total_tokens": 2846,
            }
        }))
        .expect("serialize response")
    }

    fn expected_response() -> ImageResponse {
        ImageResponse {
            created: 1778832973,
            background: Some(ImageBackground::Opaque),
            data: vec![ImageData {
                b64_json: "REDACT".to_string(),
            }],
            quality: Some(ImageQuality::Medium),
            size: Some("1024x1536".to_string()),
        }
    }

    fn captured_request(transport: &CapturingTransport) -> Request {
        transport
            .last_request
            .lock()
            .expect("lock request store")
            .clone()
            .expect("request should be captured")
    }

    #[tokio::test]
    async fn generate_posts_typed_request_and_parses_image_response() {
        let transport = CapturingTransport::new(response_body());
        let client = ImagesClient::new(transport.clone(), provider(), Arc::new(DummyAuth));

        let response = client
            .generate(
                &ImageGenerationRequest {
                    prompt: "a red fox in a field".to_string(),
                    background: Some(ImageBackground::Opaque),
                    model: "gpt-image-1.5".to_string(),
                    n: None,
                    quality: Some(ImageQuality::Medium),
                    size: Some("1024x1536".to_string()),
                },
                HeaderMap::new(),
            )
            .await
            .expect("image generation request should succeed");

        assert_eq!(response, expected_response());

        let request = captured_request(&transport);
        assert_eq!(
            request.url,
            "https://example.com/api/codex/images/generations"
        );
        assert_eq!(
            request.body.as_ref().and_then(RequestBody::json),
            Some(&json!({
                "prompt": "a red fox in a field",
                "background": "opaque",
                "model": "gpt-image-1.5",
                "quality": "medium",
                "size": "1024x1536",
            }))
        );
    }

    #[tokio::test]
    async fn edit_posts_typed_request_and_parses_image_response() {
        let transport = CapturingTransport::new(response_body());
        let client = ImagesClient::new(transport.clone(), provider(), Arc::new(DummyAuth));

        let response = client
            .edit(
                &ImageEditRequest {
                    images: vec![ImageUrl {
                        image_url: "data:image/png;base64,Zm9v".to_string(),
                    }],
                    prompt: "add a red hat".to_string(),
                    background: None,
                    model: "gpt-image-1.5".to_string(),
                    n: None,
                    quality: None,
                    size: None,
                },
                HeaderMap::new(),
            )
            .await
            .expect("image edit request should succeed");

        assert_eq!(response, expected_response());

        let request = captured_request(&transport);
        assert_eq!(request.url, "https://example.com/api/codex/images/edits");
        assert_eq!(
            request.body.as_ref().and_then(RequestBody::json),
            Some(&json!({
                "images": [{"image_url": "data:image/png;base64,Zm9v"}],
                "prompt": "add a red hat",
                "model": "gpt-image-1.5",
            }))
        );
    }

    #[tokio::test]
    async fn image_response_requires_image_data() {
        let transport = CapturingTransport::new(
            serde_json::to_vec(&json!({"created": 1778832973u64})).expect("serialize response"),
        );
        let client = ImagesClient::new(transport, provider(), Arc::new(DummyAuth));

        let error = client
            .generate(
                &ImageGenerationRequest {
                    prompt: "a red fox in a field".to_string(),
                    background: None,
                    model: "gpt-image-1.5".to_string(),
                    n: None,
                    quality: None,
                    size: None,
                },
                HeaderMap::new(),
            )
            .await
            .expect_err("image response without data should fail");

        let ApiError::Stream(message) = error else {
            panic!("expected image response decode error");
        };
        assert!(
            message.starts_with("failed to decode image generation response: missing field `data`"),
            "{message}"
        );
    }
}
