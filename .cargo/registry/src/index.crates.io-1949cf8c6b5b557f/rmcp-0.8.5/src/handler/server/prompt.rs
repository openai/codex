//! Prompt handling infrastructure for MCP servers
//!
//! This module provides the core types and traits for implementing prompt handlers
//! in MCP servers. Prompts allow servers to provide reusable templates for LLM
//! interactions with customizable arguments.

use std::{future::Future, marker::PhantomData};

use futures::future::{BoxFuture, FutureExt};
use serde::de::DeserializeOwned;

use super::common::{AsRequestContext, FromContextPart};
pub use super::common::{Extension, RequestId};
use crate::{
    RoleServer,
    handler::server::wrapper::Parameters,
    model::{GetPromptResult, PromptMessage},
    service::RequestContext,
};

/// Context for prompt retrieval operations
pub struct PromptContext<'a, S> {
    pub server: &'a S,
    pub name: String,
    pub arguments: Option<serde_json::Map<String, serde_json::Value>>,
    pub context: RequestContext<RoleServer>,
}

impl<'a, S> PromptContext<'a, S> {
    pub fn new(
        server: &'a S,
        name: String,
        arguments: Option<serde_json::Map<String, serde_json::Value>>,
        context: RequestContext<RoleServer>,
    ) -> Self {
        Self {
            server,
            name,
            arguments,
            context,
        }
    }
}

impl<S> AsRequestContext for PromptContext<'_, S> {
    fn as_request_context(&self) -> &RequestContext<RoleServer> {
        &self.context
    }

    fn as_request_context_mut(&mut self) -> &mut RequestContext<RoleServer> {
        &mut self.context
    }
}

/// Trait for handling prompt retrieval
pub trait GetPromptHandler<S, A> {
    fn handle(
        self,
        context: PromptContext<'_, S>,
    ) -> BoxFuture<'_, Result<GetPromptResult, crate::ErrorData>>;
}

/// Type alias for dynamic prompt handlers
pub type DynGetPromptHandler<S> = dyn for<'a> Fn(PromptContext<'a, S>) -> BoxFuture<'a, Result<GetPromptResult, crate::ErrorData>>
    + Send
    + Sync;

/// Adapter type for async methods that return `Vec<PromptMessage>`
pub struct AsyncMethodAdapter<T>(PhantomData<T>);

/// Adapter type for async methods with parameters that return `Vec<PromptMessage>`
pub struct AsyncMethodWithArgsAdapter<T>(PhantomData<T>);

/// Adapter types for macro-generated implementations
#[allow(clippy::type_complexity)]
pub struct AsyncPromptAdapter<P, Fut, R>(PhantomData<fn(P) -> fn(Fut) -> R>);
pub struct SyncPromptAdapter<P, R>(PhantomData<fn(P) -> R>);
pub struct AsyncPromptMethodAdapter<P, R>(PhantomData<fn(P) -> R>);
pub struct SyncPromptMethodAdapter<P, R>(PhantomData<fn(P) -> R>);

/// Trait for types that can be converted into GetPromptResult
pub trait IntoGetPromptResult {
    fn into_get_prompt_result(self) -> Result<GetPromptResult, crate::ErrorData>;
}

impl IntoGetPromptResult for GetPromptResult {
    fn into_get_prompt_result(self) -> Result<GetPromptResult, crate::ErrorData> {
        Ok(self)
    }
}

impl IntoGetPromptResult for Vec<PromptMessage> {
    fn into_get_prompt_result(self) -> Result<GetPromptResult, crate::ErrorData> {
        Ok(GetPromptResult {
            description: None,
            messages: self,
        })
    }
}

impl<T: IntoGetPromptResult> IntoGetPromptResult for Result<T, crate::ErrorData> {
    fn into_get_prompt_result(self) -> Result<GetPromptResult, crate::ErrorData> {
        self.and_then(|v| v.into_get_prompt_result())
    }
}

