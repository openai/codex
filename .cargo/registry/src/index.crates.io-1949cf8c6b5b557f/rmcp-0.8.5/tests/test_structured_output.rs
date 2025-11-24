//cargo test --test test_structured_output --features "client server macros"
use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, tool::IntoCallToolResult, wrapper::Parameters},
    model::{CallToolResult, Content, Tool},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CalculationRequest {
    pub a: i32,
    pub b: i32,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CalculationResult {
    pub sum: i32,
    pub product: i32,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct UserInfo {
    pub name: String,
    pub age: u32,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TestServer {}

#[derive(Debug, Clone)]
pub struct TestServer {
    tool_router: ToolRouter<Self>,
}

impl Default for TestServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(router = tool_router)]
impl TestServer {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// Tool that returns structured output
    #[tool(name = "calculate", description = "Perform calculations")]
    pub async fn calculate(
        &self,
        params: Parameters<CalculationRequest>,
    ) -> Result<Json<CalculationResult>, String> {
        Ok(Json(CalculationResult {
            sum: params.0.a + params.0.b,
            product: params.0.a * params.0.b,
        }))
    }

    /// Tool that returns regular string output
    #[tool(name = "get-greeting", description = "Get a greeting")]
    pub async fn get_greeting(&self, name: Parameters<String>) -> String {
        format!("Hello, {}!", name.0)
    }

    /// Tool that returns structured user info
    #[tool(name = "get-user", description = "Get user info")]
    pub async fn get_user(&self, user_id: Parameters<String>) -> Result<Json<UserInfo>, String> {
        if user_id.0 == "123" {
            Ok(Json(UserInfo {
                name: "Alice".to_string(),
                age: 30,
            }))
        } else {
            Err("User not found".to_string())
        }
    }
}

#[tokio::test]
async fn test_tool_with_output_schema() {
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    // Find the calculate tool
    let calculate_tool = tools.iter().find(|t| t.name == "calculate").unwrap();

    // Verify it has an output schema
    assert!(calculate_tool.output_schema.is_some());

    let schema = calculate_tool.output_schema.as_ref().unwrap();

    // Check that the schema contains expected fields
    let schema_str = serde_json::to_string(schema).unwrap();
    assert!(schema_str.contains("sum"));
    assert!(schema_str.contains("product"));
}

#[tokio::test]
async fn test_tool_without_output_schema() {
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    // Find the get-greeting tool
    let greeting_tool = tools.iter().find(|t| t.name == "get-greeting").unwrap();

    // Verify it doesn't have an output schema (returns String)
    assert!(greeting_tool.output_schema.is_none());
}

#[tokio::test]
async fn test_structured_content_in_call_result() {
    // Test creating a CallToolResult with structured content
    let structured_data = json!({
        "sum": 7,
        "product": 12
    });

    let result = CallToolResult::structured(structured_data.clone());

    assert!(!result.content.is_empty());
    assert!(result.structured_content.is_some());

    let contents = result.content;

    assert_eq!(contents.len(), 1);

    let content_text = contents.first().unwrap().as_text();

    assert!(content_text.is_some());

    let content_value: Value = serde_json::from_str(&content_text.unwrap().text).unwrap();

    assert_eq!(content_value, structured_data);
    assert_eq!(result.structured_content.unwrap(), structured_data);
    assert_eq!(result.is_error, Some(false));
}

#[tokio::test]
async fn test_structured_error_in_call_result() {
    // Test creating a CallToolResult with structured error
    let error_data = json!({
        "error_code": "NOT_FOUND",
        "message": "User not found"
    });

    let result = CallToolResult::structured_error(error_data.clone());

    assert!(!result.content.is_empty());
    assert!(result.structured_content.is_some());

    let contents = result.content;

    assert_eq!(contents.len(), 1);

    let content_text = contents.first().unwrap().as_text();

    assert!(content_text.is_some());

    let content_value: Value = serde_json::from_str(&content_text.unwrap().text).unwrap();

    assert_eq!(content_value, error_data);
    assert_eq!(result.structured_content.unwrap(), error_data);
    assert_eq!(result.is_error, Some(true));
}

#[tokio::test]
async fn test_mutual_exclusivity_validation() {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
    pub struct Response {
        message: String,
    }
    let response = Response {
        message: "Hello".into(),
    };
    // Test that content and structured_content can both be passed separately
    let content_result = CallToolResult::success(vec![Content::json(response.clone()).unwrap()]);
    let structured_result = CallToolResult::structured(json!({"message": "Hello"}));

    // Verify the validation
    content_result
        .into_typed::<Response>()
        .expect("Failed to extract content");
    structured_result
        .into_typed::<Response>()
        .expect("Failed to extract content");

    // Try to create a result with both fields
    let json_with_both = json!({
        "content": [{"type": "text", "text": "Hello"}],
        "structuredContent": {"message": "Hello"}
    });

    // The deserialization itself should not fail
    let deserialized: Result<CallToolResult, _> = serde_json::from_value(json_with_both);
    assert!(deserialized.is_ok());
}

#[tokio::test]
async fn test_structured_return_conversion() {
    // Test that Json<T> converts to CallToolResult with structured_content
    let calc_result = CalculationResult {
        sum: 7,
        product: 12,
    };

    let structured = Json(calc_result);
    let result: Result<CallToolResult, rmcp::ErrorData> =
        rmcp::handler::server::tool::IntoCallToolResult::into_call_tool_result(structured);

    assert!(result.is_ok());
    let call_result = result.unwrap();

    // Tools which return structured content should also return a serialized version as
    // Content::text for backwards compatibility.
    assert!(!call_result.content.is_empty());
    assert!(call_result.structured_content.is_some());

    let contents = call_result.content;

    assert_eq!(contents.len(), 1);

    let content_text = contents.first().unwrap().as_text();

    assert!(content_text.is_some());

    let content_value: Value = serde_json::from_str(&content_text.unwrap().text).unwrap();
    let structured_value = call_result.structured_content.unwrap();

    assert_eq!(content_value, structured_value);

    assert_eq!(structured_value["sum"], 7);
    assert_eq!(structured_value["product"], 12);
}

#[tokio::test]
async fn test_tool_serialization_with_output_schema() {
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    let calculate_tool = tools.iter().find(|t| t.name == "calculate").unwrap();

    // Serialize the tool
    let serialized = serde_json::to_value(calculate_tool).unwrap();

    // Check that outputSchema is included
    assert!(serialized["outputSchema"].is_object());

    // Deserialize back
    let deserialized: Tool = serde_json::from_value(serialized).unwrap();
    assert!(deserialized.output_schema.is_some());
}

#[tokio::test]
async fn test_output_schema_requires_structured_content() {
    // Test that tools with output_schema must use structured_content
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    // The calculate tool should have output_schema
    let calculate_tool = tools.iter().find(|t| t.name == "calculate").unwrap();
    assert!(calculate_tool.output_schema.is_some());

    // Directly call the tool and verify its result structure
    let params = Parameters(CalculationRequest { a: 5, b: 3 });
    let result = server.calculate(params).await.unwrap();

    // Convert the Json<CalculationResult> to CallToolResult
    let call_result: Result<CallToolResult, rmcp::ErrorData> =
        IntoCallToolResult::into_call_tool_result(result);

    assert!(call_result.is_ok());
    let call_result = call_result.unwrap();

    // Verify it has structured_content and content
    assert!(call_result.structured_content.is_some());
    assert!(!call_result.content.is_empty());
}
