mod conversions;
mod delegate;
mod events;
mod routing;
mod session;
mod validation;
mod waits;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_code_mode_protocol::CellId;
use codex_code_mode_protocol::WaitRequest;
use codex_code_mode_protocol::grpc as proto;
use codex_code_mode_protocol::grpc::code_mode_host_server::CodeModeHost;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::Request;
use tonic::Response;
use tonic::Status;

use self::session::GrpcHostState;
use self::session::GrpcSession;
use self::waits::WaitRegistration;

type GrpcStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send + 'static>>;
type GrpcFuture<'a, T> = Pin<Box<dyn Future<Output = Result<Response<T>, Status>> + Send + 'a>>;

/// Serves transport-independent, leased code-mode sessions over gRPC.
#[derive(Clone)]
pub struct GrpcCodeModeHost {
    state: Arc<GrpcHostState>,
}

impl GrpcCodeModeHost {
    /// Creates a host with independent session, execution, and callback limits.
    pub fn new() -> Self {
        Self {
            state: Arc::new(GrpcHostState::new()),
        }
    }

    async fn open_session_request(
        &self,
        request: proto::OpenSessionRequest,
    ) -> Result<Response<GrpcStream<proto::SessionEvent>>, Status> {
        let _permit = self.state.request_permit()?;
        let limits = conversions::session_limits(request.cell_execution_limits)?;
        Ok(Response::new(self.state.open_session(limits)?))
    }

    async fn close_session_request(
        &self,
        request: proto::CloseSessionRequest,
    ) -> Result<Response<proto::CloseSessionResponse>, Status> {
        let _permit = self.state.control_permit()?;
        self.state.close_session(&request.session_id).await?;
        Ok(Response::new(proto::CloseSessionResponse {}))
    }

    async fn subscribe_request(
        &self,
        request: proto::SubscribeToToolCallsRequest,
    ) -> Result<Response<GrpcStream<proto::ToolCall>>, Status> {
        let _permit = self.state.request_permit()?;
        let session = self.state.session(&request.session_id)?;
        Ok(Response::new(session.subscribe(request.tool_names)?))
    }

    async fn complete_tool_request(
        &self,
        request: proto::CompleteToolCallRequest,
    ) -> Result<Response<proto::CompleteToolCallResponse>, Status> {
        let _permit = self.state.control_permit()?;
        let session = self.state.session(&request.session_id)?;
        let invocation_id = validation::uuid(&request.invocation_id, "tool invocation ID")?;
        let result = match request.outcome {
            Some(proto::complete_tool_call_request::Outcome::Succeeded(result)) => Ok(
                serde_json::from_slice(&result.output_json).map_err(|error| {
                    Status::invalid_argument(format!("invalid code-mode tool output JSON: {error}"))
                })?,
            ),
            Some(proto::complete_tool_call_request::Outcome::Failed(error)) => Err(error.message),
            None => {
                return Err(Status::invalid_argument(
                    "tool completion is missing its outcome",
                ));
            }
        };
        session.complete_invocation(invocation_id, result)?;
        Ok(Response::new(proto::CompleteToolCallResponse {}))
    }

    async fn acknowledge_notification_request(
        &self,
        request: proto::AcknowledgeNotificationRequest,
    ) -> Result<Response<proto::AcknowledgeNotificationResponse>, Status> {
        let _permit = self.state.control_permit()?;
        self.state.session(&request.session_id)?;
        validation::uuid(&request.notification_id, "notification ID")?;
        Ok(Response::new(proto::AcknowledgeNotificationResponse {}))
    }

    async fn execute_request(
        &self,
        request: proto::ExecuteRequest,
    ) -> Result<Response<GrpcStream<proto::ExecuteEvent>>, Status> {
        let session = self.state.session(&request.session_id)?;
        validation::identifier(&request.execution_id, "execution ID")?;
        let request_permit = self.state.request_permit()?;
        let execution_id = request.execution_id.clone();
        let request = conversions::execute_request(request)?;
        let cell_permit = self.state.cell_permit()?;
        session.reserve_execution(&execution_id)?;
        let mut admission = ExecutionAdmission {
            session: Arc::clone(&session),
            execution_id: Some(execution_id.clone()),
        };
        let started = tokio::select! {
            _ = session.closed.cancelled() => {
                return Err(Status::cancelled("code-mode session is closed"));
            }
            result = session.runtime.execute(request) => {
                result.map_err(Status::failed_precondition)?
            }
        };
        let cell_id = started.cell_id.clone();
        session.admit_execution(execution_id.clone(), cell_id.to_string(), cell_permit)?;

        let (sender, receiver) = mpsc::channel(/*buffer*/ 2);
        sender
            .try_send(Ok(proto::ExecuteEvent {
                event: Some(proto::execute_event::Event::Started(
                    proto::ExecutionStarted {
                        execution_id,
                        cell_id: cell_id.to_string(),
                    },
                )),
            }))
            .map_err(|_| Status::internal("failed to publish code-mode execution admission"))?;
        tokio::spawn(async move {
            let _request_permit = request_permit;
            tokio::select! {
                biased;
                _ = sender.closed() => {}
                response = started.initial_response() => {
                    let event = response.map(|response| proto::ExecuteEvent {
                        event: Some(proto::execute_event::Event::Outcome(
                            conversions::execution_outcome(response),
                        )),
                    }).map_err(Status::internal);
                    let _ = sender.send(event).await;
                }
                _ = session.closed.cancelled() => {}
            }
        });

        let stream = ReceiverStream::new(receiver).inspect(move |event| {
            if matches!(
                event,
                Ok(proto::ExecuteEvent {
                    event: Some(proto::execute_event::Event::Outcome(_)),
                })
            ) {
                admission.disarm();
            }
        });
        Ok(Response::new(Box::pin(stream)))
    }

