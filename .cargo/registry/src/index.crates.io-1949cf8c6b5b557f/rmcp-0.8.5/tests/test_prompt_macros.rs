//cargo test --test test_prompt_macros --features "client server"
#![allow(dead_code)]
use std::sync::Arc;

use rmcp::{
    ClientHandler, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::prompt::PromptRouter, wrapper::Parameters},
    model::{
        ClientInfo, GetPromptRequestParam, GetPromptResult, ListPromptsResult,
        PaginatedRequestParam, PromptMessage, PromptMessageRole,
    },
    prompt, prompt_handler, prompt_router,
    service::RequestContext,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CodeReviewRequest {
    pub file_path: String,
    pub language: String,
}

#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for Server {}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Server {
    prompt_router: PromptRouter<Self>,
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

#[prompt_router]
impl Server {
    pub fn new() -> Self {
        Self {
            prompt_router: Self::prompt_router(),
        }
    }

    /// This prompt is used to review code for best practices.
    #[prompt(
        name = "code-review",
        description = "Review code for best practices and issues."
    )]
    pub async fn code_review(&self, params: Parameters<CodeReviewRequest>) -> Vec<PromptMessage> {
        vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Please review the {} code in: {}",
                    params.0.language, params.0.file_path
                ),
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                "I'll review this code for best practices and potential issues.".to_string(),
            ),
        ]
    }

    #[prompt]
    async fn empty_param(&self) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "This is a prompt with no parameters.".to_string(),
        )]
    }
}

// define generic service trait
pub trait DataService: Send + Sync + 'static {
    fn get_context(&self) -> String;
}

// mock service for test
#[derive(Clone)]
struct MockDataService;
impl DataService for MockDataService {
    fn get_context(&self) -> String {
        "mock context data".to_string()
    }
}

// define generic server
#[derive(Debug, Clone)]
pub struct GenericServer<DS: DataService> {
    data_service: Arc<DS>,
    prompt_router: PromptRouter<Self>,
}

#[prompt_router]
impl<DS: DataService> GenericServer<DS> {
    pub fn new(data_service: DS) -> Self {
        Self {
            data_service: Arc::new(data_service),
            prompt_router: Self::prompt_router(),
        }
    }

    #[prompt(description = "Get contextual help from the service")]
    async fn get_help(&self) -> GetPromptResult {
        let context = self.data_service.get_context();
        GetPromptResult {
            description: Some("Contextual help based on service data".to_string()),
            messages: vec![
                PromptMessage::new_text(
                    PromptMessageRole::User,
                    "I need help with the current context.".to_string(),
                ),
                PromptMessage::new_text(
                    PromptMessageRole::Assistant,
                    format!(
                        "Based on the context '{}', here's how I can help...",
                        context
                    ),
                ),
            ],
        }
    }
}

#[prompt_handler]
impl<DS: DataService> ServerHandler for GenericServer<DS> {}

#[tokio::test]
async fn test_prompt_macros() {
    let server = Server::new();
    let _attr = Server::code_review_prompt_attr();
    let _code_review_prompt_attr_fn = Server::code_review_prompt_attr;
    let _code_review_fn = Server::code_review;
    let result = server
        .code_review(Parameters(CodeReviewRequest {
            file_path: "/src/main.rs".into(),
            language: "rust".into(),
        }))
        .await;
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].role, PromptMessageRole::User);
    assert_eq!(result[1].role, PromptMessageRole::Assistant);
}

#[tokio::test]
async fn test_prompt_macros_with_empty_param() {
    let _attr = Server::empty_param_prompt_attr();
    println!("{_attr:?}");
    assert!(
        _attr.arguments.is_none(),
        "Empty param prompt should have no arguments"
    );
}

#[tokio::test]
async fn test_prompt_macros_with_generics() {
    let mock_service = MockDataService;
    let server = GenericServer::new(mock_service);
    let _attr = GenericServer::<MockDataService>::get_help_prompt_attr();
    let _get_help_call_fn = GenericServer::<MockDataService>::get_help;
    let _get_help_fn = GenericServer::<MockDataService>::get_help;
    let result = server.get_help().await;
    assert!(result.description.is_some());
    assert_eq!(result.messages.len(), 2);
    match &result.messages[1].content {
        rmcp::model::PromptMessageContent::Text { text } => {
            assert!(text.contains("mock context data"));
        }
        _ => panic!("Expected text content"),
    }
}

#[tokio::test]
async fn test_prompt_macros_with_optional_param() {
    let _attr = Server::code_review_prompt_attr();
    let arguments = _attr.arguments.as_ref().unwrap();

    // Check that we have the expected number of arguments
    assert_eq!(arguments.len(), 2);

    // Verify file_path is required
    let file_path_arg = arguments.iter().find(|a| a.name == "file_path").unwrap();
    assert_eq!(file_path_arg.required, Some(true));

    // Verify language is required
    let language_arg = arguments.iter().find(|a| a.name == "language").unwrap();
    assert_eq!(language_arg.required, Some(true));
}

