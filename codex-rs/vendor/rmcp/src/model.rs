use std::{borrow::Cow, sync::Arc};
mod annotated;
mod capabilities;
mod content;
mod elicitation_schema;
mod extension;
mod meta;
mod prompt;
mod resource;
mod serde_impl;
mod task;
mod tool;
pub use annotated::*;
pub use capabilities::*;
pub use content::*;
pub use elicitation_schema::*;
pub use extension::*;
pub use meta::*;
pub use prompt::*;
pub use resource::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
pub use task::*;
pub use tool::*;

/// A JSON object type alias for convenient handling of JSON data.
///
/// You can use [`crate::object!`] or [`crate::model::object`] to create a json object quickly.
/// This is commonly used for storing arbitrary JSON data in MCP messages.
pub type JsonObject<F = Value> = serde_json::Map<String, F>;

/// unwrap the JsonObject under [`serde_json::Value`]
///
/// # Panic
/// This will panic when the value is not a object in debug mode.
pub fn object(value: serde_json::Value) -> JsonObject {
    debug_assert!(value.is_object());
    match value {
        serde_json::Value::Object(map) => map,
        _ => JsonObject::default(),
    }
}

/// Use this macro just like [`serde_json::json!`]
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
#[macro_export]
macro_rules! object {
    ({$($tt:tt)*}) => {
        $crate::model::object(serde_json::json! {
            {$($tt)*}
        })
    };
}

/// This is commonly used for representing empty objects in MCP messages.
///
/// without returning any specific data.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Copy, Eq)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "server", derive(schemars::JsonSchema))]
pub struct EmptyObject {}

pub trait ConstString: Default {
    const VALUE: &str;
    fn as_str(&self) -> &'static str {
        Self::VALUE
    }
}
#[macro_export]
macro_rules! const_string {
    ($name:ident = $value:literal) => {
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub struct $name;

        impl ConstString for $name {
            const VALUE: &str = $value;
        }

        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                $value.serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<$name, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s: String = serde::Deserialize::deserialize(deserializer)?;
                if s == $value {
                    Ok($name)
                } else {
                    Err(serde::de::Error::custom(format!(concat!(
                        "expect const string value \"",
                        $value,
                        "\""
                    ))))
                }
            }
        }

        #[cfg(feature = "schemars")]
        impl schemars::JsonSchema for $name {
            fn schema_name() -> Cow<'static, str> {
                Cow::Borrowed(stringify!($name))
            }

            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                use serde_json::{Map, json};

                let mut schema_map = Map::new();
                schema_map.insert("type".to_string(), json!("string"));
                schema_map.insert("format".to_string(), json!("const"));
                schema_map.insert("const".to_string(), json!($value));

                schemars::Schema::from(schema_map)
            }
        }
    };
}

const_string!(JsonRpcVersion2_0 = "2.0");

// =============================================================================
// CORE PROTOCOL TYPES
// =============================================================================

/// Represents the MCP protocol version used for communication.
///
/// This ensures compatibility between clients and servers by specifying
/// which version of the Model Context Protocol is being used.
#[derive(Debug, Clone, Eq, PartialEq, Hash, PartialOrd)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProtocolVersion(Cow<'static, str>);

impl Default for ProtocolVersion {
    fn default() -> Self {
        Self::LATEST
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl ProtocolVersion {
    pub const V_2025_06_18: Self = Self(Cow::Borrowed("2025-06-18"));
    pub const V_2025_03_26: Self = Self(Cow::Borrowed("2025-03-26"));
    pub const V_2024_11_05: Self = Self(Cow::Borrowed("2024-11-05"));
    //  Keep LATEST at 2025-03-26 until full 2025-06-18 compliance and automated testing are in place.
    pub const LATEST: Self = Self::V_2025_03_26;
}

impl Serialize for ProtocolVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProtocolVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        #[allow(clippy::single_match)]
        match s.as_str() {
            "2024-11-05" => return Ok(ProtocolVersion::V_2024_11_05),
            "2025-03-26" => return Ok(ProtocolVersion::V_2025_03_26),
            "2025-06-18" => return Ok(ProtocolVersion::V_2025_06_18),
            _ => {}
        }
        Ok(ProtocolVersion(Cow::Owned(s)))
    }
}

/// A flexible identifier type that can be either a number or a string.
///
/// This is commonly used for request IDs and other identifiers in JSON-RPC
/// where the specification allows both numeric and string values.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum NumberOrString {
    /// A numeric identifier
    Number(i64),
    /// A string identifier
    String(Arc<str>),
}

impl NumberOrString {
    pub fn into_json_value(self) -> Value {
        match self {
            NumberOrString::Number(n) => Value::Number(serde_json::Number::from(n)),
            NumberOrString::String(s) => Value::String(s.to_string()),
        }
    }
}

impl std::fmt::Display for NumberOrString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumberOrString::Number(n) => n.fmt(f),
            NumberOrString::String(s) => s.fmt(f),
        }
    }
}

impl Serialize for NumberOrString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            NumberOrString::Number(n) => n.serialize(serializer),
            NumberOrString::String(s) => s.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for NumberOrString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(NumberOrString::Number(i))
                } else if let Some(u) = n.as_u64() {
                    // Handle large unsigned numbers that fit in i64
                    if u <= i64::MAX as u64 {
                        Ok(NumberOrString::Number(u as i64))
                    } else {
                        Err(serde::de::Error::custom("Number too large for i64"))
                    }
                } else {
                    Err(serde::de::Error::custom("Expected an integer"))
                }
            }
            Value::String(s) => Ok(NumberOrString::String(s.into())),
            _ => Err(serde::de::Error::custom("Expect number or string")),
        }
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for NumberOrString {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("NumberOrString")
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use serde_json::{Map, json};

        let mut number_schema = Map::new();
        number_schema.insert("type".to_string(), json!("number"));

        let mut string_schema = Map::new();
        string_schema.insert("type".to_string(), json!("string"));

        let mut schema_map = Map::new();
        schema_map.insert("oneOf".to_string(), json!([number_schema, string_schema]));

        schemars::Schema::from(schema_map)
    }
}

/// Type alias for request identifiers used in JSON-RPC communication.
pub type RequestId = NumberOrString;

/// A token used to track the progress of long-running operations.
///
/// Progress tokens allow clients and servers to associate progress notifications
/// with specific requests, enabling real-time updates on operation status.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash, Eq)]
#[serde(transparent)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProgressToken(pub NumberOrString);

// =============================================================================
// JSON-RPC MESSAGE STRUCTURES
// =============================================================================

/// Represents a JSON-RPC request with method, parameters, and extensions.
///
/// This is the core structure for all MCP requests, containing:
/// - `method`: The name of the method being called
/// - `params`: The parameters for the method
/// - `extensions`: Additional context data (similar to HTTP headers)
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Request<M = String, P = JsonObject> {
    pub method: M,
    pub params: P,
    /// extensions will carry anything possible in the context, including [`Meta`]
    ///
    /// this is similar with the Extensions in `http` crate
    #[cfg_attr(feature = "schemars", schemars(skip))]
    pub extensions: Extensions,
}

impl<M: Default, P> Request<M, P> {
    pub fn new(params: P) -> Self {
        Self {
            method: Default::default(),
            params,
            extensions: Extensions::default(),
        }
    }
}

