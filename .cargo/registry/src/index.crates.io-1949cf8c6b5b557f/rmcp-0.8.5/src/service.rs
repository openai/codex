use futures::{FutureExt, future::BoxFuture};
use thiserror::Error;

use crate::{
    error::ErrorData as McpError,
    model::{
        CancelledNotification, CancelledNotificationParam, Extensions, GetExtensions, GetMeta,
        JsonRpcError, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, Meta,
        NumberOrString, ProgressToken, RequestId, ServerJsonRpcMessage,
    },
    transport::{DynamicTransportError, IntoTransport, Transport},
};
#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
mod client;
#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use client::*;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
mod server;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use server::*;
#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
mod tower;
use tokio_util::sync::{CancellationToken, DropGuard};
#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
pub use tower::*;
use tracing::{Instrument as _, instrument};
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ServiceError {
    #[error("Mcp error: {0}")]
    McpError(McpError),
    #[error("Transport send error: {0}")]
    TransportSend(DynamicTransportError),
    #[error("Transport closed")]
    TransportClosed,
    #[error("Unexpected response type")]
    UnexpectedResponse,
    #[error("task cancelled for reason {}", reason.as_deref().unwrap_or("<unknown>"))]
    Cancelled { reason: Option<String> },
    #[error("request timeout after {}", chrono::Duration::from_std(*timeout).unwrap_or_default())]
    Timeout { timeout: Duration },
}

trait TransferObject:
    std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static
{
}

impl<T> TransferObject for T where
    T: std::fmt::Debug
        + serde::Serialize
        + serde::de::DeserializeOwned
        + Send
        + Sync
        + 'static
        + Clone
{
}

#[allow(private_bounds, reason = "there's no the third implementation")]
pub trait ServiceRole: std::fmt::Debug + Send + Sync + 'static + Copy + Clone {
    type Req: TransferObject + GetMeta + GetExtensions;
    type Resp: TransferObject;
    type Not: TryInto<CancelledNotification, Error = Self::Not>
        + From<CancelledNotification>
        + TransferObject;
    type PeerReq: TransferObject + GetMeta + GetExtensions;
    type PeerResp: TransferObject;
    type PeerNot: TryInto<CancelledNotification, Error = Self::PeerNot>
        + From<CancelledNotification>
        + TransferObject
        + GetMeta
        + GetExtensions;
    type InitializeError;
    const IS_CLIENT: bool;
    type Info: TransferObject;
    type PeerInfo: TransferObject;
}

pub type TxJsonRpcMessage<R> =
    JsonRpcMessage<<R as ServiceRole>::Req, <R as ServiceRole>::Resp, <R as ServiceRole>::Not>;
pub type RxJsonRpcMessage<R> = JsonRpcMessage<
    <R as ServiceRole>::PeerReq,
    <R as ServiceRole>::PeerResp,
    <R as ServiceRole>::PeerNot,
>;

pub trait Service<R: ServiceRole>: Send + Sync + 'static {
    fn handle_request(
        &self,
        request: R::PeerReq,
        context: RequestContext<R>,
    ) -> impl Future<Output = Result<R::Resp, McpError>> + Send + '_;
    fn handle_notification(
        &self,
        notification: R::PeerNot,
        context: NotificationContext<R>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_;
    fn get_info(&self) -> R::Info;
}

pub trait ServiceExt<R: ServiceRole>: Service<R> + Sized {
    /// Convert this service to a dynamic boxed service
    ///
    /// This could be very helpful when you want to store the services in a collection
    fn into_dyn(self) -> Box<dyn DynService<R>> {
        Box::new(self)
    }
    fn serve<T, E, A>(
        self,
        transport: T,
    ) -> impl Future<Output = Result<RunningService<R, Self>, R::InitializeError>> + Send
    where
        T: IntoTransport<R, E, A>,
        E: std::error::Error + Send + Sync + 'static,
        Self: Sized,
    {
        Self::serve_with_ct(self, transport, Default::default())
    }
    fn serve_with_ct<T, E, A>(
        self,
        transport: T,
        ct: CancellationToken,
    ) -> impl Future<Output = Result<RunningService<R, Self>, R::InitializeError>> + Send
    where
        T: IntoTransport<R, E, A>,
        E: std::error::Error + Send + Sync + 'static,
        Self: Sized;
}

