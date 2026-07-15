use std::sync::Arc;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use futures::future::BoxFuture;

const OPENAI_DEVELOPER_DOCS_MCP_URL: &str = "https://developers.openai.com/mcp";
const OPENAI_DEVELOPER_DOCS_MCP_CODEX_URL: &str = "https://developers.openai.com/mcp?source=codex";

pub(crate) fn maybe_with_openai_docs_source_attribution(
    mcp_server_url: &str,
    http_client: Arc<dyn HttpClient>,
) -> Arc<dyn HttpClient> {
    if mcp_server_url == OPENAI_DEVELOPER_DOCS_MCP_URL {
        Arc::new(OpenAiDocsHttpClient { http_client })
    } else {
        http_client
    }
}

struct OpenAiDocsHttpClient {
    http_client: Arc<dyn HttpClient>,
}

impl OpenAiDocsHttpClient {
    fn attribute_mcp_request(&self, params: &mut HttpRequestParams) {
        if params.url == OPENAI_DEVELOPER_DOCS_MCP_URL {
            params.url = OPENAI_DEVELOPER_DOCS_MCP_CODEX_URL.to_string();
        }
    }
}

impl HttpClient for OpenAiDocsHttpClient {
    fn http_request(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        self.attribute_mcp_request(&mut params);
        self.http_client.http_request(params)
    }

    fn http_request_stream(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        self.attribute_mcp_request(&mut params);
        self.http_client.http_request_stream(params)
    }
}

#[cfg(test)]
#[path = "openai_docs_source_attribution_tests.rs"]
mod tests;