impl<M, P> GetExtensions for Request<M, P> {
    fn extensions(&self) -> &Extensions {
        &self.extensions
    }
    fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RequestOptionalParam<M = String, P = JsonObject> {
    pub method: M,
    // #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<P>,
    /// extensions will carry anything possible in the context, including [`Meta`]
    ///
    /// this is similar with the Extensions in `http` crate
    #[cfg_attr(feature = "schemars", schemars(skip))]
    pub extensions: Extensions,
}

impl<M: Default, P> RequestOptionalParam<M, P> {
    pub fn with_param(params: P) -> Self {
        Self {
            method: Default::default(),
            params: Some(params),
            extensions: Extensions::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RequestNoParam<M = String> {
    pub method: M,
    /// extensions will carry anything possible in the context, including [`Meta`]
    ///
    /// this is similar with the Extensions in `http` crate
    #[cfg_attr(feature = "schemars", schemars(skip))]
    pub extensions: Extensions,
}

impl<M> GetExtensions for RequestNoParam<M> {
    fn extensions(&self) -> &Extensions {
        &self.extensions
    }
    fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }
}
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Notification<M = String, P = JsonObject> {
    pub method: M,
    pub params: P,
    /// extensions will carry anything possible in the context, including [`Meta`]
    ///
    /// this is similar with the Extensions in `http` crate
    #[cfg_attr(feature = "schemars", schemars(skip))]
    pub extensions: Extensions,
}

impl<M: Default, P> Notification<M, P> {
    pub fn new(params: P) -> Self {
        Self {
            method: Default::default(),
            params,
            extensions: Extensions::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct NotificationNoParam<M = String> {
    pub method: M,
    /// extensions will carry anything possible in the context, including [`Meta`]
    ///
    /// this is similar with the Extensions in `http` crate
    #[cfg_attr(feature = "schemars", schemars(skip))]
    pub extensions: Extensions,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct JsonRpcRequest<R = Request> {
    pub jsonrpc: JsonRpcVersion2_0,
    pub id: RequestId,
    #[serde(flatten)]
    pub request: R,
}

type DefaultResponse = JsonObject;
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct JsonRpcResponse<R = JsonObject> {
    pub jsonrpc: JsonRpcVersion2_0,
    pub id: RequestId,
    pub result: R,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct JsonRpcError {
    pub jsonrpc: JsonRpcVersion2_0,
    pub id: RequestId,
    pub error: ErrorData,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct JsonRpcNotification<N = Notification> {
    pub jsonrpc: JsonRpcVersion2_0,
    #[serde(flatten)]
    pub notification: N,
}

/// Standard JSON-RPC error codes used throughout the MCP protocol.
///
/// These codes follow the JSON-RPC 2.0 specification and provide
/// standardized error reporting across all MCP implementations.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ErrorCode(pub i32);

impl ErrorCode {
    pub const RESOURCE_NOT_FOUND: Self = Self(-32002);
    pub const INVALID_REQUEST: Self = Self(-32600);
    pub const METHOD_NOT_FOUND: Self = Self(-32601);
    pub const INVALID_PARAMS: Self = Self(-32602);
    pub const INTERNAL_ERROR: Self = Self(-32603);
    pub const PARSE_ERROR: Self = Self(-32700);
    pub const URL_ELICITATION_REQUIRED: Self = Self(-32042);
}

/// Error information for JSON-RPC error responses.
///
/// This structure follows the JSON-RPC 2.0 specification for error reporting,
/// providing a standardized way to communicate errors between clients and servers.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ErrorData {
    /// The error type that occurred (using standard JSON-RPC error codes)
    pub code: ErrorCode,

    /// A short description of the error. The message SHOULD be limited to a concise single sentence.
    pub message: Cow<'static, str>,

    /// Additional information about the error. The value of this member is defined by the
    /// sender (e.g. detailed error information, nested errors etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ErrorData {
    pub fn new(
        code: ErrorCode,
        message: impl Into<Cow<'static, str>>,
        data: Option<Value>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }
    pub fn resource_not_found(message: impl Into<Cow<'static, str>>, data: Option<Value>) -> Self {
        Self::new(ErrorCode::RESOURCE_NOT_FOUND, message, data)
    }
    pub fn parse_error(message: impl Into<Cow<'static, str>>, data: Option<Value>) -> Self {
        Self::new(ErrorCode::PARSE_ERROR, message, data)
    }
    pub fn invalid_request(message: impl Into<Cow<'static, str>>, data: Option<Value>) -> Self {
        Self::new(ErrorCode::INVALID_REQUEST, message, data)
    }
    pub fn method_not_found<M: ConstString>() -> Self {
        Self::new(ErrorCode::METHOD_NOT_FOUND, M::VALUE, None)
    }
    pub fn invalid_params(message: impl Into<Cow<'static, str>>, data: Option<Value>) -> Self {
        Self::new(ErrorCode::INVALID_PARAMS, message, data)
    }
    pub fn internal_error(message: impl Into<Cow<'static, str>>, data: Option<Value>) -> Self {
        Self::new(ErrorCode::INTERNAL_ERROR, message, data)
    }
    pub fn url_elicitation_required(
        message: impl Into<Cow<'static, str>>,
        data: Option<Value>,
    ) -> Self {
        Self::new(ErrorCode::URL_ELICITATION_REQUIRED, message, data)
    }
}

/// Represents any JSON-RPC message that can be sent or received.
///
/// This enum covers all possible message types in the JSON-RPC protocol:
/// individual requests/responses, notifications, and errors.
/// It serves as the top-level message container for MCP communication.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum JsonRpcMessage<Req = Request, Resp = DefaultResponse, Noti = Notification> {
    /// A single request expecting a response
    Request(JsonRpcRequest<Req>),
    /// A response to a previous request
    Response(JsonRpcResponse<Resp>),
    /// A one-way notification (no response expected)
    Notification(JsonRpcNotification<Noti>),
    /// An error response
    Error(JsonRpcError),
}

impl<Req, Resp, Not> JsonRpcMessage<Req, Resp, Not> {
    #[inline]
    pub const fn request(request: Req, id: RequestId) -> Self {
        JsonRpcMessage::Request(JsonRpcRequest {
            jsonrpc: JsonRpcVersion2_0,
            id,
            request,
        })
    }
    #[inline]
    pub const fn response(response: Resp, id: RequestId) -> Self {
        JsonRpcMessage::Response(JsonRpcResponse {
            jsonrpc: JsonRpcVersion2_0,
            id,
            result: response,
        })
    }
    #[inline]
    pub const fn error(error: ErrorData, id: RequestId) -> Self {
        JsonRpcMessage::Error(JsonRpcError {
            jsonrpc: JsonRpcVersion2_0,
            id,
            error,
        })
    }
    #[inline]
    pub const fn notification(notification: Not) -> Self {
        JsonRpcMessage::Notification(JsonRpcNotification {
            jsonrpc: JsonRpcVersion2_0,
            notification,
        })
    }
    pub fn into_request(self) -> Option<(Req, RequestId)> {
        match self {
            JsonRpcMessage::Request(r) => Some((r.request, r.id)),
            _ => None,
        }
    }
    pub fn into_response(self) -> Option<(Resp, RequestId)> {
        match self {
            JsonRpcMessage::Response(r) => Some((r.result, r.id)),
            _ => None,
        }
    }
    pub fn into_notification(self) -> Option<Not> {
        match self {
            JsonRpcMessage::Notification(n) => Some(n.notification),
            _ => None,
        }
    }
    pub fn into_error(self) -> Option<(ErrorData, RequestId)> {
        match self {
            JsonRpcMessage::Error(e) => Some((e.error, e.id)),
            _ => None,
        }
    }
    pub fn into_result(self) -> Option<(Result<Resp, ErrorData>, RequestId)> {
        match self {
            JsonRpcMessage::Response(r) => Some((Ok(r.result), r.id)),
            JsonRpcMessage::Error(e) => Some((Err(e.error), e.id)),

            _ => None,
        }
    }
}

// =============================================================================
// INITIALIZATION AND CONNECTION SETUP
// =============================================================================

/// # Empty result
/// A response that indicates success but carries no data.
pub type EmptyResult = EmptyObject;

impl From<()> for EmptyResult {
    fn from(_value: ()) -> Self {
        EmptyResult {}
    }
}

impl From<EmptyResult> for () {
    fn from(_value: EmptyResult) {}
}

/// A catch-all response either side can use for custom requests.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(transparent)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CustomResult(pub Value);

impl CustomResult {
    pub fn new(result: Value) -> Self {
        Self(result)
    }

    /// Deserialize the result into a strongly-typed structure.
    pub fn result_as<T: DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.0.clone())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CancelledNotificationParam {
    pub request_id: RequestId,
    pub reason: Option<String>,
}

const_string!(CancelledNotificationMethod = "notifications/cancelled");

/// # Cancellation
/// This notification can be sent by either side to indicate that it is cancelling a previously-issued request.
///
/// The request SHOULD still be in-flight, but due to communication latency, it is always possible that this notification MAY arrive after the request has already finished.
///
/// This notification indicates that the result will be unused, so any associated processing SHOULD cease.
///
/// A client MUST NOT attempt to cancel its `initialize` request.
pub type CancelledNotification =
    Notification<CancelledNotificationMethod, CancelledNotificationParam>;

/// A catch-all notification either side can use to send custom messages to its peer.
///
/// This preserves the raw `method` name and `params` payload so handlers can
/// deserialize them into domain-specific types.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CustomNotification {
    pub method: String,
    pub params: Option<Value>,
    /// extensions will carry anything possible in the context, including [`Meta`]
    ///
    /// this is similar with the Extensions in `http` crate
    #[cfg_attr(feature = "schemars", schemars(skip))]
    pub extensions: Extensions,
}

impl CustomNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            method: method.into(),
            params,
            extensions: Extensions::default(),
        }
    }

    /// Deserialize `params` into a strongly-typed structure.
    pub fn params_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.params
            .as_ref()
            .map(|params| serde_json::from_value(params.clone()))
            .transpose()
    }
}

/// A catch-all request either side can use to send custom messages to its peer.
///
/// This preserves the raw `method` name and `params` payload so handlers can
/// deserialize them into domain-specific types.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CustomRequest {
    pub method: String,
    pub params: Option<Value>,
    /// extensions will carry anything possible in the context, including [`Meta`]
    ///
    /// this is similar with the Extensions in `http` crate
    #[cfg_attr(feature = "schemars", schemars(skip))]
    pub extensions: Extensions,
}

impl CustomRequest {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            method: method.into(),
            params,
            extensions: Extensions::default(),
        }
    }

    /// Deserialize `params` into a strongly-typed structure.
    pub fn params_as<T: DeserializeOwned>(&self) -> Result<Option<T>, serde_json::Error> {
        self.params
            .as_ref()
            .map(|params| serde_json::from_value(params.clone()))
            .transpose()
    }
}

const_string!(InitializeResultMethod = "initialize");
/// # Initialization
/// This request is sent from the client to the server when it first connects, asking it to begin initialization.
pub type InitializeRequest = Request<InitializeResultMethod, InitializeRequestParams>;

const_string!(InitializedNotificationMethod = "notifications/initialized");
/// This notification is sent from the client to the server after initialization has finished.
pub type InitializedNotification = NotificationNoParam<InitializedNotificationMethod>;

/// Parameters sent by a client when initializing a connection to an MCP server.
///
/// This contains the client's protocol version, capabilities, and implementation
/// information, allowing the server to understand what the client supports.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct InitializeRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// The MCP protocol version this client supports
    pub protocol_version: ProtocolVersion,
    /// The capabilities this client supports (sampling, roots, etc.)
    pub capabilities: ClientCapabilities,
    /// Information about the client implementation
    pub client_info: Implementation,
}

impl RequestParamsMeta for InitializeRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`InitializeRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use InitializeRequestParams instead")]
pub type InitializeRequestParam = InitializeRequestParams;

/// The server's response to an initialization request.
///
/// Contains the server's protocol version, capabilities, and implementation
/// information, along with optional instructions for the client.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct InitializeResult {
    /// The MCP protocol version this server supports
    pub protocol_version: ProtocolVersion,
    /// The capabilities this server provides (tools, resources, prompts, etc.)
    pub capabilities: ServerCapabilities,
    /// Information about the server implementation
    pub server_info: Implementation,
    /// Optional human-readable instructions about using this server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

pub type ServerInfo = InitializeResult;
pub type ClientInfo = InitializeRequestParams;

#[allow(clippy::derivable_impls)]
impl Default for ServerInfo {
    fn default() -> Self {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::default(),
            server_info: Implementation::from_build_env(),
            instructions: None,
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for ClientInfo {
    fn default() -> Self {
        ClientInfo {
            meta: None,
            protocol_version: ProtocolVersion::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation::from_build_env(),
        }
    }
}

/// A URL pointing to an icon resource or a base64-encoded data URI.
///
/// Clients that support rendering icons MUST support at least the following MIME types:
/// - image/png - PNG images (safe, universal compatibility)
/// - image/jpeg (and image/jpg) - JPEG images (safe, universal compatibility)
///
/// Clients that support rendering icons SHOULD also support:
/// - image/svg+xml - SVG images (scalable but requires security precautions)
/// - image/webp - WebP images (modern, efficient format)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Icon {
    /// A standard URI pointing to an icon resource
    pub src: String,
    /// Optional override if the server's MIME type is missing or generic
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Size specification, each string should be in WxH format (e.g., `\"48x48\"`, `\"96x96\"`) or `\"any\"` for scalable formats like SVG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Implementation {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
}

impl Default for Implementation {
    fn default() -> Self {
        Self::from_build_env()
    }
}