    async fn wait_request(
        &self,
        request: proto::WaitRequest,
    ) -> Result<Response<proto::WaitResponse>, Status> {
        let session = self.state.session(&request.session_id)?;
        validation::identifier(&request.cell_id, "cell ID")?;
        validation::identifier(&request.wait_id, "wait ID")?;
        let _permit = self.state.request_permit()?;
        let registration = WaitRegistration::new(Arc::clone(&session), request.wait_id)?;
        let request = WaitRequest {
            cell_id: CellId::new(request.cell_id),
            yield_time_ms: request.yield_time_ms,
        };
        let outcome = tokio::select! {
            biased;
            _ = registration.cancellation().cancelled() => {
                return Err(Status::cancelled("code-mode wait was cancelled"));
            }
            _ = session.closed.cancelled() => {
                return Err(Status::cancelled("code-mode session is closed"));
            }
            outcome = session.runtime.wait(request) => {
                outcome.map_err(Status::failed_precondition)?
            }
        };
        Ok(Response::new(conversions::wait_response(outcome)))
    }

    async fn cancel_wait_request(
        &self,
        request: proto::CancelWaitRequest,
    ) -> Result<Response<proto::CancelWaitResponse>, Status> {
        let _permit = self.state.control_permit()?;
        let session = self.state.session(&request.session_id)?;
        validation::identifier(&request.wait_id, "wait ID")?;
        session.cancel_wait(&request.wait_id).await?;
        Ok(Response::new(proto::CancelWaitResponse {}))
    }

    async fn terminate_request(
        &self,
        request: proto::TerminateRequest,
    ) -> Result<Response<proto::WaitResponse>, Status> {
        let session = self.state.session(&request.session_id)?;
        validation::identifier(&request.cell_id, "cell ID")?;
        let _permit = self.state.request_permit()?;
        let result = session.terminate(CellId::new(request.cell_id)).await?;
        Ok(Response::new(conversions::wait_response(result)))
    }
}

impl Default for GrpcCodeModeHost {
    fn default() -> Self {
        Self::new()
    }
}

impl CodeModeHost for GrpcCodeModeHost {
    type OpenSessionStream = GrpcStream<proto::SessionEvent>;
    type SubscribeToToolCallsStream = GrpcStream<proto::ToolCall>;
    type ExecuteStream = GrpcStream<proto::ExecuteEvent>;

    fn open_session<'a, 'async_trait>(
        &'a self,
        request: Request<proto::OpenSessionRequest>,
    ) -> GrpcFuture<'async_trait, Self::OpenSessionStream>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.open_session_request(request.into_inner()))
    }

    fn close_session<'a, 'async_trait>(
        &'a self,
        request: Request<proto::CloseSessionRequest>,
    ) -> GrpcFuture<'async_trait, proto::CloseSessionResponse>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.close_session_request(request.into_inner()))
    }

    fn subscribe_to_tool_calls<'a, 'async_trait>(
        &'a self,
        request: Request<proto::SubscribeToToolCallsRequest>,
    ) -> GrpcFuture<'async_trait, Self::SubscribeToToolCallsStream>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.subscribe_request(request.into_inner()))
    }

    fn complete_tool_call<'a, 'async_trait>(
        &'a self,
        request: Request<proto::CompleteToolCallRequest>,
    ) -> GrpcFuture<'async_trait, proto::CompleteToolCallResponse>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.complete_tool_request(request.into_inner()))
    }

    fn acknowledge_notification<'a, 'async_trait>(
        &'a self,
        request: Request<proto::AcknowledgeNotificationRequest>,
    ) -> GrpcFuture<'async_trait, proto::AcknowledgeNotificationResponse>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.acknowledge_notification_request(request.into_inner()))
    }

    fn execute<'a, 'async_trait>(
        &'a self,
        request: Request<proto::ExecuteRequest>,
    ) -> GrpcFuture<'async_trait, Self::ExecuteStream>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.execute_request(request.into_inner()))
    }

    fn wait<'a, 'async_trait>(
        &'a self,
        request: Request<proto::WaitRequest>,
    ) -> GrpcFuture<'async_trait, proto::WaitResponse>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.wait_request(request.into_inner()))
    }

    fn cancel_wait<'a, 'async_trait>(
        &'a self,
        request: Request<proto::CancelWaitRequest>,
    ) -> GrpcFuture<'async_trait, proto::CancelWaitResponse>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.cancel_wait_request(request.into_inner()))
    }

    fn terminate<'a, 'async_trait>(
        &'a self,
        request: Request<proto::TerminateRequest>,
    ) -> GrpcFuture<'async_trait, proto::WaitResponse>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.terminate_request(request.into_inner()))
    }
}

struct ExecutionAdmission {
    session: Arc<GrpcSession>,
    execution_id: Option<String>,
}

impl ExecutionAdmission {
    fn disarm(&mut self) {
        self.execution_id = None;
    }
}

impl Drop for ExecutionAdmission {
    fn drop(&mut self) {
        if let Some(execution_id) = self.execution_id.take() {
            self.session.abandon_execution(&execution_id);
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "robustness_tests.rs"]
mod robustness_tests;
