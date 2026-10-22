//! Drives modern tool continuations, including the native verification extension.
//! Inputs use the existing client service; proofs never bypass its validation.
//! Session recovery cannot replay a submitted proof; connection closure drops pending inputs.

use std::collections::BTreeMap;
use std::time::Duration;

use rmcp::RoleClient;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResult;
use rmcp::model::DEFAULT_MRTR_MAX_ROUNDS;
use rmcp::model::GetExtensions;
use rmcp::model::GetMeta;
use rmcp::model::InputRequest;
use rmcp::model::RequestId;
use rmcp::model::ServerRequest;
use rmcp::model::ServerResult;
use rmcp::service::PeerRequestOptions;
use rmcp::service::RequestContext;
use rmcp::service::RunningService;
use rmcp::service::Service;
use rmcp::service::ServiceError;
use rmcp::transport::streamable_http_client::StreamableHttpError;
use serde::Deserialize;
use serde_json::Value;

use crate::elicitation_client_service::ElicitationClientService;
use crate::http_client_adapter::StreamableHttpClientAdapterError;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolInput {
    input_requests: Option<BTreeMap<String, Value>>,
    request_state: Option<String>,
}

pub(crate) async fn call_tool(
    service: &RunningService<RoleClient, ElicitationClientService>,
    mut params: CallToolRequestParams,
) -> Result<CallToolResult, ServiceError> {
    let mut state_only_rounds = 0;
    let mut submitted_verification_proof = false;
    for round in 0..DEFAULT_MRTR_MAX_ROUNDS {
        let response = crate::tool_request::call_tool(
            service.peer(),
            params.clone(),
            PeerRequestOptions::no_options(),
        )
        .await;
        let (id, result) = response.map_err(|error| {
            if submitted_verification_proof
                && let ServiceError::TransportSend(transport) = &error
                && !matches!(
                    transport
                        .error
                        .downcast_ref::<StreamableHttpError<StreamableHttpClientAdapterError>>(),
                    Some(StreamableHttpError::AuthRequired(_))
                )
            {
                // Session recovery must not replay the original operation after a
                // continuation (which may already have consumed a signed proof).
                // Auth challenges already return without replaying the operation.
                invalid("MCP tool continuation failed; the tool was not restarted")
            } else {
                error
            }
        })?;
        let input = match result {
            ServerResult::CallToolResult(result) => return Ok(result),
            ServerResult::InputRequiredResult(result) => serde_json::to_value(result)
                .map_err(|_| invalid("invalid MCP tool input request"))?,
            // RMCP's InputRequest union intentionally excludes custom methods.
            ServerResult::CustomResult(result)
                if result.0.get("resultType").and_then(Value::as_str) == Some("input_required") =>
            {
                result.0
            }
            _ => return Err(ServiceError::UnexpectedResponse),
        };
        if round + 1 == DEFAULT_MRTR_MAX_ROUNDS {
            break;
        }
        let input: ToolInput =
            serde_json::from_value(input).map_err(|_| invalid("invalid MCP tool input request"))?;
        let requests = input.input_requests.unwrap_or_default();
        if requests.is_empty() && input.request_state.is_none() {
            return Err(ServiceError::UnexpectedResponse);
        }
        // Parse the whole round before presenting any prompts. Only standard MCP
        // inputs and supported OpenAI elicitation modes are accepted.
        let requests = requests
            .into_iter()
            .map(|(key, value)| parse_input(value).map(|request| (key, request)))
            .collect::<Result<Vec<_>, _>>()?;
        if requests.is_empty() {
            let millis = (50_u64 << state_only_rounds.min(/*other*/ 3)).min(/*other*/ 250);
            tokio::time::sleep(Duration::from_millis(millis)).await;
            state_only_rounds += 1;
        } else {
            state_only_rounds = 0;
        }
        let responses = futures::future::try_join_all(requests.into_iter().enumerate().map(
            |(index, (key, mut request))| {
                // Native prompts need distinct UI/cancellation ownership when
                // concurrent tool calls use the same server-assigned input key.
                let native_verification = matches!(&request, ServerRequest::CustomRequest(request)
                    if request.params.as_ref().and_then(|params| params.get("mode"))
                        .and_then(Value::as_str) == Some(crate::user_verification::MODE));
                let request_id = if native_verification {
                    RequestId::String(format!("tool-input/{id}/{index}").into())
                } else {
                    RequestId::String(key.clone().into())
                };
                let mut context = RequestContext::new(request_id, service.peer().clone());
                context.meta = std::mem::take(request.get_meta_mut());
                context.extensions = std::mem::take(request.extensions_mut());
                async move {
                    let result = service
                        .service()
                        .handle_request(request, context)
                        .await
                        .map_err(ServiceError::McpError)?;
                    let result = serde_json::to_value(result)
                        .map_err(|_| invalid("invalid MCP tool input response"))?;
                    let contains_proof = native_verification
                        && result.get("action").and_then(Value::as_str) == Some("accept");
                    Ok::<_, ServiceError>((key, result, contains_proof))
                }
            },
        ));
        let responses = tokio::select! {
            biased;
            _ = async {
                // RMCP exposes closure status but no awaitable closure notification.
                // Include explicit cancellation and transport EOF; dropping the joined
                // handlers releases both their UI callbacks and timeout pause guards.
                while !service.is_closed() && !service.peer().is_transport_closed() {
                    tokio::time::sleep(Duration::from_millis(/*millis*/ 50)).await;
                }
            } => return Err(ServiceError::TransportClosed),
            responses = responses => responses?,
        };
        params.input_responses = (!responses.is_empty()).then(|| {
            responses
                .into_iter()
                .map(|(key, result, contains_proof)| {
                    submitted_verification_proof |= contains_proof;
                    (key, result)
                })
                .collect()
        });
        params.request_state = input.request_state;
    }
    Err(ServiceError::InputRequiredRoundsExceeded {
        max_rounds: DEFAULT_MRTR_MAX_ROUNDS,
    })
}

fn parse_input(value: Value) -> Result<ServerRequest, ServiceError> {
    if let Ok(request) = serde_json::from_value::<InputRequest>(value.clone()) {
        return match request {
            InputRequest::CreateMessage(request) => {
                Ok(ServerRequest::CreateMessageRequest(request))
            }
            InputRequest::Elicitation(request) => Ok(ServerRequest::ElicitRequest(request)),
            InputRequest::ListRoots(request) => Ok(ServerRequest::ListRootsRequest(request)),
            _ => Err(ServiceError::UnexpectedResponse),
        };
    }
    let request: ServerRequest =
        serde_json::from_value(value).map_err(|_| invalid("invalid MCP tool input request"))?;
    if let ServerRequest::CustomRequest(request) = &request
        && request.method == "openai/elicitation/create"
    {
        match request
            .params
            .as_ref()
            .and_then(|params| params.get("mode"))
            .and_then(Value::as_str)
        {
            Some("form") => {
                crate::elicitation_client_service::openai_elicitation_form(request.clone())
                    .map_err(ServiceError::McpError)?;
            }
            Some(crate::user_verification::MODE) => {
                crate::user_verification::parse_request(request.clone())
                    .map_err(ServiceError::McpError)?;
            }
            _ => return Err(invalid("unsupported OpenAI elicitation mode")),
        }
        return Ok(ServerRequest::CustomRequest(request.clone()));
    }
    Err(invalid("unsupported MCP tool input request"))
}

fn invalid(message: &'static str) -> ServiceError {
    ServiceError::McpError(rmcp::ErrorData::invalid_request(
        message, /*data*/ None,
    ))
}
