use std::sync::Arc;

use rmcp::RoleClient;
use rmcp::model::ClientInfo;
use rmcp::model::ClientResult;
use rmcp::model::CustomResult;
use rmcp::model::ElicitationAction;
use rmcp::model::Meta;
use rmcp::model::RequestParamsMeta;
use rmcp::model::ServerNotification;
use rmcp::model::ServerRequest;
use rmcp::service::NotificationContext;
use rmcp::service::RequestContext;
use rmcp::service::Service;
use serde::Serialize;
use serde_json::Value;

use crate::logging_client_handler::LoggingClientHandler;
use crate::rmcp_client::Elicitation;
use crate::rmcp_client::ElicitationResponse;
use crate::rmcp_client::SendElicitation;

const MCP_PROGRESS_TOKEN_META_KEY: &str = "progressToken";

#[derive(Clone)]
pub(crate) struct ElicitationClientService {
    handler: LoggingClientHandler,
    send_elicitation: Arc<SendElicitation>,
}

impl ElicitationClientService {
    pub(crate) fn new(client_info: ClientInfo, send_elicitation: SendElicitation) -> Self {
        let send_elicitation = Arc::new(send_elicitation);
        Self {
            handler: LoggingClientHandler::new(
                client_info,
                clone_send_elicitation(Arc::clone(&send_elicitation)),
            ),
            send_elicitation,
        }
    }

    async fn create_elicitation(
        &self,
        request: Elicitation,
        context: RequestContext<RoleClient>,
    ) -> Result<ElicitationResponse, rmcp::ErrorData> {
        let RequestContext { id, meta, .. } = context;
        let request = restore_context_meta(request, meta);
        (self.send_elicitation)(id, request)
            .await
            .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))
    }
}

fn clone_send_elicitation(send_elicitation: Arc<SendElicitation>) -> SendElicitation {
    Box::new(move |request_id, request| send_elicitation(request_id, request))
}

impl Service<RoleClient> for ElicitationClientService {
    async fn handle_request(
        &self,
        request: ServerRequest,
        context: RequestContext<RoleClient>,
    ) -> Result<ClientResult, rmcp::ErrorData> {
        match request {
            ServerRequest::CreateElicitationRequest(request) => {
                let response = self.create_elicitation(request.params, context).await?;
                // RMCP's typed CreateElicitationResult does not model result-level `_meta`.
                let result = elicitation_response_result(response)?;
                Ok(ClientResult::CustomResult(result))
            }
            request => {
                <LoggingClientHandler as Service<RoleClient>>::handle_request(
                    &self.handler,
                    request,
                    context,
                )
                .await
            }
        }
    }

    async fn handle_notification(
        &self,
        notification: ServerNotification,
        context: NotificationContext<RoleClient>,
    ) -> Result<(), rmcp::ErrorData> {
        <LoggingClientHandler as Service<RoleClient>>::handle_notification(
            &self.handler,
            notification,
            context,
        )
        .await
    }

    fn get_info(&self) -> ClientInfo {
        <LoggingClientHandler as Service<RoleClient>>::get_info(&self.handler)
    }
}

fn restore_context_meta(mut request: Elicitation, mut context_meta: Meta) -> Elicitation {
    // RMCP lifts JSON-RPC `_meta` into RequestContext before invoking services.
    context_meta.remove(MCP_PROGRESS_TOKEN_META_KEY);
    if context_meta.is_empty() {
        return request;
    }

    request
        .meta_mut()
        .get_or_insert_with(Meta::new)
        .extend(context_meta);
    request
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateElicitationResultWithMeta {
    action: ElicitationAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    meta: Option<Value>,
}

fn elicitation_response_result(
    response: ElicitationResponse,
) -> Result<CustomResult, rmcp::ErrorData> {
    let ElicitationResponse {
        action,
        content,
        meta,
    } = response;
    let result = CreateElicitationResultWithMeta {
        action,
        content,
        meta,
    };

    serde_json::to_value(result)
        .map(CustomResult)
        .map_err(|err| rmcp::ErrorData::internal_error(err.to_string(), None))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use rmcp::model::BooleanSchema;
    use rmcp::model::CreateElicitationRequestParams;
    use rmcp::model::ElicitationSchema;
    use rmcp::model::PrimitiveSchema;
    use serde_json::Value;
    use serde_json::json;

    use super::*;

    #[test]
    fn restore_context_meta_adds_elicitation_meta_and_removes_progress_token() {
        let request = restore_context_meta(
            form_request(None),
            meta(json!({
                "progressToken": "progress-token",
                "persist": ["session", "always"],
            })),
        );

        assert_eq!(
            request,
            form_request(Some(meta(json!({
                "persist": ["session", "always"],
            }))))
        );
    }

    #[test]
    fn elicitation_response_result_serializes_response_meta() {
        let result = rmcp::model::ClientResult::CustomResult(
            elicitation_response_result(ElicitationResponse {
                action: ElicitationAction::Accept,
                content: Some(json!({ "confirmed": true })),
                meta: Some(json!({ "persist": "always" })),
            })
            .expect("elicitation response should serialize"),
        );

        assert_eq!(
            serde_json::to_value(result).expect("client result should serialize"),
            json!({
                "action": "accept",
                "content": { "confirmed": true },
                "_meta": { "persist": "always" },
            })
        );
    }

    fn form_request(meta: Option<Meta>) -> CreateElicitationRequestParams {
        CreateElicitationRequestParams::FormElicitationParams {
            meta,
            message: "Confirm?".to_string(),
            requested_schema: ElicitationSchema::builder()
                .required_property("confirmed", PrimitiveSchema::Boolean(BooleanSchema::new()))
                .build()
                .expect("schema should build"),
        }
    }

    fn meta(value: Value) -> Meta {
        let Value::Object(map) = value else {
            panic!("meta must be an object");
        };
        Meta(map)
    }
}