impl Implementation {
    pub fn from_build_env() -> Self {
        Implementation {
            name: env!("CARGO_CRATE_NAME").to_owned(),
            title: None,
            version: env!("CARGO_PKG_VERSION").to_owned(),
            description: None,
            icons: None,
            website_url: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PaginatedRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

impl RequestParamsMeta for PaginatedRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`PaginatedRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use PaginatedRequestParams instead")]
pub type PaginatedRequestParam = PaginatedRequestParams;
// =============================================================================
// PROGRESS AND PAGINATION
// =============================================================================

const_string!(PingRequestMethod = "ping");
pub type PingRequest = RequestNoParam<PingRequestMethod>;

const_string!(ProgressNotificationMethod = "notifications/progress");
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProgressNotificationParam {
    pub progress_token: ProgressToken,
    /// The progress thus far. This should increase every time progress is made, even if the total is unknown.
    pub progress: f64,
    /// Total number of items to process (or total progress required), if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    /// An optional message describing the current progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

pub type ProgressNotification = Notification<ProgressNotificationMethod, ProgressNotificationParam>;

pub type Cursor = String;

macro_rules! paginated_result {
    ($t:ident {
        $i_item: ident: $t_item: ty
    }) => {
        #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
        #[serde(rename_all = "camelCase")]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        pub struct $t {
            #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
            pub meta: Option<Meta>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub next_cursor: Option<Cursor>,
            pub $i_item: $t_item,
        }

        impl $t {
            pub fn with_all_items(
                items: $t_item,
            ) -> Self {
                Self {
                    meta: None,
                    next_cursor: None,
                    $i_item: items,
                }
            }
        }
    };
}

// =============================================================================
// RESOURCE MANAGEMENT
// =============================================================================

const_string!(ListResourcesRequestMethod = "resources/list");
/// Request to list all available resources from a server
pub type ListResourcesRequest =
    RequestOptionalParam<ListResourcesRequestMethod, PaginatedRequestParams>;

paginated_result!(ListResourcesResult {
    resources: Vec<Resource>
});

const_string!(ListResourceTemplatesRequestMethod = "resources/templates/list");
/// Request to list all available resource templates from a server
pub type ListResourceTemplatesRequest =
    RequestOptionalParam<ListResourceTemplatesRequestMethod, PaginatedRequestParams>;

paginated_result!(ListResourceTemplatesResult {
    resource_templates: Vec<ResourceTemplate>
});

const_string!(ReadResourceRequestMethod = "resources/read");
/// Parameters for reading a specific resource
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ReadResourceRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// The URI of the resource to read
    pub uri: String,
}

impl RequestParamsMeta for ReadResourceRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`ReadResourceRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use ReadResourceRequestParams instead")]
pub type ReadResourceRequestParam = ReadResourceRequestParams;

/// Result containing the contents of a read resource
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ReadResourceResult {
    /// The actual content of the resource
    pub contents: Vec<ResourceContents>,
}

/// Request to read a specific resource
pub type ReadResourceRequest = Request<ReadResourceRequestMethod, ReadResourceRequestParams>;

const_string!(ResourceListChangedNotificationMethod = "notifications/resources/list_changed");
/// Notification sent when the list of available resources changes
pub type ResourceListChangedNotification =
    NotificationNoParam<ResourceListChangedNotificationMethod>;

const_string!(SubscribeRequestMethod = "resources/subscribe");
/// Parameters for subscribing to resource updates
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct SubscribeRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// The URI of the resource to subscribe to
    pub uri: String,
}

impl RequestParamsMeta for SubscribeRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`SubscribeRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use SubscribeRequestParams instead")]
pub type SubscribeRequestParam = SubscribeRequestParams;

/// Request to subscribe to resource updates
pub type SubscribeRequest = Request<SubscribeRequestMethod, SubscribeRequestParams>;

const_string!(UnsubscribeRequestMethod = "resources/unsubscribe");
/// Parameters for unsubscribing from resource updates
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct UnsubscribeRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// The URI of the resource to unsubscribe from
    pub uri: String,
}

impl RequestParamsMeta for UnsubscribeRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`UnsubscribeRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use UnsubscribeRequestParams instead")]
pub type UnsubscribeRequestParam = UnsubscribeRequestParams;

/// Request to unsubscribe from resource updates
pub type UnsubscribeRequest = Request<UnsubscribeRequestMethod, UnsubscribeRequestParams>;

const_string!(ResourceUpdatedNotificationMethod = "notifications/resources/updated");
/// Parameters for a resource update notification
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ResourceUpdatedNotificationParam {
    /// The URI of the resource that was updated
    pub uri: String,
}
/// Notification sent when a subscribed resource is updated
pub type ResourceUpdatedNotification =
    Notification<ResourceUpdatedNotificationMethod, ResourceUpdatedNotificationParam>;

// =============================================================================
// PROMPT MANAGEMENT
// =============================================================================

const_string!(ListPromptsRequestMethod = "prompts/list");
/// Request to list all available prompts from a server
pub type ListPromptsRequest =
    RequestOptionalParam<ListPromptsRequestMethod, PaginatedRequestParams>;

paginated_result!(ListPromptsResult {
    prompts: Vec<Prompt>
});

const_string!(GetPromptRequestMethod = "prompts/get");
/// Parameters for retrieving a specific prompt
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct GetPromptRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<JsonObject>,
}

impl RequestParamsMeta for GetPromptRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`GetPromptRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use GetPromptRequestParams instead")]
pub type GetPromptRequestParam = GetPromptRequestParams;

/// Request to get a specific prompt
pub type GetPromptRequest = Request<GetPromptRequestMethod, GetPromptRequestParams>;

const_string!(PromptListChangedNotificationMethod = "notifications/prompts/list_changed");
/// Notification sent when the list of available prompts changes
pub type PromptListChangedNotification = NotificationNoParam<PromptListChangedNotificationMethod>;

const_string!(ToolListChangedNotificationMethod = "notifications/tools/list_changed");
/// Notification sent when the list of available tools changes
pub type ToolListChangedNotification = NotificationNoParam<ToolListChangedNotificationMethod>;

// =============================================================================
// LOGGING
// =============================================================================

/// Logging levels supported by the MCP protocol
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Copy)]
#[serde(rename_all = "lowercase")] //match spec
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum LoggingLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

const_string!(SetLevelRequestMethod = "logging/setLevel");
/// Parameters for setting the logging level
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct SetLevelRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// The desired logging level
    pub level: LoggingLevel,
}

impl RequestParamsMeta for SetLevelRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`SetLevelRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use SetLevelRequestParams instead")]
pub type SetLevelRequestParam = SetLevelRequestParams;

/// Request to set the logging level
pub type SetLevelRequest = Request<SetLevelRequestMethod, SetLevelRequestParams>;

const_string!(LoggingMessageNotificationMethod = "notifications/message");
/// Parameters for a logging message notification
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct LoggingMessageNotificationParam {
    /// The severity level of this log message
    pub level: LoggingLevel,
    /// Optional logger name that generated this message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    /// The actual log data
    pub data: Value,
}
/// Notification containing a log message
pub type LoggingMessageNotification =
    Notification<LoggingMessageNotificationMethod, LoggingMessageNotificationParam>;

// =============================================================================
// SAMPLING (LLM INTERACTION)
// =============================================================================

const_string!(CreateMessageRequestMethod = "sampling/createMessage");
pub type CreateMessageRequest = Request<CreateMessageRequestMethod, CreateMessageRequestParams>;

/// Represents the role of a participant in a conversation or message exchange.
///
/// Used in sampling and chat contexts to distinguish between different
/// types of message senders in the conversation flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum Role {
    /// A human user or client making a request
    User,
    /// An AI assistant or server providing a response
    Assistant,
}

/// Tool selection mode (SEP-1577).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum ToolChoiceMode {
    /// Model decides whether to use tools
    Auto,
    /// Model must use at least one tool
    Required,
    /// Model must not use tools
    None,
}

impl Default for ToolChoiceMode {
    fn default() -> Self {
        Self::Auto
    }
}

/// Tool choice configuration (SEP-1577).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ToolChoice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<ToolChoiceMode>,
}

impl ToolChoice {
    pub fn auto() -> Self {
        Self {
            mode: Some(ToolChoiceMode::Auto),
        }
    }

    pub fn required() -> Self {
        Self {
            mode: Some(ToolChoiceMode::Required),
        }
    }

    pub fn none() -> Self {
        Self {
            mode: Some(ToolChoiceMode::None),
        }
    }
}

/// Single or array content wrapper (SEP-1577).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum SamplingContent<T> {
    Single(T),
    Multiple(Vec<T>),
}

impl<T> SamplingContent<T> {
    /// Convert to a Vec regardless of whether it's single or multiple
    pub fn into_vec(self) -> Vec<T> {
        match self {
            SamplingContent::Single(item) => vec![item],
            SamplingContent::Multiple(items) => items,
        }
    }

    /// Check if the content is empty
    pub fn is_empty(&self) -> bool {
        match self {
            SamplingContent::Single(_) => false,
            SamplingContent::Multiple(items) => items.is_empty(),
        }
    }

    /// Get the number of content items
    pub fn len(&self) -> usize {
        match self {
            SamplingContent::Single(_) => 1,
            SamplingContent::Multiple(items) => items.len(),
        }
    }
}

impl<T> Default for SamplingContent<T> {
    fn default() -> Self {
        SamplingContent::Multiple(Vec::new())
    }
}

impl<T> SamplingContent<T> {
    /// Get the first item if present
    pub fn first(&self) -> Option<&T> {
        match self {
            SamplingContent::Single(item) => Some(item),
            SamplingContent::Multiple(items) => items.first(),
        }
    }

    /// Iterate over all content items
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let items: Vec<&T> = match self {
            SamplingContent::Single(item) => vec![item],
            SamplingContent::Multiple(items) => items.iter().collect(),
        };
        items.into_iter()
    }
}

impl SamplingMessageContent {
    /// Get the text content if this is a Text variant
    pub fn as_text(&self) -> Option<&RawTextContent> {
        match self {
            SamplingMessageContent::Text(text) => Some(text),
            _ => None,
        }
    }

    /// Get the tool use content if this is a ToolUse variant
    pub fn as_tool_use(&self) -> Option<&ToolUseContent> {
        match self {
            SamplingMessageContent::ToolUse(tool_use) => Some(tool_use),
            _ => None,
        }
    }

    /// Get the tool result content if this is a ToolResult variant
    pub fn as_tool_result(&self) -> Option<&ToolResultContent> {
        match self {
            SamplingMessageContent::ToolResult(tool_result) => Some(tool_result),
            _ => None,
        }
    }
}

impl<T> From<T> for SamplingContent<T> {
    fn from(item: T) -> Self {
        SamplingContent::Single(item)
    }
}

impl<T> From<Vec<T>> for SamplingContent<T> {
    fn from(items: Vec<T>) -> Self {
        SamplingContent::Multiple(items)
    }
}

