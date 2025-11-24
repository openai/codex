use futures::Stream;
use thiserror::Error;

use super::{ServerSseMessage, SessionId, SessionManager};
use crate::{
    RoleServer,
    model::{ClientJsonRpcMessage, ServerJsonRpcMessage},
    transport::Transport,
};

#[derive(Debug, Clone, Error)]
#[error("Session management is not supported")]
pub struct ErrorSessionManagementNotSupported;
#[derive(Debug, Clone, Default)]
pub struct NeverSessionManager {}
pub enum NeverTransport {}
impl Transport<RoleServer> for NeverTransport {
    type Error = ErrorSessionManagementNotSupported;

    fn send(
        &mut self,
        _item: ServerJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        futures::future::ready(Err(ErrorSessionManagementNotSupported))
    }

    fn receive(&mut self) -> impl Future<Output = Option<ClientJsonRpcMessage>> {
        futures::future::ready(None)
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        Err(ErrorSessionManagementNotSupported)
    }
}

impl SessionManager for NeverSessionManager {
    type Error = ErrorSessionManagementNotSupported;
    type Transport = NeverTransport;

    fn create_session(
        &self,
    ) -> impl Future<Output = Result<(SessionId, Self::Transport), Self::Error>> + Send {
        futures::future::ready(Err(ErrorSessionManagementNotSupported))
    }

    fn initialize_session(
        &self,
        _id: &SessionId,
        _message: ClientJsonRpcMessage,
    ) -> impl Future<Output = Result<ServerJsonRpcMessage, Self::Error>> + Send {
        futures::future::ready(Err(ErrorSessionManagementNotSupported))
    }

    fn has_session(
        &self,
        _id: &SessionId,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send {
        futures::future::ready(Err(ErrorSessionManagementNotSupported))
    }

    fn close_session(
        &self,
        _id: &SessionId,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        futures::future::ready(Err(ErrorSessionManagementNotSupported))
    }

    fn create_stream(
        &self,
        _id: &SessionId,
        _message: ClientJsonRpcMessage,
    ) -> impl Future<
        Output = Result<impl Stream<Item = ServerSseMessage> + Send + 'static, Self::Error>,
    > + Send {
        futures::future::ready(Result::<futures::stream::Pending<_>, _>::Err(
            ErrorSessionManagementNotSupported,
        ))
    }
    fn create_standalone_stream(
        &self,
        _id: &SessionId,
    ) -> impl Future<
        Output = Result<impl Stream<Item = ServerSseMessage> + Send + 'static, Self::Error>,
    > + Send {
        futures::future::ready(Result::<futures::stream::Pending<_>, _>::Err(
            ErrorSessionManagementNotSupported,
        ))
    }
    fn resume(
        &self,
        _id: &SessionId,
        _last_event_id: String,
    ) -> impl Future<
        Output = Result<impl Stream<Item = ServerSseMessage> + Send + 'static, Self::Error>,
    > + Send {
        futures::future::ready(Result::<futures::stream::Pending<_>, _>::Err(
            ErrorSessionManagementNotSupported,
        ))
    }
    fn accept_message(
        &self,
        _id: &SessionId,
        _message: ClientJsonRpcMessage,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        futures::future::ready(Err(ErrorSessionManagementNotSupported))
    }
}
