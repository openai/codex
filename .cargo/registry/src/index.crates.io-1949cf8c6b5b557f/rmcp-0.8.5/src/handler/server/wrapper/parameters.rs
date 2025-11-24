use schemars::JsonSchema;

/// Parameter extractor for tools and prompts
///
/// When used in tool and prompt handlers, this wrapper extracts and deserializes
/// parameters from the incoming request. The framework will automatically parse
/// the JSON arguments from tool calls or prompt arguments and deserialize them
/// into the specified type `P`.
///
/// The `#[serde(transparent)]` attribute ensures that the wrapper doesn't add
/// an extra layer in the JSON structure - it directly delegates serialization
/// and deserialization to the inner type `P`.
///
/// # Usage
///
/// Use `Parameters<T>` as a parameter in your tool or prompt handler functions:
///
/// ```rust
/// # use rmcp::handler::server::wrapper::Parameters;
/// # use schemars::JsonSchema;
/// # use serde::{Deserialize, Serialize};
/// #[derive(Deserialize, JsonSchema)]
/// struct CalculationRequest {
///     operation: String,
///     a: f64,
///     b: f64,
/// }
///
/// // In a tool handler
/// async fn calculate(params: Parameters<CalculationRequest>) -> Result<String, String> {
///     let request = params.0; // Extract the inner value
///     match request.operation.as_str() {
///         "add" => Ok((request.a + request.b).to_string()),
///         _ => Err("Unknown operation".to_string()),
///     }
/// }
/// ```
///
/// The framework handles the extraction automatically:
/// - For tools: Parses the `arguments` field from tool call requests
/// - For prompts: Parses the `arguments` field from prompt requests
/// - Returns appropriate error responses if deserialization fails
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Parameters<P>(pub P);

impl<P: JsonSchema> JsonSchema for Parameters<P> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        P::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        P::json_schema(generator)
    }
}
