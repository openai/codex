use std::ops::{Deref, DerefMut};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    ClientNotification, ClientRequest, CustomNotification, CustomRequest, Extensions, JsonObject,
    JsonRpcMessage, NumberOrString, ProgressToken, ServerNotification, ServerRequest,
};

pub trait GetMeta {
    fn get_meta_mut(&mut self) -> &mut Meta;
    fn get_meta(&self) -> &Meta;
}

pub trait GetExtensions {
    fn extensions(&self) -> &Extensions;
    fn extensions_mut(&mut self) -> &mut Extensions;
}

/// Trait for request params that contain the `_meta` field.
///
/// Per the MCP 2025-11-25 spec, all request params should have an optional `_meta`
/// field that can contain a `progressToken` for tracking long-running operations.
pub trait RequestParamsMeta {
    /// Get a reference to the meta field
    fn meta(&self) -> Option<&Meta>;
    /// Get a mutable reference to the meta field
    fn meta_mut(&mut self) -> &mut Option<Meta>;
    /// Set the meta field
    fn set_meta(&mut self, meta: Meta) {
        *self.meta_mut() = Some(meta);
    }
    /// Get the progress token from meta, if present
    fn progress_token(&self) -> Option<ProgressToken> {
        self.meta().and_then(|m| m.get_progress_token())
    }
    /// Set a progress token in meta
    fn set_progress_token(&mut self, token: ProgressToken) {
        match self.meta_mut() {
            Some(meta) => meta.set_progress_token(token),
            none => {
                let mut meta = Meta::new();
                meta.set_progress_token(token);
                *none = Some(meta);
            }
        }
    }
}

/// Trait for task-augmented request params that contain both `_meta` and `task` fields.
///
/// Per the MCP 2025-11-25 spec, certain requests (like `tools/call` and `sampling/createMessage`)
/// can include a `task` field to signal that the caller wants task-augmented execution.
pub trait TaskAugmentedRequestParamsMeta: RequestParamsMeta {
    /// Get a reference to the task field
    fn task(&self) -> Option<&JsonObject>;
    /// Get a mutable reference to the task field
    fn task_mut(&mut self) -> &mut Option<JsonObject>;
    /// Set the task field
    fn set_task(&mut self, task: JsonObject) {
        *self.task_mut() = Some(task);
    }
}

impl GetExtensions for CustomNotification {
    fn extensions(&self) -> &Extensions {
        &self.extensions
    }
    fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }
}

impl GetMeta for CustomNotification {
    fn get_meta_mut(&mut self) -> &mut Meta {
        self.extensions_mut().get_or_insert_default()
    }
    fn get_meta(&self) -> &Meta {
        self.extensions()
            .get::<Meta>()
            .unwrap_or(Meta::static_empty())
    }
}

impl GetExtensions for CustomRequest {
    fn extensions(&self) -> &Extensions {
        &self.extensions
    }
    fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }
}

impl GetMeta for CustomRequest {
    fn get_meta_mut(&mut self) -> &mut Meta {
        self.extensions_mut().get_or_insert_default()
    }
    fn get_meta(&self) -> &Meta {
        self.extensions()
            .get::<Meta>()
            .unwrap_or(Meta::static_empty())
    }
}

macro_rules! variant_extension {
    (
        $Enum: ident {
            $($variant: ident)*
        }
    ) => {
        impl GetExtensions for $Enum {
            fn extensions(&self) -> &Extensions {
                match self {
                    $(
                        $Enum::$variant(v) => &v.extensions,
                    )*
                }
            }
            fn extensions_mut(&mut self) -> &mut Extensions {
                match self {
                    $(
                        $Enum::$variant(v) => &mut v.extensions,
                    )*
                }
            }
        }
        impl GetMeta for $Enum {
            fn get_meta_mut(&mut self) -> &mut Meta {
                self.extensions_mut().get_or_insert_default()
            }
            fn get_meta(&self) -> &Meta {
                self.extensions().get::<Meta>().unwrap_or(Meta::static_empty())
            }
        }
    };
}

variant_extension! {
    ClientRequest {
        PingRequest
        InitializeRequest
        CompleteRequest
        SetLevelRequest
        GetPromptRequest
        ListPromptsRequest
        ListResourcesRequest
        ListResourceTemplatesRequest
        ReadResourceRequest
        SubscribeRequest
        UnsubscribeRequest
        CallToolRequest
        ListToolsRequest
        CustomRequest
        GetTaskInfoRequest
        ListTasksRequest
        GetTaskResultRequest
        CancelTaskRequest
    }
}

variant_extension! {
    ServerRequest {
        PingRequest
        CreateMessageRequest
        ListRootsRequest
        CreateElicitationRequest
        CustomRequest
    }
}

variant_extension! {
    ClientNotification {
        CancelledNotification
        ProgressNotification
        InitializedNotification
        RootsListChangedNotification
        CustomNotification
    }
}

variant_extension! {
    ServerNotification {
        CancelledNotification
        ProgressNotification
        LoggingMessageNotification
        ResourceUpdatedNotification
        ResourceListChangedNotification
        ToolListChangedNotification
        PromptListChangedNotification
        ElicitationCompletionNotification
        CustomNotification
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct Meta(pub JsonObject);
const PROGRESS_TOKEN_FIELD: &str = "progressToken";
impl Meta {
    pub fn new() -> Self {
        Self(JsonObject::new())
    }

    /// Create a new Meta with a progress token set
    pub fn with_progress_token(token: ProgressToken) -> Self {
        let mut meta = Self::new();
        meta.set_progress_token(token);
        meta
    }

    pub(crate) fn static_empty() -> &'static Self {
        static EMPTY: std::sync::OnceLock<Meta> = std::sync::OnceLock::new();
        EMPTY.get_or_init(Default::default)
    }

    pub fn get_progress_token(&self) -> Option<ProgressToken> {
        self.0.get(PROGRESS_TOKEN_FIELD).and_then(|v| match v {
            Value::String(s) => Some(ProgressToken(NumberOrString::String(s.to_string().into()))),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(ProgressToken(NumberOrString::Number(i)))
                } else if let Some(u) = n.as_u64() {
                    if u <= i64::MAX as u64 {
                        Some(ProgressToken(NumberOrString::Number(u as i64)))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        })
    }

    pub fn set_progress_token(&mut self, token: ProgressToken) {
        match token.0 {
            NumberOrString::String(ref s) => self.0.insert(
                PROGRESS_TOKEN_FIELD.to_string(),
                Value::String(s.to_string()),
            ),
            NumberOrString::Number(n) => self
                .0
                .insert(PROGRESS_TOKEN_FIELD.to_string(), Value::Number(n.into())),
        };
    }

    pub fn extend(&mut self, other: Meta) {
        for (k, v) in other.0.into_iter() {
            self.0.insert(k, v);
        }
    }
}

impl Deref for Meta {
    type Target = JsonObject;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Meta {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Req, Resp, Noti> JsonRpcMessage<Req, Resp, Noti>
where
    Req: GetExtensions,
    Noti: GetExtensions,
{
    pub fn insert_extension<T: Clone + Send + Sync + 'static>(&mut self, value: T) {
        match self {
            JsonRpcMessage::Request(json_rpc_request) => {
                json_rpc_request.request.extensions_mut().insert(value);
            }
            JsonRpcMessage::Notification(json_rpc_notification) => {
                json_rpc_notification
                    .notification
                    .extensions_mut()
                    .insert(value);
            }
            _ => {}
        }
    }
}
