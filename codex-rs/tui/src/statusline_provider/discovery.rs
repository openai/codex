//! Stream the first inventory or named lookup result for each server in a picker refresh.
//! Inventory stays open to discover new servers while named lookups are in flight.

use super::LOOKUP_TIMEOUT;
use super::McpServerName;
use super::ProviderLookupError;
use super::StatuslineProvider;
use super::inventory;
use codex_app_server_client::AppServerRequestHandle;
use futures::StreamExt;
use futures::TryStreamExt;
use futures::stream;
use std::collections::BTreeSet;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio_stream::StreamMap;
use tokio_stream::wrappers::UnboundedReceiverStream;

const INVENTORY_HEAD_START: Duration = Duration::from_secs(2);
const MAX_CONCURRENT_PROBES: usize = 4;

#[derive(Debug)]
#[cfg_attr(not(test), expect(dead_code, reason = "staged picker"))]
pub(crate) enum DiscoveryUpdate {
    Server(
        McpServerName,
        Result<Vec<StatuslineProvider>, ProviderLookupError>,
    ),
    InventoryFailed(ProviderLookupError),
}

/// Pass the saved and previously observed server names. Dropping the stream stops the TUI's
/// outstanding waits. The host may finish requests it already received.
#[cfg_attr(not(test), expect(dead_code, reason = "staged picker"))]
pub(crate) fn refresh_statusline_providers(
    client: AppServerRequestHandle,
    known: Vec<McpServerName>,
) -> impl futures::Stream<Item = DiscoveryUpdate> + Unpin {
    let start = Instant::now();
    let (updates, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        tokio::select! {
            biased;
            () = updates.closed() => {}
            result = discover(&client, start, known, &updates) => {
                if let Err(error) = result {
                    let _ = updates.send(DiscoveryUpdate::InventoryFailed(error));
                }
            }
        }
    });
    UnboundedReceiverStream::new(receiver)
}

async fn discover(
    client: &AppServerRequestHandle,
    start: Instant,
    known: Vec<McpServerName>,
    updates: &mpsc::UnboundedSender<DiscoveryUpdate>,
) -> Result<(), ProviderLookupError> {
    let mut queued: BTreeSet<_> = known.into_iter().collect();
    let mut published = BTreeSet::new();
    let mut probes = StreamMap::<McpServerName, _>::new();
    let pages = inventory::global_pages(client);
    tokio::pin!(pages);
    let mut listing = None;
    while listing.is_none() || !queued.is_empty() || !probes.is_empty() {
        tokio::select! {
            biased;
            () = tokio::time::sleep_until(start + LOOKUP_TIMEOUT) => break,
            Some((name, providers)) = probes.next(), if !probes.is_empty() => {
                probes.remove(&name);
                published.insert(name.clone());
                let _ = updates.send(DiscoveryUpdate::Server(name, providers));
            }
            page = pages.try_next(), if listing.is_none() => match page {
                Ok(Some(servers)) => for server in servers {
                    if published.insert(server.name.clone()) {
                        queued.remove(&server.name);
                        // A late catalog frees a named lookup slot even if that server never replies.
                        probes.remove(&server.name);
                        let providers = server.providers().map_err(Into::into);
                        let _ = updates.send(DiscoveryUpdate::Server(server.name, providers));
                    }
                },
                Ok(None) => listing = Some(Ok(())),
                Err(error) => listing = Some(Err(error.into())),
            },
            Some(name) = async {
                if listing.is_none() {
                    tokio::time::sleep_until(start + INVENTORY_HEAD_START).await;
                }
                queued.pop_first()
            }, if !queued.is_empty() && probes.len() < MAX_CONCURRENT_PROBES => {
                let key = name.clone();
                let lookup = async move {
                    Ok(match inventory::server(client, &name, /*thread*/ None).await? {
                        Some(server) => server.providers()?,
                        None => Vec::new(),
                    })
                };
                probes.insert(key, Box::pin(stream::once(lookup)));
            }
        }
    }
    for name in queued.into_iter().chain(probes.keys().cloned()) {
        let _ = updates.send(DiscoveryUpdate::Server(
            name,
            Err(ProviderLookupError::TimedOut),
        ));
    }
    listing.unwrap_or(Err(ProviderLookupError::TimedOut))
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
