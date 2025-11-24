use crate::transport::{
    auth::AuthClient,
    streamable_http_client::{StreamableHttpClient, StreamableHttpError},
};
impl<C> StreamableHttpClient for AuthClient<C>
where
    C: StreamableHttpClient + Send + Sync,
{
    type Error = C::Error;

    async fn delete_session(
        &self,
        uri: std::sync::Arc<str>,
        session_id: std::sync::Arc<str>,
        mut auth_token: Option<String>,
    ) -> Result<(), crate::transport::streamable_http_client::StreamableHttpError<Self::Error>>
    {
        if auth_token.is_none() {
            auth_token = Some(self.get_access_token().await?);
        }
        self.http_client
            .delete_session(uri, session_id, auth_token)
            .await
    }

    async fn get_stream(
        &self,
        uri: std::sync::Arc<str>,
        session_id: std::sync::Arc<str>,
        last_event_id: Option<String>,
        mut auth_token: Option<String>,
    ) -> Result<
        futures::stream::BoxStream<'static, Result<sse_stream::Sse, sse_stream::Error>>,
        crate::transport::streamable_http_client::StreamableHttpError<Self::Error>,
    > {
        if auth_token.is_none() {
            auth_token = Some(self.get_access_token().await?);
        }
        self.http_client
            .get_stream(uri, session_id, last_event_id, auth_token)
            .await
    }

    async fn post_message(
        &self,
        uri: std::sync::Arc<str>,
        message: crate::model::ClientJsonRpcMessage,
        session_id: Option<std::sync::Arc<str>>,
        mut auth_token: Option<String>,
    ) -> Result<
        crate::transport::streamable_http_client::StreamableHttpPostResponse,
        StreamableHttpError<Self::Error>,
    > {
        if auth_token.is_none() {
            auth_token = Some(self.get_access_token().await?);
        }
        self.http_client
            .post_message(uri, message, session_id, auth_token)
            .await
    }
}