/// A message in a sampling conversation, containing a role and content.
///
/// This represents a single message in a conversation flow, used primarily
/// in LLM sampling requests where the conversation history is important
/// for generating appropriate responses.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct SamplingMessage {
    /// The role of the message sender (User or Assistant)
    pub role: Role,
    /// The actual content of the message (text, image, audio, tool use, or tool result)
    pub content: SamplingContent<SamplingMessageContent>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Content types for sampling messages (SEP-1577).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum SamplingMessageContent {
    Text(RawTextContent),
    Image(RawImageContent),
    Audio(RawAudioContent),
    /// Assistant only
    ToolUse(ToolUseContent),
    /// User only
    ToolResult(ToolResultContent),
}

impl SamplingMessageContent {
    /// Create a text content
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(RawTextContent {
            text: text.into(),
            meta: None,
        })
    }

    pub fn tool_use(id: impl Into<String>, name: impl Into<String>, input: JsonObject) -> Self {
        Self::ToolUse(ToolUseContent::new(id, name, input))
    }

    pub fn tool_result(tool_use_id: impl Into<String>, content: Vec<Content>) -> Self {
        Self::ToolResult(ToolResultContent::new(tool_use_id, content))
    }
}

impl SamplingMessage {
    pub fn new(role: Role, content: impl Into<SamplingMessageContent>) -> Self {
        Self {
            role,
            content: SamplingContent::Single(content.into()),
            meta: None,
        }
    }

    pub fn new_multiple(role: Role, contents: Vec<SamplingMessageContent>) -> Self {
        Self {
            role,
            content: SamplingContent::Multiple(contents),
            meta: None,
        }
    }

    pub fn user_text(text: impl Into<String>) -> Self {
        Self::new(Role::User, SamplingMessageContent::text(text))
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        Self::new(Role::Assistant, SamplingMessageContent::text(text))
    }

    pub fn user_tool_result(tool_use_id: impl Into<String>, content: Vec<Content>) -> Self {
        Self::new(
            Role::User,
            SamplingMessageContent::tool_result(tool_use_id, content),
        )
    }

    pub fn assistant_tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: JsonObject,
    ) -> Self {
        Self::new(
            Role::Assistant,
            SamplingMessageContent::tool_use(id, name, input),
        )
    }
}

// Conversion from RawTextContent to SamplingMessageContent
impl From<RawTextContent> for SamplingMessageContent {
    fn from(text: RawTextContent) -> Self {
        SamplingMessageContent::Text(text)
    }
}

// Conversion from String to SamplingMessageContent (as text)
impl From<String> for SamplingMessageContent {
    fn from(text: String) -> Self {
        SamplingMessageContent::text(text)
    }
}

impl From<&str> for SamplingMessageContent {
    fn from(text: &str) -> Self {
        SamplingMessageContent::text(text)
    }
}

// Backward compatibility: Convert Content to SamplingMessageContent
// Note: Resource and ResourceLink variants are not supported in sampling messages
impl TryFrom<Content> for SamplingMessageContent {
    type Error = &'static str;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        match content.raw {
            RawContent::Text(text) => Ok(SamplingMessageContent::Text(text)),
            RawContent::Image(image) => Ok(SamplingMessageContent::Image(image)),
            RawContent::Audio(audio) => Ok(SamplingMessageContent::Audio(audio)),
            RawContent::Resource(_) => {
                Err("Resource content is not supported in sampling messages")
            }
            RawContent::ResourceLink(_) => {
                Err("ResourceLink content is not supported in sampling messages")
            }
        }
    }
}

// Backward compatibility: Convert Content to SamplingContent<SamplingMessageContent>
impl TryFrom<Content> for SamplingContent<SamplingMessageContent> {
    type Error = &'static str;

    fn try_from(content: Content) -> Result<Self, Self::Error> {
        Ok(SamplingContent::Single(content.try_into()?))
    }
}

/// Specifies how much context should be included in sampling requests.
///
/// This allows clients to control what additional context information
/// should be provided to the LLM when processing sampling requests.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum ContextInclusion {
    /// Include context from all connected MCP servers
    #[serde(rename = "allServers")]
    AllServers,
    /// Include no additional context
    #[serde(rename = "none")]
    None,
    /// Include context only from the requesting server
    #[serde(rename = "thisServer")]
    ThisServer,
}

/// Parameters for creating a message through LLM sampling.
///
/// This structure contains all the necessary information for a client to
/// generate an LLM response, including conversation history, model preferences,
/// and generation parameters.
///
/// This implements `TaskAugmentedRequestParamsMeta` as sampling requests can be
/// long-running and may benefit from task-based execution.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CreateMessageRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// Task metadata for async task management (SEP-1319)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<JsonObject>,
    /// The conversation history and current messages
    pub messages: Vec<SamplingMessage>,
    /// Preferences for model selection and behavior
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<ModelPreferences>,
    /// System prompt to guide the model's behavior
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// How much context to include from MCP servers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_context: Option<ContextInclusion>,
    /// Temperature for controlling randomness (0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum number of tokens to generate
    pub max_tokens: u32,
    /// Sequences that should stop generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Additional metadata for the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    /// Tools available for the model to call (SEP-1577)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    /// Tool selection behavior (SEP-1577)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

impl RequestParamsMeta for CreateMessageRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

impl TaskAugmentedRequestParamsMeta for CreateMessageRequestParams {
    fn task(&self) -> Option<&JsonObject> {
        self.task.as_ref()
    }
    fn task_mut(&mut self) -> &mut Option<JsonObject> {
        &mut self.task
    }
}

impl CreateMessageRequestParams {
    /// Validate the sampling request parameters per SEP-1577 spec requirements.
    ///
    /// Checks:
    /// - ToolUse content is only allowed in assistant messages
    /// - ToolResult content is only allowed in user messages
    /// - Messages with tool result content MUST NOT contain other content types
    /// - Every assistant ToolUse must be balanced with a corresponding user ToolResult
    pub fn validate(&self) -> Result<(), String> {
        for msg in &self.messages {
            for content in msg.content.iter() {
                // ToolUse only in assistant messages, ToolResult only in user messages
                match content {
                    SamplingMessageContent::ToolUse(_) if msg.role != Role::Assistant => {
                        return Err("ToolUse content is only allowed in assistant messages".into());
                    }
                    SamplingMessageContent::ToolResult(_) if msg.role != Role::User => {
                        return Err("ToolResult content is only allowed in user messages".into());
                    }
                    _ => {}
                }
            }

            // Tool result messages MUST NOT contain other content types
            let contents: Vec<_> = msg.content.iter().collect();
            let has_tool_result = contents
                .iter()
                .any(|c| matches!(c, SamplingMessageContent::ToolResult(_)));
            if has_tool_result
                && contents
                    .iter()
                    .any(|c| !matches!(c, SamplingMessageContent::ToolResult(_)))
            {
                return Err(
                    "SamplingMessage with tool result content MUST NOT contain other content types"
                        .into(),
                );
            }
        }

        // Every assistant ToolUse must be balanced with a user ToolResult
        self.validate_tool_use_result_balance()?;

        Ok(())
    }

    fn validate_tool_use_result_balance(&self) -> Result<(), String> {
        let mut pending_tool_use_ids: Vec<String> = Vec::new();
        for msg in &self.messages {
            if msg.role == Role::Assistant {
                for content in msg.content.iter() {
                    if let SamplingMessageContent::ToolUse(tu) = content {
                        pending_tool_use_ids.push(tu.id.clone());
                    }
                }
            } else if msg.role == Role::User {
                for content in msg.content.iter() {
                    if let SamplingMessageContent::ToolResult(tr) = content {
                        if !pending_tool_use_ids.contains(&tr.tool_use_id) {
                            return Err(format!(
                                "ToolResult with toolUseId '{}' has no matching ToolUse",
                                tr.tool_use_id
                            ));
                        }
                        pending_tool_use_ids.retain(|id| id != &tr.tool_use_id);
                    }
                }
            }
        }
        if !pending_tool_use_ids.is_empty() {
            return Err(format!(
                "ToolUse with id(s) {:?} not balanced with ToolResult",
                pending_tool_use_ids
            ));
        }
        Ok(())
    }
}

/// Deprecated: Use [`CreateMessageRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use CreateMessageRequestParams instead")]
pub type CreateMessageRequestParam = CreateMessageRequestParams;

/// Preferences for model selection and behavior in sampling requests.
///
/// This allows servers to express their preferences for which model to use
/// and how to balance different priorities when the client has multiple
/// model options available.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ModelPreferences {
    /// Specific model names or families to prefer (e.g., "claude", "gpt")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hints: Option<Vec<ModelHint>>,
    /// Priority for cost optimization (0.0 to 1.0, higher = prefer cheaper models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_priority: Option<f32>,
    /// Priority for speed/latency (0.0 to 1.0, higher = prefer faster models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_priority: Option<f32>,
    /// Priority for intelligence/capability (0.0 to 1.0, higher = prefer more capable models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intelligence_priority: Option<f32>,
}

/// A hint suggesting a preferred model name or family.
///
/// Model hints are advisory suggestions that help clients choose appropriate
/// models. They can be specific model names or general families like "claude" or "gpt".
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ModelHint {
    /// The suggested model name or family identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

// =============================================================================
// COMPLETION AND AUTOCOMPLETE
// =============================================================================

/// Context for completion requests providing previously resolved arguments.
///
/// This enables context-aware completion where subsequent argument completions
/// can take into account the values of previously resolved arguments.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CompletionContext {
    /// Previously resolved argument values that can inform completion suggestions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<std::collections::HashMap<String, String>>,
}

impl CompletionContext {
    /// Create a new empty completion context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a completion context with the given arguments
    pub fn with_arguments(arguments: std::collections::HashMap<String, String>) -> Self {
        Self {
            arguments: Some(arguments),
        }
    }

    /// Get a specific argument value by name
    pub fn get_argument(&self, name: &str) -> Option<&String> {
        self.arguments.as_ref()?.get(name)
    }

    /// Check if the context has any arguments
    pub fn has_arguments(&self) -> bool {
        self.arguments.as_ref().is_some_and(|args| !args.is_empty())
    }

