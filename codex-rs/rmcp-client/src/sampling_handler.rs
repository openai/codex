use rmcp::model::CreateMessageRequestParam;
use rmcp::model::CreateMessageResult;

/// Trait for handling MCP sampling requests.
///
/// This trait should be implemented by the Codex core to provide actual LLM
/// responses to MCP sampling requests. The default implementation in
/// `LoggingClientHandler` returns simple mock responses.
#[async_trait::async_trait]
pub trait SamplingHandler: Send + Sync {
    /// Handle a sampling/createMessage request from an MCP server.
    ///
    /// The implementation should:
    /// 1. Convert the SamplingMessage(s) to the appropriate prompt format
    /// 2. Call the LLM with the prompt
    /// 3. Convert the LLM response back to CreateMessageResult
    async fn create_message(
        &self,
        params: CreateMessageRequestParam,
    ) -> Result<CreateMessageResult, rmcp::ErrorData>;
}
