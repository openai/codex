use super::*;

#[derive(Clone)]
pub(crate) struct ToolSearchRequestProcessor {
    thread_manager: Arc<ThreadManager>,
}

impl ToolSearchRequestProcessor {
    pub(crate) fn new(thread_manager: Arc<ThreadManager>) -> Self {
        Self { thread_manager }
    }

    pub(crate) async fn tool_search_search(
        &self,
        params: ToolSearchSearchParams,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let ToolSearchSearchParams {
            thread_id,
            query,
            limit,
        } = params;
        let query = query.trim();
        if query.is_empty() {
            return Err(invalid_request("query must not be empty"));
        }
        let limit = limit.unwrap_or(codex_tools::TOOL_SEARCH_DEFAULT_LIMIT as u32);
        if limit == 0 {
            return Err(invalid_request("limit must be greater than zero"));
        }

        let thread_id = ThreadId::from_string(&thread_id)
            .map_err(|err| invalid_request(format!("invalid thread id: {err}")))?;
        let thread = self
            .thread_manager
            .get_thread(thread_id)
            .await
            .map_err(|_| invalid_request(format!("thread not found: {thread_id}")))?;
        let result = thread
            .search_tool_search(query, limit as usize)
            .await
            .map_err(|err| internal_error(format!("failed to search tool_search: {err}")))?;

        tool_search_result_to_response(result).map(|response| Some(response.into()))
    }
}

fn tool_search_result_to_response(
    result: codex_core::ToolSearchDebugResult,
) -> Result<ToolSearchSearchResponse, JSONRPCErrorError> {
    Ok(ToolSearchSearchResponse {
        indexed_tool_count: usize_to_u32(result.indexed_tool_count, "indexedToolCount")?,
        matching_tool_count: usize_to_u32(result.matching_tool_count, "matchingToolCount")?,
        requested_limit: usize_to_u32(result.requested_limit, "requestedLimit")?,
        effective_limit: usize_to_u32(result.effective_limit, "effectiveLimit")?,
        top_k_truncated: result.top_k_truncated,
        tools: serialize_tool_specs(result.tools)?,
        results: result
            .results
            .into_iter()
            .map(tool_search_result_entry_to_response)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn tool_search_result_entry_to_response(
    result: codex_core::ToolSearchDebugResultEntry,
) -> Result<ToolSearchSearchResult, JSONRPCErrorError> {
    Ok(ToolSearchSearchResult {
        rank: usize_to_u32(result.rank, "rank")?,
        index: usize_to_u32(result.index, "index")?,
        score: result.score,
        source_name: result.source_name,
        source_description: result.source_description,
        searchable_text: result.searchable_text,
        tools: serialize_tool_specs(result.tools)?,
    })
}

fn serialize_tool_specs(
    tools: Vec<codex_tools::LoadableToolSpec>,
) -> Result<Vec<serde_json::Value>, JSONRPCErrorError> {
    tools
        .into_iter()
        .map(|tool| {
            serde_json::to_value(tool).map_err(|err| {
                internal_error(format!("failed to serialize tool_search tool: {err}"))
            })
        })
        .collect()
}

fn usize_to_u32(value: usize, field_name: &str) -> Result<u32, JSONRPCErrorError> {
    u32::try_from(value).map_err(|err| {
        internal_error(format!(
            "tool_search {field_name} value {value} does not fit in u32: {err}"
        ))
    })
}
