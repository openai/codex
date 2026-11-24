#![cfg(test)]
//! Keep Cloud discovery deferred and preserve typed model-list behavior on other targets.

use super::*;
use crate::backend_id::BackendId;
use assert_matches::assert_matches;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::CloudClient;
use codex_app_server_client::CloudOperation;
use codex_app_server_protocol::JSONRPCErrorError;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test]
async fn model_catalog_prefetch_and_refresh_use_server_discovery() -> color_eyre::Result<()> {
    let model = json!({
        "id": "cloud-model", "model": "cloud-model", "displayName": "Cloud Model",
        "description": "A model supplied by the connected server", "hidden": false,
        "supportedReasoningEfforts": [{"reasoningEffort": "medium", "description": "Medium"}],
        "defaultReasoningEffort": "medium", "isDefault": true,
    });
    let preset = model_preset_from_api_model(serde_json::from_value(model.clone())?);
    for (mode, error_code) in [
        (ThreadParamsMode::Cloud, None),
        (ThreadParamsMode::Remote, None),
        (ThreadParamsMode::Embedded, None),
        (ThreadParamsMode::Cloud, Some(-32601)),
        (ThreadParamsMode::Remote, Some(-32601)),
        (ThreadParamsMode::Cloud, Some(-32603)),
    ] {
        let cloud = mode == ThreadParamsMode::Cloud;
        let (client, mut commands, _events) = CloudClient::channel(/*capacity*/ 1);
        let response = match error_code {
            None => Ok(json!({"data": [model.clone()], "nextCursor": null})),
            Some(code) => Err(JSONRPCErrorError {
                code,
                message: "Discovery unavailable".into(),
                data: None,
            }),
        };
        let worker = tokio::spawn(async move {
            for index in 0..if cloud { 1 } else { 2 } {
                let command = commands.recv().await.expect("model list request");
                let CloudOperation::Request(request) = command.operation else {
                    panic!("expected model/list");
                };
                let params =
                    assert_matches!(*request, ClientRequest::ModelList { params, .. } => params);
                let expected = ModelListParams {
                    model_provider: (index == 1).then(|| "alternate".to_string()),
                    include_hidden: Some(true),
                    ..Default::default()
                };
                assert_eq!(params, expected);
                command.response.send(Ok(response.clone())).unwrap();
            }
        });
        let client = AppServerClient::Cloud(client);
        let mut session = AppServerSession::new(BackendId::new(), client, mode);
        session.prefetch_model_list();
        let initial = session
            .prefetched_model_list
            .take()
            .unwrap()
            .await
            .map(|catalog| catalog.models);
        match error_code {
            _ if cloud => assert!(initial?.is_empty()),
            None => assert_eq!(initial?, vec![preset.clone()]),
            Some(_) => assert!(initial.is_err()),
        }

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let request_id = Uuid::new_v4();
        session.fetch_models(request_id, Some("alternate"), AppEventSender::new(tx));
        let event = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv()).await?;
        let (loaded, result) = assert_matches!(
            event,
            Some(AppEvent::ModelsLoaded { request_id, result }) => (request_id, result)
        );
        assert_eq!(loaded, request_id);
        match error_code {
            None => assert_eq!(result, Ok(vec![preset.clone()])),
            Some(-32601) if cloud => assert_eq!(result, Ok(Vec::new())),
            Some(_) => assert!(result.is_err()),
        }
        worker.await?;
    }
    Ok(())
}

#[tokio::test]
async fn legacy_servers_and_failed_discovery_keep_the_startup_catalog_unscoped() {
    for (mode, version, error) in [
        (ThreadParamsMode::Remote, Some("0.168.0"), None),
        (ThreadParamsMode::Embedded, Some("0.168.0"), None),
        (ThreadParamsMode::Remote, None, None),
        (ThreadParamsMode::Remote, Some("0.169.0"), Some(-32601)),
        (ThreadParamsMode::Remote, Some("0.170.0"), Some(-32602)),
    ] {
        let (client, mut commands, _events) = CloudClient::channel(/*capacity*/ 1);
        let client = AppServerClient::Cloud(client);
        let request = startup_list(
            mode,
            client.request_handle(),
            RequestId::Integer(1),
            version,
        );
        let (catalog, ()) = tokio::join!(request, async {
            if let Some(code) = error {
                let mut command = commands.recv().await.unwrap();
                let CloudOperation::Request(request) = command.operation else {
                    panic!("expected provider discovery");
                };
                assert_matches!(*request, ClientRequest::ModelProviderList { .. });
                if code == -32602 {
                    command.response.send(Ok(Ok(json!({"data": [], "defaultProviderId": "gateway", "canChangeDefault": false})))).unwrap();
                    command = commands.recv().await.unwrap();
                    let CloudOperation::Request(request) = command.operation else {
                        panic!("expected scoped discovery");
                    };
                    assert_matches!(*request, ClientRequest::ModelList { params, .. } if params.model_provider.as_deref() == Some("gateway"));
                }
                command
                    .response
                    .send(Ok(Err(JSONRPCErrorError {
                        code,
                        message: "unsupported".into(),
                        data: None,
                    })))
                    .unwrap();
            }
            let command = commands.recv().await.unwrap();
            let CloudOperation::Request(request) = command.operation else {
                panic!("expected unscoped discovery");
            };
            assert_matches!(*request, ClientRequest::ModelList { params, .. } if params.model_provider.is_none());
            command
                .response
                .send(Ok(Ok(json!({"data": [], "nextCursor": null}))))
                .unwrap();
        });
        assert_eq!(catalog.unwrap().provider, None);
    }
}

#[tokio::test]
async fn account_update_discards_new_thread_model_metadata() {
    use codex_app_server_client::AppServerEvent;
    use codex_app_server_protocol::AccountUpdatedNotification;
    use codex_app_server_protocol::ServerNotification;

    let (client, _commands, events) = CloudClient::channel(/*capacity*/ 1);
    let mut session = AppServerSession::new(
        BackendId::new(),
        AppServerClient::Cloud(client),
        ThreadParamsMode::Remote,
    );
    session.set_available_models(crate::test_support::TEST_MODEL_PRESETS.clone());
    events
        .send(AppServerEvent::ServerNotification(Box::new(
            ServerNotification::AccountUpdated(AccountUpdatedNotification {
                reason: None,
                auth_mode: None,
                plan_type: None,
            }),
        )))
        .unwrap();
    session.next_event().await.expect("account event");
    assert_eq!(session.default_model, None);
    assert!(session.available_models.is_empty());
}
