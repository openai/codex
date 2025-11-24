use std::borrow::Cow;

use thiserror::Error;

use super::*;
#[cfg(feature = "elicitation")]
use crate::model::{
    CreateElicitationRequest, CreateElicitationRequestParam, CreateElicitationResult,
};
use crate::{
    model::{
        CancelledNotification, CancelledNotificationParam, ClientInfo, ClientJsonRpcMessage,
        ClientNotification, ClientRequest, ClientResult, CreateMessageRequest,
        CreateMessageRequestParam, CreateMessageResult, ErrorData, ListRootsRequest,
        ListRootsResult, LoggingMessageNotification, LoggingMessageNotificationParam,
        ProgressNotification, ProgressNotificationParam, PromptListChangedNotification,
        ProtocolVersion, ResourceListChangedNotification, ResourceUpdatedNotification,
        ResourceUpdatedNotificationParam, ServerInfo, ServerNotification, ServerRequest,
        ServerResult, ToolListChangedNotification,
    },
    transport::DynamicTransportError,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoleServer;

impl ServiceRole for RoleServer {
    type Req = ServerRequest;
    type Resp = ServerResult;
    type Not = ServerNotification;
    type PeerReq = ClientRequest;
    type PeerResp = ClientResult;
    type PeerNot = ClientNotification;
    type Info = ServerInfo;
    type PeerInfo = ClientInfo;

    type InitializeError = ServerInitializeError;
    const IS_CLIENT: bool = false;
}

/// It represents the error that may occur when serving the server.
///
/// if you want to handle the error, you can use `serve_server_with_ct` or `serve_server` with `Result<RunningService<RoleServer, S>, ServerError>`
#[derive(Error, Debug)]
pub enum ServerInitializeError {
    #[error("expect initialized request, but received: {0:?}")]
    ExpectedInitializeRequest(Option<ClientJsonRpcMessage>),

    #[error("expect initialized notification, but received: {0:?}")]
    ExpectedInitializedNotification(Option<ClientJsonRpcMessage>),

    #[error("connection closed: {0}")]
    ConnectionClosed(String),

    #[error("unexpected initialize result: {0:?}")]
    UnexpectedInitializeResponse(ServerResult),

    #[error("initialize failed: {0}")]
    InitializeFailed(ErrorData),

    #[error("unsupported protocol version: {0}")]
    UnsupportedProtocolVersion(ProtocolVersion),

    #[error("Send message error {error}, when {context}")]
    TransportError {
        error: DynamicTransportError,
        context: Cow<'static, str>,
    },

    #[error("Cancelled")]
    Cancelled,
}

impl ServerInitializeError {
    pub fn transport<T: Transport<RoleServer> + 'static>(
        error: T::Error,
        context: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::TransportError {
            error: DynamicTransportError::new::<T, _>(error),
            context: context.into(),
        }
    }
}
pub type ClientSink = Peer<RoleServer>;

impl<S: Service<RoleServer>> ServiceExt<RoleServer> for S {
    fn serve_with_ct<T, E, A>(
        self,
        transport: T,
        ct: CancellationToken,
    ) -> impl Future<Output = Result<RunningService<RoleServer, Self>, ServerInitializeError>> + Send
    where
        T: IntoTransport<RoleServer, E, A>,
        E: std::error::Error + Send + Sync + 'static,
        Self: Sized,
    {
        serve_server_with_ct(self, transport, ct)
    }
}

pub async fn serve_server<S, T, E, A>(
    service: S,
    transport: T,
) -> Result<RunningService<RoleServer, S>, ServerInitializeError>
where
    S: Service<RoleServer>,
    T: IntoTransport<RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    serve_server_with_ct(service, transport, CancellationToken::new()).await
}

/// Helper function to get the next message from the stream
async fn expect_next_message<T>(
    transport: &mut T,
    context: &str,
) -> Result<ClientJsonRpcMessage, ServerInitializeError>
where
    T: Transport<RoleServer>,
{
    transport
        .receive()
        .await
        .ok_or_else(|| ServerInitializeError::ConnectionClosed(context.to_string()))
}

