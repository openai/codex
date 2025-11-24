//! Test tool macros, including documentation for generated fns.

//cargo test --test test_tool_macros --features "client server"
// Enforce that all generated code has sufficient docs to pass missing_docs lint
#![deny(missing_docs)]
#![allow(dead_code)]
use std::sync::Arc;

use rmcp::{
    ClientHandler, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolRequestParam, ClientInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for weather tool.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetWeatherRequest {
    /// City of interest.
    pub city: String,
    /// Date of interest.
    pub date: String,
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {}

/// Trivial stateless server.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Server {
    tool_router: ToolRouter<Self>,
}

impl Server {
    /// Create weather server.
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(router = tool_router)]
impl Server {
    /// This tool is used to get the weather of a city.
    #[tool(name = "get-weather", description = "Get the weather of a city.")]
    pub async fn get_weather(&self, city: Parameters<GetWeatherRequest>) -> String {
        drop(city);
        "rain".to_string()
    }

    #[tool]
    async fn empty_param(&self) {}
}

/// Generic service trait.
pub trait DataService: Send + Sync + 'static {
    /// Get data from service.
    fn get_data(&self) -> String;
}

// mock service for test
#[derive(Clone)]
struct MockDataService;
impl DataService for MockDataService {
    fn get_data(&self) -> String {
        "mock data".to_string()
    }
}