    /// Get all argument names
    pub fn argument_names(&self) -> impl Iterator<Item = &str> {
        self.arguments
            .as_ref()
            .into_iter()
            .flat_map(|args| args.keys())
            .map(|k| k.as_str())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CompleteRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub r#ref: Reference,
    pub argument: ArgumentInfo,
    /// Optional context containing previously resolved argument values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<CompletionContext>,
}

impl RequestParamsMeta for CompleteRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`CompleteRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use CompleteRequestParams instead")]
pub type CompleteRequestParam = CompleteRequestParams;

pub type CompleteRequest = Request<CompleteRequestMethod, CompleteRequestParams>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CompletionInfo {
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

impl CompletionInfo {
    /// Maximum number of completion values allowed per response according to MCP specification
    pub const MAX_VALUES: usize = 100;

    /// Create a new CompletionInfo with validation for maximum values
    pub fn new(values: Vec<String>) -> Result<Self, String> {
        if values.len() > Self::MAX_VALUES {
            return Err(format!(
                "Too many completion values: {} (max: {})",
                values.len(),
                Self::MAX_VALUES
            ));
        }
        Ok(Self {
            values,
            total: None,
            has_more: None,
        })
    }

    /// Create CompletionInfo with all values and no pagination
    pub fn with_all_values(values: Vec<String>) -> Result<Self, String> {
        let completion = Self::new(values)?;
        Ok(Self {
            total: Some(completion.values.len() as u32),
            has_more: Some(false),
            ..completion
        })
    }

    /// Create CompletionInfo with pagination information
    pub fn with_pagination(
        values: Vec<String>,
        total: Option<u32>,
        has_more: bool,
    ) -> Result<Self, String> {
        let completion = Self::new(values)?;
        Ok(Self {
            total,
            has_more: Some(has_more),
            ..completion
        })
    }

    /// Check if this completion response indicates more results are available
    pub fn has_more_results(&self) -> bool {
        self.has_more.unwrap_or(false)
    }

    /// Get the total number of available completions, if known
    pub fn total_available(&self) -> Option<u32> {
        self.total
    }

    /// Validate that the completion info complies with MCP specification
    pub fn validate(&self) -> Result<(), String> {
        if self.values.len() > Self::MAX_VALUES {
            return Err(format!(
                "Too many completion values: {} (max: {})",
                self.values.len(),
                Self::MAX_VALUES
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CompleteResult {
    pub completion: CompletionInfo,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum Reference {
    #[serde(rename = "ref/resource")]
    Resource(ResourceReference),
    #[serde(rename = "ref/prompt")]
    Prompt(PromptReference),
}

impl Reference {
    /// Create a prompt reference
    pub fn for_prompt(name: impl Into<String>) -> Self {
        // Not accepting `title` currently as it'll break the API
        // Until further decision, keep it `None`, modify later
        // if required, add `title` to the API
        Self::Prompt(PromptReference {
            name: name.into(),
            title: None,
        })
    }

    /// Create a resource reference
    pub fn for_resource(uri: impl Into<String>) -> Self {
        Self::Resource(ResourceReference { uri: uri.into() })
    }

    /// Get the reference type as a string
    pub fn reference_type(&self) -> &'static str {
        match self {
            Self::Prompt(_) => "ref/prompt",
            Self::Resource(_) => "ref/resource",
        }
    }

    /// Extract prompt name if this is a prompt reference
    pub fn as_prompt_name(&self) -> Option<&str> {
        match self {
            Self::Prompt(prompt_ref) => Some(&prompt_ref.name),
            _ => None,
        }
    }

    /// Extract resource URI if this is a resource reference
    pub fn as_resource_uri(&self) -> Option<&str> {
        match self {
            Self::Resource(resource_ref) => Some(&resource_ref.uri),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ResourceReference {
    pub uri: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PromptReference {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

const_string!(CompleteRequestMethod = "completion/complete");
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ArgumentInfo {
    pub name: String,
    pub value: String,
}

// =============================================================================
// ROOTS AND WORKSPACE MANAGEMENT
// =============================================================================

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Root {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

const_string!(ListRootsRequestMethod = "roots/list");
pub type ListRootsRequest = RequestNoParam<ListRootsRequestMethod>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ListRootsResult {
    pub roots: Vec<Root>,
}

const_string!(RootsListChangedNotificationMethod = "notifications/roots/list_changed");
pub type RootsListChangedNotification = NotificationNoParam<RootsListChangedNotificationMethod>;

// =============================================================================
// ELICITATION (INTERACTIVE USER INPUT)
// =============================================================================

// Method constants for elicitation operations.
// Elicitation allows servers to request interactive input from users during tool execution.
const_string!(ElicitationCreateRequestMethod = "elicitation/create");
const_string!(ElicitationResponseNotificationMethod = "notifications/elicitation/response");
const_string!(ElicitationCompletionNotificationMethod = "notifications/elicitation/complete");

/// Represents the possible actions a user can take in response to an elicitation request.
///
/// When a server requests user input through elicitation, the user can:
/// - Accept: Provide the requested information and continue
/// - Decline: Refuse to provide the information but continue the operation
/// - Cancel: Stop the entire operation
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum ElicitationAction {
    /// User accepts the request and provides the requested information
    Accept,
    /// User declines to provide the information but allows the operation to continue
    Decline,
    /// User cancels the entire operation
    Cancel,
}

/// Helper enum for deserializing CreateElicitationRequestParam with backward compatibility.
/// When mode is missing, it defaults to FormElicitationParam.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "mode")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
enum CreateElicitationRequestParamDeserializeHelper {
    #[serde(rename = "form", rename_all = "camelCase")]
    FormElicitationParam {
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
        message: String,
        requested_schema: ElicitationSchema,
    },
    #[serde(rename = "url", rename_all = "camelCase")]
    UrlElicitationParam {
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
        message: String,
        url: String,
        elicitation_id: String,
    },
    #[serde(untagged, rename_all = "camelCase")]
    FormElicitationParamBackwardsCompat {
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
        message: String,
        requested_schema: ElicitationSchema,
    },
}

impl TryFrom<CreateElicitationRequestParamDeserializeHelper> for CreateElicitationRequestParams {
    type Error = serde_json::Error;

    fn try_from(
        value: CreateElicitationRequestParamDeserializeHelper,
    ) -> Result<Self, Self::Error> {
        match value {
            CreateElicitationRequestParamDeserializeHelper::FormElicitationParam {
                meta,
                message,
                requested_schema,
            }
            | CreateElicitationRequestParamDeserializeHelper::FormElicitationParamBackwardsCompat {
                meta,
                message,
                requested_schema,
            } => Ok(CreateElicitationRequestParams::FormElicitationParams {
                meta,
                message,
                requested_schema,
            }),
            CreateElicitationRequestParamDeserializeHelper::UrlElicitationParam {
                meta,
                message,
                url,
                elicitation_id,
            } => Ok(CreateElicitationRequestParams::UrlElicitationParams {
                meta,
                message,
                url,
                elicitation_id,
            }),
        }
    }
}

/// Parameters for creating an elicitation request to gather user input.
///
/// This structure contains everything needed to request interactive input from a user:
/// - A human-readable message explaining what information is needed
/// - A type-safe schema defining the expected structure of the response
///
/// # Example
/// 1. Form-based elicitation request
/// ```rust
/// use rmcp::model::*;
///
/// let params = CreateElicitationRequestParams::FormElicitationParams {
///    meta: None,
///     message: "Please provide your email".to_string(),
///     requested_schema: ElicitationSchema::builder()
///         .required_email("email")
///         .build()
///         .unwrap(),
/// };
/// ```
/// 2. URL-based elicitation request
/// ```rust
/// use rmcp::model::*;
/// let params = CreateElicitationRequestParams::UrlElicitationParams {
///     meta: None,
///     message: "Please provide your feedback at the following URL".to_string(),
///     url: "https://example.com/feedback".to_string(),
///     elicitation_id: "unique-id-123".to_string(),
/// };
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(
    tag = "mode",
    try_from = "CreateElicitationRequestParamDeserializeHelper"
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum CreateElicitationRequestParams {
    #[serde(rename = "form", rename_all = "camelCase")]
    FormElicitationParams {
        /// Protocol-level metadata for this request (SEP-1319)
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
        /// Human-readable message explaining what input is needed from the user.
        /// This should be clear and provide sufficient context for the user to understand
        /// what information they need to provide.
        message: String,

        /// Type-safe schema defining the expected structure and validation rules for the user's response.
        /// This enforces the MCP 2025-06-18 specification that elicitation schemas must be objects
        /// with primitive-typed properties.
        requested_schema: ElicitationSchema,
    },
    #[serde(rename = "url", rename_all = "camelCase")]
    UrlElicitationParams {
        /// Protocol-level metadata for this request (SEP-1319)
        #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
        meta: Option<Meta>,
        /// Human-readable message explaining what input is needed from the user.
        /// This should be clear and provide sufficient context for the user to understand
        /// what information they need to provide.
        message: String,

        /// The URL where the user can provide the requested information.
        /// The client should direct the user to this URL to complete the elicitation.
        url: String,
        /// The unique identifier for this elicitation request.
        elicitation_id: String,
    },
}

impl RequestParamsMeta for CreateElicitationRequestParams {
    fn meta(&self) -> Option<&Meta> {
        match self {
            CreateElicitationRequestParams::FormElicitationParams { meta, .. } => meta.as_ref(),
            CreateElicitationRequestParams::UrlElicitationParams { meta, .. } => meta.as_ref(),
        }
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        match self {
            CreateElicitationRequestParams::FormElicitationParams { meta, .. } => meta,
            CreateElicitationRequestParams::UrlElicitationParams { meta, .. } => meta,
        }
    }
}

/// Deprecated: Use [`CreateElicitationRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use CreateElicitationRequestParams instead")]
pub type CreateElicitationRequestParam = CreateElicitationRequestParams;

/// The result returned by a client in response to an elicitation request.
///
/// Contains the user's decision (accept/decline/cancel) and optionally their input data
/// if they chose to accept the request.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CreateElicitationResult {
    /// The user's decision on how to handle the elicitation request
    pub action: ElicitationAction,

    /// The actual data provided by the user, if they accepted the request.
    /// Must conform to the JSON schema specified in the original request.
    /// Only present when action is Accept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
}

/// Request type for creating an elicitation to gather user input
pub type CreateElicitationRequest =
    Request<ElicitationCreateRequestMethod, CreateElicitationRequestParams>;

/// Notification parameters for an url elicitation completion notification.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ElicitationResponseNotificationParam {
    pub elicitation_id: String,
}

/// Notification sent when an url elicitation process is completed.
pub type ElicitationCompletionNotification =
    Notification<ElicitationCompletionNotificationMethod, ElicitationResponseNotificationParam>;

// =============================================================================
// TOOL EXECUTION RESULTS
// =============================================================================

/// The result of a tool call operation.
///
/// Contains the content returned by the tool execution and an optional
/// flag indicating whether the operation resulted in an error.
#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CallToolResult {
    /// The content returned by the tool (text, images, etc.)
    pub content: Vec<Content>,
    /// An optional JSON object that represents the structured result of the tool call
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    /// Whether this result represents an error condition
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
    /// Optional protocol-level metadata for this result
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