/// Helper function to expect a request from the stream
async fn expect_request<T>(
    transport: &mut T,
    context: &str,
) -> Result<(ClientRequest, RequestId), ServerInitializeError>
where
    T: Transport<RoleServer>,
{
    let msg = expect_next_message(transport, context).await?;
    let msg_clone = msg.clone();
    msg.into_request()
        .ok_or(ServerInitializeError::ExpectedInitializeRequest(Some(
            msg_clone,
        )))
}

/// Helper function to expect a notification from the stream
async fn expect_notification<T>(
    transport: &mut T,
    context: &str,
) -> Result<ClientNotification, ServerInitializeError>
where
    T: Transport<RoleServer>,
{
    let msg = expect_next_message(transport, context).await?;
    let msg_clone = msg.clone();
    msg.into_notification()
        .ok_or(ServerInitializeError::ExpectedInitializedNotification(
            Some(msg_clone),
        ))
}

pub async fn serve_server_with_ct<S, T, E, A>(
    service: S,
    transport: T,
    ct: CancellationToken,
) -> Result<RunningService<RoleServer, S>, ServerInitializeError>
where
    S: Service<RoleServer>,
    T: IntoTransport<RoleServer, E, A>,
    E: std::error::Error + Send + Sync + 'static,
{
    tokio::select! {
        result = serve_server_with_ct_inner(service, transport.into_transport(), ct.clone()) => { result }
        _ = ct.cancelled() => {
            Err(ServerInitializeError::Cancelled)
        }
    }
}

async fn serve_server_with_ct_inner<S, T>(
    service: S,
    transport: T,
    ct: CancellationToken,
) -> Result<RunningService<RoleServer, S>, ServerInitializeError>
where
    S: Service<RoleServer>,
    T: Transport<RoleServer> + 'static,
{
    let mut transport = transport.into_transport();
    let id_provider = <Arc<AtomicU32RequestIdProvider>>::default();

    // Get initialize request
    let (request, id) = expect_request(&mut transport, "initialized request").await?;

    let ClientRequest::InitializeRequest(peer_info) = &request else {
        return Err(ServerInitializeError::ExpectedInitializeRequest(Some(
            ClientJsonRpcMessage::request(request, id),
        )));
    };
    let (peer, peer_rx) = Peer::new(id_provider, Some(peer_info.params.clone()));
    let context = RequestContext {
        ct: ct.child_token(),
        id: id.clone(),
        meta: request.get_meta().clone(),
        extensions: request.extensions().clone(),
        peer: peer.clone(),
    };
    // Send initialize response
    let init_response = service.handle_request(request.clone(), context).await;
    let mut init_response = match init_response {
        Ok(ServerResult::InitializeResult(init_response)) => init_response,
        Ok(result) => {
            return Err(ServerInitializeError::UnexpectedInitializeResponse(result));
        }
        Err(e) => {
            transport
                .send(ServerJsonRpcMessage::error(e.clone(), id))
                .await
                .map_err(|error| {
                    ServerInitializeError::transport::<T>(error, "sending error response")
                })?;
            return Err(ServerInitializeError::InitializeFailed(e));
        }
    };
    let peer_protocol_version = peer_info.params.protocol_version.clone();
    let protocol_version = match peer_protocol_version
        .partial_cmp(&init_response.protocol_version)
        .ok_or(ServerInitializeError::UnsupportedProtocolVersion(
            peer_protocol_version,
        ))? {
        std::cmp::Ordering::Less => peer_info.params.protocol_version.clone(),
        _ => init_response.protocol_version,
    };
    init_response.protocol_version = protocol_version;
    transport
        .send(ServerJsonRpcMessage::response(
            ServerResult::InitializeResult(init_response),
            id,
        ))
        .await
        .map_err(|error| {
            ServerInitializeError::transport::<T>(error, "sending initialize response")
        })?;

    // Wait for initialize notification
    let notification = expect_notification(&mut transport, "initialize notification").await?;
    let ClientNotification::InitializedNotification(_) = notification else {
        return Err(ServerInitializeError::ExpectedInitializedNotification(
            Some(ClientJsonRpcMessage::notification(notification)),
        ));
    };
    let context = NotificationContext {
        meta: notification.get_meta().clone(),
        extensions: notification.extensions().clone(),
        peer: peer.clone(),
    };
    let _ = service.handle_notification(notification, context).await;
    // Continue processing service
    Ok(serve_inner(service, transport, peer, peer_rx, ct))
}

