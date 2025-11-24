use futures::Stream;

pub use crate::transport::common::server_side_http::SessionId;
use crate::{
    RoleServer,
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    transport::common::server_side_http::ServerSseMessage,
};

pub mod local;
pub mod never;

pub trait SessionManager: Send + Sync + 'static {
    type Error: std::error::Error + Send + 'static;
    type Transport: crate::transport::Transport<RoleServer>;
    /// Create a new session with the given id and configuration.
    fn create_session(
        &self,
    ) -> impl Future<Output = Result<(SessionId, Self::Transport), Self::Error>> + Send;
    fn initialize_session(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> impl Future<Output = Result<ServerJsonRpcMessage, Self::Error>> + Send;
    fn has_session(&self, id: &SessionId)
    -> impl Future<Output = Result<bool, Self::Error>> + Send;
    fn close_session(&self, id: &SessionId)
    -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn create_stream(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> impl Future<
        Output = Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error>,
    > + Send;
    fn accept_message(
        &self,
        id: &SessionId,
        message: ClientJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn create_standalone_stream(
        &self,
        id: &SessionId,
    ) -> impl Future<
        Output = Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error>,
    > + Send;
    fn resume(
        &self,
        id: &SessionId,
        last_event_id: String,
    ) -> impl Future<
        Output = Result<impl Stream<Item = ServerSseMessage> + Send + Sync + 'static, Self::Error>,
    > + Send;
}
