use std::sync::Arc;

use codex_exec_server::ExecServerError;
use codex_exec_server::HttpClient;
use codex_exec_server::HttpHeader;
use codex_exec_server::HttpRequestParams;
use codex_exec_server::HttpRequestResponse;
use codex_exec_server::HttpResponseBodyStream;
use futures::future::BoxFuture;

pub(crate) struct ExecutorEnvironmentHttpClient {
    pub(crate) bearer_token_env_var: String,
    pub(crate) http_client: Arc<dyn HttpClient>,
}

impl ExecutorEnvironmentHttpClient {
    fn attach_authorization(&self, params: &mut HttpRequestParams) {
        params
            .headers
            .retain(|header| !header.name.eq_ignore_ascii_case("authorization"));
        params.headers.push(HttpHeader {
            name: "authorization".to_string(),
            value: "Bearer ".to_string(),
            value_env_var: Some(self.bearer_token_env_var.clone()),
        });
    }
}

impl HttpClient for ExecutorEnvironmentHttpClient {
    fn http_request(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<HttpRequestResponse, ExecServerError>> {
        self.attach_authorization(&mut params);
        self.http_client.http_request(params)
    }

    fn http_request_stream(
        &self,
        mut params: HttpRequestParams,
    ) -> BoxFuture<'_, Result<(HttpRequestResponse, HttpResponseBodyStream), ExecServerError>> {
        self.attach_authorization(&mut params);
        self.http_client.http_request_stream(params)
    }
}
