use std::sync::Arc;

use futures::{Sink, Stream};
use tokio::sync::Mutex;

use super::{IntoTransport, Transport};
use crate::service::{RxJsonRpcMessage, ServiceRole, TxJsonRpcMessage};

pub struct SinkStreamTransport<Si, St> {
    stream: St,
    sink: Arc<Mutex<Si>>,
}

impl<Si, St> SinkStreamTransport<Si, St> {
    pub fn new(sink: Si, stream: St) -> Self {
        Self {
            stream,
            sink: Arc::new(Mutex::new(sink)),
        }
    }
}

impl<Role: ServiceRole, Si, St> Transport<Role> for SinkStreamTransport<Si, St>
where
    St: Send + Stream<Item = RxJsonRpcMessage<Role>> + Unpin,
    Si: Send + Sink<TxJsonRpcMessage<Role>> + Unpin + 'static,
    Si::Error: std::error::Error + Send + Sync + 'static,
{
    type Error = Si::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<Role>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        use futures::SinkExt;
        let lock = self.sink.clone();
        async move {
            let mut write = lock.lock().await;
            write.send(item).await
        }
    }

    fn receive(&mut self) -> impl Future<Output = Option<RxJsonRpcMessage<Role>>> {
        use futures::StreamExt;
        self.stream.next()
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub enum TransportAdapterSinkStream {}

impl<Role, Si, St> IntoTransport<Role, Si::Error, TransportAdapterSinkStream> for (Si, St)
where
    Role: ServiceRole,
    Si: Send + Sink<TxJsonRpcMessage<Role>> + Unpin + 'static,
    St: Send + Stream<Item = RxJsonRpcMessage<Role>> + Unpin + 'static,
    Si::Error: std::error::Error + Send + Sync + 'static,
{
    fn into_transport(self) -> impl Transport<Role, Error = Si::Error> + 'static {
        SinkStreamTransport::new(self.0, self.1)
    }
}

pub enum TransportAdapterAsyncCombinedRW {}
impl<Role, S> IntoTransport<Role, S::Error, TransportAdapterAsyncCombinedRW> for S
where
    Role: ServiceRole,
    S: Sink<TxJsonRpcMessage<Role>> + Stream<Item = RxJsonRpcMessage<Role>> + Send + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    fn into_transport(self) -> impl Transport<Role, Error = S::Error> + 'static {
        use futures::StreamExt;
        IntoTransport::<Role, S::Error, TransportAdapterSinkStream>::into_transport(self.split())
    }
}