impl<R: ServiceRole> Service<R> for Box<dyn DynService<R>> {
    fn handle_request(
        &self,
        request: R::PeerReq,
        context: RequestContext<R>,
    ) -> impl Future<Output = Result<R::Resp, McpError>> + Send + '_ {
        DynService::handle_request(self.as_ref(), request, context)
    }

    fn handle_notification(
        &self,
        notification: R::PeerNot,
        context: NotificationContext<R>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        DynService::handle_notification(self.as_ref(), notification, context)
    }

    fn get_info(&self) -> R::Info {
        DynService::get_info(self.as_ref())
    }
}

pub trait DynService<R: ServiceRole>: Send + Sync {
    fn handle_request(
        &self,
        request: R::PeerReq,
        context: RequestContext<R>,
    ) -> BoxFuture<'_, Result<R::Resp, McpError>>;
    fn handle_notification(
        &self,
        notification: R::PeerNot,
        context: NotificationContext<R>,
    ) -> BoxFuture<'_, Result<(), McpError>>;
    fn get_info(&self) -> R::Info;
}

impl<R: ServiceRole, S: Service<R>> DynService<R> for S {
    fn handle_request(
        &self,
        request: R::PeerReq,
        context: RequestContext<R>,
    ) -> BoxFuture<'_, Result<R::Resp, McpError>> {
        Box::pin(self.handle_request(request, context))
    }
    fn handle_notification(
        &self,
        notification: R::PeerNot,
        context: NotificationContext<R>,
    ) -> BoxFuture<'_, Result<(), McpError>> {
        Box::pin(self.handle_notification(notification, context))
    }
    fn get_info(&self) -> R::Info {
        self.get_info()
    }
}

use std::{
    collections::{HashMap, VecDeque},
    ops::Deref,
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};

use tokio::sync::mpsc;

pub trait RequestIdProvider: Send + Sync + 'static {
    fn next_request_id(&self) -> RequestId;
}

pub trait ProgressTokenProvider: Send + Sync + 'static {
    fn next_progress_token(&self) -> ProgressToken;
}

pub type AtomicU32RequestIdProvider = AtomicU32Provider;
pub type AtomicU32ProgressTokenProvider = AtomicU32Provider;

#[derive(Debug, Default)]
pub struct AtomicU32Provider {
    id: AtomicU64,
}

impl RequestIdProvider for AtomicU32Provider {
    fn next_request_id(&self) -> RequestId {
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Safe conversion: we start at 0 and increment by 1, so we won't overflow i64::MAX in practice
        RequestId::Number(id as i64)
    }
}

impl ProgressTokenProvider for AtomicU32Provider {
    fn next_progress_token(&self) -> ProgressToken {
        let id = self.id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        ProgressToken(NumberOrString::Number(id as i64))
    }
}

type Responder<T> = tokio::sync::oneshot::Sender<T>;

/// A handle to a remote request
///
/// You can cancel it by call [`RequestHandle::cancel`] with a reason,
///
/// or wait for response by call [`RequestHandle::await_response`]
#[derive(Debug)]
pub struct RequestHandle<R: ServiceRole> {
    pub rx: tokio::sync::oneshot::Receiver<Result<R::PeerResp, ServiceError>>,
    pub options: PeerRequestOptions,
    pub peer: Peer<R>,
    pub id: RequestId,
    pub progress_token: ProgressToken,
}