macro_rules! method {
    (peer_req $method:ident $Req:ident() => $Resp: ident ) => {
        pub async fn $method(&self) -> Result<$Resp, ServiceError> {
            let result = self
                .send_request(ServerRequest::$Req($Req {
                    method: Default::default(),
                    extensions: Default::default(),
                }))
                .await?;
            match result {
                ClientResult::$Resp(result) => Ok(result),
                _ => Err(ServiceError::UnexpectedResponse),
            }
        }
    };
    (peer_req $method:ident $Req:ident($Param: ident) => $Resp: ident ) => {
        pub async fn $method(&self, params: $Param) -> Result<$Resp, ServiceError> {
            let result = self
                .send_request(ServerRequest::$Req($Req {
                    method: Default::default(),
                    params,
                    extensions: Default::default(),
                }))
                .await?;
            match result {
                ClientResult::$Resp(result) => Ok(result),
                _ => Err(ServiceError::UnexpectedResponse),
            }
        }
    };
    (peer_req $method:ident $Req:ident($Param: ident)) => {
        pub fn $method(
            &self,
            params: $Param,
        ) -> impl Future<Output = Result<(), ServiceError>> + Send + '_ {
            async move {
                let result = self
                    .send_request(ServerRequest::$Req($Req {
                        method: Default::default(),
                        params,
                    }))
                    .await?;
                match result {
                    ClientResult::EmptyResult(_) => Ok(()),
                    _ => Err(ServiceError::UnexpectedResponse),
                }
            }
        }
    };

    (peer_not $method:ident $Not:ident($Param: ident)) => {
        pub async fn $method(&self, params: $Param) -> Result<(), ServiceError> {
            self.send_notification(ServerNotification::$Not($Not {
                method: Default::default(),
                params,
                extensions: Default::default(),
            }))
            .await?;
            Ok(())
        }
    };
    (peer_not $method:ident $Not:ident) => {
        pub async fn $method(&self) -> Result<(), ServiceError> {
            self.send_notification(ServerNotification::$Not($Not {
                method: Default::default(),
                extensions: Default::default(),
            }))
            .await?;
            Ok(())
        }
    };

    // Timeout-only variants (base method should be created separately with peer_req)
    (peer_req_with_timeout $method_with_timeout:ident $Req:ident() => $Resp: ident) => {
        pub async fn $method_with_timeout(
            &self,
            timeout: Option<std::time::Duration>,
        ) -> Result<$Resp, ServiceError> {
            let request = ServerRequest::$Req($Req {
                method: Default::default(),
                extensions: Default::default(),
            });
            let options = crate::service::PeerRequestOptions {
                timeout,
                meta: None,
            };
            let result = self
                .send_request_with_option(request, options)
                .await?
                .await_response()
                .await?;
            match result {
                ClientResult::$Resp(result) => Ok(result),
                _ => Err(ServiceError::UnexpectedResponse),
            }
        }
    };

    (peer_req_with_timeout $method_with_timeout:ident $Req:ident($Param: ident) => $Resp: ident) => {
        pub async fn $method_with_timeout(
            &self,
            params: $Param,
            timeout: Option<std::time::Duration>,
        ) -> Result<$Resp, ServiceError> {
            let request = ServerRequest::$Req($Req {
                method: Default::default(),
                params,
                extensions: Default::default(),
            });
            let options = crate::service::PeerRequestOptions {
                timeout,
                meta: None,
            };
            let result = self
                .send_request_with_option(request, options)
                .await?
                .await_response()
                .await?;
            match result {
                ClientResult::$Resp(result) => Ok(result),
                _ => Err(ServiceError::UnexpectedResponse),
            }
        }
    };
}

