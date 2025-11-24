use std::collections::HashMap;

use futures::future::BoxFuture;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::CallToolHandler, wrapper::Parameters},
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

#[rmcp::tool_router(router = test_router_1)]
impl<T> TestHandler<T> {
    #[rmcp::tool]
    async fn async_method(&self, Parameters(Request { fields }): Parameters<Request>) {
        drop(fields)
    }
}

#[rmcp::tool_router(router = test_router_2)]
impl<T> TestHandler<T> {
    #[rmcp::tool]
    fn sync_method(&self, Parameters(Request { fields }): Parameters<Request>) {
        drop(fields)
    }
}

#[rmcp::tool]
async fn async_function(Parameters(Request { fields }): Parameters<Request>) {
    drop(fields)
}

#[rmcp::tool]
fn async_function2<T>(_callee: &TestHandler<T>) -> BoxFuture<'_, ()> {
    Box::pin(async move {})
}

#[test]
fn test_tool_router() {
    let test_tool_router: ToolRouter<TestHandler<()>> = ToolRouter::<TestHandler<()>>::new()
        .with_route((async_function_tool_attr(), async_function))
        .with_route((async_function2_tool_attr(), async_function2))
        + TestHandler::<()>::test_router_1()
        + TestHandler::<()>::test_router_2();
    let tools = test_tool_router.list_all();
    assert_eq!(tools.len(), 4);
    assert_handler(TestHandler::<()>::async_method);
}

fn assert_handler<S, H, A>(_handler: H)
where
    H: CallToolHandler<S, A>,
{
}
