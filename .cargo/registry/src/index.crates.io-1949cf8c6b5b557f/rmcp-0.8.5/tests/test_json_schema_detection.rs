//cargo test --test test_json_schema_detection --features "client server macros"
use rmcp::{
    Json, ServerHandler, handler::server::router::tool::ToolRouter, tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct TestData {
    pub value: String,
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

    /// Tool that returns Json<T> - should have output schema
    #[tool(name = "with-json")]
    pub async fn with_json(&self) -> Result<Json<TestData>, String> {
        Ok(Json(TestData {
            value: "test".to_string(),
        }))
    }

    /// Tool that returns regular type - should NOT have output schema
    #[tool(name = "without-json")]
    pub async fn without_json(&self) -> Result<String, String> {
        Ok("test".to_string())
    }

    /// Tool that returns Result with inner Json - should have output schema  
    #[tool(name = "result-with-json")]
    pub async fn result_with_json(&self) -> Result<Json<TestData>, rmcp::ErrorData> {
        Ok(Json(TestData {
            value: "test".to_string(),
        }))
    }

    /// Tool with explicit output_schema attribute - should have output schema
    #[tool(name = "explicit-schema", output_schema = rmcp::handler::server::tool::cached_schema_for_type::<TestData>())]
    pub async fn explicit_schema(&self) -> Result<String, String> {
        Ok("test".to_string())
    }
}

#[tokio::test]
async fn test_json_type_generates_schema() {
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    // Find the with-json tool
    let json_tool = tools.iter().find(|t| t.name == "with-json").unwrap();
    assert!(
        json_tool.output_schema.is_some(),
        "Json<T> return type should generate output schema"
    );
}

#[tokio::test]
async fn test_non_json_type_no_schema() {
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    // Find the without-json tool
    let non_json_tool = tools.iter().find(|t| t.name == "without-json").unwrap();
    assert!(
        non_json_tool.output_schema.is_none(),
        "Regular return type should NOT generate output schema"
    );
}

#[tokio::test]
async fn test_result_with_json_generates_schema() {
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    // Find the result-with-json tool
    let result_json_tool = tools.iter().find(|t| t.name == "result-with-json").unwrap();
    assert!(
        result_json_tool.output_schema.is_some(),
        "Result<Json<T>, E> return type should generate output schema"
    );
}

#[tokio::test]
async fn test_explicit_schema_override() {
    let server = TestServer::new();
    let tools = server.tool_router.list_all();

    // Find the explicit-schema tool
    let explicit_tool = tools.iter().find(|t| t.name == "explicit-schema").unwrap();
    assert!(
        explicit_tool.output_schema.is_some(),
        "Explicit output_schema attribute should work"
    );
}