impl Peer<RoleServer> {
    pub async fn create_message(
        &self,
        params: CreateMessageRequestParam,
    ) -> Result<CreateMessageResult, ServiceError> {
        let result = self
            .send_request(ServerRequest::CreateMessageRequest(CreateMessageRequest {
                method: Default::default(),
                params,
                extensions: Default::default(),
            }))
            .await?;
        match result {
            ClientResult::CreateMessageResult(result) => Ok(*result),
            _ => Err(ServiceError::UnexpectedResponse),
        }
    }
    method!(peer_req list_roots ListRootsRequest() => ListRootsResult);
    #[cfg(feature = "elicitation")]
    method!(peer_req create_elicitation CreateElicitationRequest(CreateElicitationRequestParam) => CreateElicitationResult);
    #[cfg(feature = "elicitation")]
    method!(peer_req_with_timeout create_elicitation_with_timeout CreateElicitationRequest(CreateElicitationRequestParam) => CreateElicitationResult);

    method!(peer_not notify_cancelled CancelledNotification(CancelledNotificationParam));
    method!(peer_not notify_progress ProgressNotification(ProgressNotificationParam));
    method!(peer_not notify_logging_message LoggingMessageNotification(LoggingMessageNotificationParam));
    method!(peer_not notify_resource_updated ResourceUpdatedNotification(ResourceUpdatedNotificationParam));
    method!(peer_not notify_resource_list_changed ResourceListChangedNotification);
    method!(peer_not notify_tool_list_changed ToolListChangedNotification);
    method!(peer_not notify_prompt_list_changed PromptListChangedNotification);
}

// =============================================================================
// ELICITATION CONVENIENCE METHODS
// These methods are specific to server role and provide typed elicitation functionality
// =============================================================================

/// Errors that can occur during typed elicitation operations
#[cfg(feature = "elicitation")]
#[derive(Error, Debug)]
pub enum ElicitationError {
    /// The elicitation request failed at the service level
    #[error("Service error: {0}")]
    Service(#[from] ServiceError),

    /// User explicitly declined to provide the requested information
    /// This indicates a conscious decision by the user to reject the request
    /// (e.g., clicked "Reject", "Decline", "No", etc.)
    #[error("User explicitly declined the request")]
    UserDeclined,

    /// User dismissed the request without making an explicit choice
    /// This indicates the user cancelled without explicitly declining
    /// (e.g., closed dialog, clicked outside, pressed Escape, etc.)
    #[error("User cancelled/dismissed the request")]
    UserCancelled,

    /// The response data could not be parsed into the requested type
    #[error("Failed to parse response data: {error}\nReceived data: {data}")]
    ParseError {
        error: serde_json::Error,
        data: serde_json::Value,
    },

    /// No response content was provided by the user
    #[error("No response content provided")]
    NoContent,

    /// Client does not support elicitation capability
    #[error("Client does not support elicitation - capability not declared during initialization")]
    CapabilityNotSupported,
}

/// Marker trait to ensure that elicitation types generate object-type JSON schemas.
///
/// This trait provides compile-time safety to ensure that types used with
/// `elicit<T>()` methods will generate JSON schemas of type "object", which
/// aligns with MCP client expectations for structured data input.
///
/// # Type Safety Rationale
///
/// MCP clients typically expect JSON objects for elicitation schemas to
/// provide structured forms and validation. This trait prevents common
/// mistakes like:
///
/// ```compile_fail
/// // These would not compile due to missing ElicitationSafe bound:
/// let name: String = server.elicit("Enter name").await?;        // Primitive
/// let items: Vec<i32> = server.elicit("Enter items").await?;    // Array
/// ```
#[cfg(feature = "elicitation")]
pub trait ElicitationSafe: schemars::JsonSchema {}

/// Macro to mark types as safe for elicitation by verifying they generate object schemas.
///
/// This macro automatically implements the `ElicitationSafe` trait for struct types
/// that should be used with `elicit<T>()` methods.
///
/// # Example
///
/// ```rust
/// use rmcp::elicit_safe;
/// use schemars::JsonSchema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct UserProfile {
///     name: String,
///     email: String,
/// }
///
/// elicit_safe!(UserProfile);
///
/// // Now safe to use in async context:
/// // let profile: UserProfile = server.elicit("Enter profile").await?;
/// ```
#[cfg(feature = "elicitation")]
#[macro_export]
macro_rules! elicit_safe {
    ($($t:ty),* $(,)?) => {
        $(
            impl $crate::service::ElicitationSafe for $t {}
        )*
    };
}

#[cfg(feature = "elicitation")]
impl Peer<RoleServer> {
    /// Check if the client supports elicitation capability
    ///
    /// Returns true if the client declared elicitation capability during initialization,
    /// false otherwise. According to MCP 2025-06-18 specification, clients that support
    /// elicitation MUST declare the capability during initialization.
    pub fn supports_elicitation(&self) -> bool {
        if let Some(client_info) = self.peer_info() {
            client_info.capabilities.elicitation.is_some()
        } else {
            false
        }
    }