impl<R: ServiceRole> RequestHandle<R> {
    pub const REQUEST_TIMEOUT_REASON: &str = "request timeout";
    pub async fn await_response(self) -> Result<R::PeerResp, ServiceError> {
        if let Some(timeout) = self.options.timeout {
            let timeout_result = tokio::time::timeout(timeout, async move {
                self.rx.await.map_err(|_e| ServiceError::TransportClosed)?
            })
            .await;
            match timeout_result {
                Ok(response) => response,
                Err(_) => {
                    let error = Err(ServiceError::Timeout { timeout });
                    // cancel this request
                    let notification = CancelledNotification {
                        params: CancelledNotificationParam {
                            request_id: self.id,
                            reason: Some(Self::REQUEST_TIMEOUT_REASON.to_owned()),
                        },
                        method: crate::model::CancelledNotificationMethod,
                        extensions: Default::default(),
                    };
                    let _ = self.peer.send_notification(notification.into()).await;
                    error
                }
            }
        } else {
            self.rx.await.map_err(|_e| ServiceError::TransportClosed)?
        }
    }

    /// Cancel this request
    pub async fn cancel(self, reason: Option<String>) -> Result<(), ServiceError> {
        let notification = CancelledNotification {
            params: CancelledNotificationParam {
                request_id: self.id,
                reason,
            },
            method: crate::model::CancelledNotificationMethod,
            extensions: Default::default(),
        };
        self.peer.send_notification(notification.into()).await?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum PeerSinkMessage<R: ServiceRole> {
    Request {
        request: R::Req,
        id: RequestId,
        responder: Responder<Result<R::PeerResp, ServiceError>>,
    },
    Notification {
        notification: R::Not,
        responder: Responder<Result<(), ServiceError>>,
    },
}

/// An interface to fetch the remote client or server
///
/// For general purpose, call [`Peer::send_request`] or [`Peer::send_notification`] to send message to remote peer.
///
/// To create a cancellable request, call [`Peer::send_request_with_option`].
#[derive(Clone)]
pub struct Peer<R: ServiceRole> {
    tx: mpsc::Sender<PeerSinkMessage<R>>,
    request_id_provider: Arc<dyn RequestIdProvider>,
    progress_token_provider: Arc<dyn ProgressTokenProvider>,
    info: Arc<tokio::sync::OnceCell<R::PeerInfo>>,
}

impl<R: ServiceRole> std::fmt::Debug for Peer<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerSink")
            .field("tx", &self.tx)
            .field("is_client", &R::IS_CLIENT)
            .finish()
    }
}

type ProxyOutbound<R> = mpsc::Receiver<PeerSinkMessage<R>>;

#[derive(Debug, Default)]
pub struct PeerRequestOptions {
    pub timeout: Option<Duration>,
    pub meta: Option<Meta>,
}

impl PeerRequestOptions {
    pub fn no_options() -> Self {
        Self::default()
    }
}

impl<R: ServiceRole> Peer<R> {
    const CLIENT_CHANNEL_BUFFER_SIZE: usize = 1024;
    pub(crate) fn new(
        request_id_provider: Arc<dyn RequestIdProvider>,
        peer_info: Option<R::PeerInfo>,
    ) -> (Peer<R>, ProxyOutbound<R>) {
        let (tx, rx) = mpsc::channel(Self::CLIENT_CHANNEL_BUFFER_SIZE);
        (
            Self {
                tx,
                request_id_provider,
                progress_token_provider: Arc::new(AtomicU32ProgressTokenProvider::default()),
                info: Arc::new(tokio::sync::OnceCell::new_with(peer_info)),
            },
            rx,
        )
    }
    pub async fn send_notification(&self, notification: R::Not) -> Result<(), ServiceError> {
        let (responder, receiver) = tokio::sync::oneshot::channel();
        self.tx
            .send(PeerSinkMessage::Notification {
                notification,
                responder,
            })
            .await
            .map_err(|_m| ServiceError::TransportClosed)?;
        receiver.await.map_err(|_e| ServiceError::TransportClosed)?
    }
    pub async fn send_request(&self, request: R::Req) -> Result<R::PeerResp, ServiceError> {
        self.send_request_with_option(request, PeerRequestOptions::no_options())
            .await?
            .await_response()
            .await
    }