/// Generic server.
#[derive(Debug, Clone)]
pub struct GenericServer<DS: DataService> {
    data_service: Arc<DS>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<DS: DataService> GenericServer<DS> {
    /// Create data server instance.
    pub fn new(data_service: DS) -> Self {
        Self {
            data_service: Arc::new(data_service),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get data from the service")]
    async fn get_data(&self) -> String {
        self.data_service.get_data()
    }
}

#[tool_handler]
impl<DS: DataService> ServerHandler for GenericServer<DS> {}

#[tokio::test]
async fn test_tool_macros() {
    let server = Server::new();
    let _attr = Server::get_weather_tool_attr();
    let _get_weather_tool_attr_fn = Server::get_weather_tool_attr;
    let _get_weather_fn = Server::get_weather;
    server
        .get_weather(Parameters(GetWeatherRequest {
            city: "Harbin".into(),
            date: "Yesterday".into(),
        }))
        .await;
}

#[tokio::test]
async fn test_tool_macros_with_empty_param() {
    let _attr = Server::empty_param_tool_attr();
    println!("{_attr:?}");
    assert_eq!(
        _attr.input_schema.get("type"),
        Some(&serde_json::Value::String("object".to_string()))
    );
    assert_eq!(
        _attr.input_schema.get("properties"),
        Some(&serde_json::Value::Object(serde_json::Map::new()))
    );
}

#[tokio::test]
async fn test_tool_macros_with_generics() {
    let mock_service = MockDataService;
    let server = GenericServer::new(mock_service);
    let _attr = GenericServer::<MockDataService>::get_data_tool_attr();
    let _get_data_call_fn = GenericServer::<MockDataService>::get_data;
    let _get_data_fn = GenericServer::<MockDataService>::get_data;
    assert_eq!(server.get_data().await, "mock data");
}

#[tokio::test]
async fn test_tool_macros_with_optional_param() {
    let _attr = Server::get_weather_tool_attr();
    // println!("{_attr:?}");
    let attr_type = _attr
        .input_schema
        .get("properties")
        .unwrap()
        .get("city")
        .unwrap()
        .get("type")
        .unwrap();
    println!("_attr.input_schema: {:?}", attr_type);
    assert_eq!(attr_type.as_str().unwrap(), "string");
}

impl GetWeatherRequest {}

/// Struct defined for testing optional field schema generation.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct OptionalFieldTestSchema {
    /// Field description.
    #[schemars(description = "An optional description field")]
    pub description: Option<String>,
}

/// Struct defined for testing optional i64 field schema generation and null handling.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct OptionalI64TestSchema {
    /// Optional count field.
    #[schemars(description = "An optional i64 field")]
    pub count: Option<i64>,

    /// Added to ensure non-empty object schema.
    pub mandatory_field: String,
}

/// Dummy struct to host the test tool method.
#[derive(Debug, Clone)]
pub struct OptionalSchemaTester {
    tool_router: ToolRouter<Self>,
}

impl Default for OptionalSchemaTester {
    fn default() -> Self {
        Self::new()
    }
}

impl OptionalSchemaTester {
    /// Create instance of optional schema tester service.
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl OptionalSchemaTester {
    // Dummy tool function using the test schema as an aggregated parameter
    #[tool(description = "A tool to test optional schema generation")]
    async fn test_optional(&self, _req: Parameters<OptionalFieldTestSchema>) {
        // Implementation doesn't matter for schema testing
        // Return type changed to () to satisfy IntoCallToolResult
    }

    // Tool function to test optional i64 handling
    #[tool(description = "A tool to test optional i64 schema generation")]
    async fn test_optional_i64(
        &self,
        Parameters(req): Parameters<OptionalI64TestSchema>,
    ) -> String {
        match req.count {
            Some(c) => format!("Received count: {}", c),
            None => "Received null count".to_string(),
        }
    }
}
#[tool_handler]
// Implement ServerHandler to route tool calls for OptionalSchemaTester
impl ServerHandler for OptionalSchemaTester {}

#[test]
fn test_optional_field_schema_generation_via_macro() {
    // tests https://github.com/modelcontextprotocol/rust-sdk/issues/135

    // Get the attributes generated by the #[tool] macro helper
    let tool_attr = OptionalSchemaTester::test_optional_tool_attr();

    // Print the actual generated schema for debugging
    println!(
        "Actual input schema generated by macro: {:#?}",
        tool_attr.input_schema
    );

    // Verify the schema generated for the aggregated OptionalFieldTestSchema
    // by the macro infrastructure (which should now use OpenAPI 3 settings)
    let input_schema_map = &*tool_attr.input_schema; // Dereference Arc<JsonObject>

    // Check the schema for the 'description' property within the input schema
    let properties = input_schema_map
        .get("properties")
        .expect("Schema should have properties")
        .as_object()
        .unwrap();
    let description_schema = properties
        .get("description")
        .expect("Properties should include description")
        .as_object()
        .unwrap();

    // Assert that the format is now `type: "string", nullable: true`
    assert_eq!(
        description_schema.get("type").map(|v| v.as_str().unwrap()),
        Some("string"),
        "Schema for Option<String> generated by macro should be type: \"string\""
    );
    assert_eq!(
        description_schema
            .get("nullable")
            .map(|v| v.as_bool().unwrap()),
        Some(true),
        "Schema for Option<String> generated by macro should have nullable: true"
    );
    // We still check the description is correct
    assert_eq!(
        description_schema
            .get("description")
            .map(|v| v.as_str().unwrap()),
        Some("An optional description field")
    );

    // Ensure the old 'type: [T, null]' format is NOT used
    let type_value = description_schema.get("type").unwrap();
    assert!(
        !type_value.is_array(),
        "Schema type should not be an array [T, null]"
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

    // Create a simple client handler that just forwards tool calls
    let client_handler = DummyClientHandler::default();
    let client = client_handler.serve(client_transport).await?;

    // Test null case
    let result = client
        .call_tool(CallToolRequestParam {
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

    let result_text = result
        .content
        .first()
        .and_then(|content| content.raw.as_text())
        .map(|text| text.text.as_str())
        .expect("Expected text content");

    assert_eq!(
        result_text, "Received null count",
        "Null case should return expected message"
    );

    // Test Some case
    let some_result = client
        .call_tool(CallToolRequestParam {
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

    let some_result_text = some_result
        .content
        .first()
        .and_then(|content| content.raw.as_text())
        .map(|text| text.text.as_str())
        .expect("Expected text content");

    assert_eq!(
        some_result_text, "Received count: 42",
        "Some case should return expected message"
    );

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}