impl CallToolResult {
    /// Create a successful tool result with unstructured content
    pub fn success(content: Vec<Content>) -> Self {
        CallToolResult {
            content,
            structured_content: None,
            is_error: Some(false),
            meta: None,
        }
    }
    /// Create an error tool result with unstructured content
    pub fn error(content: Vec<Content>) -> Self {
        CallToolResult {
            content,
            structured_content: None,
            is_error: Some(true),
            meta: None,
        }
    }
    /// Create a successful tool result with structured content
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use rmcp::model::CallToolResult;
    /// use serde_json::json;
    ///
    /// let result = CallToolResult::structured(json!({
    ///     "temperature": 22.5,
    ///     "humidity": 65,
    ///     "description": "Partly cloudy"
    /// }));
    /// ```
    pub fn structured(value: Value) -> Self {
        CallToolResult {
            content: vec![Content::text(value.to_string())],
            structured_content: Some(value),
            is_error: Some(false),
            meta: None,
        }
    }
    /// Create an error tool result with structured content
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use rmcp::model::CallToolResult;
    /// use serde_json::json;
    ///
    /// let result = CallToolResult::structured_error(json!({
    ///     "error_code": "INVALID_INPUT",
    ///     "message": "Temperature value out of range",
    ///     "details": {
    ///         "min": -50,
    ///         "max": 50,
    ///         "provided": 100
    ///     }
    /// }));
    /// ```
    pub fn structured_error(value: Value) -> Self {
        CallToolResult {
            content: vec![Content::text(value.to_string())],
            structured_content: Some(value),
            is_error: Some(true),
            meta: None,
        }
    }

    /// Convert the `structured_content` part of response into a certain type.
    ///
    /// # About json schema validation
    /// Since rust is a strong type language, we don't need to do json schema validation here.
    ///
    /// But if you do have to validate the response data, you can use [`jsonschema`](https://crates.io/crates/jsonschema) crate.
    pub fn into_typed<T>(self) -> Result<T, serde_json::Error>
    where
        T: DeserializeOwned,
    {
        let raw_text = match (self.structured_content, &self.content.first()) {
            (Some(value), _) => return serde_json::from_value(value),
            (None, Some(contents)) => {
                if let Some(text) = contents.as_text() {
                    let text = &text.text;
                    Some(text)
                } else {
                    None
                }
            }
            (None, None) => None,
        };
        if let Some(text) = raw_text {
            return serde_json::from_str(text);
        }
        serde_json::from_value(serde_json::Value::Null)
    }
}

// Custom deserialize implementation to validate mutual exclusivity
impl<'de> Deserialize<'de> for CallToolResult {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct CallToolResultHelper {
            #[serde(skip_serializing_if = "Option::is_none")]
            content: Option<Vec<Content>>,
            #[serde(skip_serializing_if = "Option::is_none")]
            structured_content: Option<Value>,
            #[serde(skip_serializing_if = "Option::is_none")]
            is_error: Option<bool>,
            /// Accept `_meta` during deserialization
            #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
            meta: Option<Meta>,
        }

        let helper = CallToolResultHelper::deserialize(deserializer)?;
        let result = CallToolResult {
            content: helper.content.unwrap_or_default(),
            structured_content: helper.structured_content,
            is_error: helper.is_error,
            meta: helper.meta,
        };

        // Validate mutual exclusivity
        if result.content.is_empty() && result.structured_content.is_none() {
            return Err(serde::de::Error::custom(
                "CallToolResult must have either content or structured_content",
            ));
        }

        Ok(result)
    }
}

const_string!(ListToolsRequestMethod = "tools/list");
/// Request to list all available tools from a server
pub type ListToolsRequest = RequestOptionalParam<ListToolsRequestMethod, PaginatedRequestParams>;

paginated_result!(
    ListToolsResult {
        tools: Vec<Tool>
    }
);

const_string!(CallToolRequestMethod = "tools/call");
/// Parameters for calling a tool provided by an MCP server.
///
/// Contains the tool name and optional arguments needed to execute
/// the tool operation.
///
/// This implements `TaskAugmentedRequestParamsMeta` as tool calls can be
/// long-running and may benefit from task-based execution.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CallToolRequestParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// The name of the tool to call
    pub name: Cow<'static, str>,
    /// Arguments to pass to the tool (must match the tool's input schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<JsonObject>,
    /// Task metadata for async task management (SEP-1319)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<JsonObject>,
}

impl RequestParamsMeta for CallToolRequestParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

impl TaskAugmentedRequestParamsMeta for CallToolRequestParams {
    fn task(&self) -> Option<&JsonObject> {
        self.task.as_ref()
    }
    fn task_mut(&mut self) -> &mut Option<JsonObject> {
        &mut self.task
    }
}

/// Deprecated: Use [`CallToolRequestParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use CallToolRequestParams instead")]
pub type CallToolRequestParam = CallToolRequestParams;

/// Request to call a specific tool
pub type CallToolRequest = Request<CallToolRequestMethod, CallToolRequestParams>;

/// Result of sampling/createMessage (SEP-1577).
/// The result of a sampling/createMessage request containing the generated response.
///
/// This structure contains the generated message along with metadata about
/// how the generation was performed and why it stopped.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CreateMessageResult {
    /// The identifier of the model that generated the response
    pub model: String,
    /// The reason why generation stopped (e.g., "endTurn", "maxTokens")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// The generated message with role and content
    #[serde(flatten)]
    pub message: SamplingMessage,
}

impl CreateMessageResult {
    pub const STOP_REASON_END_TURN: &str = "endTurn";
    pub const STOP_REASON_END_SEQUENCE: &str = "stopSequence";
    pub const STOP_REASON_END_MAX_TOKEN: &str = "maxTokens";
    pub const STOP_REASON_TOOL_USE: &str = "toolUse";