    pub async fn send_cancellable_request(
        &self,
        request: R::Req,
        options: PeerRequestOptions,
    ) -> Result<RequestHandle<R>, ServiceError> {
        self.send_request_with_option(request, options).await
    }

    pub async fn send_request_with_option(
        &self,
        mut request: R::Req,
        options: PeerRequestOptions,
    ) -> Result<RequestHandle<R>, ServiceError> {
        let id = self.request_id_provider.next_request_id();
        let progress_token = self.progress_token_provider.next_progress_token();
        request
            .get_meta_mut()
            .set_progress_token(progress_token.clone());
        if let Some(meta) = options.meta.clone() {
            request.get_meta_mut().extend(meta);
        }
        let (responder, receiver) = tokio::sync::oneshot::channel();
        self.tx
            .send(PeerSinkMessage::Request {
                request,
                id: id.clone(),
                responder,
            })
            .await
            .map_err(|_m| ServiceError::TransportClosed)?;
        Ok(RequestHandle {
            id,
            rx: receiver,
            progress_token,
            options,
            peer: self.clone(),
        })
    }
    pub fn peer_info(&self) -> Option<&R::PeerInfo> {
        self.info.get()
    }

    pub fn set_peer_info(&self, info: R::PeerInfo) {
        if self.info.initialized() {
            tracing::warn!("trying to set peer info, which is already initialized");
        } else {
            let _ = self.info.set(info);
        }
    }

    pub fn is_transport_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

#[derive(Debug)]
pub struct RunningService<R: ServiceRole, S: Service<R>> {
    service: Arc<S>,
    peer: Peer<R>,
    handle: tokio::task::JoinHandle<QuitReason>,
    cancellation_token: CancellationToken,
    dg: DropGuard,
}
impl<R: ServiceRole, S: Service<R>> Deref for RunningService<R, S> {
    type Target = Peer<R>;

    fn deref(&self) -> &Self::Target {
        &self.peer
    }
}

impl<R: ServiceRole, S: Service<R>> RunningService<R, S> {
    #[inline]
    pub fn peer(&self) -> &Peer<R> {
        &self.peer
    }
    #[inline]
    pub fn service(&self) -> &S {
        self.service.as_ref()
    }
    #[inline]
    pub fn cancellation_token(&self) -> RunningServiceCancellationToken {
        RunningServiceCancellationToken(self.cancellation_token.clone())
    }
    #[inline]
    pub async fn waiting(self) -> Result<QuitReason, tokio::task::JoinError> {
        self.handle.await
    }
    pub async fn cancel(self) -> Result<QuitReason, tokio::task::JoinError> {
        let RunningService { dg, handle, .. } = self;
        dg.disarm().cancel();
        handle.await
    }
}

// use a wrapper type so we can tweak the implementation if needed
pub struct RunningServiceCancellationToken(CancellationToken);

impl RunningServiceCancellationToken {
    pub fn cancel(self) {
        self.0.cancel();
    }
}

#[derive(Debug)]
pub enum QuitReason {
    Cancelled,
    Closed,
    JoinError(tokio::task::JoinError),
}

/// Request execution context
#[derive(Debug, Clone)]
pub struct RequestContext<R: ServiceRole> {
    /// this token will be cancelled when the [`CancelledNotification`] is received.
    pub ct: CancellationToken,
    pub id: RequestId,
    pub meta: Meta,
    pub extensions: Extensions,
    /// An interface to fetch the remote client or server
    pub peer: Peer<R>,
}

/// Request execution context
#[derive(Debug, Clone)]
pub struct NotificationContext<R: ServiceRole> {
    pub meta: Meta,
    pub extensions: Extensions,
    /// An interface to fetch the remote client or server
    pub peer: Peer<R>,
}

/// Use this function to skip initialization process
pub fn serve_directly<R, S, T, E, A>(
    service: S,
    transport: T,
    peer_info: Option<R::PeerInfo>,
) -> RunningService<R, S>
where
    R: ServiceRole,
    S: Service<R>,
    T: IntoTransport<R, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    serve_directly_with_ct(service, transport, peer_info, Default::default())
}