// Future wrapper that automatically handles IntoGetPromptResult conversion
pin_project_lite::pin_project! {
    #[project = IntoGetPromptResultFutProj]
    pub enum IntoGetPromptResultFut<F, R> {
        Pending {
            #[pin]
            fut: F,
            _marker: PhantomData<R>,
        },
        Ready {
            #[pin]
            result: futures::future::Ready<Result<GetPromptResult, crate::ErrorData>>,
        }
    }
}

impl<F, R> Future for IntoGetPromptResultFut<F, R>
where
    F: Future<Output = R>,
    R: IntoGetPromptResult,
{
    type Output = Result<GetPromptResult, crate::ErrorData>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.project() {
            IntoGetPromptResultFutProj::Pending { fut, _marker } => fut
                .poll(cx)
                .map(IntoGetPromptResult::into_get_prompt_result),
            IntoGetPromptResultFutProj::Ready { result } => result.poll(cx),
        }
    }
}

// Prompt-specific extractor for prompt name
pub struct PromptName(pub String);

impl<S> FromContextPart<PromptContext<'_, S>> for PromptName {
    fn from_context_part(context: &mut PromptContext<S>) -> Result<Self, crate::ErrorData> {
        Ok(Self(context.name.clone()))
    }
}

// Special implementation for Parameters that handles prompt arguments
impl<S, P> FromContextPart<PromptContext<'_, S>> for Parameters<P>
where
    P: DeserializeOwned,
{
    fn from_context_part(context: &mut PromptContext<S>) -> Result<Self, crate::ErrorData> {
        let params = if let Some(args_map) = context.arguments.take() {
            let args_value = serde_json::Value::Object(args_map);
            serde_json::from_value::<P>(args_value).map_err(|e| {
                crate::ErrorData::invalid_params(format!("Failed to parse parameters: {}", e), None)
            })?
        } else {
            // Try to deserialize from empty object for optional fields
            serde_json::from_value::<P>(serde_json::json!({})).map_err(|e| {
                crate::ErrorData::invalid_params(
                    format!("Missing required parameters: {}", e),
                    None,
                )
            })?
        };
        Ok(Parameters(params))
    }
}