    /// Validate the result per SEP-1577: role must be "assistant".
    pub fn validate(&self) -> Result<(), String> {
        if self.message.role != Role::Assistant {
            return Err("CreateMessageResult role must be 'assistant'".into());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct GetPromptResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
}

// =============================================================================
// TASK MANAGEMENT
// =============================================================================

const_string!(GetTaskInfoMethod = "tasks/get");
pub type GetTaskInfoRequest = Request<GetTaskInfoMethod, GetTaskInfoParams>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct GetTaskInfoParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub task_id: String,
}

impl RequestParamsMeta for GetTaskInfoParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`GetTaskInfoParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use GetTaskInfoParams instead")]
pub type GetTaskInfoParam = GetTaskInfoParams;

const_string!(ListTasksMethod = "tasks/list");
pub type ListTasksRequest = RequestOptionalParam<ListTasksMethod, PaginatedRequestParams>;

const_string!(GetTaskResultMethod = "tasks/result");
pub type GetTaskResultRequest = Request<GetTaskResultMethod, GetTaskResultParams>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct GetTaskResultParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub task_id: String,
}

impl RequestParamsMeta for GetTaskResultParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`GetTaskResultParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use GetTaskResultParams instead")]
pub type GetTaskResultParam = GetTaskResultParams;

const_string!(CancelTaskMethod = "tasks/cancel");
pub type CancelTaskRequest = Request<CancelTaskMethod, CancelTaskParams>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CancelTaskParams {
    /// Protocol-level metadata for this request (SEP-1319)
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    pub task_id: String,
}

impl RequestParamsMeta for CancelTaskParams {
    fn meta(&self) -> Option<&Meta> {
        self.meta.as_ref()
    }
    fn meta_mut(&mut self) -> &mut Option<Meta> {
        &mut self.meta
    }
}

/// Deprecated: Use [`CancelTaskParams`] instead (SEP-1319 compliance).
#[deprecated(since = "0.13.0", note = "Use CancelTaskParams instead")]
pub type CancelTaskParam = CancelTaskParams;
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct GetTaskInfoResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<crate::model::Task>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ListTasksResult {
    pub tasks: Vec<crate::model::Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
}

// =============================================================================
// MESSAGE TYPE UNIONS
// =============================================================================

macro_rules! ts_union {
    (
        export type $U:ident =
            $($rest:tt)*
    ) => {
        ts_union!(@declare $U { $($rest)* });
        ts_union!(@impl_from $U { $($rest)* });
    };
    (@declare $U:ident { $($variant:tt)* }) => {
        ts_union!(@declare_variant $U { } {$($variant)*} );
    };
    (@declare_variant $U:ident { $($declared:tt)* } {$(|)? box $V:ident $($rest:tt)*}) => {
        ts_union!(@declare_variant $U { $($declared)* $V(Box<$V>), }  {$($rest)*});
    };
    (@declare_variant $U:ident { $($declared:tt)* } {$(|)? $V:ident $($rest:tt)*}) => {
        ts_union!(@declare_variant $U { $($declared)* $V($V), } {$($rest)*});
    };
    (@declare_variant $U:ident { $($declared:tt)* }  { ; }) => {
        ts_union!(@declare_end $U { $($declared)* } );
    };
    (@declare_end $U:ident { $($declared:tt)* }) => {
        #[derive(Debug, Serialize, Deserialize, Clone)]
        #[serde(untagged)]
        #[allow(clippy::large_enum_variant)]
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        pub enum $U {
            $($declared)*
        }
    };
    (@impl_from $U: ident {$(|)? box $V:ident $($rest:tt)*}) => {
        impl From<$V> for $U {
            fn from(value: $V) -> Self {
                $U::$V(Box::new(value))
            }
        }
        ts_union!(@impl_from $U {$($rest)*});
    };
    (@impl_from $U: ident {$(|)? $V:ident $($rest:tt)*}) => {
        impl From<$V> for $U {
            fn from(value: $V) -> Self {
                $U::$V(value)
            }
        }
        ts_union!(@impl_from $U {$($rest)*});
    };
    (@impl_from $U: ident  { ; }) => {};
    (@impl_from $U: ident  { }) => {};
}

ts_union!(
    export type ClientRequest =
    | PingRequest
    | InitializeRequest
    | CompleteRequest
    | SetLevelRequest
    | GetPromptRequest
    | ListPromptsRequest
    | ListResourcesRequest
    | ListResourceTemplatesRequest
    | ReadResourceRequest
    | SubscribeRequest
    | UnsubscribeRequest
    | CallToolRequest
    | ListToolsRequest
    | GetTaskInfoRequest
    | ListTasksRequest
    | GetTaskResultRequest
    | CancelTaskRequest
    | CustomRequest;
);

impl ClientRequest {
    pub fn method(&self) -> &str {
        match &self {
            ClientRequest::PingRequest(r) => r.method.as_str(),
            ClientRequest::InitializeRequest(r) => r.method.as_str(),
            ClientRequest::CompleteRequest(r) => r.method.as_str(),
            ClientRequest::SetLevelRequest(r) => r.method.as_str(),
            ClientRequest::GetPromptRequest(r) => r.method.as_str(),
            ClientRequest::ListPromptsRequest(r) => r.method.as_str(),
            ClientRequest::ListResourcesRequest(r) => r.method.as_str(),
            ClientRequest::ListResourceTemplatesRequest(r) => r.method.as_str(),
            ClientRequest::ReadResourceRequest(r) => r.method.as_str(),
            ClientRequest::SubscribeRequest(r) => r.method.as_str(),
            ClientRequest::UnsubscribeRequest(r) => r.method.as_str(),
            ClientRequest::CallToolRequest(r) => r.method.as_str(),
            ClientRequest::ListToolsRequest(r) => r.method.as_str(),
            ClientRequest::GetTaskInfoRequest(r) => r.method.as_str(),
            ClientRequest::ListTasksRequest(r) => r.method.as_str(),
            ClientRequest::GetTaskResultRequest(r) => r.method.as_str(),
            ClientRequest::CancelTaskRequest(r) => r.method.as_str(),
            ClientRequest::CustomRequest(r) => r.method.as_str(),
        }
    }
}

ts_union!(
    export type ClientNotification =
    | CancelledNotification
    | ProgressNotification
    | InitializedNotification
    | RootsListChangedNotification
    | CustomNotification;
);

ts_union!(
    export type ClientResult =
    box CreateMessageResult
    | ListRootsResult
    | CreateElicitationResult
    | EmptyResult
    | CustomResult;
);

impl ClientResult {
    pub fn empty(_: ()) -> ClientResult {
        ClientResult::EmptyResult(EmptyResult {})
    }
}

pub type ClientJsonRpcMessage = JsonRpcMessage<ClientRequest, ClientResult, ClientNotification>;

ts_union!(
    export type ServerRequest =
    | PingRequest
    | CreateMessageRequest
    | ListRootsRequest
    | CreateElicitationRequest
    | CustomRequest;
);

ts_union!(
    export type ServerNotification =
    | CancelledNotification
    | ProgressNotification
    | LoggingMessageNotification
    | ResourceUpdatedNotification
    | ResourceListChangedNotification
    | ToolListChangedNotification
    | PromptListChangedNotification
    | ElicitationCompletionNotification
    | CustomNotification;
);

ts_union!(
    export type ServerResult =
    | InitializeResult
    | CompleteResult
    | GetPromptResult
    | ListPromptsResult
    | ListResourcesResult
    | ListResourceTemplatesResult
    | ReadResourceResult
    | CallToolResult
    | ListToolsResult
    | CreateElicitationResult
    | EmptyResult
    | CreateTaskResult
    | ListTasksResult
    | GetTaskInfoResult
    | TaskResult
    | CustomResult
    ;
);

impl ServerResult {
    pub fn empty(_: ()) -> ServerResult {
        ServerResult::EmptyResult(EmptyResult {})
    }
}

pub type ServerJsonRpcMessage = JsonRpcMessage<ServerRequest, ServerResult, ServerNotification>;

impl TryInto<CancelledNotification> for ServerNotification {
    type Error = ServerNotification;
    fn try_into(self) -> Result<CancelledNotification, Self::Error> {
        if let ServerNotification::CancelledNotification(t) = self {
            Ok(t)
        } else {
            Err(self)
        }
    }
}

impl TryInto<CancelledNotification> for ClientNotification {
    type Error = ClientNotification;
    fn try_into(self) -> Result<CancelledNotification, Self::Error> {
        if let ClientNotification::CancelledNotification(t) = self {
            Ok(t)
        } else {
            Err(self)
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_notification_serde() {
        let raw = json!( {
            "jsonrpc": JsonRpcVersion2_0,
            "method": InitializedNotificationMethod,
        });
        let message: ClientJsonRpcMessage =
            serde_json::from_value(raw.clone()).expect("invalid notification");
        match &message {
            ClientJsonRpcMessage::Notification(JsonRpcNotification {
                notification: ClientNotification::InitializedNotification(_n),
                ..
            }) => {}
            _ => panic!("Expected Notification"),
        }
        let json = serde_json::to_value(message).expect("valid json");
        assert_eq!(json, raw);
    }

    #[test]
    fn test_custom_client_notification_roundtrip() {
        let raw = json!( {
            "jsonrpc": JsonRpcVersion2_0,
            "method": "notifications/custom",
            "params": {"foo": "bar"},
        });

        let message: ClientJsonRpcMessage =
            serde_json::from_value(raw.clone()).expect("invalid notification");
        match &message {
            ClientJsonRpcMessage::Notification(JsonRpcNotification {
                notification: ClientNotification::CustomNotification(notification),
                ..
            }) => {
                assert_eq!(notification.method, "notifications/custom");
                assert_eq!(
                    notification
                        .params
                        .as_ref()
                        .and_then(|p| p.get("foo"))
                        .expect("foo present"),
                    "bar"
                );
            }
            _ => panic!("Expected custom client notification"),
        }

        let json = serde_json::to_value(message).expect("valid json");
        assert_eq!(json, raw);
    }

    #[test]
    fn test_custom_server_notification_roundtrip() {
        let raw = json!( {
            "jsonrpc": JsonRpcVersion2_0,
            "method": "notifications/custom-server",
            "params": {"hello": "world"},
        });

        let message: ServerJsonRpcMessage =
            serde_json::from_value(raw.clone()).expect("invalid notification");
        match &message {
            ServerJsonRpcMessage::Notification(JsonRpcNotification {
                notification: ServerNotification::CustomNotification(notification),
                ..
            }) => {
                assert_eq!(notification.method, "notifications/custom-server");
                assert_eq!(
                    notification
                        .params
                        .as_ref()
                        .and_then(|p| p.get("hello"))
                        .expect("hello present"),
                    "world"
                );
            }
            _ => panic!("Expected custom server notification"),
        }

        let json = serde_json::to_value(message).expect("valid json");
        assert_eq!(json, raw);
    }

    #[test]
    fn test_custom_request_roundtrip() {
        let raw = json!( {
            "jsonrpc": JsonRpcVersion2_0,
            "id": 42,
            "method": "requests/custom",
            "params": {"foo": "bar"},
        });

        let message: ClientJsonRpcMessage =
            serde_json::from_value(raw.clone()).expect("invalid request");
        match &message {
            ClientJsonRpcMessage::Request(JsonRpcRequest { id, request, .. }) => {
                assert_eq!(id, &RequestId::Number(42));
                match request {
                    ClientRequest::CustomRequest(custom) => {
                        let expected_request = json!({
                            "method": "requests/custom",
                            "params": {"foo": "bar"},
                        });
                        let actual_request =
                            serde_json::to_value(custom).expect("serialize custom request");
                        assert_eq!(actual_request, expected_request);
                    }
                    other => panic!("Expected custom request, got: {other:?}"),
                }
            }
            other => panic!("Expected request, got: {other:?}"),
        }

        let json = serde_json::to_value(message).expect("valid json");
        assert_eq!(json, raw);
    }

    #[test]
    fn test_request_conversion() {
        let raw = json!( {
            "jsonrpc": JsonRpcVersion2_0,
            "id": 1,
            "method": "request",
            "params": {"key": "value"},
        });
        let message: JsonRpcMessage = serde_json::from_value(raw.clone()).expect("invalid request");

        match &message {
            JsonRpcMessage::Request(r) => {
                assert_eq!(r.id, RequestId::Number(1));
                assert_eq!(r.request.method, "request");
                assert_eq!(
                    &r.request.params,
                    json!({"key": "value"})
                        .as_object()
                        .expect("should be an object")
                );
            }
            _ => panic!("Expected Request"),
        }
        let json = serde_json::to_value(&message).expect("valid json");
        assert_eq!(json, raw);
    }

    #[test]
    fn test_initial_request_response_serde() {
        let request = json!({
          "jsonrpc": "2.0",
          "id": 1,
          "method": "initialize",
          "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
              "roots": {
                "listChanged": true
              },
              "sampling": {}
            },
            "clientInfo": {
              "name": "ExampleClient",
              "version": "1.0.0"
            }
          }
        });
        let raw_response_json = json!({
          "jsonrpc": "2.0",
          "id": 1,
          "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
              "logging": {},
              "prompts": {
                "listChanged": true
              },
              "resources": {
                "subscribe": true,
                "listChanged": true
              },
              "tools": {
                "listChanged": true
              }
            },
            "serverInfo": {
              "name": "ExampleServer",
              "version": "1.0.0"
            }
          }
        });
        let request: ClientJsonRpcMessage =
            serde_json::from_value(request.clone()).expect("invalid request");
        let (request, id) = request.into_request().expect("should be a request");
        assert_eq!(id, RequestId::Number(1));
        #[allow(deprecated)]
        match request {
            ClientRequest::InitializeRequest(Request {
                method: _,
                params:
                    InitializeRequestParam {
                        meta: _,
                        protocol_version: _,
                        capabilities,
                        client_info,
                    },
                ..
            }) => {
                assert_eq!(capabilities.roots.unwrap().list_changed, Some(true));
                let sampling = capabilities.sampling.unwrap();
                assert_eq!(sampling.tools, None);
                assert_eq!(sampling.context, None);
                assert_eq!(client_info.name, "ExampleClient");
                assert_eq!(client_info.version, "1.0.0");
            }
            _ => panic!("Expected InitializeRequest"),
        }
        let server_response: ServerJsonRpcMessage =
            serde_json::from_value(raw_response_json.clone()).expect("invalid response");
        let (response, id) = server_response
            .clone()
            .into_response()
            .expect("expect response");
        assert_eq!(id, RequestId::Number(1));
        match response {
            ServerResult::InitializeResult(InitializeResult {
                protocol_version: _,
                capabilities,
                server_info,
                instructions,
            }) => {
                assert_eq!(capabilities.logging.unwrap().len(), 0);
                assert_eq!(capabilities.prompts.unwrap().list_changed, Some(true));
                assert_eq!(
                    capabilities.resources.as_ref().unwrap().subscribe,
                    Some(true)
                );
                assert_eq!(capabilities.resources.unwrap().list_changed, Some(true));
                assert_eq!(capabilities.tools.unwrap().list_changed, Some(true));
                assert_eq!(server_info.name, "ExampleServer");
                assert_eq!(server_info.version, "1.0.0");
                assert_eq!(server_info.icons, None);
                assert_eq!(instructions, None);
            }
            other => panic!("Expected InitializeResult, got {other:?}"),
        }

