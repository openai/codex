#![cfg(test)]
//! Drive global and selected inventory using independently controlled app-server replies.

use super::super::ProviderId;
use super::super::ProviderSelection;
use super::super::ResolvedStatuslineProvider;
use super::super::test_support::test_client;
use super::*;
use codex_app_server_client::CloudCommand;
use codex_app_server_client::CloudOperation;
use codex_app_server_client::RequestResult;
use codex_app_server_protocol::ClientRequest;
use codex_protocol::ThreadId;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeMap;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

type Reply = oneshot::Sender<std::io::Result<RequestResult>>;
const URI: &str = "ui://test/footer";

#[derive(Debug)]
enum Call<'a> {
    Page(Option<&'a str>),
    Global(&'a str),
}
use Call::Global;
use Call::Page;
use DiscoveryUpdate::InventoryFailed;
use DiscoveryUpdate::Server;
use ProviderLookupError::Request;
use ProviderLookupError::TimedOut;

struct Peer(mpsc::Receiver<CloudCommand>);

impl Peer {
    fn connect() -> (AppServerRequestHandle, Self) {
        let (client, commands) = test_client();
        (client, Self(commands))
    }

    async fn request(&mut self) -> (ClientRequest, Reply) {
        let command = self.0.recv().await.unwrap();
        let CloudOperation::Request(request) = command.operation else {
            panic!("expected an app-server request");
        };
        (*request, command.response)
    }

    async fn config(&mut self, cwd: &str) -> Reply {
        let (ClientRequest::ConfigRead { params, .. }, response) = self.request().await else {
            panic!("expected config/read");
        };
        assert_eq!(
            (params.include_layers, params.cwd.as_deref()),
            (true, Some(cwd))
        );
        response
    }

    async fn expect<const N: usize>(&mut self, calls: [Call<'_>; N]) -> [Reply; N] {
        let mut replies = BTreeMap::new();
        for _ in 0..N {
            let (ClientRequest::McpServerStatusList { params, .. }, response) =
                self.request().await
            else {
                panic!("expected mcpServerStatus/list");
            };
            assert_eq!((params.thread_id, params.limit), (None, None));
            let key = (params.server_name, params.cursor);
            assert!(replies.insert(key, response).is_none());
        }
        calls.map(|call| {
            let key = match call {
                Page(cursor) => (None, cursor.map(str::to_owned)),
                Global(name) => (Some(name.to_owned()), None),
            };
            replies
                .remove(&key)
                .unwrap_or_else(|| panic!("missing {call:?}"))
        })
    }
}

fn reply(to: Reply, data: Value) {
    to.send(Ok(Ok(data))).unwrap();
}

fn row(name: &str, uri: Option<&str>) -> Value {
    let mut row = super::super::test_support::row(name, vec![]);
    row["runtimeStatus"] = Value::Null;
    row["pluginId"] = json!("daily@local");
    if let Some(uri) = uri {
        row["tools"]["footer"] = json!({"name":"footer","inputSchema":{},"_meta":{"openai/ui":{"entrypoints":[{"type":"statusline","resourceUri":uri}]}}});
    }
    row
}

fn reply_page(to: Reply, rows: &[Value], next: Option<&str>) {
    reply(to, json!({"data":rows,"nextCursor":next}));
}

fn catalog(update: Option<DiscoveryUpdate>) -> (McpServerName, Vec<String>) {
    let Some(Server(name, Ok(providers))) = update else {
        panic!("expected a provider catalog, got {update:?}");
    };
    (
        name,
        providers.into_iter().map(|provider| provider.uri).collect(),
    )
}

fn assert_uri(update: Option<DiscoveryUpdate>, server: &str, uri: &str) {
    assert_eq!(catalog(update), (server.into(), vec![uri.to_owned()]));
}

#[tokio::test(start_paused = true)]
async fn a_fast_inventory_publishes_each_server_without_named_probes() {
    let (client, mut peer) = Peer::connect();
    let known = ["ready", "empty", "failed"].map(McpServerName::from).into();
    let mut updates = refresh_statusline_providers(client, known);
    let [first] = peer.expect([Page(None)]).await;
    tokio::time::advance(Duration::from_millis(1500)).await;
    let mut failed = row("failed", /*uri*/ None);
    failed["toolsError"] = json!("offline");
    reply_page(
        first,
        &[row("ready", Some(URI)), row("empty", /*uri*/ None), failed],
        Some("next"),
    );
    assert_uri(updates.next().await, "ready", URI);
    assert_eq!(catalog(updates.next().await), ("empty".into(), vec![]));
    assert!(
        matches!(updates.next().await, Some(Server(name, Err(Request(e))))
        if name.as_str() == "failed" && e.to_string() == "offline")
    );
    let [last] = peer.expect([Page(Some("next"))]).await;
    reply_page(last, &[], /*next*/ None);
    assert!(updates.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn late_inventory_frees_probe_slots_and_keeps_discovering_servers() {
    let (client, mut peer) = Peer::connect();
    let known = ["a", "b", "c", "d", "e", "queued"]
        .map(McpServerName::from)
        .into();
    let start = Instant::now();
    let mut updates = refresh_statusline_providers(client, known);
    let [first] = peer.expect([Page(None)]).await;
    let [a, b, c, d] = peer
        .expect([Global("a"), Global("b"), Global("c"), Global("d")])
        .await;
    assert_eq!(Instant::now() - start, INVENTORY_HEAD_START);
    assert!(matches!(
        peer.0.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));

    reply_page(
        first,
        &[row("b", Some(URI)), row("queued", Some(URI))],
        Some("next"),
    );
    assert_uri(updates.next().await, "b", URI);
    assert_uri(updates.next().await, "queued", URI);
    let [e, last] = peer.expect([Global("e"), Page(Some("next"))]).await;
    assert!(b.is_closed());
    assert_eq!(Instant::now() - start, INVENTORY_HEAD_START);
    for (name, probe) in [("a", a), ("c", c), ("d", d), ("e", e)] {
        reply_page(probe, &[row(name, Some(URI))], /*next*/ None);
        assert_uri(updates.next().await, name, URI);
    }
    reply_page(
        last,
        &[row("e", Some("ui://old/e")), row("new-plugin", Some(URI))],
        /*next*/ None,
    );
    assert_uri(updates.next().await, "new-plugin", URI);
    assert!(updates.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_late_page_preserves_completed_probes_and_unanswered_names_time_out() {
    let (client, mut peer) = Peer::connect();
    let known = ["empty", "failed", "slow"].map(McpServerName::from).into();
    let start = Instant::now();
    let mut updates = refresh_statusline_providers(client, known);
    let [page, empty, failed, _slow] = peer
        .expect([
            Page(None),
            Global("empty"),
            Global("failed"),
            Global("slow"),
        ])
        .await;

    reply_page(empty, &[], /*next*/ None);
    assert_eq!(catalog(updates.next().await), ("empty".into(), vec![]));
    let mut error = row("failed", /*uri*/ None);
    error["toolsError"] = json!("no catalog");
    reply_page(failed, &[error], /*next*/ None);
    assert!(
        matches!(updates.next().await, Some(Server(name, Err(Request(e))))
        if name.as_str() == "failed" && e.to_string() == "no catalog")
    );
    reply_page(
        page,
        &[row("empty", Some(URI)), row("failed", Some(URI))],
        /*next*/ None,
    );
    assert!(
        matches!(updates.next().await, Some(Server(name, Err(TimedOut))) if name.as_str() == "slow")
    );
    assert!(updates.next().await.is_none());
    assert_eq!(Instant::now() - start, LOOKUP_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn a_cold_picker_and_configured_provider_have_independent_deadlines() {
    let (client, mut peer) = Peer::connect();
    let start = Instant::now();
    let mut picker = refresh_statusline_providers(client.clone(), Vec::new());
    let [_inventory] = peer.expect([Page(None)]).await;
    tokio::time::advance(Duration::from_secs(1)).await;
    let selected_start = Instant::now();
    let selected = tokio::spawn(async move {
        let cwd = PathUri::parse("file:///workspace").unwrap();
        let id = ProviderId {
            server: "weather".into(),
            tool: "footer".into(),
        };
        ResolvedStatuslineProvider::lookup(
            &client,
            ThreadId::new(),
            &cwd,
            ProviderSelection::Saved(&id),
        )
        .await
    });
    let config = peer.config("/workspace").await;
    tokio::time::advance(Duration::from_secs(2)).await;
    reply(config, json!({"config":{},"layers":[]}));
    let [_global] = peer.expect([Global("weather")]).await;
    assert!(matches!(
        picker.next().await,
        Some(InventoryFailed(TimedOut))
    ));
    assert!(picker.next().await.is_none());
    assert_eq!(Instant::now() - start, LOOKUP_TIMEOUT);
    assert!(matches!(selected.await.unwrap(), Err(TimedOut)));
    assert_eq!(Instant::now() - selected_start, LOOKUP_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn closing_cancels_requests_and_reopening_reads_the_current_inventory() {
    let (client, mut peer) = Peer::connect();
    let mut first =
        refresh_statusline_providers(client.clone(), vec!["calendar".into(), "slow".into()]);
    let [mut inventory, calendar, mut probe] = peer
        .expect([Page(None), Global("calendar"), Global("slow")])
        .await;
    reply_page(calendar, &[row("calendar", Some(URI))], /*next*/ None);
    assert_uri(first.next().await, "calendar", URI);
    drop(first);
    tokio::join!(inventory.closed(), probe.closed());

    let mut reopened = refresh_statusline_providers(client, vec!["calendar".into()]);
    let [page] = peer.expect([Page(None)]).await;
    let new_uri = "ui://calendar/changed";
    reply_page(page, &[row("calendar", Some(new_uri))], /*next*/ None);
    assert_uri(reopened.next().await, "calendar", new_uri);
    assert!(reopened.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn completed_inventory_probes_a_missing_cached_name_immediately() {
    let (client, mut peer) = Peer::connect();
    let start = Instant::now();
    let mut updates = refresh_statusline_providers(client, vec!["removed".into()]);
    let [page] = peer.expect([Page(None)]).await;
    reply_page(page, &[], /*next*/ None);
    let [removed] = peer.expect([Global("removed")]).await;
    reply_page(removed, &[], /*next*/ None);
    assert_eq!(catalog(updates.next().await), ("removed".into(), vec![]));
    assert!(updates.next().await.is_none());
    assert_eq!(Instant::now(), start);
}

#[tokio::test(start_paused = true)]
async fn repeated_inventory_cursor_reports_failure_after_the_first_catalog() {
    let (client, mut peer) = Peer::connect();
    let mut updates = refresh_statusline_providers(client, Vec::new());
    let [first] = peer.expect([Page(None)]).await;
    reply_page(first, &[row("plugin", Some(URI))], Some("again"));
    assert_uri(updates.next().await, "plugin", URI);
    let [second] = peer.expect([Page(Some("again"))]).await;
    reply_page(
        second,
        &[row("plugin", Some("ui://different/footer"))],
        Some("again"),
    );
    assert!(
        matches!(updates.next().await, Some(InventoryFailed(Request(error)))
        if error.to_string() == "MCP inventory repeated a cursor")
    );
    assert!(updates.next().await.is_none());
}
