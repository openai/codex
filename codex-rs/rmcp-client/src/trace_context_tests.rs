//! Checks that trace metadata uses the SDK's envelope/params merge precedence.

use super::merged_request_meta;
use pretty_assertions::assert_eq;
use rmcp::model::ClientRequest;
use rmcp::model::CustomRequest;
use rmcp::model::GetMeta;
use rmcp::model::ListToolsRequest;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::RequestMetaObject;
use serde_json::json;

#[test]
fn merged_metadata_preserves_params_and_prefers_extensions() -> serde_json::Result<()> {
    let params_meta = json!({"application": "preserved", "traceparent": "params-parent"});
    let mut params = PaginatedRequestParams::default();
    params.meta = Some(serde_json::from_value::<RequestMetaObject>(
        params_meta.clone(),
    )?);
    let requests = [
        ClientRequest::ListToolsRequest(ListToolsRequest::with_param(params)),
        ClientRequest::CustomRequest(CustomRequest::new(
            "test/custom",
            Some(json!({"_meta": params_meta})),
        )),
    ];
    for mut request in requests {
        let params_only = serde_json::to_value(&request)?;
        assert_eq!(merged_request_meta(&params_only), params_meta.as_object());

        request.get_meta_mut().set_traceparent("extension-parent");
        request.get_meta_mut().set_tracestate("vendor=value");
        let merged = serde_json::to_value(&request)?;
        assert_eq!(
            merged_request_meta(&merged),
            json!({
                "application": "preserved",
                "traceparent": "extension-parent",
                "tracestate": "vendor=value",
            })
            .as_object(),
        );
    }
    Ok(())
}