        let server_response_json: Value = serde_json::to_value(&server_response).expect("msg");

        assert_eq!(server_response_json, raw_response_json);
    }

    #[test]
    fn test_negative_and_large_request_ids() {
        // Test negative ID
        let negative_id_json = json!({
            "jsonrpc": "2.0",
            "id": -1,
            "method": "test",
            "params": {}
        });

        let message: JsonRpcMessage =
            serde_json::from_value(negative_id_json.clone()).expect("Should parse negative ID");

        match &message {
            JsonRpcMessage::Request(r) => {
                assert_eq!(r.id, RequestId::Number(-1));
            }
            _ => panic!("Expected Request"),
        }

        // Test roundtrip serialization
        let serialized = serde_json::to_value(&message).expect("Should serialize");
        assert_eq!(serialized, negative_id_json);

        // Test large negative ID
        let large_negative_json = json!({
            "jsonrpc": "2.0",
            "id": -9007199254740991i64,  // JavaScript's MIN_SAFE_INTEGER
            "method": "test",
            "params": {}
        });

        let message: JsonRpcMessage = serde_json::from_value(large_negative_json.clone())
            .expect("Should parse large negative ID");

        match &message {
            JsonRpcMessage::Request(r) => {
                assert_eq!(r.id, RequestId::Number(-9007199254740991i64));
            }
            _ => panic!("Expected Request"),
        }

        // Test large positive ID (JavaScript's MAX_SAFE_INTEGER)
        let large_positive_json = json!({
            "jsonrpc": "2.0",
            "id": 9007199254740991i64,
            "method": "test",
            "params": {}
        });

        let message: JsonRpcMessage = serde_json::from_value(large_positive_json.clone())
            .expect("Should parse large positive ID");

        match &message {
            JsonRpcMessage::Request(r) => {
                assert_eq!(r.id, RequestId::Number(9007199254740991i64));
            }
            _ => panic!("Expected Request"),
        }

        // Test zero ID
        let zero_id_json = json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "test",
            "params": {}
        });

        let message: JsonRpcMessage =
            serde_json::from_value(zero_id_json.clone()).expect("Should parse zero ID");

        match &message {
            JsonRpcMessage::Request(r) => {
                assert_eq!(r.id, RequestId::Number(0));
            }
            _ => panic!("Expected Request"),
        }
    }

    #[test]
    fn test_protocol_version_order() {
        let v1 = ProtocolVersion::V_2024_11_05;
        let v2 = ProtocolVersion::V_2025_03_26;
        assert!(v1 < v2);
    }

    #[test]
    fn test_icon_serialization() {
        let icon = Icon {
            src: "https://example.com/icon.png".to_string(),
            mime_type: Some("image/png".to_string()),
            sizes: Some(vec!["48x48".to_string()]),
        };

        let json = serde_json::to_value(&icon).unwrap();
        assert_eq!(json["src"], "https://example.com/icon.png");
        assert_eq!(json["mimeType"], "image/png");
        assert_eq!(json["sizes"][0], "48x48");

        // Test deserialization
        let deserialized: Icon = serde_json::from_value(json).unwrap();
        assert_eq!(deserialized, icon);
    }

    #[test]
    fn test_icon_minimal() {
        let icon = Icon {
            src: "data:image/svg+xml;base64,PHN2Zy8+".to_string(),
            mime_type: None,
            sizes: None,
        };

        let json = serde_json::to_value(&icon).unwrap();
        assert_eq!(json["src"], "data:image/svg+xml;base64,PHN2Zy8+");
        assert!(json.get("mimeType").is_none());
        assert!(json.get("sizes").is_none());
    }

    #[test]
    fn test_implementation_with_icons() {
        let implementation = Implementation {
            name: "test-server".to_string(),
            title: Some("Test Server".to_string()),
            version: "1.0.0".to_string(),
            description: Some("A test server for unit testing".to_string()),
            icons: Some(vec![
                Icon {
                    src: "https://example.com/icon.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                    sizes: Some(vec!["48x48".to_string()]),
                },
                Icon {
                    src: "https://example.com/icon.svg".to_string(),
                    mime_type: Some("image/svg+xml".to_string()),
                    sizes: Some(vec!["any".to_string()]),
                },
            ]),
            website_url: Some("https://example.com".to_string()),
        };

        let json = serde_json::to_value(&implementation).unwrap();
        assert_eq!(json["name"], "test-server");
        assert_eq!(json["description"], "A test server for unit testing");
        assert_eq!(json["websiteUrl"], "https://example.com");
        assert!(json["icons"].is_array());
        assert_eq!(json["icons"][0]["src"], "https://example.com/icon.png");
        assert_eq!(json["icons"][0]["sizes"][0], "48x48");
        assert_eq!(json["icons"][1]["mimeType"], "image/svg+xml");
        assert_eq!(json["icons"][1]["sizes"][0], "any");
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that old JSON without icons still deserializes correctly
        let old_json = json!({
            "name": "legacy-server",
            "version": "0.9.0"
        });

        let implementation: Implementation = serde_json::from_value(old_json).unwrap();
        assert_eq!(implementation.name, "legacy-server");
        assert_eq!(implementation.version, "0.9.0");
        assert_eq!(implementation.description, None);
        assert_eq!(implementation.icons, None);
        assert_eq!(implementation.website_url, None);
    }

    #[test]
    fn test_initialize_with_icons() {
        let init_result = InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::default(),
            server_info: Implementation {
                name: "icon-server".to_string(),
                title: None,
                version: "2.0.0".to_string(),
                description: None,
                icons: Some(vec![Icon {
                    src: "https://example.com/server.png".to_string(),
                    mime_type: Some("image/png".to_string()),
                    sizes: Some(vec!["48x48".to_string()]),
                }]),
                website_url: Some("https://docs.example.com".to_string()),
            },
            instructions: None,
        };

        let json = serde_json::to_value(&init_result).unwrap();
        assert!(json["serverInfo"]["icons"].is_array());
        assert_eq!(
            json["serverInfo"]["icons"][0]["src"],
            "https://example.com/server.png"
        );
        assert_eq!(json["serverInfo"]["icons"][0]["sizes"][0], "48x48");
        assert_eq!(json["serverInfo"]["websiteUrl"], "https://docs.example.com");
    }

    #[test]
    fn test_elicitation_deserialization_untagged() {
        // Test deserialization without the "type" field (should default to FormElicitationParam)
        let json_data_without_tag = json!({
            "message": "Please provide more details.",
            "requestedSchema": {
                "title": "User Details",
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "age": { "type": "integer" }
                },
                "required": ["name", "age"]
            }
        });
        let elicitation: CreateElicitationRequestParams =
            serde_json::from_value(json_data_without_tag).expect("Deserialization failed");
        if let CreateElicitationRequestParams::FormElicitationParams {
            meta,
            message,
            requested_schema,
        } = elicitation
        {
            assert_eq!(meta, None);
            assert_eq!(message, "Please provide more details.");
            assert_eq!(requested_schema.title, Some(Cow::from("User Details")));
            assert_eq!(requested_schema.type_, ObjectTypeConst);
        } else {
            panic!("Expected FormElicitationParam");
        }
    }

    #[test]
    fn test_elicitation_deserialization() {
        let json_data_form = json!({
            "_meta": { "meta_form_key_1": "meta form value 1" },
            "mode": "form",
            "message": "Please provide more details.",
            "requestedSchema": {
                "title": "User Details",
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "age": { "type": "integer" }
                },
                "required": ["name", "age"]
            }
        });
        let elicitation_form: CreateElicitationRequestParams =
            serde_json::from_value(json_data_form).expect("Deserialization failed");
        if let CreateElicitationRequestParams::FormElicitationParams {
            meta,
            message,
            requested_schema,
        } = elicitation_form
        {
            assert_eq!(
                meta,
                Some(Meta(object!({ "meta_form_key_1": "meta form value 1" })))
            );
            assert_eq!(message, "Please provide more details.");
            assert_eq!(requested_schema.title, Some(Cow::from("User Details")));
            assert_eq!(requested_schema.type_, ObjectTypeConst);
        } else {
            panic!("Expected FormElicitationParam");
        }

        let json_data_url = json!({
                "_meta": { "meta_url_key_1": "meta url value 1" },
            "mode": "url",
            "message": "Please fill out the form at the following URL.",
            "url": "https://example.com/form",
            "elicitationId": "elicitation-123"
        });
        let elicitation_url: CreateElicitationRequestParams =
            serde_json::from_value(json_data_url).expect("Deserialization failed");
        if let CreateElicitationRequestParams::UrlElicitationParams {
            meta,
            message,
            url,
            elicitation_id,
        } = elicitation_url
        {
            assert_eq!(
                meta,
                Some(Meta(object!({ "meta_url_key_1": "meta url value 1" })))
            );
            assert_eq!(message, "Please fill out the form at the following URL.");
            assert_eq!(url, "https://example.com/form");
            assert_eq!(elicitation_id, "elicitation-123");
        } else {
            panic!("Expected UrlElicitationParam");
        }
    }

    #[test]
    fn test_elicitation_serialization() {
        let form_elicitation = CreateElicitationRequestParams::FormElicitationParams {
            meta: Some(Meta(object!({ "meta_form_key_1": "meta form value 1" }))),
            message: "Please provide more details.".to_string(),
            requested_schema: ElicitationSchema::builder()
                .title("User Details")
                .string_property("name", |s| s)
                .build()
                .expect("Valid schema"),
        };
        let json_form = serde_json::to_value(&form_elicitation).expect("Serialization failed");
        let expected_form_json = json!({
            "_meta": { "meta_form_key_1": "meta form value 1" },
            "mode": "form",
            "message": "Please provide more details.",
            "requestedSchema": {
                "title":"User Details",
                "type":"object",
                "properties":{
                    "name": { "type": "string" },
                },
            }
        });
        assert_eq!(json_form, expected_form_json);

        let url_elicitation = CreateElicitationRequestParams::UrlElicitationParams {
            meta: Some(Meta(object!({ "meta_url_key_1": "meta url value 1" }))),
            message: "Please fill out the form at the following URL.".to_string(),
            url: "https://example.com/form".to_string(),
            elicitation_id: "elicitation-123".to_string(),
        };
        let json_url = serde_json::to_value(&url_elicitation).expect("Serialization failed");
        let expected_url_json = json!({
            "_meta": { "meta_url_key_1": "meta url value 1" },
            "mode": "url",
            "message": "Please fill out the form at the following URL.",
            "url": "https://example.com/form",
            "elicitationId": "elicitation-123"
        });
        assert_eq!(json_url, expected_url_json);
    }
}
