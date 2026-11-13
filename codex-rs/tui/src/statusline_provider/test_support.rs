#![cfg(test)]
//! Drive statusline RPCs through the same request handle a remote TUI uses.

use codex_app_server_client::AppServerClient;
use codex_app_server_client::AppServerRequestHandle;
use codex_app_server_client::CloudClient;
use codex_app_server_client::CloudCommand;
use codex_app_server_client::CloudOperation;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;

pub(super) type Requests = mpsc::Receiver<CloudCommand>;

pub(super) fn test_client() -> (AppServerRequestHandle, Requests) {
    let (cloud, commands, _events) = CloudClient::channel(/*capacity*/ 8);
    (AppServerClient::Cloud(cloud).request_handle(), commands)
}

pub(super) async fn reply(commands: &mut Requests, method: &str, params: Value, result: Value) {
    let command = commands.recv().await.unwrap();
    let CloudOperation::Request(request) = command.operation else {
        panic!("expected {method}");
    };
    let wire = serde_json::to_value(request).unwrap();
    assert_eq!(wire["method"], method);
    assert_eq!(wire["params"], params);
    command.response.send(Ok(Ok(result))).unwrap();
}

pub(super) fn row(name: &str, tools: Vec<Value>) -> Value {
    let tools = tools
        .into_iter()
        .map(|tool| (tool["name"].as_str().unwrap().to_owned(), tool))
        .collect::<serde_json::Map<_, _>>();
    json!({"name":name,"runtimeStatus":"connected","httpOrigin":null,"tools":tools,"authStatus":"unsupported","resources":[],"resourceTemplates":[]})
}
