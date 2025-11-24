//cargo test --test test_prompt_handler --features "client server"
// Tests for verifying that the #[prompt_handler] macro correctly generates
// the ServerHandler trait implementation methods.
#![allow(dead_code)]

use rmcp::{
    RoleServer, ServerHandler,
    handler::server::router::prompt::PromptRouter,
    model::{GetPromptRequestParam, GetPromptResult, ListPromptsResult, PaginatedRequestParam},
    prompt_handler,
    service::RequestContext,
};

#[derive(Debug, Clone)]
pub struct TestPromptServer {
    prompt_router: PromptRouter<Self>,
}

impl Default for TestPromptServer {
    fn default() -> Self {
        Self::new()
    }
}

impl TestPromptServer {
    pub fn new() -> Self {
        Self {
            prompt_router: PromptRouter::new(),
        }
    }
}

#[prompt_handler]
impl ServerHandler for TestPromptServer {}

#[derive(Debug, Clone)]
pub struct CustomRouterServer {
    custom_router: PromptRouter<Self>,
}

impl Default for CustomRouterServer {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomRouterServer {
    pub fn new() -> Self {
        Self {
            custom_router: PromptRouter::new(),
        }
    }

    pub fn get_custom_router(&self) -> &PromptRouter<Self> {
        &self.custom_router
    }
}

#[prompt_handler(router = self.custom_router)]
impl ServerHandler for CustomRouterServer {}

#[derive(Debug, Clone)]
pub struct GenericPromptServer<T: Send + Sync + 'static> {
    prompt_router: PromptRouter<Self>,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> Default for GenericPromptServer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + Sync + 'static> GenericPromptServer<T> {
    pub fn new() -> Self {
        Self {
            prompt_router: PromptRouter::new(),
            _marker: std::marker::PhantomData,
        }
    }
}

#[prompt_handler]
impl<T: Send + Sync + 'static> ServerHandler for GenericPromptServer<T> {}

#[test]
fn test_prompt_handler_basic() {
    let server = TestPromptServer::new();

    // Test that the server implements ServerHandler
    fn assert_server_handler<T: ServerHandler>(_: &T) {}
    assert_server_handler(&server);

    // Test that the prompt router is accessible
    assert_eq!(server.prompt_router.list_all().len(), 0);
}

#[test]
fn test_prompt_handler_custom_router() {
    let server = CustomRouterServer::new();

    // Test that the server implements ServerHandler
    fn assert_server_handler<T: ServerHandler>(_: &T) {}
    assert_server_handler(&server);

    // Test that the custom router is used
    assert_eq!(server.custom_router.list_all().len(), 0);
}

#[test]
fn test_prompt_handler_with_generics() {
    let server = GenericPromptServer::<String>::new();

    // Test that generic server implements ServerHandler
    fn assert_server_handler<T: ServerHandler>(_: &T) {}
    assert_server_handler(&server);

    // Test with a different generic type
    let server2 = GenericPromptServer::<i32>::new();
    assert_server_handler(&server2);
}

#[test]
fn test_prompt_handler_trait_implementation() {
    // This test verifies that the prompt_handler macro generates proper ServerHandler implementation
    // The actual method signatures are tested through the ServerHandler trait bound
    fn compile_time_check<T: ServerHandler>() {}

    compile_time_check::<TestPromptServer>();
    compile_time_check::<CustomRouterServer>();
    compile_time_check::<GenericPromptServer<String>>();
}

// Test that the macro works with different server configurations
mod nested {
    use super::*;

    #[derive(Debug, Clone)]
    pub struct NestedServer {
        prompt_router: PromptRouter<Self>,
    }

    impl NestedServer {
        pub fn new() -> Self {
            Self {
                prompt_router: PromptRouter::new(),
            }
        }
    }

    #[prompt_handler]
    impl ServerHandler for NestedServer {}

    #[test]
    fn test_nested_prompt_handler() {
        let server = NestedServer::new();
        // Verify it implements ServerHandler
        fn assert_server_handler<T: ServerHandler>(_: &T) {}
        assert_server_handler(&server);
    }
}
