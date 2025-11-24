use http::Uri;

use crate::transport::{
    auth::AuthClient,
    sse_client::{SseClient, SseTransportError},
};
impl<C> SseClient for AuthClient<C>
where
    C: SseClient,
{
    type Error = SseTransportError<C::Error>;

    async fn post_message(
        &self,
        uri: Uri,
        message: crate::model::ClientJsonRpcMessage,
        mut auth_token: Option<String>,
    ) -> Result<(), SseTransportError<Self::Error>> {
        if auth_token.is_none() {
            auth_token = Some(self.get_access_token().await?);
        }
        self.http_client
            .post_message(uri, message, auth_token)
            .await
            .map_err(SseTransportError::Client)
    }

    async fn get_stream(
        &self,
        uri: Uri,
        last_event_id: Option<String>,
        mut auth_token: Option<String>,
    ) -> Result<
        crate::transport::common::client_side_sse::BoxedSseResponse,
        SseTransportError<Self::Error>,
    > {
        if auth_token.is_none() {
            auth_token = Some(self.get_access_token().await?);
        }
        self.http_client
            .get_stream(uri, last_event_id, auth_token)
            .await
            .map_err(SseTransportError::Client)
    }
}
