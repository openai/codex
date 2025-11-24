use std::collections::HashMap;

use futures::future::BoxFuture;
use rmcp::{
    ServerHandler,
    handler::server::wrapper::Parameters,
    model::{GetPromptResult, PromptMessage, PromptMessageRole},
};

#[derive(Debug, Default)]
pub struct TestHandler<T: 'static = ()> {
    pub _marker: std::marker::PhantomData<fn(*const T)>,
}

impl<T: 'static> ServerHandler for TestHandler<T> {}

#[derive(Debug, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
pub struct Request {
    pub fields: HashMap<String, String>,
}

#[derive(Debug, schemars::JsonSchema, serde::Deserialize, serde::Serialize)]
pub struct Sum {
    pub a: i32,
    pub b: i32,
}

#[rmcp::prompt_router(router = "test_router")]
impl<T> TestHandler<T> {
    #[rmcp::prompt]
    async fn async_method(
        &self,
        Parameters(Request { fields }): Parameters<Request>,
    ) -> Vec<PromptMessage> {
        drop(fields);
        vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "Async method response",
        )]
    }

    #[rmcp::prompt]
    fn sync_method(
        &self,
        Parameters(Request { fields }): Parameters<Request>,
    ) -> Vec<PromptMessage> {
        drop(fields);
        vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "Sync method response",
        )]
    }
}

#[rmcp::prompt]
async fn async_function(Parameters(Request { fields }): Parameters<Request>) -> Vec<PromptMessage> {
    drop(fields);
    vec![PromptMessage::new_text(
        PromptMessageRole::Assistant,
        "Async function response",
    )]
}

#[rmcp::prompt]
fn async_function2<T>(_callee: &TestHandler<T>) -> BoxFuture<'_, GetPromptResult> {
    Box::pin(async move {
        GetPromptResult {
            description: Some("Async function 2".to_string()),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::Assistant,
                "Async function 2 response",
            )],
        }
    })
}

#[test]
fn test_prompt_router() {
    let test_prompt_router = TestHandler::<()>::test_router()
        .with_route(rmcp::handler::server::router::prompt::PromptRoute::new_dyn(
            async_function_prompt_attr(),
            |mut context| {
                Box::pin(async move {
                    use rmcp::handler::server::{
                        common::FromContextPart, prompt::IntoGetPromptResult,
                    };
                    let params = Parameters::<Request>::from_context_part(&mut context)?;
                    let result = async_function(params).await;
                    result.into_get_prompt_result()
                })
            },
        ))
        .with_route(rmcp::handler::server::router::prompt::PromptRoute::new_dyn(
            async_function2_prompt_attr(),
            |context| {
                Box::pin(async move {
                    use rmcp::handler::server::prompt::IntoGetPromptResult;
                    let result = async_function2(context.server).await;
                    result.into_get_prompt_result()
                })
            },
        ));
    let prompts = test_prompt_router.list_all();
    assert_eq!(prompts.len(), 4);
}