    /// Request typed data from the user with automatic schema generation.
    ///
    /// This method automatically generates the JSON schema from the Rust type using `schemars`,
    /// eliminating the need to manually create schemas. The response is automatically parsed
    /// into the requested type.
    ///
    /// **Requires the `elicitation` feature to be enabled.**
    ///
    /// # Type Requirements
    /// The type `T` must implement:
    /// - `schemars::JsonSchema` - for automatic schema generation
    /// - `serde::Deserialize` - for parsing the response
    ///
    /// # Arguments
    /// * `message` - The prompt message for the user
    ///
    /// # Returns
    /// * `Ok(Some(data))` if user provided valid data that matches type T
    /// * `Err(ElicitationError::UserDeclined)` if user explicitly declined the request
    /// * `Err(ElicitationError::UserCancelled)` if user cancelled/dismissed the request
    /// * `Err(ElicitationError::ParseError { .. })` if response data couldn't be parsed into type T
    /// * `Err(ElicitationError::NoContent)` if no response content was provided
    /// * `Err(ElicitationError::Service(_))` if the underlying service call failed
    ///
    /// # Example
    ///
    /// Add to your `Cargo.toml`:
    /// ```toml
    /// [dependencies]
    /// rmcp = { version = "0.3", features = ["elicitation"] }
    /// serde = { version = "1.0", features = ["derive"] }
    /// schemars = "1.0"
    /// ```
    ///
    /// ```rust,no_run
    /// # use rmcp::*;
    /// # use rmcp::service::ElicitationError;
    /// # use serde::{Deserialize, Serialize};
    /// # use schemars::JsonSchema;
    /// #
    /// #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    /// struct UserProfile {
    ///     #[schemars(description = "Full name")]
    ///     name: String,
    ///     #[schemars(description = "Email address")]
    ///     email: String,
    ///     #[schemars(description = "Age")]
    ///     age: u8,
    /// }
    ///
    /// // Mark as safe for elicitation (generates object schema)
    /// rmcp::elicit_safe!(UserProfile);
    ///
    /// # async fn example(peer: Peer<RoleServer>) -> Result<(), Box<dyn std::error::Error>> {
    /// match peer.elicit::<UserProfile>("Please enter your profile information").await {
    ///     Ok(Some(profile)) => {
    ///         println!("Name: {}, Email: {}, Age: {}", profile.name, profile.email, profile.age);
    ///     }
    ///     Ok(None) => {
    ///         println!("User provided no content");
    ///     }
    ///     Err(ElicitationError::UserDeclined) => {
    ///         println!("User explicitly declined to provide information");
    ///         // Handle explicit decline - perhaps offer alternatives
    ///     }
    ///     Err(ElicitationError::UserCancelled) => {
    ///         println!("User cancelled the request");
    ///         // Handle cancellation - perhaps prompt again later
    ///     }
    ///     Err(ElicitationError::ParseError { error, data }) => {
    ///         println!("Failed to parse response: {}\nData: {}", error, data);
    ///     }
    ///     Err(e) => return Err(e.into()),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(all(feature = "schemars", feature = "elicitation"))]
    pub async fn elicit<T>(&self, message: impl Into<String>) -> Result<Option<T>, ElicitationError>
    where
        T: ElicitationSafe + for<'de> serde::Deserialize<'de>,
    {
        self.elicit_with_timeout(message, None).await
    }