impl CodeReviewRequest {}

// Struct defined for testing optional field schema generation
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct OptionalFieldTestSchema {
    #[schemars(description = "An optional description field")]
    pub description: Option<String>,
}

// Struct defined for testing optional i64 field schema generation and null handling
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct OptionalI64TestSchema {
    #[schemars(description = "An optional i64 field")]
    pub count: Option<i64>,
    pub mandatory_field: String, // Added to ensure non-empty object schema
}

// Dummy struct to host the test prompt method
#[derive(Debug, Clone)]
pub struct OptionalSchemaTester {
    prompt_router: PromptRouter<Self>,
}

impl Default for OptionalSchemaTester {
    fn default() -> Self {
        Self::new()
    }
}

impl OptionalSchemaTester {
    pub fn new() -> Self {
        Self {
            prompt_router: Self::prompt_router(),
        }
    }
}

#[prompt_router]
impl OptionalSchemaTester {
    // Dummy prompt function using the test schema as an aggregated parameter
    #[prompt(description = "A prompt to test optional schema generation")]
    async fn test_optional(&self, _req: Parameters<OptionalFieldTestSchema>) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            PromptMessageRole::Assistant,
            "Testing optional fields".to_string(),
        )]
    }

    // Prompt function to test optional i64 handling
    #[prompt(description = "A prompt to test optional i64 schema generation")]
    async fn test_optional_i64(
        &self,
        Parameters(req): Parameters<OptionalI64TestSchema>,
    ) -> GetPromptResult {
        let message = match req.count {
            Some(c) => format!("Received count: {}", c),
            None => "Received null count".to_string(),
        };

        GetPromptResult {
            description: Some("Test result for optional i64".to_string()),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::Assistant,
                message,
            )],
        }
    }
}

#[prompt_handler]
// Implement ServerHandler to route prompt calls for OptionalSchemaTester
impl ServerHandler for OptionalSchemaTester {}

#[test]
fn test_optional_field_schema_generation_via_macro() {
    // tests https://github.com/modelcontextprotocol/rust-sdk/issues/135

    // Get the attributes generated by the #[prompt] macro helper
    let prompt_attr = OptionalSchemaTester::test_optional_prompt_attr();

    // Print the actual generated schema for debugging
    println!(
        "Actual arguments generated by macro: {:#?}",
        prompt_attr.arguments
    );

    // Verify the schema generated for the aggregated OptionalFieldTestSchema
    let arguments = prompt_attr.arguments.expect("Should have arguments");

    // Check that we have an argument for the optional description field
    let description_arg = arguments
        .iter()
        .find(|arg| arg.name == "description")
        .expect("Should have description argument");

    // Assert that optional fields are marked as not required
    assert_eq!(
        description_arg.required,
        Some(false),
        "Optional fields should be marked as not required"
    );

    // Check the description is correct
    assert_eq!(
        description_arg.description.as_deref(),
        Some("An optional description field")
    );
}

// Define a dummy client handler
#[derive(Debug, Clone, Default)]
struct DummyClientHandler {}

impl ClientHandler for DummyClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::default()
    }
}

#[tokio::test]
async fn test_optional_i64_field_with_null_input() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    // Server setup
    let server = OptionalSchemaTester::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    // Create a simple client handler that just forwards prompt calls
    let client_handler = DummyClientHandler::default();
    let client = client_handler.serve(client_transport).await?;

    // Test null case
    let result = client
        .get_prompt(GetPromptRequestParam {
            name: "test_optional_i64".into(),
            arguments: Some(
                serde_json::json!({
                    "count": null,
                    "mandatory_field": "test_null"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await?;

    let result_text = match &result.messages.first().unwrap().content {
        rmcp::model::PromptMessageContent::Text { text } => text.as_str(),
        _ => panic!("Expected text content"),
    };

    assert_eq!(
        result_text, "Received null count",
        "Null case should return expected message"
    );

    // Test Some case
    let some_result = client
        .get_prompt(GetPromptRequestParam {
            name: "test_optional_i64".into(),
            arguments: Some(
                serde_json::json!({
                    "count": 42,
                    "mandatory_field": "test_some"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        })
        .await?;

    let some_result_text = match &some_result.messages.first().unwrap().content {
        rmcp::model::PromptMessageContent::Text { text } => text.as_str(),
        _ => panic!("Expected text content"),
    };

    assert_eq!(
        some_result_text, "Received count: 42",
        "Some case should return expected message"
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
