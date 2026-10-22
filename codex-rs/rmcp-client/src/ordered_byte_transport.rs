//! Preserve byte-stream message order even when rmcp polls send futures out of order.
//! Only wrap transports whose send completes after writing, without awaiting a response.

use std::future::Future;
use std::io;
use std::sync::Arc;

use futures::FutureExt;
use rmcp::RoleClient;
use rmcp::service::RxJsonRpcMessage;
use rmcp::service::TxJsonRpcMessage;
use rmcp::transport::Transport;
use tokio::sync::Semaphore;

pub(crate) struct OrderedByteTransport<T> {
    inner: T,
    writes: Arc<Semaphore>,
}

impl<T> OrderedByteTransport<T> {
    pub(crate) fn new(inner: T) -> Self {
        Self {
            inner,
            writes: Arc::new(Semaphore::new(1)),
        }
    }
}

impl<T: Transport<RoleClient, Error = io::Error>> Transport<RoleClient>
    for OrderedByteTransport<T>
{
    type Error = io::Error;

    fn send(
        &mut self,
        item: TxJsonRpcMessage<RoleClient>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + 'static {
        // Reserve FIFO order now, before rmcp independently schedules each send.
        // Ignore cooperative budgeting for this one poll so it always queues.
        let mut acquire = Box::pin(Arc::clone(&self.writes).acquire_owned());
        let acquired = tokio::task::unconstrained(acquire.as_mut()).now_or_never();
        let send = self.inner.send(item);
        async move {
            let _permit = match acquired {
                Some(result) => result,
                None => acquire.await,
            }
            .map_err(io::Error::other)?;
            // Serialize only writes, never the corresponding tool responses.
            send.await
        }
    }

    async fn receive(&mut self) -> Option<RxJsonRpcMessage<RoleClient>> {
        self.inner.receive().await
    }

    async fn close(&mut self) -> Result<(), Self::Error> {
        self.inner.close().await
    }
}

#[cfg(test)]
#[path = "ordered_byte_transport_tests.rs"]
mod tests;