    /// Request typed data from the user with custom timeout.
    ///
    /// Same as `elicit()` but allows specifying a custom timeout for the request.
    /// If the user doesn't respond within the timeout, the request will be cancelled.
    ///
    /// # Arguments
    /// * `message` - The prompt message for the user
    /// * `timeout` - Optional timeout duration. If None, uses default timeout behavior
    ///
    /// # Returns
    /// Same as `elicit()` but may also return `ServiceError::Timeout` if timeout expires
    ///
    /// # Example
    /// ```rust,no_run
    /// # use rmcp::*;
    /// # use rmcp::service::ElicitationError;
    /// # use serde::{Deserialize, Serialize};
    /// # use schemars::JsonSchema;
    /// # use std::time::Duration;
    /// #
    /// #[derive(Debug, Serialize, Deserialize, JsonSchema)]
    /// struct QuickResponse {
    ///     answer: String,
    /// }
    ///
    /// // Mark as safe for elicitation
    /// rmcp::elicit_safe!(QuickResponse);
    ///
    /// # async fn example(peer: Peer<RoleServer>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Give user 30 seconds to respond
    /// let timeout = Some(Duration::from_secs(30));
    /// match peer.elicit_with_timeout::<QuickResponse>(
    ///     "Quick question - what's your answer?",
    ///     timeout
    /// ).await {
    ///     Ok(Some(response)) => println!("Got answer: {}", response.answer),
    ///     Ok(None) => println!("User provided no content"),
    ///     Err(ElicitationError::UserDeclined) => {
    ///         println!("User explicitly declined");
    ///         // Handle explicit decline
    ///     }
    ///     Err(ElicitationError::UserCancelled) => {
    ///         println!("User cancelled/dismissed");
    ///         // Handle cancellation
    ///     }
    ///     Err(ElicitationError::Service(ServiceError::Timeout { .. })) => {
    ///         println!("User didn't respond in time");
    ///     }
    ///     Err(e) => return Err(e.into()),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(all(feature = "schemars", feature = "elicitation"))]
    pub async fn elicit_with_timeout<T>(
        &self,
        message: impl Into<String>,
        timeout: Option<std::time::Duration>,
    ) -> Result<Option<T>, ElicitationError>
    where
        T: ElicitationSafe + for<'de> serde::Deserialize<'de>,
    {
        // Check if client supports elicitation capability
        if !self.supports_elicitation() {
            return Err(ElicitationError::CapabilityNotSupported);
        }

        // Generate schema automatically from type
        let schema = crate::model::ElicitationSchema::from_type::<T>().map_err(|e| {
            ElicitationError::Service(ServiceError::McpError(crate::ErrorData::invalid_params(
                format!(
                    "Invalid schema for type {}: {}",
                    std::any::type_name::<T>(),
                    e
                ),
                None,
            )))
        })?;

        let response = self
            .create_elicitation_with_timeout(
                CreateElicitationRequestParam {
                    message: message.into(),
                    requested_schema: schema,
                },
                timeout,
            )
            .await?;

        match response.action {
            crate::model::ElicitationAction::Accept => {
                if let Some(value) = response.content {
                    match serde_json::from_value::<T>(value.clone()) {
                        Ok(parsed) => Ok(Some(parsed)),
                        Err(error) => Err(ElicitationError::ParseError { error, data: value }),
                    }
                } else {
                    Err(ElicitationError::NoContent)
                }
            }
            crate::model::ElicitationAction::Decline => Err(ElicitationError::UserDeclined),
            crate::model::ElicitationAction::Cancel => Err(ElicitationError::UserCancelled),
        }
    }
}