// Macro to generate GetPromptHandler implementations for various parameter combinations
macro_rules! impl_prompt_handler_for {
    ($($T: ident)*) => {
        impl_prompt_handler_for!([] [$($T)*]);
    };
    // finished
    ([$($Tn: ident)*] []) => {
        impl_prompt_handler_for!(@impl $($Tn)*);
    };
    ([$($Tn: ident)*] [$Tn_1: ident $($Rest: ident)*]) => {
        impl_prompt_handler_for!(@impl $($Tn)*);
        impl_prompt_handler_for!([$($Tn)* $Tn_1] [$($Rest)*]);
    };
    (@impl $($Tn: ident)*) => {
        // Implementation for async methods (transformed by #[prompt] macro)
        impl<$($Tn,)* S, F, R> GetPromptHandler<S, ($($Tn,)*)> for F
        where
            $(
                $Tn: for<'a> FromContextPart<PromptContext<'a, S>> + Send,
            )*
            F: FnOnce(&S, $($Tn,)*) -> BoxFuture<'_, R> + Send,
            R: IntoGetPromptResult + Send + 'static,
            S: Send + Sync + 'static,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn handle(
                self,
                mut context: PromptContext<'_, S>,
            ) -> BoxFuture<'_, Result<GetPromptResult, crate::ErrorData>>
            {
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*
                let service = context.server;
                let fut = self(service, $($Tn,)*);
                async move {
                    let result = fut.await;
                    result.into_get_prompt_result()
                }.boxed()
            }
        }


        // Implementation for sync methods
        impl<$($Tn,)* S, F, R> GetPromptHandler<S, SyncPromptMethodAdapter<($($Tn,)*), R>> for F
        where
            $(
                $Tn: for<'a> FromContextPart<PromptContext<'a, S>> + Send,
            )*
            F: FnOnce(&S, $($Tn,)*) -> R + Send,
            R: IntoGetPromptResult + Send,
            S: Send + Sync,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn handle(
                self,
                mut context: PromptContext<'_, S>,
            ) -> BoxFuture<'_, Result<GetPromptResult, crate::ErrorData>>
            {
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*
                let service = context.server;
                let result = self(service, $($Tn,)*);
                std::future::ready(result.into_get_prompt_result()).boxed()
            }
        }


        // AsyncPromptAdapter - for standalone functions returning GetPromptResult
        impl<$($Tn,)* S, F, Fut, R> GetPromptHandler<S, AsyncPromptAdapter<($($Tn,)*), Fut, R>> for F
        where
            $(
                $Tn: for<'a> FromContextPart<PromptContext<'a, S>> + Send + 'static,
            )*
            F: FnOnce($($Tn,)*) -> Fut + Send + 'static,
            Fut: Future<Output = Result<R, crate::ErrorData>> + Send + 'static,
            R: IntoGetPromptResult + Send + 'static,
            S: Send + Sync + 'static,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn handle(
                self,
                mut context: PromptContext<'_, S>,
            ) -> BoxFuture<'_, Result<GetPromptResult, crate::ErrorData>>
            {
                // Extract all parameters before moving into the async block
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*

                // Since we're dealing with standalone functions that don't take &S,
                // we can return a 'static future
                Box::pin(async move {
                    let result = self($($Tn,)*).await?;
                    result.into_get_prompt_result()
                })
            }
        }


        // SyncPromptAdapter - for standalone sync functions returning Result
        impl<$($Tn,)* S, F, R> GetPromptHandler<S, SyncPromptAdapter<($($Tn,)*), R>> for F
        where
            $(
                $Tn: for<'a> FromContextPart<PromptContext<'a, S>> + Send + 'static,
            )*
            F: FnOnce($($Tn,)*) -> Result<R, crate::ErrorData> + Send + 'static,
            R: IntoGetPromptResult + Send + 'static,
            S: Send + Sync,
        {
            #[allow(unused_variables, non_snake_case, unused_mut)]
            fn handle(
                self,
                mut context: PromptContext<'_, S>,
            ) -> BoxFuture<'_, Result<GetPromptResult, crate::ErrorData>>
            {
                $(
                    let result = $Tn::from_context_part(&mut context);
                    let $Tn = match result {
                        Ok(value) => value,
                        Err(e) => return std::future::ready(Err(e)).boxed(),
                    };
                )*
                let result = self($($Tn,)*);
                std::future::ready(result.and_then(|r| r.into_get_prompt_result())).boxed()
            }
        }

    };
}

// Invoke the macro to generate implementations for up to 16 parameters
impl_prompt_handler_for!(T0 T1 T2 T3 T4 T5 T6 T7 T8 T9 T10 T11 T12 T13 T14 T15);

/// Extract prompt arguments from a type's JSON schema
/// This function analyzes the schema of a type and extracts the properties
/// as PromptArgument entries with name, description, and required status
pub fn cached_arguments_from_schema<T: schemars::JsonSchema + std::any::Any>()
-> Option<Vec<crate::model::PromptArgument>> {
    let schema = super::common::cached_schema_for_type::<T>();
    let schema_value = serde_json::Value::Object((*schema).clone());

    let properties = schema_value.get("properties").and_then(|p| p.as_object());

    if let Some(props) = properties {
        let required = schema_value
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<std::collections::HashSet<_>>()
            })
            .unwrap_or_default();

        let mut arguments = Vec::new();
        for (name, prop_schema) in props {
            let description = prop_schema
                .get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());

            arguments.push(crate::model::PromptArgument {
                name: name.clone(),
                title: None,
                description,
                required: Some(required.contains(name.as_str())),
            });
        }

        if arguments.is_empty() {
            None
        } else {
            Some(arguments)
        }
    } else {
        None
    }
}