/// Use this function to skip initialization process
pub fn serve_directly_with_ct<R, S, T, E, A>(
    service: S,
    transport: T,
    peer_info: Option<R::PeerInfo>,
    ct: CancellationToken,
) -> RunningService<R, S>
where
    R: ServiceRole,
    S: Service<R>,
    T: IntoTransport<R, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    let (peer, peer_rx) = Peer::new(Arc::new(AtomicU32RequestIdProvider::default()), peer_info);
    serve_inner(service, transport.into_transport(), peer, peer_rx, ct)
}

#[instrument(skip_all)]
fn serve_inner<R, S, T>(
    service: S,
    transport: T,
    peer: Peer<R>,
    mut peer_rx: tokio::sync::mpsc::Receiver<PeerSinkMessage<R>>,
    ct: CancellationToken,
) -> RunningService<R, S>
where
    R: ServiceRole,
    S: Service<R>,
    T: Transport<R> + 'static,
{
    const SINK_PROXY_BUFFER_SIZE: usize = 64;
    let (sink_proxy_tx, mut sink_proxy_rx) =
        tokio::sync::mpsc::channel::<TxJsonRpcMessage<R>>(SINK_PROXY_BUFFER_SIZE);
    let peer_info = peer.peer_info();
    if R::IS_CLIENT {
        tracing::info!(?peer_info, "Service initialized as client");
    } else {
        tracing::info!(?peer_info, "Service initialized as server");
    }

    let mut local_responder_pool =
        HashMap::<RequestId, Responder<Result<R::PeerResp, ServiceError>>>::new();
    let mut local_ct_pool = HashMap::<RequestId, CancellationToken>::new();
    let shared_service = Arc::new(service);
    // for return
    let service = shared_service.clone();

    // let message_sink = tokio::sync::
    // let mut stream = std::pin::pin!(stream);
    let serve_loop_ct = ct.child_token();
    let peer_return: Peer<R> = peer.clone();
    let current_span = tracing::Span::current();
    let handle = tokio::spawn(async move {
        let mut transport = transport.into_transport();
        let mut batch_messages = VecDeque::<RxJsonRpcMessage<R>>::new();
        let mut send_task_set = tokio::task::JoinSet::<SendTaskResult>::new();
        #[derive(Debug)]
        enum SendTaskResult {
            Request {
                id: RequestId,
                result: Result<(), DynamicTransportError>,
            },
            Notification {
                responder: Responder<Result<(), ServiceError>>,
                cancellation_param: Option<CancelledNotificationParam>,
                result: Result<(), DynamicTransportError>,
            },
        }
        #[derive(Debug)]
        enum Event<R: ServiceRole> {
            ProxyMessage(PeerSinkMessage<R>),
            PeerMessage(RxJsonRpcMessage<R>),
            ToSink(TxJsonRpcMessage<R>),
            SendTaskResult(SendTaskResult),
        }

        let quit_reason = loop {
            let evt = if let Some(m) = batch_messages.pop_front() {
                Event::PeerMessage(m)
            } else {
                tokio::select! {
                    m = sink_proxy_rx.recv(), if !sink_proxy_rx.is_closed() => {
                        if let Some(m) = m {
                            Event::ToSink(m)
                        } else {
                            continue
                        }
                    }
                    m = transport.receive() => {
                        if let Some(m) = m {
                            Event::PeerMessage(m)
                        } else {
                            // input stream closed
                            tracing::info!("input stream terminated");
                            break QuitReason::Closed
                        }
                    }
                    m = peer_rx.recv(), if !peer_rx.is_closed() => {
                        if let Some(m) = m {
                            Event::ProxyMessage(m)
                        } else {
                            continue
                        }
                    }
                    m = send_task_set.join_next(), if !send_task_set.is_empty() => {
                        let Some(result) = m else {
                            continue
                        };
                        match result {
                            Err(e) => {
                                // join error, which is serious, we should quit.
                                tracing::error!(%e, "send request task encounter a tokio join error");
                                break QuitReason::JoinError(e)
                            }
                            Ok(result) => {
                                Event::SendTaskResult(result)
                            }
                        }
                    }
                    _ = serve_loop_ct.cancelled() => {
                        tracing::info!("task cancelled");
                        break QuitReason::Cancelled
                    }
                }
            };

            tracing::trace!(?evt, "new event");
            match evt {
                Event::SendTaskResult(SendTaskResult::Request { id, result }) => {
                    if let Err(e) = result {
                        if let Some(responder) = local_responder_pool.remove(&id) {
                            let _ = responder.send(Err(ServiceError::TransportSend(e)));
                        }
                    }
                }
                Event::SendTaskResult(SendTaskResult::Notification {
                    responder,
                    result,
                    cancellation_param,
                }) => {
                    let response = if let Err(e) = result {
                        Err(ServiceError::TransportSend(e))
                    } else {
                        Ok(())
                    };
                    let _ = responder.send(response);
                    if let Some(param) = cancellation_param {
                        if let Some(responder) = local_responder_pool.remove(&param.request_id) {
                            tracing::info!(id = %param.request_id, reason = param.reason, "cancelled");
                            let _response_result = responder.send(Err(ServiceError::Cancelled {
                                reason: param.reason.clone(),
                            }));
                        }
                    }
                }
                // response and error
                Event::ToSink(m) => {
                    if let Some(id) = match &m {
                        JsonRpcMessage::Response(response) => Some(&response.id),
                        JsonRpcMessage::Error(error) => Some(&error.id),
                        _ => None,
                    } {
                        if let Some(ct) = local_ct_pool.remove(id) {
                            ct.cancel();
                        }
                        let send = transport.send(m);
                        let current_span = tracing::Span::current();
                        tokio::spawn(async move {
                            let send_result = send.await;
                            if let Err(error) = send_result {
                                tracing::error!(%error, "fail to response message");
                            }
                        }.instrument(current_span));
                    }
                }
                Event::ProxyMessage(PeerSinkMessage::Request {
                    request,
                    id,
                    responder,
                }) => {
                    local_responder_pool.insert(id.clone(), responder);
                    let send = transport.send(JsonRpcMessage::request(request, id.clone()));
                    {
                        let id = id.clone();
                        let current_span = tracing::Span::current();
                        send_task_set.spawn(send.map(move |r| SendTaskResult::Request {
                            id,
                            result: r.map_err(DynamicTransportError::new::<T, R>),
                        }).instrument(current_span));
                    }
                }
                Event::ProxyMessage(PeerSinkMessage::Notification {
                    notification,
                    responder,
                }) => {
                    // catch cancellation notification
                    let mut cancellation_param = None;
                    let notification = match notification.try_into() {
                        Ok::<CancelledNotification, _>(cancelled) => {
                            cancellation_param.replace(cancelled.params.clone());
                            cancelled.into()
                        }
                        Err(notification) => notification,
                    };
                    let send = transport.send(JsonRpcMessage::notification(notification));
                    let current_span = tracing::Span::current();
                    send_task_set.spawn(send.map(move |result| SendTaskResult::Notification {
                        responder,
                        cancellation_param,
                        result: result.map_err(DynamicTransportError::new::<T, R>),
                    }).instrument(current_span));
                }
                Event::PeerMessage(JsonRpcMessage::Request(JsonRpcRequest {
                    id,
                    mut request,
                    ..
                })) => {
                    tracing::debug!(%id, ?request, "received request");
                    {
                        let service = shared_service.clone();
                        let sink = sink_proxy_tx.clone();
                        let request_ct = serve_loop_ct.child_token();
                        let context_ct = request_ct.child_token();
                        local_ct_pool.insert(id.clone(), request_ct);
                        let mut extensions = Extensions::new();
                        let mut meta = Meta::new();
                        // avoid clone
                        // swap meta firstly, otherwise progress token will be lost
                        std::mem::swap(&mut meta, request.get_meta_mut());
                        std::mem::swap(&mut extensions, request.extensions_mut());
                        let context = RequestContext {
                            ct: context_ct,
                            id: id.clone(),
                            peer: peer.clone(),
                            meta,
                            extensions,
                        };
                        let current_span = tracing::Span::current();
                        tokio::spawn(async move {
                            let result = service
                                .handle_request(request, context)
                                .await;
                            let response = match result {
                                Ok(result) => {
                                    tracing::debug!(%id, ?result, "response message");
                                    JsonRpcMessage::response(result, id)
                                }
                                Err(error) => {
                                    tracing::warn!(%id, ?error, "response error");
                                    JsonRpcMessage::error(error, id)
                                }
                            };
                            let _send_result = sink.send(response).await;
                        }.instrument(current_span));
                    }
                }
                Event::PeerMessage(JsonRpcMessage::Notification(JsonRpcNotification {
                    notification,
                    ..
                })) => {
                    tracing::info!(?notification, "received notification");
                    // catch cancelled notification
                    let mut notification = match notification.try_into() {
                        Ok::<CancelledNotification, _>(cancelled) => {
                            if let Some(ct) = local_ct_pool.remove(&cancelled.params.request_id) {
                                tracing::info!(id = %cancelled.params.request_id, reason = cancelled.params.reason, "cancelled");
                                ct.cancel();
                            }
                            cancelled.into()
                        }
                        Err(notification) => notification,
                    };
                    {
                        let service = shared_service.clone();
                        let mut extensions = Extensions::new();
                        let mut meta = Meta::new();
                        // avoid clone
                        std::mem::swap(&mut extensions, notification.extensions_mut());
                        std::mem::swap(&mut meta, notification.get_meta_mut());
                        let context = NotificationContext {
                            peer: peer.clone(),
                            meta,
                            extensions,
                        };
                        let current_span = tracing::Span::current();
                        tokio::spawn(async move {
                            let result = service.handle_notification(notification, context).await;
                            if let Err(error) = result {
                                tracing::warn!(%error, "Error sending notification");
                            }
                        }.instrument(current_span));
                    }
                }
                Event::PeerMessage(JsonRpcMessage::Response(JsonRpcResponse {
                    result,
                    id,
                    ..
                })) => {
                    if let Some(responder) = local_responder_pool.remove(&id) {
                        let response_result = responder.send(Ok(result));
                        if let Err(_error) = response_result {
                            tracing::warn!(%id, "Error sending response");
                        }
                    }
                }
                Event::PeerMessage(JsonRpcMessage::Error(JsonRpcError { error, id, .. })) => {
                    if let Some(responder) = local_responder_pool.remove(&id) {
                        let _response_result = responder.send(Err(ServiceError::McpError(error)));
                        if let Err(_error) = _response_result {
                            tracing::warn!(%id, "Error sending response");
                        }
                    }
                }
            }
        };
        let sink_close_result = transport.close().await;
        if let Err(e) = sink_close_result {
            tracing::error!(%e, "fail to close sink");
        }
        tracing::info!(?quit_reason, "serve finished");
        quit_reason
    }.instrument(current_span));
    RunningService {
        service,
        peer: peer_return,
        handle,
        cancellation_token: ct.clone(),
        dg: ct.drop_guard(),
    }
}
